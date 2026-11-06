// SPDX-License-Identifier: GPL-3.0-or-later
// State of the "Send" screen (synthetic VP8 test stream, no screen capture yet).

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';

enum SenderPhase { idle, starting, running, finished, ended }

class SenderController extends ChangeNotifier {
  SenderController({required this.core, this.udpBind});

  final CoreApi core;
  final String? udpBind;

  StreamSubscription<SenderUiEvent>? _sub;

  SenderPhase phase = SenderPhase.idle;
  UiConnectionState? connectionState;
  int framesSent = 0;
  bool receiverLeft = false;

  /// Technical error text in English (shown after a translated "Error:" prefix).
  String? error;

  bool get isActive =>
      phase == SenderPhase.starting || phase == SenderPhase.running;

  /// Join room [code] on [server] and send the test video for [seconds] seconds.
  void start({
    required String code,
    required String server,
    required int seconds,
  }) {
    _cancel();
    phase = SenderPhase.starting;
    connectionState = null;
    framesSent = 0;
    receiverLeft = false;
    error = null;
    notifyListeners();
    _sub = core
        .startSender(
          code: code,
          server: server.trim(),
          seconds: seconds,
          udpBind: udpBind,
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

  void _onEvent(SenderUiEvent e) {
    switch (e) {
      case SenderUiEvent_Joined():
        phase = SenderPhase.running;
      case SenderUiEvent_ConnectionState(:final state):
        connectionState = state;
      case SenderUiEvent_Streaming(:final framesSent):
        this.framesSent = framesSent.toInt();
      case SenderUiEvent_Finished(:final framesSent):
        this.framesSent = framesSent.toInt();
        phase = SenderPhase.finished;
      case SenderUiEvent_PeerLeft():
        receiverLeft = true;
        phase = SenderPhase.ended;
      case SenderUiEvent_Error(:final message):
        error = message;
        phase = SenderPhase.ended;
    }
    notifyListeners();
  }

  Future<void> stop() async {
    _cancel();
    phase = SenderPhase.idle;
    notifyListeners();
    await core.stopSender();
  }

  void _cancel() {
    _sub?.cancel();
    _sub = null;
  }

  @override
  void dispose() {
    _cancel();
    unawaited(core.stopSender());
    super.dispose();
  }
}
