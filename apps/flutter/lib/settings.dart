// SPDX-License-Identifier: GPL-3.0-or-later
// App settings that more than one screen uses. The last used server is remembered in a small
// file (see [PrefsSettingsStore]).

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'core/core_api.dart';
import 'discovery.dart';

/// Used only if the core offers no server at all (never the case with the real core).
const fallbackServerAddress = '127.0.0.1:8443';

/// Where the last used server is kept between runs.
abstract class SettingsStore {
  /// The saved server, or null (nothing saved, or the store is not available).
  Future<String?> loadServer();

  /// Save the server. Failures are ignored: remembering is a convenience.
  Future<void> saveServer(String server);

  /// What the person agreed to download, as `host|version` (see AppSettings.codecConsentValue);
  /// null if never asked. The agreement belongs to that host and version only: a new version
  /// or another host asks again. The old boolean of earlier releases is not read.
  Future<String?> loadCodecConsent() async => null;

  /// Remember the agreement. Failures are ignored.
  Future<void> saveCodecConsent(String consent) async {}
}

/// Keeps the value in memory only (tests).
class MemorySettingsStore implements SettingsStore {
  MemorySettingsStore([this.server]);

  String? server;

  @override
  Future<String?> loadServer() async => server;

  @override
  Future<void> saveServer(String server) async => this.server = server;

  String? codecConsent;

  @override
  Future<String?> loadCodecConsent() async => codecConsent;

  @override
  Future<void> saveCodecConsent(String consent) async => codecConsent = consent;
}

/// Saves the value with the `shared_preferences` package.
// UNCERTAIN: on Linux that package writes a JSON file `shared_preferences.json` into the
// application support directory (XDG data home, normally `~/.local/share/<app id>/`); Windows,
// macOS and Android use their own storage. Only the Linux file was looked at.
class PrefsSettingsStore implements SettingsStore {
  const PrefsSettingsStore();

  static const _key = 'lastServer';

  @override
  Future<String?> loadServer() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final v = prefs.getString(_key);
      return (v == null || v.trim().isEmpty) ? null : v;
    } catch (_) {
      return null;
    }
  }

  @override
  Future<void> saveServer(String server) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(_key, server);
    } catch (_) {
      // Not saved; the app works without.
    }
  }

  // The key of the first release held a boolean ('codecConsent'); it is not migrated: the
  // person is asked again once, now for a host and version.
  static const _consentKey = 'codecConsentFor';

  @override
  Future<String?> loadCodecConsent() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      return prefs.getString(_consentKey);
    } catch (_) {
      return null;
    }
  }

  @override
  Future<void> saveCodecConsent(String consent) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(_consentKey, consent);
    } catch (_) {
      // Not saved; the person is asked again next time.
    }
  }
}

/// Like a tiny Pinia store: screens read [serverAddress] and listen for changes.
///
/// The server list follows the owner's order (docs/discovery.md): policy servers, DNS-SRV,
/// announcements by displays, the built-in list. The pre-fill is the first policy server; if the
/// policy names none, the server used last time (only when the policy does not [locked] the
/// field); otherwise the first entry that is not an announcement. **A server that any device
/// in the local network announced is never pre-filled** (the room code would go to it): it is
/// only a labelled suggestion that the person taps. Whatever the person typed or tapped is
/// never overwritten.
class AppSettings extends ChangeNotifier {
  AppSettings({
    String? serverAddress,
    List<ServerSuggestion> serverSuggestions = const [],
    bool locked = false,
    bool announcementsAllowed = true,
    Set<DiscoveryWarning> notices = const {},
    this.savedServer,
    this.devFallback,
    this.udpBind,
    this.captureDisplay,
    this.store,
  }) : _suggestions = serverSuggestions,
       _locked = locked,
       // ignore: prefer_initializing_formals (named parameters cannot be private)
       _announcementsAllowed = announcementsAllowed,
       // ignore: prefer_initializing_formals
       _notices = notices,
       _userChose = serverAddress != null && !locked,
       _serverAddress =
           (locked ? null : serverAddress) ??
           _prefill(serverSuggestions, locked: locked, saved: savedServer) ??
           (locked ? '' : fallbackServerAddress);

  /// Settings with the offline answer of the core (policy file and built-in list). The
  /// network search is started with [discover]. [serverAddress] (development override) and
  /// [savedServer] (last used) are ignored where a policy locks the server.
  factory AppSettings.fromDefaults(
    ServerDefaults defaults, {
    String? serverAddress,
    String? savedServer,
    String? udpBind,
    String? captureDisplay,
    SettingsStore? store,
  }) {
    final saved = defaults.locked ? null : savedServer;
    return AppSettings(
      serverAddress: serverAddress,
      serverSuggestions: compose(
        found: defaults.servers,
        locked: defaults.locked,
        saved: saved,
        devFallback: defaults.devFallback,
      ),
      locked: defaults.locked,
      announcementsAllowed: defaults.announcementsAllowed,
      notices: defaults.notices.toSet(),
      savedServer: saved,
      devFallback: defaults.devFallback,
      udpBind: udpBind,
      captureDisplay: captureDisplay,
      store: store,
    );
  }

  /// Start-up: the server is, in this order, [envServer] (development), the saved server
  /// (unless a policy locks the field), the first entry of the core's list.
  static Future<AppSettings> load({
    required CoreApi core,
    SettingsStore store = const PrefsSettingsStore(),
    String? envServer,
    String? udpBind,
    String? captureDisplay,
  }) async {
    final saved = await store.loadServer();
    final defaults = core.serverDefaults();
    return AppSettings.fromDefaults(
      defaults,
      // The development override and the saved server beat the list, but not a policy server.
      serverAddress: envServer ?? (_hasPolicyServer(defaults) ? null : saved),
      savedServer: saved,
      udpBind: udpBind,
      captureDisplay: captureDisplay,
      store: store,
    );
  }

  static bool _hasPolicyServer(ServerDefaults d) => d.servers.any(
    (s) => ServerSourceKind.fromRust(s.source) == ServerSourceKind.policy,
  );

  /// The list for the field: [found] (from the core, best first), then, unless [locked], the
  /// [saved] server after the policy servers, and the development server at the end.
  static List<ServerSuggestion> compose({
    required List<ServerCandidate> found,
    required bool locked,
    String? saved,
    String? devFallback,
  }) {
    final out = <ServerSuggestion>[];
    bool has(String a) =>
        out.any((s) => s.address.toLowerCase() == a.toLowerCase());
    for (final c in found) {
      if (!has(c.address)) out.add(ServerSuggestion.fromCandidate(c));
    }
    if (locked) return out;
    final savedAddress = saved?.trim();
    if (savedAddress != null && savedAddress.isNotEmpty && !has(savedAddress)) {
      final at = out.takeWhile((s) => s.kind == ServerSourceKind.policy).length;
      out.insert(at, ServerSuggestion(savedAddress, ServerSourceKind.lastUsed));
    }
    if (devFallback != null && devFallback.isNotEmpty && !has(devFallback)) {
      out.add(ServerSuggestion(devFallback, ServerSourceKind.development));
    }
    return out;
  }

  /// Sources that may fill the field without a tap by the person. Announcements (any device
  /// in the network can make one) and unknown sources may not.
  static bool _mayPrefill(ServerSourceKind k) =>
      k != ServerSourceKind.announced && k != ServerSourceKind.unknown;

  /// The address that fills the field: the first policy server, else the saved one (not when
  /// locked), else the first suggestion that [_mayPrefill], else null.
  static String? _prefill(
    List<ServerSuggestion> list, {
    required bool locked,
    String? saved,
  }) {
    for (final s in list) {
      if (s.kind == ServerSourceKind.policy) return s.address;
    }
    final savedAddress = saved?.trim();
    if (!locked && savedAddress != null && savedAddress.isNotEmpty) {
      return savedAddress;
    }
    for (final s in list) {
      if (_mayPrefill(s.kind)) return s.address;
    }
    return null;
  }

  final SettingsStore? store;

  /// The server used last time (null if none, or a policy locks the field).
  final String? savedServer;

  /// The development server, offered last.
  final String? devFallback;

  String _serverAddress;
  List<ServerSuggestion> _suggestions;
  bool _locked;
  bool _announcementsAllowed;
  Set<DiscoveryWarning> _notices;
  bool _userChose;

  /// Where the current value comes from if the person took it from a suggestion or a nearby
  /// receiver; null if it was typed or pre-filled.
  ServerSourceKind? _valueSource;
  bool _discovering = false;
  int _discoveryRun = 0;

  /// Suggested servers, best first, each with its source.
  List<ServerSuggestion> get serverSuggestions => _suggestions;

  /// A policy sets the server: the field is read-only and only [serverSuggestions] can be used.
  bool get locked => _locked;

  /// Announcements of displays may be used (the policy does not forbid them).
  bool get announcementsAllowed => _announcementsAllowed;

  /// Notes about problems while looking for servers, as codes the screens translate.
  Set<DiscoveryWarning> get notices => _notices;

  /// The network search runs.
  bool get discovering => _discovering;

  /// Signalling server as `host:port`.
  String get serverAddress => _serverAddress;

  /// Where the current address comes from, when the person took it from a chip or a nearby
  /// receiver (the screen shows it next to the field). Null after typing.
  ServerSourceKind? get valueSource => _valueSource;

  /// The person typed a server. Under a policy lock only listed servers are taken.
  set serverAddress(String value) {
    if (value == _serverAddress) return;
    if (_locked && !_suggestions.any((s) => s.address == value)) return;
    _serverAddress = value;
    _valueSource = null;
    _userChose = true;
    notifyListeners();
  }

  /// The person tapped [s] (a chip or a nearby receiver): the address is taken and its
  /// source is remembered, so that it stays visible and an announced server is not saved as
  /// "last used".
  void chooseSuggestion(ServerSuggestion s) {
    if (_locked && !_suggestions.any((x) => x.address == s.address)) return;
    if (s.address == _serverAddress && s.kind == _valueSource) return;
    _serverAddress = s.address;
    _valueSource = s.kind;
    _userChose = true;
    notifyListeners();
  }

  /// Ask all sources for servers (policy, DNS-SRV, announcements; the time limits are in
  /// Rust) without blocking the screen. The answer changes the list, the lock and the notes,
  /// and fills the field unless the person chose a server meanwhile. [cancelDiscovery] makes a
  /// late answer harmless.
  Future<void> discover(CoreApi core) async {
    final run = ++_discoveryRun;
    // No notification here: this is called from initState; the first build reads the flag.
    _discovering = true;
    ServerDiscovery? result;
    try {
      result = await core.discoverServers();
    } catch (_) {
      result = null;
    }
    if (run != _discoveryRun) return; // cancelled or replaced
    _discovering = false;
    if (result == null) {
      _notices = {..._notices, DiscoveryWarning.other};
    } else {
      _apply(result);
    }
    notifyListeners();
  }

  /// Ignore the answer of a running [discover] (call when the screen closes).
  void cancelDiscovery() {
    _discoveryRun++;
    _discovering = false;
  }

  void _apply(ServerDiscovery r) {
    _locked = r.locked;
    _announcementsAllowed = r.announcementsAllowed;
    _notices = r.notices.toSet();
    _suggestions = compose(
      found: r.servers,
      locked: r.locked,
      saved: savedServer,
      devFallback: devFallback,
    );
    final prefill = _prefill(_suggestions, locked: _locked, saved: savedServer);
    if (_locked) {
      // A policy beats whatever was chosen before.
      if (!_suggestions.any((s) => s.address == _serverAddress)) {
        _serverAddress = prefill ?? '';
        _valueSource = null;
      }
    } else if (!_userChose && prefill != null) {
      _serverAddress = prefill;
      _valueSource = null;
    }
  }

  String? _codecConsent;

  /// The text stored for an agreement to download [info]: `host|version`.
  static String codecConsentValue(UiCodecInfo info) =>
      '${info.host}|${info.version}';

  /// The person agreed to download exactly this host and version (remembered between runs).
  bool codecConsentFor(UiCodecInfo? info) =>
      info != null && _codecConsent == codecConsentValue(info);

  /// Read the remembered consent from the store.
  Future<void> loadCodecConsent() async {
    _codecConsent = await store?.loadCodecConsent() ?? _codecConsent;
  }

  /// Remember that the person agreed to download [info].
  Future<void> rememberCodecConsent(UiCodecInfo info) async {
    _codecConsent = codecConsentValue(info);
    await store?.saveCodecConsent(_codecConsent!);
  }

  /// Remember the current server for the next start (call when it is used). Not under a lock,
  /// and not a server that was announced in the local network: "last used" is pre-filled
  /// next time, which would turn an unverified announcement into a trusted default.
  Future<void> rememberServer() async {
    final v = _serverAddress.trim();
    if (v.isEmpty || _locked || _valueSource == ServerSourceKind.announced) {
      return;
    }
    await store?.saveServer(v);
  }

  /// Local UDP address for WebRTC, for development only (`COMETCAST_UDP_BIND`, for example
  /// `127.0.0.1:0` for tests on one machine). `null` = default (all interfaces).
  final String? udpBind;

  /// X11 display to share instead of `$DISPLAY`, for development and tests
  /// (`COMETCAST_CAPTURE_DISPLAY`, for example `:99`). `null` = the normal display.
  final String? captureDisplay;
}
