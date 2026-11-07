// SPDX-License-Identifier: GPL-3.0-or-later
// State of the "Send" screen (synthetic VP8 test stream, the screen of this computer, or on
// Android the screen captured by the native MediaCodec encoder).

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'capture_feeder.dart';
import 'core/android_capture.dart';
import 'core/core_api.dart';
import 'join_state.dart';

enum SenderPhase { idle, starting, waitingApproval, running, finished, ended }

class SenderController extends ChangeNotifier {
  SenderController({required this.core, this.udpBind, this.capture});

  final CoreApi core;
  final String? udpBind;

  /// The Android screen capture; null where there is none (then [startCapture] must not be
  /// called).
  final AndroidCapture? capture;

  CaptureFeeder? _feeder;

  /// Counts starts and stops, so a start that waited for the consent dialog notices that it
  /// was overtaken by a stop.
  int _generation = 0;

  StreamSubscription<SenderUiEvent>? _sub;

  SenderPhase phase = SenderPhase.idle;
  UiConnectionState? connectionState;
  int framesSent = 0;
  bool receiverLeft = false;

  /// While [SenderPhase.waitingApproval]: the check number and time limit.
  ApprovalWait? approval;

  /// Why this sender is not in the room (denied, locked, timed out, removed), or null.
  JoinRefusal? refusal;

  /// When the join was refused (for the pause before asking again).
  DateTime? refusedAt;

  /// Seconds until a denied sender may ask again (0 = now, or not denied).
  int get cooldownLeft {
    final at = refusedAt;
    if (refusal != JoinRefusal.denied || at == null) return 0;
    final left = deniedCooldownSeconds - joinClock().difference(at).inSeconds;
    return left < 0 ? 0 : left;
  }

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
  int droppedUnits = 0;

  /// Technical error text in English (shown after a translated "Error:" prefix).
  String? error;

  bool get isActive =>
      phase == SenderPhase.starting ||
      phase == SenderPhase.waitingApproval ||
      phase == SenderPhase.running;

  /// Join room [code] on [server] and send the test video for [seconds] seconds, or with
  /// [screen] the screen of this computer until stopped.
  void start({
    required String code,
    required String server,
    int? seconds,
    ScreenRequest? screen,
    String? name,
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
          name: _nameOrNull(name),
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
    approval = null;
    refusal = null;
    refusedAt = null;
    awaitingConsent = false;
    captureDenied = false;
    captureStopped = false;
    error = null;
  }

  /// Android: ask for permission to capture the screen, start the native capture and join the
  /// room [code] on [server] to send what it encodes, until stopped. The permission dialog
  /// comes first: if the person refuses, nothing connects ([captureDenied]).
  Future<void> startCapture({
    required String code,
    required String server,
    required CaptureNotificationTexts notification,
    String? name,
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
        .startEncodedSender(
          code: code,
          server: server.trim(),
          udpBind: udpBind,
          name: _nameOrNull(name),
        )
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
      case SenderUiEvent_WaitingForApproval(:final timeoutSecs, :final check):
        approval = ApprovalWait(check: check, timeoutSecs: timeoutSecs);
        phase = SenderPhase.waitingApproval;
      case SenderUiEvent_JoinRefused(:final code):
        refusal = refusalFromCode(
          code,
          wasWaiting: phase == SenderPhase.waitingApproval,
        );
        refusedAt = joinClock();
        approval = null;
        phase = SenderPhase.ended;
        _releaseCapture();
      case SenderUiEvent_RemovedByHost():
        refusal = JoinRefusal.removed;
        refusedAt = joinClock();
        phase = SenderPhase.ended;
        _releaseCapture();
      case SenderUiEvent_Joined():
        approval = null;
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

  static String? _nameOrNull(String? name) {
    final n = name?.trim();
    return (n == null || n.isEmpty) ? null : n;
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
    _cancel();
    phase = SenderPhase.idle;
    notifyListeners();
    await feeder?.stop();
    await core.stopSender();
  }

  void _cancel() {
    _generation++;
    _sub?.cancel();
    _sub = null;
  }

  @override
  void dispose() {
    _cancel();
    final feeder = _feeder;
    _feeder = null;
    if (feeder != null) unawaited(feeder.stop());
    unawaited(core.stopSender());
    super.dispose();
  }
}
