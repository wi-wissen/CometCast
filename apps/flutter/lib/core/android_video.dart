// SPDX-License-Identifier: GPL-3.0-or-later
// The Dart side of the hardware video decoder (Kotlin: android/.../video/VideoBridge.kt).
//
// Flow: Rust receiver (pass-through mode) --EncodedReady / takeEncodedVideo--> ReceiverController
//       --queue--> MediaCodec --Surface--> Flutter Texture (shown with `Texture(textureId: ..)`).
//
// UNCERTAIN: never run on a device (docs/android.md, "Receiver video").

import 'dart:async';

import 'package:flutter/services.dart';

/// Something the decoder reports.
sealed class VideoDecoderEvent {
  const VideoDecoderEvent();
}

/// The picture size (for the aspect ratio of the texture).
class VideoSize extends VideoDecoderEvent {
  const VideoSize(this.width, this.height);
  final int width;
  final int height;
}

/// A unit was dropped or the surface is new: ask the sender for a key frame.
class VideoNeedKeyFrame extends VideoDecoderEvent {
  const VideoNeedKeyFrame();
}

/// The decoder failed; the caller falls back to the software decoder.
class VideoDecoderError extends VideoDecoderEvent {
  const VideoDecoderError(this.message);
  final String message;
}

abstract class AndroidVideoDecoder {
  Stream<VideoDecoderEvent> get events;

  /// Create the texture and the decoder; returns the Flutter texture id. Throws if the native
  /// side cannot.
  Future<int> start();

  /// Hand one Annex B access unit to the decoder. Never waits for the decoder; a unit it cannot
  /// take is dropped by the native side (which then asks for a key frame).
  void queue(Uint8List data, {required int ptsUs, required bool key});

  /// Release decoder and texture. Safe when nothing runs.
  Future<void> stop();
}

/// The real implementation over the platform channels.
class ChannelAndroidVideoDecoder implements AndroidVideoDecoder {
  const ChannelAndroidVideoDecoder();

  static const _method = MethodChannel('de.cometcast/video');
  static const _events = EventChannel('de.cometcast/video/events');

  @override
  Stream<VideoDecoderEvent> get events => _events
      .receiveBroadcastStream()
      .map(parseVideoEvent)
      .where((e) => e != null)
      .cast<VideoDecoderEvent>();

  @override
  Future<int> start() async {
    final m = await _method.invokeMapMethod<String, Object?>('start');
    final id = m?['textureId'];
    if (id is! num) {
      throw PlatformException(code: 'video_start', message: 'no texture id');
    }
    return id.toInt();
  }

  @override
  void queue(Uint8List data, {required int ptsUs, required bool key}) {
    // Not awaited: the native side answers at once and drops what it cannot take.
    _method
        .invokeMethod<bool>('queue', {'data': data, 'ptsUs': ptsUs, 'key': key})
        .catchError((Object _) => false);
  }

  @override
  Future<void> stop() async {
    try {
      await _method.invokeMethod<void>('stop');
    } on PlatformException {
      // Nothing to release.
    } on MissingPluginException {
      // Not on Android.
    }
  }
}

/// A map of the event channel to an event; null for something unknown (ignored).
VideoDecoderEvent? parseVideoEvent(dynamic raw) {
  if (raw is! Map) return null;
  switch (raw['type']) {
    case 'size':
      final w = raw['width'];
      final h = raw['height'];
      if (w is num && h is num && w > 0 && h > 0) {
        return VideoSize(w.toInt(), h.toInt());
      }
      return null;
    case 'needKeyFrame':
      return const VideoNeedKeyFrame();
    case 'error':
      return VideoDecoderError('${raw['message'] ?? 'unknown error'}');
  }
  return null;
}
