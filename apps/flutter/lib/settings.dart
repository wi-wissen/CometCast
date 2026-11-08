// SPDX-License-Identifier: GPL-3.0-or-later
// App settings that more than one screen uses. The last used server is remembered in a small
// file (see [PrefsSettingsStore]).

import 'dart:async';
import 'dart:convert';

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

  /// The join mode the person chose last for new rooms (`open`, `confirm`, `locked`), or null.
  Future<String?> loadJoinMode() async => null;

  /// Remember it. Failures are ignored: a convenience only.
  Future<void> saveJoinMode(String mode) async {}

  /// The name shown to a host that has to approve this device, or null.
  Future<String?> loadDisplayName() async => null;

  /// Remember it. Failures are ignored.
  Future<void> saveDisplayName(String name) async {}

  /// The screens the person joined from the nearby list (room name and verified id; trust on
  /// first use, docs/design/signed-announcements.md). At most 50, oldest first.
  Future<List<RememberedScreen>> loadRememberedScreens() async => const [];

  /// Save the remembered screens.
  Future<void> saveRememberedScreens(List<RememberedScreen> screens) async {}

  /// The servers the person confirmed on this device (the relay confirmation page, spec 7.2),
  /// as canonical addresses.
  Future<List<String>> loadConfirmedServers() async => const [];

  /// Remember them. Failures are ignored: the page asks again.
  Future<void> saveConfirmedServers(List<String> servers) async {}

  /// Whether the install referrer was read already (it exists on the first launch only).
  Future<bool> loadReferrerRead() async => false;

  /// Remember that it was read.
  Future<void> saveReferrerRead() async {}

  /// A stored on/off setting of this device (round 5: `officeConvert`, `otherTvs`,
  /// `airplayReceive`, `miracastReceive`); null = never set, the caller applies its default.
  Future<bool?> loadFlag(String key) async => null;

  Future<void> saveFlag(String key, bool value) async {}
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

  String? joinMode;
  String? displayName;

  @override
  Future<String?> loadJoinMode() async => joinMode;

  @override
  Future<void> saveJoinMode(String mode) async => joinMode = mode;

  @override
  Future<String?> loadDisplayName() async => displayName;

  @override
  Future<void> saveDisplayName(String name) async => displayName = name;

  List<String> confirmedServers = const [];
  List<RememberedScreen> rememberedScreens = const [];

  @override
  Future<List<RememberedScreen>> loadRememberedScreens() async =>
      rememberedScreens;

  @override
  Future<void> saveRememberedScreens(List<RememberedScreen> screens) async =>
      rememberedScreens = screens;
  bool referrerRead = false;

  @override
  Future<List<String>> loadConfirmedServers() async => confirmedServers;

  @override
  Future<void> saveConfirmedServers(List<String> servers) async =>
      confirmedServers = servers;

  @override
  Future<bool> loadReferrerRead() async => referrerRead;

  @override
  Future<void> saveReferrerRead() async => referrerRead = true;

  final flags = <String, bool>{};

  @override
  Future<bool?> loadFlag(String key) async => flags[key];

  @override
  Future<void> saveFlag(String key, bool value) async => flags[key] = value;
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

  static const _joinModeKey = 'joinMode';
  static const _displayNameKey = 'displayName';

  @override
  Future<String?> loadJoinMode() => _getString(_joinModeKey);

  @override
  Future<void> saveJoinMode(String mode) => _setString(_joinModeKey, mode);

  @override
  Future<String?> loadDisplayName() => _getString(_displayNameKey);

  @override
  Future<void> saveDisplayName(String name) =>
      _setString(_displayNameKey, name);

  static const _rememberedKey = 'rememberedScreens';

  @override
  Future<List<RememberedScreen>> loadRememberedScreens() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final raw = prefs.getString(_rememberedKey);
      if (raw == null) return const [];
      final list = jsonDecode(raw) as List<dynamic>;
      return [
        for (final e in list)
          if (e is Map<String, dynamic> && e['n'] is String && e['i'] is String)
            RememberedScreen(name: e['n'] as String, id: e['i'] as String),
      ];
    } catch (_) {
      return const [];
    }
  }

  @override
  Future<void> saveRememberedScreens(List<RememberedScreen> screens) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(
        _rememberedKey,
        jsonEncode([
          for (final s in screens) {'n': s.name, 'i': s.id},
        ]),
      );
    } catch (_) {
      // Not saved; the screens are marked as new again next time.
    }
  }

  static const _confirmedKey = 'confirmedServers';
  static const _referrerReadKey = 'installReferrerRead';

  @override
  Future<List<String>> loadConfirmedServers() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      return prefs.getStringList(_confirmedKey) ?? const [];
    } catch (_) {
      return const [];
    }
  }

  @override
  Future<void> saveConfirmedServers(List<String> servers) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setStringList(_confirmedKey, servers);
    } catch (_) {
      // Not saved; the page asks again.
    }
  }

  @override
  Future<bool> loadReferrerRead() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      return prefs.getBool(_referrerReadKey) ?? false;
    } catch (_) {
      return false;
    }
  }

  @override
  Future<void> saveReferrerRead() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool(_referrerReadKey, true);
    } catch (_) {
      // Read again next time: harmless, the referrer is the same.
    }
  }

  @override
  Future<bool?> loadFlag(String key) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      return prefs.getBool('flag.$key');
    } catch (_) {
      return null;
    }
  }

  @override
  Future<void> saveFlag(String key, bool value) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool('flag.$key', value);
    } catch (_) {
      // Not saved; the setting is back to its default next time.
    }
  }

  Future<String?> _getString(String key) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      return prefs.getString(key);
    } catch (_) {
      return null;
    }
  }

  Future<void> _setString(String key, String value) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(key, value);
    } catch (_) {
      // Not saved; the app works without.
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
    UiJoinMode joinMode = UiJoinMode.open,
    String displayName = '',
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
           (locked ? '' : fallbackServerAddress) {
    _joinMode = joinMode;
    _displayName = displayName;
  }

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
    UiJoinMode joinMode = UiJoinMode.open,
    String displayName = '',
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
      joinMode: joinMode,
      displayName: displayName,
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
    final savedMode = await store.loadJoinMode();
    final savedName = await store.loadDisplayName();
    final defaults = core.serverDefaults();
    final settings = AppSettings.fromDefaults(
      defaults,
      // The development override and the saved server beat the list, but not a policy server.
      serverAddress: envServer ?? (_hasPolicyServer(defaults) ? null : saved),
      savedServer: saved,
      udpBind: udpBind,
      captureDisplay: captureDisplay,
      store: store,
      // The first-run default is open; the last choice is only a convenience.
      joinMode: UiJoinMode.values.firstWhere(
        (m) => m.name == savedMode,
        orElse: () => UiJoinMode.open,
      ),
      displayName: cleanDisplayName(savedName ?? ''),
    );
    await settings.loadConfirmedServers();
    await settings.loadFlags();
    return settings;
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

  Set<String> _confirmed = {};

  /// Servers the person confirmed on this device (canonical addresses): used at once from now on.
  Set<String> get confirmedServers => _confirmed;

  /// Read the confirmed servers from the store.
  Future<void> loadConfirmedServers() async {
    final saved = await store?.loadConfirmedServers() ?? const <String>[];
    _confirmed = {..._confirmed, ...saved};
  }

  /// The most remembered screens (the core looks at the first 50 as well).
  static const maxRemembered = 50;

  List<RememberedScreen> _remembered = [];

  /// The screens the person joined from the nearby list, oldest first.
  List<RememberedScreen> get rememberedScreens =>
      List.unmodifiable(_remembered);

  /// Read the remembered screens from the store.
  Future<void> loadRememberedScreens() async {
    final saved = await store?.loadRememberedScreens() ?? const [];
    _remembered = saved.length > maxRemembered
        ? saved.sublist(saved.length - maxRemembered)
        : List.of(saved);
  }

  /// The person joined the screen [id] (named [name]) from the nearby list: remember the pair
  /// (trust on first use). A screen without a name is not remembered (there is nothing to
  /// compare). The newest pair goes last; beyond [maxRemembered] the oldest is dropped.
  Future<void> rememberScreen(String name, String id) async {
    if (name.isEmpty) return;
    final pair = RememberedScreen(name: name, id: id);
    _remembered = [
      for (final r in _remembered)
        if (!(r.name == name && r.id == id)) r,
      pair,
    ];
    if (_remembered.length > maxRemembered) {
      _remembered = _remembered.sublist(_remembered.length - maxRemembered);
    }
    await store?.saveRememberedScreens(_remembered);
    notifyListeners();
  }

  /// The person pressed "Use this relay": remember [server] (canonical address) as confirmed.
  Future<void> confirmServer(String server) async {
    if (_confirmed.contains(server)) return;
    _confirmed = {..._confirmed, server};
    await store?.saveConfirmedServers(_confirmed.toList()..sort());
    notifyListeners();
  }

  // --- on/off settings of this device (spec 15.4, 15.8; defaults: N81, N89) ---

  bool _officeConvert = true;
  bool _otherTvs = false;
  bool _airplayReceive = false;
  bool _miracastReceive = false;

  /// Convert Office files to PDF on this device (default on, N81).
  bool get officeConvert => _officeConvert;

  /// Show Google Cast and AirPlay TVs on the start page (Beta, default off, N89).
  bool get otherTvs => _otherTvs;

  /// Receive AirPlay on this display (Beta, default off).
  bool get airplayReceive => _airplayReceive;

  /// Receive Miracast on this display (Beta, default off).
  bool get miracastReceive => _miracastReceive;

  set officeConvert(bool v) =>
      _setFlag('officeConvert', v, _officeConvert, (x) {
        _officeConvert = x;
      });
  set otherTvs(bool v) =>
      _setFlag('otherTvs', v, _otherTvs, (x) => _otherTvs = x);
  set airplayReceive(bool v) => _setFlag(
    'airplayReceive',
    v,
    _airplayReceive,
    (x) => _airplayReceive = x,
  );
  set miracastReceive(bool v) => _setFlag(
    'miracastReceive',
    v,
    _miracastReceive,
    (x) => _miracastReceive = x,
  );

  void _setFlag(String key, bool v, bool old, void Function(bool) apply) {
    if (v == old) return;
    apply(v);
    unawaited(store?.saveFlag(key, v) ?? Future<void>.value());
    notifyListeners();
  }

  Future<void> loadFlags() async {
    final s = store;
    if (s == null) return;
    _officeConvert = await s.loadFlag('officeConvert') ?? _officeConvert;
    _otherTvs = await s.loadFlag('otherTvs') ?? _otherTvs;
    _airplayReceive = await s.loadFlag('airplayReceive') ?? _airplayReceive;
    _miracastReceive = await s.loadFlag('miracastReceive') ?? _miracastReceive;
  }

  late UiJoinMode _joinMode;
  late String _displayName;

  /// Who may join the rooms this device creates (first run: open; remembered).
  UiJoinMode get joinMode => _joinMode;

  /// Remember the join mode that the person chose for new rooms.
  void setJoinMode(UiJoinMode mode) {
    if (mode == _joinMode) return;
    _joinMode = mode;
    unawaited(store?.saveJoinMode(mode.name) ?? Future<void>.value());
    notifyListeners();
  }

  /// The name a host sees when it has to approve this device (empty = none: the server uses
  /// "Guest N"). Cleaned and at most [maxDisplayNameChars] characters.
  String get displayName => _displayName;

  void setDisplayName(String name) {
    final clean = cleanDisplayName(name);
    if (clean == _displayName) return;
    _displayName = clean;
    unawaited(store?.saveDisplayName(clean) ?? Future<void>.value());
    notifyListeners();
  }

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

/// Most characters of a display name (the server cuts at the same length).
const maxDisplayNameChars = 40;

/// Removes control, bidi and invisible characters, collapses white space and cuts the name. The
/// server does the same; doing it here too keeps the field honest.
String cleanDisplayName(String raw) {
  final cleaned = raw
      .replaceAll(RegExp(r'[\t\n\r]'), ' ')
      .replaceAll(
        RegExp(
          '[\\u0000-\\u001f\\u007f-\\u009f\\u061c\\u200b-\\u200f\\u2028-\\u202e'
          '\\u2060-\\u206f\\ufeff\\ue000-\\uf8ff]',
        ),
        '',
      );
  final single = cleaned.replaceAll(RegExp(r'\s+'), ' ').trim();
  final runes = single.runes.toList();
  return String.fromCharCodes(runes.take(maxDisplayNameChars));
}
