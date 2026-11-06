// SPDX-License-Identifier: GPL-3.0-or-later
// App settings that more than one screen uses. The last used server is remembered in a small
// file (see [PrefsSettingsStore]).

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'core/core_api.dart';

/// Used only if the core offers no server at all (never the case with the real core).
const fallbackServerAddress = '127.0.0.1:8443';

/// Where the last used server is kept between runs.
abstract class SettingsStore {
  /// The saved server, or null (nothing saved, or the store is not available).
  Future<String?> loadServer();

  /// Save the server. Failures are ignored: remembering is a convenience.
  Future<void> saveServer(String server);
}

/// Keeps the value in memory only (tests).
class MemorySettingsStore implements SettingsStore {
  MemorySettingsStore([this.server]);

  String? server;

  @override
  Future<String?> loadServer() async => server;

  @override
  Future<void> saveServer(String server) async => this.server = server;
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
}

/// Like a tiny Pinia store: screens read [serverAddress] and listen for changes.
class AppSettings extends ChangeNotifier {
  AppSettings({
    String? serverAddress,
    this.serverSuggestions = const [],
    this.udpBind,
    this.captureDisplay,
    this.store,
  }) : _serverAddress =
           serverAddress ??
           (serverSuggestions.isNotEmpty
               ? serverSuggestions.first
               : fallbackServerAddress);

  /// Settings with the server list of the core (cc-discovery) and no saved value.
  factory AppSettings.fromDefaults(
    ServerDefaults defaults, {
    String? serverAddress,
    String? udpBind,
    String? captureDisplay,
    SettingsStore? store,
  }) => AppSettings(
    serverAddress: serverAddress,
    serverSuggestions: suggestionsOf(defaults),
    udpBind: udpBind,
    captureDisplay: captureDisplay,
    store: store,
  );

  /// Start-up: the server is, in this order, [envServer] (development), the saved server,
  /// the first entry of the core's list.
  static Future<AppSettings> load({
    required CoreApi core,
    SettingsStore store = const PrefsSettingsStore(),
    String? envServer,
    String? udpBind,
    String? captureDisplay,
  }) async {
    final saved = await store.loadServer();
    return AppSettings.fromDefaults(
      core.serverDefaults(),
      serverAddress: envServer ?? saved,
      udpBind: udpBind,
      captureDisplay: captureDisplay,
      store: store,
    );
  }

  /// The core's default servers followed by the development fallback, without repeats.
  static List<String> suggestionsOf(ServerDefaults d) =>
      {...d.servers, d.devFallback}.toList();

  final SettingsStore? store;

  String _serverAddress;

  /// Server addresses offered as one-tap suggestions.
  final List<String> serverSuggestions;

  /// Signalling server as `host:port`.
  String get serverAddress => _serverAddress;

  set serverAddress(String value) {
    if (value == _serverAddress) return;
    _serverAddress = value;
    notifyListeners();
  }

  /// Remember the current server for the next start (call when it is used).
  Future<void> rememberServer() async {
    final v = _serverAddress.trim();
    if (v.isEmpty) return;
    await store?.saveServer(v);
  }

  /// Local UDP address for WebRTC, for development only (`COMETCAST_UDP_BIND`, for example
  /// `127.0.0.1:0` for tests on one machine). `null` = default (all interfaces).
  final String? udpBind;

  /// X11 display to share instead of `$DISPLAY`, for development and tests
  /// (`COMETCAST_CAPTURE_DISPLAY`, for example `:99`). `null` = the normal display.
  final String? captureDisplay;
}
