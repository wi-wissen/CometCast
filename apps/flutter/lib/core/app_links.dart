// SPDX-License-Identifier: GPL-3.0-or-later
// Links that open the app and the install referrer, from the platform (native-apps.md 20.4):
// Android App Links and iOS Universal Links for the built-in relay's host, and the Google Play
// install referrer. The platform code is android/.../links/LinkBridge.kt and
// ios/Runner/Links/LinkBridge.swift. No widgets and no strings.

import 'dart:async';

import 'package:flutter/services.dart';

abstract class AppLinks {
  /// The https link that started the app, or null (also where the platform has no such link).
  Future<String?> initialLink();

  /// Links that arrive while the app runs.
  Stream<String> get links;

  /// The Google Play install referrer (`code=K7QM2W&server=...`), or null: not Android, not
  /// installed from Google Play, no referrer, or the service is not available.
  Future<String?> installReferrer();
}

/// The platform channel implementation. Every call answers null where the platform has nothing
/// (a missing plugin is not an error: the app then behaves as without a link).
class ChannelAppLinks implements AppLinks {
  ChannelAppLinks([MethodChannel? channel])
    : _channel = channel ?? const MethodChannel('de.cometcast/links') {
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'link') {
        final l = call.arguments;
        if (l is String && !_controller.isClosed) _controller.add(l);
      }
      return null;
    });
  }

  final MethodChannel _channel;
  final _controller = StreamController<String>.broadcast();

  @override
  Stream<String> get links => _controller.stream;

  @override
  Future<String?> initialLink() => _string('initialLink');

  @override
  Future<String?> installReferrer() => _string('installReferrer');

  Future<String?> _string(String method) async {
    try {
      final v = await _channel.invokeMethod<String>(method);
      return (v == null || v.isEmpty) ? null : v;
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }
}
