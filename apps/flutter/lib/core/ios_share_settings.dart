// SPDX-License-Identifier: GPL-3.0-or-later
// What the iOS share extension reads from the App Group (native-apps.md 21.3): the name, the
// servers confirmed on this device and the last used server, written by the app through the
// channel of `ios/Runner/Broadcast/BroadcastBridge.swift`. The extension writes back only the
// name. No widgets and no strings.

import 'package:flutter/services.dart';

class ShareSettingsData {
  const ShareSettingsData({
    this.name = '',
    this.confirmedServers = const [],
    this.lastServer,
  });

  final String name;
  final List<String> confirmedServers;
  final String? lastServer;

  @override
  bool operator ==(Object other) =>
      other is ShareSettingsData &&
      other.name == name &&
      other.lastServer == lastServer &&
      other.confirmedServers.length == confirmedServers.length &&
      Iterable.generate(confirmedServers.length)
          .every((i) => confirmedServers[i] == other.confirmedServers[i]);

  @override
  int get hashCode => Object.hash(name, lastServer, confirmedServers.length);
}

abstract class IosShareSettings {
  /// Hand the settings to the extension. False where there is no App Group (not iOS, signing).
  Future<bool> write(ShareSettingsData data);

  /// What the extension may have changed (the name); null where there is no App Group.
  Future<ShareSettingsData?> read();
}

class ChannelIosShareSettings implements IosShareSettings {
  ChannelIosShareSettings([MethodChannel? channel])
    : _channel = channel ?? const MethodChannel('de.cometcast/ios_broadcast');

  final MethodChannel _channel;

  @override
  Future<bool> write(ShareSettingsData data) async {
    try {
      return await _channel.invokeMethod<bool>('writeShareSettings', {
            'name': data.name,
            'confirmedServers': data.confirmedServers,
            'lastServer': data.lastServer,
          }) ??
          false;
    } on PlatformException {
      return false;
    } on MissingPluginException {
      return false;
    }
  }

  @override
  Future<ShareSettingsData?> read() async {
    try {
      final m = await _channel.invokeMethod<Map<Object?, Object?>>(
        'readShareSettings',
      );
      if (m == null) return null;
      final servers = m['confirmedServers'];
      return ShareSettingsData(
        name: m['name'] is String ? m['name']! as String : '',
        confirmedServers: servers is List
            ? servers.whereType<String>().toList()
            : const [],
        lastServer: m['lastServer'] is String
            ? m['lastServer']! as String
            : null,
      );
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }
}
