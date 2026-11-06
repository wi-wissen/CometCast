// SPDX-License-Identifier: GPL-3.0-or-later
// Smoke test with the REAL Rust core, the real UI and a real signalling server.
//
// What it does: opens "Receive on this device", starts receiving, reads the room code from
// the screen, starts `cometcast-cli send <CODE>` as a separate process and checks that the
// packet/frame counters on the screen rise.
//
// It needs a running server and the CLI; both are given as --dart-define values, see
// apps/flutter/README.md ("Smoke test"):
//   COMETCAST_TEST_SERVER = host:port of cometcast-server
//   COMETCAST_CLI         = path of the cometcast-cli binary

import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/rust_loader.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/settings.dart';

const _server = String.fromEnvironment('COMETCAST_TEST_SERVER');
const _cli = String.fromEnvironment('COMETCAST_CLI');

String _text(WidgetTester tester, String key) {
  final w = tester.widget(find.byKey(Key(key)));
  // The room code is a SelectableText, the counters are plain Text.
  if (w is SelectableText) return w.data ?? '';
  return (w as Text).data ?? '';
}

/// Pump frames (and let real events arrive) until [done] is true or [timeout] passes.
Future<bool> _waitFor(
  WidgetTester tester,
  bool Function() done, {
  Duration timeout = const Duration(seconds: 30),
}) async {
  final end = DateTime.now().add(timeout);
  while (DateTime.now().isBefore(end)) {
    await tester.pump(const Duration(milliseconds: 250));
    if (done()) return true;
  }
  return done();
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    await initRustCore();
  });

  testWidgets('receiver shows a room code and the counters rise', (
    tester,
  ) async {
    expect(
      _server,
      isNotEmpty,
      reason: 'pass --dart-define=COMETCAST_TEST_SERVER=host:port',
    );
    expect(
      _cli,
      isNotEmpty,
      reason: 'pass --dart-define=COMETCAST_CLI=/path/to/cometcast-cli',
    );

    await tester.pumpWidget(
      CometCastApp(
        locale: const Locale('en'),
        core: const RustCoreApi(),
        // Same-machine test: bind WebRTC to loopback only.
        settings: AppSettings(serverAddress: _server, udpBind: '127.0.0.1:0'),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Receive on this device'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('startButton')));

    final gotCode = await _waitFor(
      tester,
      () => _text(tester, 'roomCode') != '······',
    );
    expect(
      gotCode,
      isTrue,
      reason: 'no room code appeared; status: ${_text(tester, 'statusText')}',
    );
    final code = _text(tester, 'roomCode');
    // ignore: avoid_print
    print('SMOKE room code on screen: $code');
    expect(RegExp(r'^[A-HJ-NP-Z2-9]{6}$').hasMatch(code), isTrue);

    final sender = await Process.start(_cli, [
      'send',
      code,
      '--server',
      _server,
      '--seconds',
      '8',
      '--bind',
      '127.0.0.1:0',
    ]);
    unawaited(sender.stdout.drain<void>());
    unawaited(sender.stderr.drain<void>());

    int packets() => int.tryParse(_text(tester, 'statPackets')) ?? 0;
    final rising = await _waitFor(tester, () => packets() > 0);
    expect(rising, isTrue, reason: 'packet counter never rose');
    final first = packets();
    // ignore: avoid_print
    print('SMOKE packets after first rise: $first');

    // Keeps rising while the sender streams.
    final more = await _waitFor(
      tester,
      () => packets() > first + 20,
      timeout: const Duration(seconds: 10),
    );
    expect(more, isTrue, reason: 'packet counter stopped rising');
    expect(int.parse(_text(tester, 'statFrames')), greaterThan(0));
    expect(int.parse(_text(tester, 'statBytes')), greaterThan(0));
    expect(int.parse(_text(tester, 'statKeyframes')), greaterThan(0));
    expect(_text(tester, 'statCodec'), 'vp8');
    // ignore: avoid_print
    print(
      'SMOKE final: packets=${_text(tester, 'statPackets')} '
      'bytes=${_text(tester, 'statBytes')} frames=${_text(tester, 'statFrames')} '
      'keyframes=${_text(tester, 'statKeyframes')} codec=${_text(tester, 'statCodec')}',
    );

    // The CLI stops after 8 s; the UI then reports that the sender left.
    final left = await _waitFor(
      tester,
      () => find.text('The sender has left.').evaluate().isNotEmpty,
      timeout: const Duration(seconds: 30),
    );
    expect(left, isTrue, reason: 'peer-left status never appeared');
    sender.kill();
  });

  testWidgets('send screen streams the test video to a receiver', (
    tester,
  ) async {
    expect(_server, isNotEmpty);
    const core = RustCoreApi();
    // A receiver without UI, started through the same bridge.
    final events = core.startReceiver(server: _server, udpBind: '127.0.0.1:0');
    String? code;
    var packets = BigInt.zero;
    var sawTrack = false;
    final sub = events.listen((e) {
      if (e is ReceiverUiEvent_RoomCode) code = e.code;
      if (e is ReceiverUiEvent_VideoTrack) sawTrack = true;
      if (e is ReceiverUiEvent_Stats) packets = e.packets;
    });
    expect(await _waitFor(tester, () => code != null), isTrue);

    await tester.pumpWidget(
      CometCastApp(
        locale: const Locale('en'),
        core: core,
        settings: AppSettings(serverAddress: _server, udpBind: '127.0.0.1:0'),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Send screen'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const Key('codeField')), code!);
    await tester.enterText(find.byKey(const Key('secondsField')), '5');
    await tester.tap(find.byKey(const Key('startButton')));

    final streaming = await _waitFor(
      tester,
      () => _text(tester, 'statusText').startsWith('Sending the test video'),
    );
    expect(
      streaming,
      isTrue,
      reason: 'sender status: ${_text(tester, 'statusText')}',
    );
    expect(
      await _waitFor(tester, () => packets > BigInt.from(10)),
      isTrue,
      reason: 'receiver saw no packets',
    );
    expect(sawTrack, isTrue);
    // After the duration the sender reports that it finished.
    expect(
      await _waitFor(
        tester,
        () => _text(tester, 'statusText').startsWith('Finished:'),
      ),
      isTrue,
    );
    // ignore: avoid_print
    print(
      'SMOKE send: ${_text(tester, 'statusText')}, receiver packets=$packets',
    );
    await sub.cancel();
    await core.stopReceiver();
  });
}
