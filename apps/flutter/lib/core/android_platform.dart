// SPDX-License-Identifier: GPL-3.0-or-later
// Platform facts for Android (Kotlin: android/.../platform/PlatformBridge.kt) and the
// [DeviceProfile] that the screens read: is this a television (receiver-only, remote control),
// which keep-screen-on hook to use. Plain interface so widget tests can plug in fakes.
//
// UNCERTAIN: never run on a device (docs/android.md).

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import 'android_video.dart';
import 'core_api.dart';

abstract class AndroidPlatform {
  /// Android TV / Google TV (UI mode television or the leanback feature).
  Future<bool> isTelevision();

  /// Folder of the app's native libraries (where the PDF and Office workers are).
  Future<String?> nativeLibraryDir();

  /// Sets or clears `FLAG_KEEP_SCREEN_ON` on the window.
  Future<void> setKeepScreenOn(bool on);

  /// SDK level, model, ABIs and TV flag, for the log of a device test.
  Future<Map<String, Object?>> deviceInfo();
}

/// The real implementation over the platform channel `de.cometcast/platform`.
class ChannelAndroidPlatform implements AndroidPlatform {
  const ChannelAndroidPlatform();

  static const _channel = MethodChannel('de.cometcast/platform');

  @override
  Future<bool> isTelevision() async =>
      await _channel.invokeMethod<bool>('isTelevision') ?? false;

  @override
  Future<String?> nativeLibraryDir() =>
      _channel.invokeMethod<String>('nativeLibraryDir');

  @override
  Future<void> setKeepScreenOn(bool on) =>
      _channel.invokeMethod<void>('setKeepScreenOn', on);

  @override
  Future<Map<String, Object?>> deviceInfo() async {
    final m = await _channel.invokeMapMethod<String, Object?>('deviceInfo');
    return m ?? const {};
  }
}

/// What the screens need to know about the device. Provided above the `MaterialApp`.
class DeviceProfile extends InheritedWidget {
  const DeviceProfile({
    super.key,
    required super.child,
    this.tv = false,
    this.platform,
    this.videoDecoder,
    this.deviceName,
  });

  /// A television: receiver only, big room code, no pointer-only controls.
  final bool tv;

  /// Android-only hooks (keep-screen-on); null elsewhere.
  final AndroidPlatform? platform;

  /// The hardware video decoder to use for received video, or null for the software decoder.
  /// Chosen in `main.dart`: on a television by default, elsewhere only when asked for with
  /// `--dart-define=COMETCAST_VIDEO=hardware` (docs/android.md, "Receiver video").
  final AndroidVideoDecoder? videoDecoder;

  /// Name this device announces itself with in the local network (the model name on Android,
  /// where the host name is not meaningful), or null to use the host name.
  final String? deviceName;

  static const none = DeviceProfile(child: SizedBox.shrink());

  /// The profile in effect; a plain desktop profile when none is provided (tests).
  static DeviceProfile of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<DeviceProfile>() ?? none;

  /// Like [of] but usable in `initState` (does not subscribe to changes; the profile is fixed
  /// for the life of the app).
  static DeviceProfile peek(BuildContext context) =>
      context.getInheritedWidgetOfExactType<DeviceProfile>() ?? none;

  @override
  bool updateShouldNotify(DeviceProfile old) =>
      tv != old.tv ||
      platform != old.platform ||
      videoDecoder != old.videoDecoder;
}

/// Where the PDF and Office workers are shipped on Android: `lib<name>.so` files in the native
/// library directory (android/app/build.gradle.kts, "PDF and Office workers").
const androidDocsWorkerFile = 'libcc_docs_worker.so';
const androidOfficeWorkerFile = 'libcc_office_worker.so';

/// Gives the Rust core the worker programs that were packaged as native libraries. A missing
/// file leaves the core without that worker, which it reports with a translated note (PDF) or
/// refusal (Office); nothing is guessed. [exists] is replaced in tests.
Future<void> configureAndroidWorkers(
  CoreApi core,
  AndroidPlatform platform, {
  bool Function(String path) exists = _fileExists,
}) async {
  String? dir;
  try {
    dir = await platform.nativeLibraryDir();
  } catch (e) {
    debugPrint('cometcast: no native library directory: $e');
  }
  if (dir == null || dir.isEmpty) return;
  final docs = '$dir/$androidDocsWorkerFile';
  final office = '$dir/$androidOfficeWorkerFile';
  if (exists(docs)) core.setDocsWorker(docs);
  if (exists(office)) core.setOfficeWorker(office);
}

bool _fileExists(String path) => File(path).existsSync();
