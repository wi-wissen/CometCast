// SPDX-License-Identifier: GPL-3.0-or-later
// A stand-in for the Rust core in widget tests (no native library needed).

import 'dart:async';

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
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
  }) {
    senderCode = code;
    senderServer = server;
    senderSeconds = seconds;
    return senderEvents.stream;
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
