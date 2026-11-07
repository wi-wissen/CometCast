// SPDX-License-Identifier: GPL-3.0-or-later
// The platform part of "screens on this network" (spec 14, native-apps.md 4.1): which browse API
// finds the screens, the state of the local-network permission, whether there is a local network,
// and the page of the system settings. Android (NsdManager), iOS and macOS (NWBrowser) browse in Kotlin
// and Swift and hand the raw records to the Rust core, which checks and keeps the list; macOS
// browses with NWBrowser through the same bridge; Linux and Windows browse in the core itself
// (mdns-sd). Plain interface so tests can plug in fakes.
//
// UNCERTAIN: the Android and iOS bridges were never run on a device (docs/android.md,
// docs/ios.md); they are only syntax-checked.

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/services.dart';
import 'package:url_launcher/url_launcher.dart';

import 'core_api.dart';

/// What the search needs from the platform.
abstract class NearbyPlatform {
  /// True where the platform's own browse API finds the screens (Android, iOS): the bridge feeds
  /// the core; false where the core browses itself.
  bool get nativeBrowse;

  /// Phones and tablets stop searching after 60 s (spec 14.5); computers never do.
  bool get limitedTime;

  /// The state of the permission before a search: [NearbyPermission.notAsked] only where the
  /// platform has a permission that was never asked (iOS, iPadOS); it is asked by the first
  /// search, as the direct result of the person's tap.
  Future<NearbyPermission> permission();

  /// Start the platform's browse (nothing to do for the core's own browse). On iOS this is the
  /// call that makes the system ask. Results arrive through the core (`nearbyNative...`).
  Future<void> startBrowse();

  Future<void> stopBrowse();

  /// Is there a Wi-Fi or Ethernet network (or, on computers, a private interface)?
  Future<bool> hasLocalNetwork();

  /// Changes of [hasLocalNetwork] while the app runs.
  Stream<bool> get networkChanges;

  /// Open the page of the system settings where the permission is given. False if there is none.
  Future<bool> openSettings();
}

/// Computers: the core browses with mdns-sd; the network is a private, non-loopback interface,
/// asked every few seconds while somebody listens (there is no change notification).
class DesktopNearbyPlatform implements NearbyPlatform {
  DesktopNearbyPlatform(this._core, {this.poll = const Duration(seconds: 5)});

  final CoreApi _core;
  final Duration poll;

  @override
  bool get nativeBrowse => false;

  @override
  bool get limitedTime => false;

  @override
  Future<NearbyPermission> permission() async => NearbyPermission.granted;

  @override
  Future<void> startBrowse() async {}

  @override
  Future<void> stopBrowse() async {}

  @override
  Future<bool> hasLocalNetwork() async => _core.localNetworkAvailable();

  @override
  Stream<bool> get networkChanges {
    late final StreamController<bool> c;
    Timer? t;
    bool? last;
    c = StreamController<bool>(
      onListen: () {
        t = Timer.periodic(poll, (_) {
          final now = _core.localNetworkAvailable();
          if (now != last) {
            last = now;
            c.add(now);
          }
        });
        last = _core.localNetworkAvailable();
      },
      onCancel: () => t?.cancel(),
    );
    return c.stream;
  }

  @override
  Future<bool> openSettings() async {
    // macOS: the Privacy & Security pane, Local Network. Elsewhere there is none.
    if (!Platform.isMacOS) return false;
    try {
      return await launchUrl(
        Uri.parse(
          'x-apple.systempreferences:com.apple.preference.security'
          '?Privacy_LocalNetwork',
        ),
      );
    } catch (_) {
      return false;
    }
  }
}

/// Android and iOS: the bridge in Kotlin / Swift over the channel `de.cometcast/nearby`.
///
/// Calls to the platform: `permission` (`notAsked` | `granted` | `denied`), `start`, `stop`,
/// `hasNetwork`, `openSettings`. Calls from the platform: `seen` (instance, addresses, txt),
/// `gone` (instance), `reset`, `permission` (state), `network` (bool).
class ChannelNearbyPlatform implements NearbyPlatform {
  ChannelNearbyPlatform(
    this._core, {
    MethodChannel? channel,
    this.limitedTime = true,
  }) : _channel = channel ?? const MethodChannel('de.cometcast/nearby') {
    _channel.setMethodCallHandler(_onCall);
  }

  final CoreApi _core;
  final MethodChannel _channel;
  final _network = StreamController<bool>.broadcast();

  @override
  bool get nativeBrowse => true;

  /// Phones and tablets stop after 60 s; macOS browses as long as the page is visible.
  @override
  final bool limitedTime;

  Future<dynamic> _onCall(MethodCall call) async {
    final a = call.arguments;
    switch (call.method) {
      case 'seen':
        if (a is Map) {
          final txt = <(String, String)>[];
          final raw = a['txt'];
          if (raw is Map) {
            for (final e in raw.entries.take(32)) {
              if (e.key is String && e.value is String) {
                txt.add((e.key as String, e.value as String));
              }
            }
          }
          final addresses = (a['addresses'] is List)
              ? [
                  for (final x in (a['addresses'] as List).take(8))
                    if (x is String) x,
                ]
              : <String>[];
          final instance = a['instance'];
          if (instance is String) {
            _core.nearbyNativeSeen(
              instance: instance,
              addresses: addresses,
              txt: txt,
            );
          }
        }
      case 'gone':
        if (a is String) _core.nearbyNativeGone(a);
      case 'reset':
        _core.nearbyNativeReset();
      case 'permission':
        _core.nearbyNativePermission(_permission(a));
      case 'network':
        if (a is bool) _network.add(a);
    }
    return null;
  }

  static NearbyPermission _permission(Object? s) => switch (s) {
    'denied' => NearbyPermission.denied,
    'notAsked' => NearbyPermission.notAsked,
    _ => NearbyPermission.granted,
  };

  @override
  Future<NearbyPermission> permission() async {
    try {
      return _permission(await _channel.invokeMethod<String>('permission'));
    } catch (_) {
      return NearbyPermission.granted;
    }
  }

  @override
  Future<void> startBrowse() async {
    try {
      await _channel.invokeMethod<void>('start');
    } catch (_) {
      // The core shows "nothing found"; there is nothing better to say.
    }
  }

  @override
  Future<void> stopBrowse() async {
    try {
      await _channel.invokeMethod<void>('stop');
    } catch (_) {}
  }

  @override
  Future<bool> hasLocalNetwork() async {
    try {
      return await _channel.invokeMethod<bool>('hasNetwork') ?? true;
    } catch (_) {
      return true;
    }
  }

  @override
  Stream<bool> get networkChanges => _network.stream;

  @override
  Future<bool> openSettings() async {
    try {
      return await _channel.invokeMethod<bool>('openSettings') ?? false;
    } catch (_) {
      return false;
    }
  }
}

/// The platform part for this device: the bridge on Android and iOS, the core's own browse on
/// computers. A television passes none to the presenter (a display shows no list).
NearbyPlatform nearbyPlatformFor(CoreApi core) {
  if (Platform.isAndroid || Platform.isIOS) return ChannelNearbyPlatform(core);
  // macOS browses with NWBrowser, like iOS, so the Local Network permission of macOS 15 gives
  // "Not asked yet" and "Not allowed" (decision N50); Linux and Windows browse in the core.
  if (Platform.isMacOS) return ChannelNearbyPlatform(core, limitedTime: false);
  return DesktopNearbyPlatform(core);
}

/// Android, display role (decision N50): hold or drop the `WifiManager.MulticastLock` that the
/// core's own announcement (mdns-sd) needs. A no-op on every other platform.
Future<void> setAnnounceLock(bool on) async {
  if (!Platform.isAndroid) return;
  try {
    await const MethodChannel('de.cometcast/nearby')
        .invokeMethod<void>('announceLock', on);
  } on MissingPluginException {
    // No bridge (tests, a build without it): announce without the lock.
  } on PlatformException {
    // The lock could not be taken: announce without it.
  }
}
