// SPDX-License-Identifier: GPL-3.0-or-later
// The part of the Rust core that the UI uses, as a small Dart interface.
//
// Why an interface: widget tests plug in a fake (no native library needed), the real app
// plugs in [RustCoreApi], which calls the generated flutter_rust_bridge code.

import 'dart:typed_data';

import 'package:cometcast/src/rust/api.dart' as rust;
import 'package:cometcast/src/rust/api.dart'
    show
        CaptureInfo,
        PushResult,
        ReceiverUiEvent,
        RoomCodeCheck,
        ScreenRequest,
        SenderUiEvent,
        ServerDefaults,
        VideoFrame;

// The event and result classes (generated) are re-exported for the screens and tests.
export 'package:cometcast/src/rust/api.dart'
    hide
        startReceiver,
        stopReceiver,
        startSender,
        startEncodedSender,
        pushEncodedFrame,
        pushCodecConfig,
        keyframeRequested,
        stopSender,
        checkRoomCode,
        roomCodeAlphabet,
        roomCodeLength,
        takeFrame,
        setVideoMaxSize,
        captureInfo,
        serverDefaults;

abstract class CoreApi {
  /// Connect to [server], create a room and report events. The first event is the room code.
  /// A start failure arrives as an `error` event, after which the stream ends.
  Stream<ReceiverUiEvent> startReceiver({
    required String server,
    String? udpBind,
  });

  Future<void> stopReceiver();

  /// The newest decoded picture (RGBA) that was not fetched yet, or null. Call it after a
  /// `frameReady` event; fetching re-arms that event.
  Future<VideoFrame?> takeFrame();

  /// Size of the drawing area in physical pixels; the core scales pictures down to fit.
  void setVideoMaxSize(int width, int height);

  /// Join the room [code] and send the built-in test video for [seconds] seconds, or with
  /// [screen] the screen of this computer (until stopped when [seconds] is null).
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
    ScreenRequest? screen,
  });

  /// Join the room [code] and send the H.264 access units that the host pushes with
  /// [pushEncodedFrame] (Android: the output of the native screen encoder) until stopped.
  /// Same event rules as [startSender].
  Stream<SenderUiEvent> startEncodedSender({
    required String code,
    required String server,
    String? udpBind,
  });

  /// Hand one H.264 access unit (Annex B or AVCC) to the encoded sender. The bytes are copied.
  /// Never blocks; a unit that cannot be taken is dropped and the result says why.
  PushResult pushEncodedFrame(
    Uint8List data, {
    required bool isKey,
    required int ptsUs,
  });

  /// Hand the codec configuration (SPS/PPS, Annex B or avcC) to the encoded sender. False if
  /// no encoded sender runs or the data has no parameter sets.
  bool pushCodecConfig(Uint8List data);

  /// True once per key frame request of the Rust core: ask the encoder for a sync frame.
  bool keyframeRequested();

  /// Can this system share its screen, and which monitors exist? [display] forces an X11
  /// display such as `:99` (development and tests).
  Future<CaptureInfo> captureInfo({String? display});

  /// Server addresses to pre-fill (from cc-discovery) and the development fallback.
  ServerDefaults serverDefaults();

  Future<void> stopSender();

  /// Same rules as `cc_proto::RoomCode::parse` (the Rust code is the single source of truth).
  RoomCodeCheck checkRoomCode(String input);

  /// Number of characters in a room code.
  int get roomCodeLength;

  /// The characters a room code may contain (uppercase).
  String get roomCodeAlphabet;
}

/// The real implementation: calls the Rust library.
class RustCoreApi implements CoreApi {
  const RustCoreApi();

  @override
  Stream<ReceiverUiEvent> startReceiver({
    required String server,
    String? udpBind,
  }) => rust.startReceiver(server: server, udpBind: udpBind);

  @override
  Future<void> stopReceiver() => rust.stopReceiver();

  @override
  Future<VideoFrame?> takeFrame() => rust.takeFrame();

  @override
  void setVideoMaxSize(int width, int height) =>
      rust.setVideoMaxSize(width: width, height: height);

  @override
  Future<CaptureInfo> captureInfo({String? display}) =>
      rust.captureInfo(display: display);

  @override
  ServerDefaults serverDefaults() => rust.serverDefaults();

  @override
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
    ScreenRequest? screen,
  }) => rust.startSender(
    code: code,
    server: server,
    seconds: seconds,
    udpBind: udpBind,
    screen: screen,
  );

  @override
  Stream<SenderUiEvent> startEncodedSender({
    required String code,
    required String server,
    String? udpBind,
  }) => rust.startEncodedSender(code: code, server: server, udpBind: udpBind);

  @override
  PushResult pushEncodedFrame(
    Uint8List data, {
    required bool isKey,
    required int ptsUs,
  }) => rust.pushEncodedFrame(data: data, isKey: isKey, ptsUs: ptsUs);

  @override
  bool pushCodecConfig(Uint8List data) => rust.pushCodecConfig(data: data);

  @override
  bool keyframeRequested() => rust.keyframeRequested();

  @override
  Future<void> stopSender() => rust.stopSender();

  @override
  RoomCodeCheck checkRoomCode(String input) => rust.checkRoomCode(input: input);

  @override
  int get roomCodeLength => rust.roomCodeLength();

  @override
  String get roomCodeAlphabet => rust.roomCodeAlphabet();
}
