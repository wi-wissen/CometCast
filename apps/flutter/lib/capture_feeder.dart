// SPDX-License-Identifier: GPL-3.0-or-later
// Connects the Android capture (encoded H.264 from MediaCodec) to the Rust sender:
//
//   Kotlin ScreenEncoder --EventChannel--> CaptureFeeder --pushEncodedFrame--> Rust (cc-sender)
//   Rust key frame request --keyframeRequested (polled)--> CaptureFeeder --requestSyncFrame--> Kotlin
//
// The feeder owns no UI. [SenderController] creates it for the "Share this screen" flow on
// Android and reacts to [onStopped] / [onError].
//
// UNCERTAIN: never run on a device (docs/android.md).

import 'dart:async';

import 'core/android_capture.dart';
import 'core/core_api.dart';

class CaptureFeeder {
  CaptureFeeder({
    required this.core,
    required this.capture,
    required this.onStopped,
    required this.onError,
    this.pollInterval = const Duration(milliseconds: 100),
  });

  final CoreApi core;
  final AndroidCapture capture;

  /// The projection ended (the person or the system stopped it).
  final void Function(String reason) onStopped;

  /// The encoder or the projection failed, or the event channel broke. Technical English.
  final void Function(String message) onError;

  /// How often the Rust core is asked whether it wants a key frame. The core rate limits
  /// requests to one per second, so 100 ms costs nothing noticeable.
  final Duration pollInterval;

  StreamSubscription<CaptureEvent>? _sub;
  Timer? _poll;
  CaptureConfig? _config;
  bool _configAccepted = false;
  bool _stopped = false;
  bool _ended = false;

  /// Counters for tests and diagnostics.
  int framesPushed = 0;
  int framesRefused = 0;
  int syncFrameRequests = 0;
  PushResult? lastResult;

  /// Start listening, show the consent dialog, start the capture. Returns [denied] if the
  /// person said no (nothing keeps running then). Throws if the native side fails.
  Future<CaptureStartResult> start(
    CaptureNotificationTexts notification,
  ) async {
    _stopped = false;
    _ended = false;
    // Listen first: the Kotlin side drops frames while nobody listens.
    _sub = capture.events.listen(
      _onEvent,
      onError: (Object e) => _fail('capture event channel: $e'),
    );
    final CaptureStartResult result;
    try {
      result = await capture.start(notification: notification);
    } catch (_) {
      _closeListeners();
      rethrow;
    }
    if (result != CaptureStartResult.started) {
      _closeListeners();
      return result;
    }
    _poll = Timer.periodic(pollInterval, (_) => _pollKeyFrameRequest());
    return result;
  }

  /// Stop the capture service and everything around it. Safe to call more than once.
  Future<void> stop() async {
    if (_stopped) return;
    _stopped = true;
    _closeListeners();
    try {
      await capture.stop();
    } catch (_) {
      // Nothing to do: the service is gone or never ran.
    }
  }

  void _closeListeners() {
    _poll?.cancel();
    _poll = null;
    final sub = _sub;
    _sub = null;
    // Not awaited: cancelling only detaches the listener, nothing depends on it finishing (and
    // an awaited cancel can stall under the fake clock of widget tests).
    if (sub != null) unawaited(sub.cancel());
  }

  void _pollKeyFrameRequest() {
    if (_stopped || _ended) return;
    if (core.keyframeRequested()) {
      syncFrameRequests++;
      capture.requestSyncFrame().catchError((Object _) {
        // The encoder may just have stopped; the next request tries again.
      });
    }
  }

  void _onEvent(CaptureEvent e) {
    if (_stopped || _ended) return;
    switch (e) {
      case CaptureConfig():
        _config = e;
        _configAccepted = core.pushCodecConfig(e.data);
      case CaptureFrame():
        // The sender may have started after the config event arrived (the config is replayed
        // only once): hand it over again until the core takes it.
        final config = _config;
        if (!_configAccepted && config != null) {
          _configAccepted = core.pushCodecConfig(config.data);
        }
        final r = core.pushEncodedFrame(e.data, isKey: e.isKey, ptsUs: e.ptsUs);
        lastResult = r;
        if (r == PushResult.queued) {
          framesPushed++;
        } else {
          framesRefused++;
        }
      case CaptureStopped(:final reason):
        _ended = true;
        _closeListeners();
        onStopped(reason);
      case CaptureError(:final message):
        _fail(message);
    }
  }

  void _fail(String message) {
    if (_stopped || _ended) return;
    onError(message);
  }
}
