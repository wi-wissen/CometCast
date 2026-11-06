// SPDX-License-Identifier: GPL-3.0-or-later
// The part of the Rust core that the UI uses, as a small Dart interface.
//
// Why an interface: widget tests plug in a fake (no native library needed), the real app
// plugs in [RustCoreApi], which calls the generated flutter_rust_bridge code.

import 'package:cometcast/src/rust/api.dart' as rust;
import 'package:cometcast/src/rust/api.dart'
    show ReceiverUiEvent, RoomCodeCheck, SenderUiEvent;

// The event and result classes (generated) are re-exported for the screens and tests.
export 'package:cometcast/src/rust/api.dart'
    hide
        startReceiver,
        stopReceiver,
        startSender,
        stopSender,
        checkRoomCode,
        roomCodeAlphabet,
        roomCodeLength;

abstract class CoreApi {
  /// Connect to [server], create a room and report events. The first event is the room code.
  /// A start failure arrives as an `error` event, after which the stream ends.
  Stream<ReceiverUiEvent> startReceiver({
    required String server,
    String? udpBind,
  });

  Future<void> stopReceiver();

  /// Join the room [code] and send the built-in test video for [seconds] seconds.
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
  });

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
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
  }) => rust.startSender(
    code: code,
    server: server,
    seconds: seconds,
    udpBind: udpBind,
  );

  @override
  Future<void> stopSender() => rust.stopSender();

  @override
  RoomCodeCheck checkRoomCode(String input) => rust.checkRoomCode(input: input);

  @override
  int get roomCodeLength => rust.roomCodeLength();

  @override
  String get roomCodeAlphabet => rust.roomCodeAlphabet();
}
