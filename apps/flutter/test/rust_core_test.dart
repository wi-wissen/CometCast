// SPDX-License-Identifier: GPL-3.0-or-later
// Calls the REAL Rust library through flutter_rust_bridge.
// Needs the library: `cargo build -p cc-ffi` (default path target/debug/libcc_ffi.so) or
// COMETCAST_FFI_LIB=/path/to/libcc_ffi.so. Without the library the tests are skipped.

import 'dart:io';
import 'dart:typed_data';

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

/// Checks that hold whether or not this machine has an MDM policy file.
void _expectConsistent(
  List<ServerCandidate> servers,
  bool locked,
  List<DiscoveryWarning> notices,
) {
  const known = {'mdm', 'dns-srv', 'display-announcement', 'default-list'};
  for (final s in servers) {
    expect(known, contains(s.source));
    expect(s.address.trim(), isNotEmpty);
  }
  expect(
    servers.map((s) => s.address.toLowerCase()).toSet().length,
    servers.length,
    reason: 'each address once',
  );
  // A lock offers policy servers only.
  if (locked) expect(servers.every((s) => s.source == 'mdm'), isTrue);
  // An unusable policy locks and offers nothing.
  if (notices.contains(DiscoveryWarning.policyUnusable)) {
    expect(locked, isTrue);
    expect(servers, isEmpty);
  }
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
    final events = await core
        .startSender(code: 'nope', server: '127.0.0.1:1')
        .toList();
    expect(events, hasLength(1));
    expect(events.single, isA<SenderUiEvent_Error>());
  }, skip: skip);

  test('pushing encoded video without an encoded sender is refused', () {
    const core = RustCoreApi();
    final au = Uint8List.fromList([0, 0, 0, 1, 0x65, 1, 2]);
    expect(
      core.pushEncodedFrame(au, isKey: true, ptsUs: 0),
      PushResult.noSender,
    );
    expect(core.pushCodecConfig(au), isFalse);
    expect(core.keyframeRequested(), isFalse);
  }, skip: skip);

  test('server defaults come from cc-discovery', () {
    const core = RustCoreApi();
    final d = core.serverDefaults();
    expect(d.devFallback, '127.0.0.1:8443');
    final fake = FakeCoreApi().serverDefaults();
    expect(fake.devFallback, d.devFallback);
    // A machine may have an MDM policy file (/etc/cometcast/policy.json) that changes the
    // list, so only what holds with and without a policy is checked.
    _expectConsistent(d.servers, d.locked, d.notices);
    if (!d.locked && !d.servers.any((s) => s.source == 'mdm')) {
      // No usable policy: the built-in list, as the fake has it.
      expect(
        d.servers.map((s) => (s.address, s.source)),
        fake.servers.map((s) => (s.address, s.source)),
      );
    }
  }, skip: skip);

  test(
    'server discovery answers with ordered, labelled, consistent data',
    () async {
      const core = RustCoreApi();
      final r = await core.discoverServers();
      _expectConsistent(r.servers, r.locked, r.notices);
      // Policy servers always come first.
      final sources = r.servers.map((s) => s.source).toList();
      final lastMdm = sources.lastIndexOf('mdm');
      if (lastMdm >= 0) {
        expect(sources.take(lastMdm + 1).every((s) => s == 'mdm'), isTrue);
      }
      // A policy that forbids other servers blocks announcements.
      if (r.locked) expect(r.announcementsAllowed, isFalse);
      // The raw warnings are for logs; the UI only uses the codes.
      expect(
        r.warnings.length >= r.notices.length || r.notices.isEmpty,
        isTrue,
      );
    },
    skip: skip,
  );

  test('announcing and listing receivers can be started and stopped', () async {
    const core = RustCoreApi();
    // A loopback server is refused by the checks of cc-discovery; the name alone is fine.
    final refused = await core.startAdvertising(
      name: 'Test receiver',
      preferredServer: '127.0.0.1:8443',
    );
    expect(refused, AdvertiseOutcome.refused);
    final outcome = await core.startAdvertising(name: 'Test receiver');
    // No multicast in a sandbox is possible: then it fails, but never throws.
    expect(outcome, anyOf(AdvertiseOutcome.started, AdvertiseOutcome.failed));
    await core.stopAdvertising();
    final events = core.watchReceivers().listen((_) {});
    await Future<void>.delayed(const Duration(milliseconds: 200));
    await core.stopWatchingReceivers();
    await events.cancel();
  }, skip: skip);

  test('no picture is waiting before a receiver ran', () async {
    const core = RustCoreApi();
    expect(await core.takeFrame(), isNull);
    core.setVideoMaxSize(640, 360); // must not throw
  }, skip: skip);

  test(
    'capture check: an unreachable display is reported, not thrown',
    () async {
      const core = RustCoreApi();
      final info = await core.captureInfo(display: ':9999');
      expect(info.support, isA<CaptureSupport_Failed>());
      expect(info.monitors, isEmpty);
    },
    skip: skip,
  );
}
