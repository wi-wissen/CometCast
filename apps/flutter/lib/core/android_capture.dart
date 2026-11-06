// SPDX-License-Identifier: GPL-3.0-or-later
// The Dart side of the Android screen capture (Kotlin: android/.../capture/CaptureBridge.kt).
//
// [AndroidCapture] is a small interface so that widget tests can plug in a fake. The real
// implementation, [ChannelAndroidCapture], talks to the Kotlin code over
//   MethodChannel 'de.cometcast/capture'        start / stop / requestSyncFrame
//   EventChannel 'de.cometcast/capture/frames'  config / frame / stopped / error
// On other platforms nothing here is used (the Rust core only reports a projection monitor on
// Android, see lib/send_page.dart), and constructing the class does not touch a channel.
//
// UNCERTAIN: never run on a device (docs/android.md).

import 'dart:async';

import 'package:flutter/services.dart';

/// Id of the single entry that `captureInfo` lists on Android (`ANDROID_PROJECTION_ID` in
/// crates/cc-ffi/src/api.rs; a Rust test pins the value).
const androidProjectionMonitorId = 'android-projection';

/// Texts of the notification that Android shows while the screen is captured. They are
/// translated by the UI (ARB keys `captureNotification*`); the native code has none.
class CaptureNotificationTexts {
  const CaptureNotificationTexts({
    required this.title,
    required this.text,
    required this.stopLabel,
  });

  final String title;
  final String text;
  final String stopLabel;
}

/// Something the capture service reports on the event channel.
sealed class CaptureEvent {
  const CaptureEvent();
}

/// Codec configuration (SPS/PPS, normally Annex B). Sent once per session, replayed to late
/// listeners by the Kotlin side.
class CaptureConfig extends CaptureEvent {
  const CaptureConfig(this.data);
  final Uint8List data;
}

/// One encoded access unit.
class CaptureFrame extends CaptureEvent {
  const CaptureFrame({
    required this.data,
    required this.isKey,
    required this.ptsUs,
    required this.dropped,
  });
  final Uint8List data;
  final bool isKey;

  /// Presentation time in microseconds.
  final int ptsUs;

  /// Frames dropped on the Kotlin side so far in this session.
  final int dropped;
}

/// The session ended (the person or the system stopped the projection).
class CaptureStopped extends CaptureEvent {
  const CaptureStopped(this.reason);
  final String reason;
}

/// The encoder or the projection failed. [message] is technical English.
class CaptureError extends CaptureEvent {
  const CaptureError(this.message);
  final String message;
}

/// Why `start` did not start a capture.
enum CaptureStartResult {
  /// The person agreed and the capture service runs.
  started,

  /// The person refused the system dialog.
  denied,
}

abstract class AndroidCapture {
  /// Events of the running (or starting) capture. Listening before [start] is fine: frames
  /// only flow once a session runs. Frames are dropped by the Kotlin side while nobody listens.
  Stream<CaptureEvent> get events;

  /// Show the system consent dialog (Android 13+ asks for the notification permission first)
  /// and, if the person agrees, start the capture service. Throws [PlatformException] if the
  /// native side fails (for example because a dialog is already open).
  Future<CaptureStartResult> start({
    required CaptureNotificationTexts notification,
    int maxDimension = 1280,
    int fps = 30,
    int bitrate = 4000000,
  });

  /// Stop the capture service. Safe to call when nothing runs.
  Future<void> stop();

  /// Ask the encoder for a sync (key) frame
  /// (`MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME`).
  Future<void> requestSyncFrame();
}

/// The real implementation, on top of the platform channels.
class ChannelAndroidCapture implements AndroidCapture {
  const ChannelAndroidCapture();

  static const _method = MethodChannel('de.cometcast/capture');
  static const _events = EventChannel('de.cometcast/capture/frames');

  @override
  Stream<CaptureEvent> get events => _events
      .receiveBroadcastStream()
      .map(parseCaptureEvent)
      .where((e) => e != null)
      .cast<CaptureEvent>();

  @override
  Future<CaptureStartResult> start({
    required CaptureNotificationTexts notification,
    int maxDimension = 1280,
    int fps = 30,
    int bitrate = 4000000,
  }) async {
    final ok = await _method.invokeMethod<bool>('start', {
      'maxDimension': maxDimension,
      'fps': fps,
      'bitrate': bitrate,
      'notificationTitle': notification.title,
      'notificationText': notification.text,
      'notificationStop': notification.stopLabel,
    });
    return ok == true ? CaptureStartResult.started : CaptureStartResult.denied;
  }

  @override
  Future<void> stop() => _method.invokeMethod<void>('stop');

  @override
  Future<void> requestSyncFrame() =>
      _method.invokeMethod<void>('requestSyncFrame');
}

/// Turns a map from the event channel into a [CaptureEvent]; null for something unknown or
/// malformed (it is ignored, not fatal).
CaptureEvent? parseCaptureEvent(dynamic raw) {
  if (raw is! Map) return null;
  switch (raw['type']) {
    case 'config':
      final data = _bytes(raw['data']);
      return data == null ? null : CaptureConfig(data);
    case 'frame':
      final data = _bytes(raw['data']);
      if (data == null) return null;
      return CaptureFrame(
        data: data,
        isKey: raw['keyFrame'] == true,
        ptsUs: _int(raw['ptsUs']) ?? -1,
        dropped: _int(raw['dropped']) ?? 0,
      );
    case 'stopped':
      return CaptureStopped('${raw['reason'] ?? ''}');
    case 'error':
      return CaptureError('${raw['message'] ?? 'unknown error'}');
  }
  return null;
}

Uint8List? _bytes(dynamic v) {
  if (v is Uint8List) return v;
  if (v is List<int>) return Uint8List.fromList(v);
  return null;
}

int? _int(dynamic v) => v is int ? v : (v is num ? v.toInt() : null);
