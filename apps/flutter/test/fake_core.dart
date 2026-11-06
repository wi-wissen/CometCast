// SPDX-License-Identifier: GPL-3.0-or-later
// A stand-in for the Rust core in widget tests (no native library needed).

import 'dart:async';
import 'dart:typed_data';

import 'package:cometcast/core/core_api.dart';

class FakeCoreApi implements CoreApi {
  final receiverEvents = StreamController<ReceiverUiEvent>.broadcast();
  final senderEvents = StreamController<SenderUiEvent>.broadcast();

  String? receiverServer;
  String? senderCode;
  String? senderServer;
  int? senderSeconds;
  int stopReceiverCalls = 0;
  int stopSenderCalls = 0;
  ScreenRequest? senderScreen;

  // The encoded (Android capture) path.
  String? encodedCode;
  String? encodedServer;
  int startEncodedCalls = 0;
  final pushedFrames = <({Uint8List data, bool isKey, int ptsUs})>[];
  final pushedConfigs = <Uint8List>[];

  /// What pushCodecConfig answers (false = "no encoded sender yet").
  bool acceptConfig = true;
  PushResult pushResult = PushResult.queued;

  /// Answer of the next keyframeRequested call (cleared by it, like the real flag).
  bool keyframeRequest = false;

  /// Pictures waiting for takeFrame (the newest is handed out, the others are dropped, like
  /// the real mailbox).
  final frames = <VideoFrame>[];
  int takeFrameCalls = 0;

  /// If set, the next takeFrame picks its picture at once but answers only after this.
  Completer<void>? takeFrameGate;

  /// Number of upcoming takeFrame calls that throw.
  int takeFrameFailures = 0;
  final maxSizes = <(int, int)>[];

  /// What captureInfo answers (default: X11 works, whole desktop plus one monitor).
  CaptureInfo captureInfoResult = const CaptureInfo(
    support: CaptureSupport.supported(),
    display: ':0',
    monitors: [
      CaptureMonitor(
        id: 'root',
        name: 'Whole desktop',
        x: 0,
        y: 0,
        width: 640,
        height: 480,
      ),
      CaptureMonitor(
        id: 'out:HDMI-1',
        name: 'HDMI-1',
        x: 0,
        y: 0,
        width: 640,
        height: 480,
      ),
    ],
  );
  String? captureInfoDisplay;

  @override
  Stream<ReceiverUiEvent> startReceiver({
    required String server,
    String? udpBind,
  }) {
    receiverServer = server;
    return receiverEvents.stream;
  }

  @override
  Future<void> stopReceiver() async => stopReceiverCalls++;

  @override
  Future<VideoFrame?> takeFrame() async {
    takeFrameCalls++;
    if (takeFrameFailures > 0) {
      takeFrameFailures--;
      throw StateError('takeFrame failed');
    }
    final newest = frames.isEmpty ? null : frames.last;
    frames.clear();
    final gate = takeFrameGate;
    takeFrameGate = null;
    if (gate != null) await gate.future;
    return newest;
  }

  @override
  void setVideoMaxSize(int width, int height) => maxSizes.add((width, height));

  @override
  Future<CaptureInfo> captureInfo({String? display}) async {
    captureInfoDisplay = display;
    return captureInfoResult;
  }

  // Same content as the real list of cc-discovery at the time of writing; the real values
  // are checked in rust_core_test.dart.
  @override
  ServerDefaults serverDefaults() => const ServerDefaults(
    servers: ['relay.cometcast.de:443'],
    devFallback: '127.0.0.1:8443',
  );

  @override
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
    ScreenRequest? screen,
  }) {
    senderScreen = screen;
    senderCode = code;
    senderServer = server;
    senderSeconds = seconds;
    return senderEvents.stream;
  }

  @override
  Stream<SenderUiEvent> startEncodedSender({
    required String code,
    required String server,
    String? udpBind,
  }) {
    startEncodedCalls++;
    encodedCode = code;
    encodedServer = server;
    return senderEvents.stream;
  }

  @override
  PushResult pushEncodedFrame(
    Uint8List data, {
    required bool isKey,
    required int ptsUs,
  }) {
    pushedFrames.add((data: data, isKey: isKey, ptsUs: ptsUs));
    return pushResult;
  }

  @override
  bool pushCodecConfig(Uint8List data) {
    pushedConfigs.add(data);
    return acceptConfig;
  }

  @override
  bool keyframeRequested() {
    final r = keyframeRequest;
    keyframeRequest = false;
    return r;
  }

  @override
  Future<void> stopSender() async => stopSenderCalls++;

  // The real rules live in Rust (cc-proto). This copy only exists for tests without the
  // native library; rust_core_test.dart checks that both agree.
  static const _alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';

  @override
  String get roomCodeAlphabet => _alphabet;

  @override
  int get roomCodeLength => 6;

  @override
  RoomCodeCheck checkRoomCode(String input) {
    // ASCII-only uppercase, like Rust's to_ascii_uppercase ('ſ' must stay invalid).
    final upper = input.trim().replaceAllMapped(
      RegExp('[a-z]'),
      (m) => m[0]!.toUpperCase(),
    );
    for (final c in upper.split('')) {
      if (!_alphabet.contains(c)) return RoomCodeCheck.invalidChar(ch: c);
    }
    if (upper.length != roomCodeLength) {
      return const RoomCodeCheck.wrongLength();
    }
    return RoomCodeCheck.valid(code: upper);
  }
}
