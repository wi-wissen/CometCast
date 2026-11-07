// SPDX-License-Identifier: GPL-3.0-or-later
// A stand-in for the Miracast DLL in tests (no Windows, no native library).

import 'dart:typed_data';

import 'package:cometcast/core/miracast.dart';

class FakeMiracastCore implements MiracastCore {
  MiracastProbe probeResult = const MiracastProbe(receiverAvailable: true);
  MiracastState current = MiracastState.off;
  final calls = <String>[];

  /// Thrown by start (once, then cleared).
  MiracastCallException? startError;

  /// Thrown by state() while set.
  Object? stateError;

  /// What start() does to the state: listening at once (the real one does this a moment later).
  bool listenOnStart = true;
  int generation = 0;
  String? startedName;
  MiracastPairing? startedPairing;
  final responses = <(int, bool)>[];
  final frames = <MiracastFrame>[];
  int pollFrameCalls = 0;
  bool muted = false;

  /// A sender asks to connect.
  void request(int id, String name, {String? pin}) {
    current = MiracastState(
      status: MiracastStatus.connectionPending,
      generation: generation,
      pending: MiracastRequest(id: id, name: name, pin: pin),
    );
  }

  /// The sender is connected.
  void connect(String name) {
    current = MiracastState(
      status: MiracastStatus.connected,
      generation: generation,
      connected: name,
      frames: const MiracastFrameStats(published: 1, width: 4, height: 2),
    );
  }

  void listening() {
    current = MiracastState(
      status: MiracastStatus.listening,
      generation: generation,
    );
  }

  /// The request timed out on the Rust side.
  void requestGone() => listening();

  void addFrame(int seq) {
    frames.add(
      MiracastFrame(width: 4, height: 2, seq: seq, bgra: Uint8List(4 * 2 * 4)),
    );
  }

  @override
  Future<MiracastProbe> probe() async {
    calls.add('probe');
    return probeResult;
  }

  @override
  Future<void> start({
    required String name,
    required MiracastPairing pairing,
  }) async {
    calls.add('start');
    final e = startError;
    if (e != null) {
      startError = null;
      throw e;
    }
    startedName = name;
    startedPairing = pairing;
    generation++;
    if (listenOnStart) {
      listening();
    } else {
      current = MiracastState(
        status: MiracastStatus.starting,
        generation: generation,
      );
    }
  }

  @override
  Future<void> stop() async {
    calls.add('stop');
    current = MiracastState.off;
  }

  @override
  Future<MiracastState> state() async {
    final e = stateError;
    if (e != null) throw e;
    return current;
  }

  @override
  Future<void> respond(int id, bool accept) async {
    calls.add('respond');
    responses.add((id, accept));
    // Like the real core: the request is gone; "connected" follows with a later event.
    listening();
  }

  @override
  Future<void> disconnect() async {
    calls.add('disconnect');
    listening();
  }

  @override
  Future<MiracastFrame?> pollFrame() async {
    pollFrameCalls++;
    if (frames.isEmpty) return null;
    return frames.removeAt(0);
  }

  @override
  Future<void> setMaxSize(int width, int height) async {}

  @override
  Future<void> setMuted(bool m) async {
    muted = m;
  }
}
