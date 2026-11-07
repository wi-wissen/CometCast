// SPDX-License-Identifier: GPL-3.0-or-later
// State of the "Send" screen (synthetic VP8 test stream, the screen of this computer, on
// Android the screen captured by the native MediaCodec encoder, or on iOS the screen sent by the
// ReplayKit broadcast extension, which makes its own connection: see [startBroadcast]).

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'capture_feeder.dart';
import 'core/android_capture.dart';
import 'core/core_api.dart';
import 'core/ios_broadcast.dart';

enum SenderPhase { idle, starting, running, finished, ended }

class SenderController extends ChangeNotifier {
  SenderController({
    required this.core,
    this.udpBind,
    this.capture,
    this.iosBroadcast,
    this.broadcastStartTimeout = const Duration(minutes: 2),
    this.clock = DateTime.now,
  });

  final CoreApi core;
  final String? udpBind;

  /// The Android screen capture; null where there is none (then [startCapture] must not be
  /// called).
  final AndroidCapture? capture;

  /// The iOS broadcast hand-over; null where there is none (then [startBroadcast] must not be
  /// called).
  final IosBroadcast? iosBroadcast;

  /// How long to wait for the person to start the broadcast in the system sheet. The sheet can
  /// be dismissed without any notice to the app, so without a limit the screen would wait forever.
  final Duration broadcastStartTimeout;

  /// The current time (replaced in tests).
  final DateTime Function() clock;

  /// Unix time in milliseconds of the last [startBroadcast].
  int _broadcastRequestedAtMs = 0;

  CaptureFeeder? _feeder;
  StreamSubscription<IosBroadcastStatus>? _broadcastSub;
  Timer? _broadcastTimer;

  /// Counts starts and stops, so a start that waited for the consent dialog notices that it
  /// was overtaken by a stop.
  int _generation = 0;

  StreamSubscription<SenderUiEvent>? _sub;

  SenderPhase phase = SenderPhase.idle;
  UiConnectionState? connectionState;
  int framesSent = 0;
  bool receiverLeft = false;

  /// True while the screen (not the test video) is being sent.
  bool sharingScreen = false;
  int encodedFrames = 0;
  int encodedBytes = 0;

  /// Android: the system dialog that asks for permission to capture the screen is open.
  bool awaitingConsent = false;

  /// Android: the person refused the system dialog.
  bool captureDenied = false;

  /// Android: the projection was ended by the person or the system while sending.
  bool captureStopped = false;

  /// Android: access units the Rust core dropped (queue full, waiting for a key frame, ...).
  /// iOS: units the extension dropped.
  int droppedUnits = 0;

  /// iOS: the system sheet is (or should be) open and the person has not started yet.
  bool awaitingBroadcast = false;

  /// iOS: the sheet could not be opened from the app; the person must use Control Center.
  bool broadcastPickerFailed = false;

  /// iOS: nobody started the broadcast within [broadcastStartTimeout].
  bool broadcastNotStarted = false;

  /// iOS: the last report of the extension.
  IosBroadcastStatus? broadcastStatus;

  /// iOS: why the broadcast failed or ended (null while it runs or after a clean stop).
  IosBroadcastError? broadcastError;

  /// Technical error text in English (shown after a translated "Error:" prefix).
  String? error;

  bool get isActive =>
      phase == SenderPhase.starting || phase == SenderPhase.running;

  /// Join room [code] on [server] and send the test video for [seconds] seconds, or with
  /// [screen] the screen of this computer until stopped.
  void start({
    required String code,
    required String server,
    int? seconds,
    ScreenRequest? screen,
  }) {
    _cancel();
    _releaseCapture();
    _resetState(sharing: screen != null);
    phase = SenderPhase.starting;
    notifyListeners();
    _sub = core
        .startSender(
          code: code,
          server: server.trim(),
          seconds: seconds,
          udpBind: udpBind,
          screen: screen,
        )
        .listen(
          _onEvent,
          onError: (Object e) {
            error = e.toString();
            phase = SenderPhase.ended;
            notifyListeners();
          },
          onDone: () {
            if (isActive) {
              phase = SenderPhase.ended;
              notifyListeners();
            }
          },
        );
  }

  void _resetState({required bool sharing}) {
    sharingScreen = sharing;
    encodedFrames = 0;
    encodedBytes = 0;
    droppedUnits = 0;
    connectionState = null;
    framesSent = 0;
    receiverLeft = false;
    awaitingConsent = false;
    captureDenied = false;
    captureStopped = false;
    awaitingBroadcast = false;
    broadcastPickerFailed = false;
    broadcastNotStarted = false;
    broadcastStatus = null;
    broadcastError = null;
    error = null;
  }

  /// iOS: hand the room code and the server to the broadcast extension and open the system
  /// sheet in which the person starts the broadcast. Nothing connects from here: the extension
  /// does, and reports through [broadcastStatus]. The phase stays [SenderPhase.starting] until
  /// the extension reports streaming.
  Future<void> startBroadcast({
    required String code,
    required String server,
    int maxDimension = 1280,
    int fps = 30,
    int bitrate = 2500000,
  }) async {
    final ios = iosBroadcast;
    if (ios == null) {
      throw StateError('no iOS broadcast on this platform');
    }
    _cancel();
    _releaseCapture();
    final generation = _generation;
    _resetState(sharing: true);
    _broadcastRequestedAtMs = clock().millisecondsSinceEpoch;
    phase = SenderPhase.starting;
    awaitingBroadcast = true;
    notifyListeners();
    _broadcastSub = ios.events.listen(
      (s) => _onBroadcast(generation, s),
      onError: (Object e) {
        if (_generation != generation) return;
        error = e.toString();
        _endBroadcast();
      },
    );
    try {
      await ios.prepare(
        IosBroadcastRequest(
          code: code,
          relay: server.trim(),
          maxDimension: maxDimension,
          fps: fps,
          bitrate: bitrate,
        ),
      );
      if (_generation != generation) return;
      final opened = await ios.showPicker();
      if (_generation != generation) return;
      broadcastPickerFailed = !opened;
    } catch (e) {
      if (_generation != generation) return;
      error = e.toString();
      _endBroadcast();
      return;
    }
    _broadcastTimer = Timer(broadcastStartTimeout, () {
      if (_generation != generation || !awaitingBroadcast) return;
      broadcastNotStarted = true;
      _endBroadcast(next: SenderPhase.idle);
    });
    notifyListeners();
  }

  void _onBroadcast(int generation, IosBroadcastStatus s) {
    if (_generation != generation) return;
    // A report written before this start request belongs to an earlier broadcast (for example
    // one that was still shutting down): ignore it. A report without a time is taken as new.
    final at = s.updatedAtMs;
    if (at != null && at < _broadcastRequestedAtMs - 2000) return;
    broadcastStatus = s;
    awaitingBroadcast = false;
    _broadcastTimer?.cancel();
    _broadcastTimer = null;
    encodedFrames = s.sent;
    encodedBytes = s.bytes;
    droppedUnits = s.dropped;
    framesSent = s.sent;
    switch (s.state) {
      case IosBroadcastState.starting:
      case IosBroadcastState.connecting:
      case IosBroadcastState.waiting:
        phase = SenderPhase.starting;
      case IosBroadcastState.streaming:
      case IosBroadcastState.reconnecting:
      case IosBroadcastState.paused:
        phase = SenderPhase.running;
      case IosBroadcastState.ended:
        if (s.error == IosBroadcastError.peerLeft) {
          receiverLeft = true;
        } else {
          captureStopped = true;
        }
        _endBroadcast(keepStopFlag: true);
        return;
      case IosBroadcastState.failed:
      case IosBroadcastState.lost:
        broadcastError = s.state == IosBroadcastState.lost
            ? IosBroadcastError.internalError
            : s.error;
        _endBroadcast(keepStopFlag: true);
        return;
    }
    notifyListeners();
  }

  /// End the iOS broadcast session: stop listening, tell the extension to stop (it normally
  /// has already), and go to [next].
  void _endBroadcast({
    SenderPhase next = SenderPhase.ended,
    bool keepStopFlag = false,
  }) {
    _broadcastTimer?.cancel();
    _broadcastTimer = null;
    _broadcastSub?.cancel();
    _broadcastSub = null;
    awaitingBroadcast = false;
    phase = next;
    if (!keepStopFlag) captureStopped = false;
    notifyListeners();
    final ios = iosBroadcast;
    if (ios != null) unawaited(ios.stop().catchError((Object _) {}));
  }

  /// Android: ask for permission to capture the screen, start the native capture and join the
  /// room [code] on [server] to send what it encodes, until stopped. The permission dialog
  /// comes first: if the person refuses, nothing connects ([captureDenied]).
  Future<void> startCapture({
    required String code,
    required String server,
    required CaptureNotificationTexts notification,
  }) async {
    final cap = capture;
    if (cap == null) {
      throw StateError('no Android capture on this platform');
    }
    _cancel();
    // The previous feeder (if any) must not leak or keep the service: release it first.
    _releaseCapture();
    final generation = _generation;
    _resetState(sharing: true);
    phase = SenderPhase.starting;
    awaitingConsent = true;
    notifyListeners();
    final feeder = CaptureFeeder(
      core: core,
      capture: cap,
      // Only the newest session may stop the singleton capture service.
      ownsCapture: () => _generation == generation || _feeder == null,
      onStopped: (_) {
        if (_generation != generation) return;
        captureStopped = true;
        _endCapture(SenderPhase.ended);
      },
      onError: (message) {
        if (_generation != generation) return;
        error = message;
        _endCapture(SenderPhase.ended);
      },
    );
    _feeder = feeder;
    final CaptureStartResult result;
    try {
      result = await feeder.start(notification);
    } catch (e) {
      if (_generation != generation) return;
      _feeder = null;
      awaitingConsent = false;
      error = e.toString();
      phase = SenderPhase.ended;
      notifyListeners();
      return;
    }
    // A stop (or a newer start) while the dialog was open: the stop already cleaned up, but the
    // capture may have been started in the meantime.
    if (_generation != generation) {
      await feeder.stop();
      return;
    }
    awaitingConsent = false;
    if (result == CaptureStartResult.denied) {
      _feeder = null;
      captureDenied = true;
      phase = SenderPhase.idle;
      notifyListeners();
      return;
    }
    _sub = core
        .startEncodedSender(code: code, server: server.trim(), udpBind: udpBind)
        .listen(
          _onEvent,
          onError: (Object e) {
            error = e.toString();
            _endCapture(SenderPhase.ended);
          },
          onDone: () {
            if (isActive) _endCapture(SenderPhase.ended);
          },
        );
    notifyListeners();
  }

  /// Stop the native capture and the sender, and go to [next].
  void _endCapture(SenderPhase next) {
    final feeder = _feeder;
    _feeder = null;
    if (feeder != null) unawaited(feeder.stop());
    _cancel();
    phase = next;
    notifyListeners();
    // The Rust sender ends on its own when the stream is cancelled, but only at its next event.
    unawaited(core.stopSender());
  }

  void _onEvent(SenderUiEvent e) {
    switch (e) {
      case SenderUiEvent_Joined():
        phase = SenderPhase.running;
      case SenderUiEvent_ConnectionState(:final state):
        connectionState = state;
      case SenderUiEvent_Streaming(:final framesSent):
        this.framesSent = framesSent.toInt();
      case SenderUiEvent_ScreenStats(:final encoded, :final encodedBytes):
        encodedFrames = encoded.toInt();
        this.encodedBytes = encodedBytes.toInt();
      case SenderUiEvent_EncodedStats(
        :final sent,
        :final bytes,
        :final dropped,
      ):
        encodedFrames = sent.toInt();
        encodedBytes = bytes.toInt();
        droppedUnits = dropped.toInt();
      case SenderUiEvent_Finished(:final framesSent):
        this.framesSent = framesSent.toInt();
        phase = SenderPhase.finished;
        _releaseCapture();
      case SenderUiEvent_PeerLeft():
        receiverLeft = true;
        phase = SenderPhase.ended;
        _releaseCapture();
      case SenderUiEvent_Error(:final message):
        error = message;
        phase = SenderPhase.ended;
        _releaseCapture();
    }
    notifyListeners();
  }

  /// The sender ended by itself: the native capture must not keep running (and showing its
  /// notification).
  void _releaseCapture() {
    final feeder = _feeder;
    _feeder = null;
    if (feeder != null) unawaited(feeder.stop());
  }

  Future<void> stop() async {
    _generation++;
    awaitingConsent = false;
    final feeder = _feeder;
    _feeder = null;
    final hadBroadcast = _broadcastSub != null || awaitingBroadcast;
    awaitingBroadcast = false;
    _cancel();
    phase = SenderPhase.idle;
    notifyListeners();
    await feeder?.stop();
    if (hadBroadcast) {
      try {
        await iosBroadcast?.stop();
      } catch (_) {
        // Nothing to stop, or the platform is gone.
      }
    }
    await core.stopSender();
  }

  void _cancel() {
    _generation++;
    _sub?.cancel();
    _sub = null;
    _broadcastTimer?.cancel();
    _broadcastTimer = null;
    _broadcastSub?.cancel();
    _broadcastSub = null;
  }

  @override
  void dispose() {
    final hadBroadcast = _broadcastSub != null || awaitingBroadcast;
    _cancel();
    // Leaving the screen ends the broadcast (like the Android capture): the extension is its
    // own process and would otherwise keep sending without any screen to stop it from.
    if (hadBroadcast) {
      unawaited(
        iosBroadcast?.stop().catchError((Object _) {}) ?? Future<void>.value(),
      );
    }
    final feeder = _feeder;
    _feeder = null;
    if (feeder != null) unawaited(feeder.stop());
    unawaited(core.stopSender());
    super.dispose();
  }
}
