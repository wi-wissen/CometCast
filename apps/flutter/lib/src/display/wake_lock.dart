// SPDX-License-Identifier: GPL-3.0-or-later
// The wake lock of the display: held ONLY while presenting, never on Idle (spec D5, 8.7;
// native-apps.md 15). Android uses the window flag through the existing platform channel
// (FLAG_KEEP_SCREEN_ON); iOS, macOS, Windows and Linux use the wakelock_plus package.
// UNCERTAIN: never run on a device or a desktop session.

import 'package:flutter/foundation.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import '../../core/android_platform.dart';

abstract class WakeLockControl {
  /// Hold (true) or release (false) the lock. Failures are ignored: the screen may then sleep.
  Future<void> set(bool on);
}

class PlatformWakeLock implements WakeLockControl {
  const PlatformWakeLock({this.android});

  /// The Android hooks (null elsewhere).
  final AndroidPlatform? android;

  @override
  Future<void> set(bool on) async {
    try {
      final a = android;
      if (a != null) {
        await a.setKeepScreenOn(on);
      } else {
        await WakelockPlus.toggle(enable: on);
      }
    } catch (e) {
      debugPrint('cometcast: wake lock ${on ? 'on' : 'off'} failed: $e');
    }
  }
}

/// Records the calls (tests).
class FakeWakeLock implements WakeLockControl {
  final calls = <bool>[];
  bool get held => calls.isNotEmpty && calls.last;

  @override
  Future<void> set(bool on) async => calls.add(on);
}
