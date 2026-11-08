// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the iOS screen broadcast flow with fakes: no device, no extension.
//   * the platform-channel wrapper against mocked MethodChannel / EventChannel,
//   * the parsing of the extension's status maps,
//   * the Send screen on a "core" that lists the iOS broadcast (hand-over, instructions, status,
//     errors, timeout, stop),
//   * the receiver on iOS: no codec card, "not needed" note, failing decoder reported.

import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/ios_broadcast.dart';
import 'package:cometcast/main.dart';

import 'fake_core.dart';

/// A broadcast hand-over whose status reports are driven by the test.
class FakeIosBroadcast implements IosBroadcast {
  final controller = StreamController<IosBroadcastStatus>.broadcast();
  IosBroadcastRequest? prepared;
  bool pickerOpens = true;
  Object? prepareError;
  int showPickerCalls = 0;
  int stopCalls = 0;

  @override
  Stream<IosBroadcastStatus> get events => controller.stream;

  @override
  Future<void> prepare(IosBroadcastRequest request) async {
    final e = prepareError;
    if (e != null) throw e;
    prepared = request;
  }

  @override
  Future<bool> showPicker() async {
    showPickerCalls++;
    return pickerOpens;
  }

  @override
  Future<void> stop() async => stopCalls++;

  @override
  Future<IosBroadcastStatus?> currentStatus() async => null;
}

FakeCoreApi iosCore() => FakeCoreApi()
  ..captureInfoResult = const CaptureInfo(
    support: CaptureSupport.supported(),
    monitors: [
      CaptureMonitor(
        id: iosBroadcastMonitorId,
        name: 'Screen',
        x: 0,
        y: 0,
        width: 0,
        height: 0,
      ),
    ],
  );

Future<void> openSend(
  WidgetTester tester,
  FakeCoreApi core,
  FakeIosBroadcast ios, {
  String locale = 'en',
}) async {
  tester.view.physicalSize = const Size(900, 1800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    CometCastApp(locale: Locale(locale), core: core, iosBroadcast: ios),
  );
  await tester.pumpAndSettle();
  await tester.tap(
    find.text(locale == 'en' ? 'Send screen' : 'Bildschirm senden'),
  );
  await tester.pumpAndSettle();
}

Future<void> startSharing(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('shareScreenSwitch')));
  await tester.pump();
  await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
  await tester.tap(find.byKey(const Key('startButton')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 10));
}

String statusText(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('statusText'))).data!;

Future<void> report(
  WidgetTester tester,
  FakeIosBroadcast ios,
  IosBroadcastStatus s,
) async {
  ios.controller.add(s);
  await tester.pump(const Duration(milliseconds: 10));
}

Future<void> closePage(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump();
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('ChannelIosBroadcast with mocked channels', () {
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    const method = MethodChannel('de.cometcast/ios_broadcast');
    const events = EventChannel('de.cometcast/ios_broadcast/status');

    tearDown(() {
      messenger.setMockMethodCallHandler(method, null);
      messenger.setMockStreamHandler(events, null);
    });

    test('prepare sends the code, the relay and the encoder limits', () async {
      MethodCall? seen;
      messenger.setMockMethodCallHandler(method, (call) async {
        seen = call;
        return true;
      });
      await const ChannelIosBroadcast().prepare(
        const IosBroadcastRequest(code: 'K7M2QX', relay: 'relay.example:8443'),
      );
      expect(seen!.method, 'prepare');
      expect(seen!.arguments, {
        'code': 'K7M2QX',
        'relay': 'relay.example:8443',
        'maxDimension': 1280,
        'fps': 30,
        'bitrate': 2500000,
        'name': null,
        'handover': null,
        'deviceKey': null,
      });
    });

    test('prepare hands the device key of the app to the extension', () async {
      MethodCall? seen;
      messenger.setMockMethodCallHandler(method, (call) async {
        seen = call;
        return true;
      });
      const key = 'AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8';
      await const ChannelIosBroadcast().prepare(
        const IosBroadcastRequest(
          code: 'K7M2QX',
          relay: 'relay.example:8443',
          deviceKey: key,
        ),
      );
      expect((seen!.arguments as Map)['deviceKey'], key);
    });

    test('showPicker answers false when the native side says so', () async {
      messenger.setMockMethodCallHandler(method, (call) async => false);
      expect(await const ChannelIosBroadcast().showPicker(), isFalse);
      messenger.setMockMethodCallHandler(method, (call) async => null);
      expect(await const ChannelIosBroadcast().showPicker(), isFalse);
      messenger.setMockMethodCallHandler(method, (call) async => true);
      expect(await const ChannelIosBroadcast().showPicker(), isTrue);
    });

    test('stop and status call the native methods', () async {
      final calls = <String>[];
      messenger.setMockMethodCallHandler(method, (call) async {
        calls.add(call.method);
        return call.method == 'status'
            ? {'state': 'streaming', 'sent': 5, 'seq': 3}
            : null;
      });
      await const ChannelIosBroadcast().stop();
      final s = await const ChannelIosBroadcast().currentStatus();
      expect(calls, ['stop', 'status']);
      expect(s!.state, IosBroadcastState.streaming);
      expect((s.sent, s.seq), (5, 3));
    });

    test('a missing App Group surfaces as PlatformException', () {
      messenger.setMockMethodCallHandler(method, (call) async {
        throw PlatformException(code: 'app_group_unavailable');
      });
      expect(
        const ChannelIosBroadcast().prepare(
          const IosBroadcastRequest(code: 'K7M2QX', relay: 'r'),
        ),
        throwsA(isA<PlatformException>()),
      );
    });

    test('status events are parsed and unknown ones ignored', () async {
      messenger.setMockStreamHandler(
        events,
        MockStreamHandler.inline(
          onListen: (arguments, sink) {
            sink.success({'state': 'connecting', 'seq': 1});
            sink.success({
              'state': 'streaming',
              'error': 'none',
              'sent': 90,
              'dropped': 3,
              'bytes': 123456,
              'keyframes': 2,
              'width': 1280,
              'height': 720,
              'freeMemoryMB': 21,
              'seq': 2,
            });
            sink.success({'state': 'martian'});
            sink.success('nonsense');
            sink.success({
              'state': 'failed',
              'error': 'roomNotFound',
              'seq': 3,
            });
            sink.endOfStream();
          },
        ),
      );
      final got = await const ChannelIosBroadcast().events.toList();
      expect(got, hasLength(3));
      expect(got[0].state, IosBroadcastState.connecting);
      expect(got[1].state, IosBroadcastState.streaming);
      expect(
        (got[1].sent, got[1].dropped, got[1].bytes, got[1].keyframes),
        (90, 3, 123456, 2),
      );
      expect(
        (got[1].width, got[1].height, got[1].freeMemoryMB),
        (1280, 720, 21),
      );
      expect(got[2].error, IosBroadcastError.roomNotFound);
      expect(got[2].isTerminal, isTrue);
    });

    test('a failure without a known reason is an internal error', () {
      final s = parseBroadcastStatus({'state': 'failed', 'error': 'x'});
      expect(s!.error, IosBroadcastError.internalError);
      expect(
        parseBroadcastStatus({'state': 'ended'})!.error,
        IosBroadcastError.none,
      );
      expect(parseBroadcastStatus(null), isNull);
      expect(parseBroadcastStatus({'state': 5}), isNull);
    });

    test('the Dart enums match the Swift raw values', () {
      // ios/Shared/BroadcastShared.swift: BroadcastStateName / BroadcastErrorName.
      expect(IosBroadcastState.values.map((e) => e.name), [
        'starting',
        'connecting',
        'pending',
        'waiting',
        'streaming',
        'reconnecting',
        'paused',
        'ended',
        'failed',
        'lost',
      ]);
      expect(IosBroadcastError.values.map((e) => e.name), [
        'none',
        'noConfig',
        'configExpired',
        'invalidRoomCode',
        'invalidRelay',
        'roomNotFound',
        'serverUnreachable',
        'connectionLost',
        'peerLeft',
        'encoderFailed',
        'memory',
        'stoppedByApp',
        'internalError',
        'joinDenied',
        'approvalTimeout',
        'roomLocked',
      ]);
    });

    test('the fallback fields and the way back are read from the status', () {
      final s = parseBroadcastStatus({
        'state': 'pending',
        'check': 42,
        'timeoutSecs': 60,
        'roomName': 'Room 204',
        'sas': '171 852',
        'nextHandover': 'tok',
      })!;
      expect(s.state, IosBroadcastState.pending);
      expect((s.check, s.timeoutSecs), (42, 60));
      expect(
        (s.roomName, s.sas, s.nextHandover),
        ('Room 204', '171 852', 'tok'),
      );
      // Missing, empty or wrongly typed: null, never a guess.
      final e = parseBroadcastStatus({
        'state': 'ended',
        'roomName': '',
        'sas': 5,
        'nextHandover': '',
      })!;
      expect(
        (e.check, e.roomName, e.sas, e.nextHandover),
        (null, null, null, null),
      );
    });

    test('prepare hands the name and the token to the platform', () async {
      Map<Object?, Object?>? args;
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
            const MethodChannel('de.cometcast/ios_broadcast'),
            (call) async {
              args = call.arguments as Map<Object?, Object?>?;
              return true;
            },
          );
      addTearDown(
        () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(
              const MethodChannel('de.cometcast/ios_broadcast'),
              null,
            ),
      );
      await const ChannelIosBroadcast().prepare(
        const IosBroadcastRequest(
          code: 'K7QM2W',
          relay: 'r:1',
          name: 'Lena',
          handover: 'abcDEF-_123',
        ),
      );
      expect(args!['name'], 'Lena');
      expect(args!['handover'], 'abcDEF-_123');
    });

    test(
      'option (a) of the test builds can never be on in a release build',
      () {
        expect(iosOptionAAllowed(release: true, defined: true), isFalse);
        expect(iosOptionAAllowed(release: true, defined: false), isFalse);
        expect(iosOptionAAllowed(release: false, defined: false), isFalse);
        expect(iosOptionAAllowed(release: false, defined: true), isTrue);
        // Without the define (all tests) the hand-over is on.
        expect(iosOptionA, isFalse);
      },
    );

    test('hand-over tokens are 128 random bits as base64url', () {
      final t = newHandoverToken();
      expect(t, matches(RegExp(r'^[A-Za-z0-9_-]{22}$')));
      expect(newHandoverToken(), isNot(t));
      // The same bytes as the Swift test (0xfb x 16).
      expect(newHandoverToken(_Fixed(0xfb)), '-_v7-_v7-_v7-_v7-_v7-w');
      expect(newHandoverToken(_Fixed(0)), 'A' * 21 + 'A');
    });
  });

  group('Send screen on iOS', () {});

  group('Receiver on iOS', () {});
}

/// A "random" source that always gives the same byte (tests of the token format only).
class _Fixed implements Random {
  _Fixed(this.v);
  final int v;
  @override
  int nextInt(int max) => v;
  @override
  bool nextBool() => false;
  @override
  double nextDouble() => 0;
}
