// SPDX-License-Identifier: GPL-3.0-or-later
// iPadOS, display role (decision N50, spec 14.8): the screen announces itself with `NWListener`
// in Swift (the multicast entitlement is not counted on). The Rust core still builds the record
// (code, mode, name, server; docs/discovery.md, section 3) and decides when to publish; this
// class hands its events to the Swift bridge over the channel `de.cometcast/announce`.
//
// To the platform: `publish` {instance, txt: {key: value}} (publish or replace the record and
// announce it; sent at once on a change and every 10 s), `withdraw` (goodbye).
//
// UNCERTAIN: the Swift side was never compiled or run on a device (docs/ios.md).

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/services.dart';

import 'core_api.dart';

/// True where the platform announces and the core only builds the record (iOS and iPadOS).
bool get platformAnnounces => Platform.isIOS;

class NativeAnnouncer {
  NativeAnnouncer(this._core, {MethodChannel? channel})
    : _channel = channel ?? const MethodChannel('de.cometcast/announce');

  final CoreApi _core;
  final MethodChannel _channel;
  StreamSubscription<NativeAnnounceEvent>? _sub;

  /// Take the announcement from the core. Call before the display announces.
  void start() {
    _sub ??= _core.watchNativeAnnouncements().listen(_on);
  }

  void _on(NativeAnnounceEvent e) {
    switch (e) {
      case NativeAnnounceEvent_Publish(:final instance, :final txt):
        _call('publish', {
          'instance': instance,
          'txt': {for (final (k, v) in txt) k: v},
        });
      case NativeAnnounceEvent_Withdraw():
        _call('withdraw', null);
    }
  }

  Future<void> _call(String method, Object? args) async {
    try {
      await _channel.invokeMethod<void>(method, args);
    } on MissingPluginException {
      // No bridge: nothing is announced; the display works without it.
    } on PlatformException {
      // The platform could not publish; the core publishes again in 10 s.
    }
  }

  void dispose() {
    _sub?.cancel();
    _sub = null;
    _core.stopNativeAnnouncements();
  }
}
