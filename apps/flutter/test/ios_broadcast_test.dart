// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the iOS screen broadcast flow with fakes: no device, no extension.
//   * the platform-channel wrapper against mocked MethodChannel / EventChannel,
//   * the parsing of the extension's status maps,
//   * the Send screen on a "core" that lists the iOS broadcast (hand-over, instructions, status,
//     errors, timeout, stop),
//   * the receiver on iOS: no codec card, "not needed" note, failing decoder reported.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/android_video.dart';
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
      });
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
      ]);
    });
  });

  group('Send screen on iOS', () {
    testWidgets('starting hands the room over, opens the sheet and waits', (
      tester,
    ) async {
      final core = iosCore();
      final ios = FakeIosBroadcast();
      await openSend(tester, core, ios);
      await startSharing(tester);

      expect(ios.prepared!.code, 'K7M2QX');
      expect(ios.prepared!.relay, isNotEmpty);
      expect(ios.showPickerCalls, 1);
      expect(
        core.senderCode,
        isNull,
        reason: 'the extension connects, not the app',
      );
      expect(statusText(tester), 'Waiting for you to start the broadcast …');
      expect(find.byKey(const Key('iosBroadcastInstructions')), findsOneWidget);
      expect(find.byKey(const Key('codecNotice')), findsNothing);
      expect(find.byKey(const Key('monitorDropdown')), findsNothing);
      expect(find.byKey(const Key('stopButton')), findsOneWidget);
      await closePage(tester);
    });

    testWidgets('the status of the extension is shown and ends cleanly', (
      tester,
    ) async {
      final ios = FakeIosBroadcast();
      await openSend(tester, iosCore(), ios);
      await startSharing(tester);

      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.connecting),
      );
      expect(statusText(tester), 'Connecting to the server …');
      expect(find.byKey(const Key('iosBroadcastInstructions')), findsNothing);
      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.waiting),
      );
      expect(
        statusText(tester),
        'Connected to the server. Waiting for the receiver …',
      );
      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.streaming, sent: 42),
      );
      expect(statusText(tester), contains('42'));
      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.reconnecting),
      );
      expect(
        statusText(tester),
        'The connection was lost. Trying to reconnect …',
      );
      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.paused),
      );
      expect(
        statusText(tester),
        'The broadcast is paused, for example because of a call.',
      );
      await report(
        tester,
        ios,
        const IosBroadcastStatus(
          state: IosBroadcastState.ended,
          error: IosBroadcastError.stoppedByApp,
        ),
      );
      expect(statusText(tester), 'Screen sharing was stopped.');
      expect(find.byKey(const Key('startButton')), findsOneWidget);
      await closePage(tester);
    });

    testWidgets('the receiver leaving is its own message', (tester) async {
      final ios = FakeIosBroadcast();
      await openSend(tester, iosCore(), ios);
      await startSharing(tester);
      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.streaming, sent: 1),
      );
      await report(
        tester,
        ios,
        const IosBroadcastStatus(
          state: IosBroadcastState.ended,
          error: IosBroadcastError.peerLeft,
        ),
      );
      expect(statusText(tester), contains('receiver'));
      await closePage(tester);
    });

    testWidgets('failures are translated, in English and German', (
      tester,
    ) async {
      for (final (locale, expected) in [
        ('en', 'No receiver with this room code was found'),
        ('de', 'Es wurde kein Empfänger mit diesem Raumcode gefunden'),
      ]) {
        final ios = FakeIosBroadcast();
        await openSend(tester, iosCore(), ios, locale: locale);
        await startSharing(tester);
        await report(
          tester,
          ios,
          const IosBroadcastStatus(
            state: IosBroadcastState.failed,
            error: IosBroadcastError.roomNotFound,
          ),
        );
        expect(statusText(tester), startsWith(expected));
        await closePage(tester);
      }
    });

    testWidgets('a silent extension is reported as lost', (tester) async {
      final ios = FakeIosBroadcast();
      await openSend(tester, iosCore(), ios);
      await startSharing(tester);
      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.streaming, sent: 3),
      );
      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.lost),
      );
      expect(statusText(tester), contains('unexpected problem'));
      await closePage(tester);
    });

    testWidgets('a failed picker shows the Control Center fallback', (
      tester,
    ) async {
      final ios = FakeIosBroadcast()..pickerOpens = false;
      await openSend(tester, iosCore(), ios);
      await startSharing(tester);
      expect(
        find.textContaining('Control Center'),
        findsWidgets,
        reason: 'the hand-over is done, the person starts it there',
      );
      expect(ios.prepared, isNotNull);
      await closePage(tester);
    });

    testWidgets('a missing App Group is an error, not a hang', (tester) async {
      final ios = FakeIosBroadcast()
        ..prepareError = PlatformException(code: 'app_group_unavailable');
      await openSend(tester, iosCore(), ios);
      await startSharing(tester);
      expect(statusText(tester), startsWith('Error:'));
      expect(ios.showPickerCalls, 0);
      await closePage(tester);
    });

    testWidgets('stop withdraws the hand-over', (tester) async {
      final ios = FakeIosBroadcast();
      await openSend(tester, iosCore(), ios);
      await startSharing(tester);
      await tester.tap(find.byKey(const Key('stopButton')));
      await tester.pump();
      expect(ios.stopCalls, greaterThanOrEqualTo(1));
      expect(find.byKey(const Key('startButton')), findsOneWidget);
      // Late reports of the old broadcast do not bring the screen back to life.
      await report(
        tester,
        ios,
        const IosBroadcastStatus(state: IosBroadcastState.streaming, sent: 9),
      );
      expect(find.byKey(const Key('startButton')), findsOneWidget);
      await closePage(tester);
    });

    testWidgets('nobody starting the broadcast ends the wait', (tester) async {
      final ios = FakeIosBroadcast();
      await openSend(tester, iosCore(), ios);
      await startSharing(tester);
      await tester.pump(const Duration(minutes: 3));
      expect(
        statusText(tester),
        'The broadcast was not started. Nothing is sent.',
      );
      expect(ios.stopCalls, 1);
      expect(find.byKey(const Key('startButton')), findsOneWidget);
      await closePage(tester);
    });

    testWidgets(
      'a report of an earlier broadcast is ignored, a new one is not',
      (tester) async {
        final ios = FakeIosBroadcast();
        await openSend(tester, iosCore(), ios);
        await startSharing(tester);
        final now = DateTime.now().millisecondsSinceEpoch;
        await report(
          tester,
          ios,
          IosBroadcastStatus(
            state: IosBroadcastState.failed,
            error: IosBroadcastError.connectionLost,
            updatedAtMs: now - 60000,
          ),
        );
        expect(statusText(tester), 'Waiting for you to start the broadcast …');
        // A failure of this broadcast, even as the very first report, is shown.
        await report(
          tester,
          ios,
          IosBroadcastStatus(
            state: IosBroadcastState.failed,
            error: IosBroadcastError.configExpired,
            updatedAtMs: now + 1000,
          ),
        );
        expect(
          statusText(tester),
          startsWith('The broadcast was not started in time'),
        );
        await closePage(tester);
      },
    );

    testWidgets(
      'a core that does not list the broadcast sends like a desktop',
      (tester) async {
        final ios = FakeIosBroadcast();
        await openSend(tester, FakeCoreApi(), ios);
        await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
        await tester.tap(find.byKey(const Key('startButton')));
        await tester.pump();
        expect(ios.prepared, isNull);
        await closePage(tester);
      },
    );
  });

  group('Receiver on iOS', () {
    testWidgets('no codec card, a note instead, hardware decoder used', (
      tester,
    ) async {
      final core = FakeCoreApi()
        ..codecStateResult = const UiCodecState.unsupported(
          reason: 'this build has no H.264 support',
        );
      tester.view.physicalSize = const Size(900, 1800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        CometCastApp(
          locale: const Locale('en'),
          core: core,
          videoDecoder: _NullDecoder(),
          hardwareDecoderOnly: true,
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Receive on this device'));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('codecNotice')), findsNothing);
      expect(find.byKey(const Key('codecUnsupported')), findsNothing);
      expect(find.byKey(const Key('codecNotNeeded')), findsOneWidget);
      expect(
        find.text(
          'The video is decoded by your device. No extra video codec is needed.',
        ),
        findsOneWidget,
      );
      await closePage(tester);
    });

    testWidgets('on other platforms the note is not shown', (tester) async {
      tester.view.physicalSize = const Size(900, 1800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        CometCastApp(locale: const Locale('en'), core: FakeCoreApi()),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Receive on this device'));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('codecNotNeeded')), findsNothing);
      await closePage(tester);
    });
  });
}

class _NullDecoder implements AndroidVideoDecoder {
  @override
  Stream<VideoDecoderEvent> get events => const Stream.empty();

  @override
  Future<int> start() async => 1;

  @override
  void queue(Uint8List data, {required int ptsUs, required bool key}) {}

  @override
  Future<void> stop() async {}
}
