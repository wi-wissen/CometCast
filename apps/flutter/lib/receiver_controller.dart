// SPDX-License-Identifier: GPL-3.0-or-later
// State of the "Receive on this device" screen. Like a Vue composable (store + actions):
// the screen listens to it and rebuilds when it calls notifyListeners().

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';

enum ReceiverPhase {
  /// Nothing started (or stopped by the user).
  idle,

  /// Waiting for the server to hand out a room code.
  starting,

  /// Room exists, events are flowing.
  running,

  /// The stream ended on its own (sender left or error).
  ended,
}

class ReceiverController extends ChangeNotifier {
  ReceiverController({required this.core, this.udpBind});

  final CoreApi core;
  final String? udpBind;

  StreamSubscription<ReceiverUiEvent>? _sub;

  ReceiverPhase phase = ReceiverPhase.idle;
  String? roomCode;
  bool senderJoined = false;
  bool senderLeft = false;
  UiConnectionState? connectionState;
  String? codec;
  int packets = 0;
  int bytes = 0;
  int frames = 0;
  int keyframes = 0;

  /// Technical error text in English (shown after a translated "Error:" prefix).
  String? error;

  bool get isActive =>
      phase == ReceiverPhase.starting || phase == ReceiverPhase.running;

  /// Create a room on [server] and start listening.
  void start(String server) {
    _cancel();
    phase = ReceiverPhase.starting;
    roomCode = null;
    senderJoined = false;
    senderLeft = false;
    connectionState = null;
    codec = null;
    packets = bytes = frames = keyframes = 0;
    error = null;
    notifyListeners();
    _sub = core
        .startReceiver(server: server.trim(), udpBind: udpBind)
        .listen(
          _onEvent,
          onError: (Object e) {
            error = _describe(e);
            phase = ReceiverPhase.ended;
            notifyListeners();
          },
          onDone: () {
            // The stream also ends after stop(); only mark "ended" if it was not the user.
            if (phase == ReceiverPhase.running ||
                phase == ReceiverPhase.starting) {
              phase = ReceiverPhase.ended;
              notifyListeners();
            }
          },
        );
  }

  void _onEvent(ReceiverUiEvent e) {
    switch (e) {
      case ReceiverUiEvent_RoomCode(:final code):
        roomCode = code;
        phase = ReceiverPhase.running;
      case ReceiverUiEvent_PeerJoined():
        senderJoined = true;
      case ReceiverUiEvent_ConnectionState(:final state):
        connectionState = state;
      case ReceiverUiEvent_VideoTrack(:final codec):
        this.codec = codec;
      case ReceiverUiEvent_Stats():
        packets = e.packets.toInt();
        bytes = e.bytes.toInt();
        frames = e.frames.toInt();
        keyframes = e.keyframes.toInt();
      case ReceiverUiEvent_PeerLeft():
        senderLeft = true;
        phase = ReceiverPhase.ended;
      case ReceiverUiEvent_Error(:final message):
        error = message;
        phase = ReceiverPhase.ended;
    }
    notifyListeners();
  }

  /// Stop receiving. The room code disappears (the room is gone).
  Future<void> stop() async {
    _cancel();
    phase = ReceiverPhase.idle;
    roomCode = null;
    notifyListeners();
    await core.stopReceiver();
  }

  void _cancel() {
    _sub?.cancel();
    _sub = null;
  }

  static String _describe(Object e) => e.toString();

  @override
  void dispose() {
    _cancel();
    // Fire and forget: the screen is gone, the core must not keep a room open.
    unawaited(core.stopReceiver());
    super.dispose();
  }
}
