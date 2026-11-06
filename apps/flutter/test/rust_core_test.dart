// SPDX-License-Identifier: GPL-3.0-or-later
// Calls the REAL Rust library through flutter_rust_bridge.
// Needs the library: `cargo build -p cc-ffi` (default path target/debug/libcc_ffi.so) or
// COMETCAST_FFI_LIB=/path/to/libcc_ffi.so. Without the library the tests are skipped.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/rust_loader.dart';

import 'fake_core.dart';

String? findLibrary() {
  final env = Platform.environment[ffiLibraryEnvVar];
  if (env != null && env.isNotEmpty) return env;
  // `flutter test` runs in apps/flutter; the workspace target folder is two levels up.
  const candidates = [
    '../../target/debug/libcc_ffi.so',
    '../../target/debug/libcc_ffi.dylib',
  ];
  for (final c in candidates) {
    if (File(c).existsSync()) return File(c).absolute.path;
  }
  return null;
}

void main() {
  final lib = findLibrary();
  final skip = lib == null
      ? 'Rust library not built (run: cargo build -p cc-ffi)'
      : null;

  setUpAll(() async {
    if (lib != null) await initRustCore(libraryPath: lib);
  });

  test('room code rules come from Rust and match the Dart fake', () {
    const core = RustCoreApi();
    final fake = FakeCoreApi();
    expect(core.roomCodeLength, 6);
    for (final input in [
      'K7M2QX',
      ' k7m2qx ',
      'K7M2',
      'K70',
      'K7M2QO',
      'K7M2Q1',
      '',
      'ſ7M2QX',
    ]) {
      final real = core.checkRoomCode(input);
      final copy = fake.checkRoomCode(input);
      expect(real.runtimeType, copy.runtimeType, reason: 'input "$input"');
    }
    final ok = core.checkRoomCode(' k7m2qx ');
    expect(ok, isA<RoomCodeCheck_Valid>());
    expect((ok as RoomCodeCheck_Valid).code, 'K7M2QX');
  }, skip: skip);

  test('starting a receiver on a closed port reports an error event', () async {
    const core = RustCoreApi();
    final events = await core.startReceiver(server: '127.0.0.1:1').toList();
    expect(events, hasLength(1));
    expect(events.single, isA<ReceiverUiEvent_Error>());
  }, skip: skip);

  test('starting a sender with a bad code reports an error event', () async {
    const core = RustCoreApi();
    final events =
        await core.startSender(code: 'nope', server: '127.0.0.1:1').toList();
    expect(events, hasLength(1));
    expect(events.single, isA<SenderUiEvent_Error>());
  }, skip: skip);
}
