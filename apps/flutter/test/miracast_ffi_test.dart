// SPDX-License-Identifier: GPL-3.0-or-later
// Calls the REAL Rust library of the Miracast receiver through dart:ffi.
// Needs the library: `cargo build -p cc-miracast-ffi` and COMETCAST_MIRACAST_LIB=<path to
// libcc_miracast_ffi.so> (or the default path target/debug/libcc_miracast_ffi.so). Without it the
// tests are skipped.
//
// On Linux the Rust side has no Windows receiver: `start` ends in the state "unsupported" with
// the hint "not_windows". That is enough to check the whole binding (function names, the JSON,
// the buffers, the frame struct) without a Windows machine. What it cannot check: anything the
// Windows API does (docs/miracast-device-test-plan.md).

import 'dart:ffi';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/miracast.dart';

String? findMiracastLibrary() {
  final env = Platform.environment[miracastLibraryEnvVar];
  if (env != null && env.isNotEmpty) return env;
  const candidates = [
    '../../target/debug/libcc_miracast_ffi.so',
    '../../target/debug/libcc_miracast_ffi.dylib',
  ];
  for (final c in candidates) {
    if (File(c).existsSync()) return File(c).absolute.path;
  }
  return null;
}

void main() {
  final lib = findMiracastLibrary();
  final skip = lib == null || Platform.isWindows
      ? 'libcc_miracast_ffi not built (cargo build -p cc-miracast-ffi), or running on Windows'
      : null;

  test('the interface version matches', () async {
    final core = FfiMiracastCore(opener: () => DynamicLibrary.open(lib!));
    // state() loads the library and checks the version.
    final s = await core.state();
    expect(s.status, MiracastStatus.off);
    expect(s.pending, isNull);
  }, skip: skip);

  test(
    'probe, start refusal and unsupported state through the real library',
    () async {
      final core = FfiMiracastCore(opener: () => DynamicLibrary.open(lib!));
      final probe = await core.probe();
      expect(probe.issue, MiracastIssue.notWindows);

      // Bad arguments are refused by the Rust side.
      await expectLater(
        core.start(name: '   ', pairing: MiracastPairing.confirm),
        throwsA(isA<MiracastCallException>().having((e) => e.code, 'code', -2)),
      );

      await core.start(
        name: 'Test PC',
        pairing: MiracastPairing.pinIfRequested,
      );
      MiracastState s = MiracastState.off;
      for (var i = 0; i < 100; i++) {
        s = await core.state();
        if (s.status != MiracastStatus.starting) break;
        await Future<void>.delayed(const Duration(milliseconds: 20));
      }
      expect(s.status, MiracastStatus.unsupported);
      expect(s.issue, MiracastIssue.notWindows);
      expect(s.lastError, isNotNull);
      expect(s.generation, greaterThan(0));

      // Nothing runs: no picture, answering is refused.
      expect(await core.pollFrame(), isNull);
      await expectLater(
        core.respond(1, true),
        throwsA(isA<MiracastCallException>()),
      );
      await core.setMaxSize(1280, 720);
      await core.setMuted(true);
      await core.stop();
      final after = await core.state();
      expect(after.status, MiracastStatus.off);
    },
    skip: skip,
  );
}
