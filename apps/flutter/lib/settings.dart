// SPDX-License-Identifier: GPL-3.0-or-later
// App settings that more than one screen uses. Kept in memory only for now.

import 'package:flutter/foundation.dart';

/// Server used when nothing else is configured.
// UNCERTAIN: the real default is a list of servers found by cc-discovery (MDM, SRV/DNS-SD,
// code/QR, default list). That is wired later; until then this is a plain text field with
// a local development address.
const defaultServerAddress = '127.0.0.1:8443';

/// Like a tiny Pinia store: screens read [serverAddress] and listen for changes.
class AppSettings extends ChangeNotifier {
  AppSettings({String? serverAddress, this.udpBind})
      : _serverAddress = serverAddress ?? defaultServerAddress;

  String _serverAddress;

  /// Signalling server as `host:port`.
  // UNCERTAIN: not saved to disk yet, so it resets on restart. A persistence package
  // (for example shared_preferences) has not been chosen.
  String get serverAddress => _serverAddress;

  set serverAddress(String value) {
    if (value == _serverAddress) return;
    _serverAddress = value;
    notifyListeners();
  }

  /// Local UDP address for WebRTC, for development only (`COMETCAST_UDP_BIND`, for example
  /// `127.0.0.1:0` for tests on one machine). `null` = default (all interfaces).
  final String? udpBind;
}
