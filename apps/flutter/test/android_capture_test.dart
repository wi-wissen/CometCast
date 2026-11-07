// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the Android screen capture flow with fakes: no device and no native library.
//   * the platform-channel wrapper against a mocked MethodChannel / EventChannel,
//   * the feeder (events -> Rust push calls, Rust key frame requests -> sync frame),
//   * the Send screen on a "core" that lists the Android projection.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/capture_feeder.dart';
import 'package:cometcast/core/android_capture.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/sender_controller.dart';

import 'fake_core.dart';

/// A capture whose consent dialog is answered by the test.
class FakeAndroidCapture implements AndroidCapture {
  final controller = StreamController<CaptureEvent>.broadcast();
  Completer<CaptureStartResult> consent = Completer();
  Object? startError;
  CaptureNotificationTexts? notification;
  int startCalls = 0;
  int stopCalls = 0;
  int syncFrameCalls = 0;

  @override
  Stream<CaptureEvent> get events => controller.stream;

  @override
  Future<CaptureStartResult> start({
    required CaptureNotificationTexts notification,
    int maxDimension = 1280,
    int fps = 30,
    int bitrate = 4000000,
  }) {
    startCalls++;
    this.notification = notification;
    final e = startError;
    if (e != null) return Future.error(e);
    return consent.future;
  }

  @override
  Future<void> stop() async => stopCalls++;

  @override
  Future<void> requestSyncFrame() async => syncFrameCalls++;
}

final _config = Uint8List.fromList([
  0,
  0,
  0,
  1,
  0x67,
  1,
  2,
  0,
  0,
  0,
  1,
  0x68,
  3,
]);
final _idr = Uint8List.fromList([0, 0, 0, 1, 0x65, 9, 9]);
final _delta = Uint8List.fromList([0, 0, 0, 1, 0x41, 7]);

FakeCoreApi androidCore() =>
    FakeCoreApi()
      ..captureInfoResult = const CaptureInfo(
        support: CaptureSupport.supported(),
        monitors: [
          CaptureMonitor(
            id: androidProjectionMonitorId,
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
  FakeAndroidCapture capture, {
  String locale = 'en',
}) async {
  tester.view.physicalSize = const Size(900, 1800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    CometCastApp(locale: Locale(locale), core: core, androidCapture: capture),
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
}

/// Let the chains of `await`s in the controller and the feeder run (cancelling a stream
/// subscription completes a little later than the next microtask).
Future<void> settle(WidgetTester tester) async {
  for (var i = 0; i < 4; i++) {
    await tester.pump(const Duration(milliseconds: 5));
  }
}

/// Leave the page so that its timers are cancelled before the test ends.
Future<void> closePage(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump();
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  group('ChannelAndroidCapture with mocked channels', () {
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    const method = MethodChannel('de.cometcast/capture');

    tearDown(() {
      messenger.setMockMethodCallHandler(method, null);
      messenger.setMockStreamHandler(
        const EventChannel('de.cometcast/capture/frames'),
        null,
      );
    });

    test(
      'start sends the options and the translated notification texts',
      () async {
        MethodCall? seen;
        messenger.setMockMethodCallHandler(method, (call) async {
          seen = call;
          return true;
        });
        final r = await const ChannelAndroidCapture().start(
          notification: const CaptureNotificationTexts(
            title: 'T',
            text: 'X',
            stopLabel: 'S',
          ),
        );
        expect(r, CaptureStartResult.started);
        expect(seen!.method, 'start');
        expect(seen!.arguments, {
          'maxDimension': 1280,
          'fps': 30,
          'bitrate': 4000000,
          'notificationTitle': 'T',
          'notificationText': 'X',
          'notificationStop': 'S',
        });
      },
    );

    test('false from the native side means the person refused', () async {
      messenger.setMockMethodCallHandler(method, (call) async => false);
      final r = await const ChannelAndroidCapture().start(
        notification: const CaptureNotificationTexts(
          title: 'T',
          text: 'X',
          stopLabel: 'S',
        ),
      );
      expect(r, CaptureStartResult.denied);
    });

    test('stop and requestSyncFrame call the native methods', () async {
      final calls = <String>[];
      messenger.setMockMethodCallHandler(method, (call) async {
        calls.add(call.method);
        return null;
      });
      await const ChannelAndroidCapture().stop();
      await const ChannelAndroidCapture().requestSyncFrame();
      expect(calls, ['stop', 'requestSyncFrame']);
    });

    test('a native error surfaces as PlatformException', () async {
      messenger.setMockMethodCallHandler(method, (call) async {
        throw PlatformException(code: 'busy', message: 'dialog open');
      });
      expect(
        const ChannelAndroidCapture().start(
          notification: const CaptureNotificationTexts(
            title: 'T',
            text: 'X',
            stopLabel: 'S',
          ),
        ),
        throwsA(isA<PlatformException>()),
      );
    });

    test('events from the event channel are parsed', () async {
      const channel = EventChannel('de.cometcast/capture/frames');
      messenger.setMockStreamHandler(
        channel,
        MockStreamHandler.inline(
          onListen: (arguments, sink) {
            sink.success({'type': 'config', 'data': _config});
            sink.success({
              'type': 'frame',
              'ptsUs': 1234,
              'keyFrame': true,
              'dropped': 2,
              'data': _idr,
            });
            sink.success({'type': 'stopped', 'reason': 'projection'});
            sink.success({'type': 'error', 'message': 'boom'});
            sink.success({'type': 'unknown'});
            sink.endOfStream();
          },
        ),
      );
      final got = await const ChannelAndroidCapture().events.toList();
      expect(got, hasLength(4), reason: 'the unknown event is ignored');
      expect((got[0] as CaptureConfig).data, _config);
      final f = got[1] as CaptureFrame;
      expect((f.isKey, f.ptsUs, f.dropped), (true, 1234, 2));
      expect(f.data, orderedEquals(_idr));
      expect((got[2] as CaptureStopped).reason, 'projection');
      expect((got[3] as CaptureError).message, 'boom');
    });

    test('malformed events are ignored', () {
      expect(parseCaptureEvent(null), isNull);
      expect(parseCaptureEvent({'type': 'frame'}), isNull);
      expect(parseCaptureEvent({'type': 'config', 'data': 'x'}), isNull);
    });
  });

  group('CaptureFeeder', () {
    test(
      'config and frames go to the core, key frame requests to the encoder',
      () async {
        final core = FakeCoreApi();
        final cap = FakeAndroidCapture()
          ..consent.complete(CaptureStartResult.started);
        final feeder = CaptureFeeder(
          core: core,
          capture: cap,
          onStopped: (_) {},
          onError: (_) {},
          pollInterval: const Duration(milliseconds: 10),
        );
        const texts = CaptureNotificationTexts(
          title: 'a',
          text: 'b',
          stopLabel: 'c',
        );
        expect(await feeder.start(texts), CaptureStartResult.started);

        cap.controller.add(CaptureConfig(_config));
        cap.controller.add(
          CaptureFrame(data: _idr, isKey: true, ptsUs: 10, dropped: 0),
        );
        cap.controller.add(
          CaptureFrame(data: _delta, isKey: false, ptsUs: 20, dropped: 0),
        );
        await Future<void>.delayed(const Duration(milliseconds: 5));
        expect(core.pushedConfigs, [_config]);
        expect(core.pushedFrames.map((f) => (f.isKey, f.ptsUs)), [
          (true, 10),
          (false, 20),
        ]);
        expect(feeder.framesPushed, 2);

        core.keyframeRequest = true;
        await Future<void>.delayed(const Duration(milliseconds: 40));
        expect(cap.syncFrameCalls, 1, reason: 'one request, one sync frame');

        core.pushResult = PushResult.queueFull;
        cap.controller.add(
          CaptureFrame(data: _delta, isKey: false, ptsUs: 30, dropped: 0),
        );
        await Future<void>.delayed(const Duration(milliseconds: 5));
        expect(feeder.framesRefused, 1);
        expect(feeder.lastResult, PushResult.queueFull);

        await feeder.stop();
        expect(cap.stopCalls, 1);
        cap.controller.add(
          CaptureFrame(data: _delta, isKey: false, ptsUs: 40, dropped: 0),
        );
        await Future<void>.delayed(const Duration(milliseconds: 5));
        expect(core.pushedFrames, hasLength(3), reason: 'nothing after stop');
      },
    );

    test(
      'the config is handed over again if the sender was not there yet',
      () async {
        final core = FakeCoreApi()..acceptConfig = false;
        final cap = FakeAndroidCapture()
          ..consent.complete(CaptureStartResult.started);
        final feeder = CaptureFeeder(
          core: core,
          capture: cap,
          onStopped: (_) {},
          onError: (_) {},
        );
        await feeder.start(
          const CaptureNotificationTexts(title: 'a', text: 'b', stopLabel: 'c'),
        );
        cap.controller.add(CaptureConfig(_config));
        await Future<void>.delayed(Duration.zero);
        expect(core.pushedConfigs, hasLength(1));
        core.acceptConfig = true; // the sender is up now
        cap.controller.add(
          CaptureFrame(data: _idr, isKey: true, ptsUs: 1, dropped: 0),
        );
        cap.controller.add(
          CaptureFrame(data: _delta, isKey: false, ptsUs: 2, dropped: 0),
        );
        await Future<void>.delayed(Duration.zero);
        expect(
          core.pushedConfigs,
          hasLength(2),
          reason: 'retried once, then accepted',
        );
        await feeder.stop();
      },
    );

    test('stop while the consent dialog is open, then accepted: capture stopped, no timer, no frames', () async {
      final core = FakeCoreApi();
      final cap = FakeAndroidCapture();
      final feeder = CaptureFeeder(
        core: core,
        capture: cap,
        onStopped: (_) {},
        onError: (_) {},
        pollInterval: const Duration(milliseconds: 10),
      );
      const texts = CaptureNotificationTexts(
        title: 'a',
        text: 'b',
        stopLabel: 'c',
      );
      final started = feeder.start(texts);
      await feeder.stop(); // the dialog is still open
      expect(cap.stopCalls, 1);
      cap.consent.complete(CaptureStartResult.started); // accepted late
      await started;
      expect(cap.stopCalls, 2, reason: 'the late service is stopped again');
      core.keyframeRequest = true;
      cap.controller.add(
        CaptureFrame(data: _idr, isKey: true, ptsUs: 1, dropped: 0),
      );
      await Future<void>.delayed(const Duration(milliseconds: 50));
      expect(core.pushedFrames, isEmpty);
      expect(cap.syncFrameCalls, 0, reason: 'no poll timer was created');
    });

    test(
      'a feeder that lost the service to a newer session does not stop it',
      () async {
        final cap = FakeAndroidCapture()
          ..consent.complete(CaptureStartResult.started);
        var owns = false;
        final feeder = CaptureFeeder(
          core: FakeCoreApi(),
          capture: cap,
          onStopped: (_) {},
          onError: (_) {},
          ownsCapture: () => owns,
        );
        await feeder.start(
          const CaptureNotificationTexts(title: 'a', text: 'b', stopLabel: 'c'),
        );
        await feeder.stop();
        expect(cap.stopCalls, 0);
      },
    );

    test('a refused consent leaves nothing running', () async {
      final core = FakeCoreApi();
      final cap = FakeAndroidCapture()
        ..consent.complete(CaptureStartResult.denied);
      final feeder = CaptureFeeder(
        core: core,
        capture: cap,
        onStopped: (_) {},
        onError: (_) {},
      );
      final r = await feeder.start(
        const CaptureNotificationTexts(title: 'a', text: 'b', stopLabel: 'c'),
      );
      expect(r, CaptureStartResult.denied);
      expect(cap.controller.hasListener, isFalse);
    });
  });

  group('Send screen on Android', () {
    testWidgets(
      'a second start releases the first feeder and keeps the new service',
      (tester) async {
        final core = androidCore();
        final cap = FakeAndroidCapture();
        final c = SenderController(core: core, capture: cap);
        const texts = CaptureNotificationTexts(
          title: 'a',
          text: 'b',
          stopLabel: 'c',
        );
        final first = Completer<CaptureStartResult>();
        cap.consent = first;
        final f1 = c.startCapture(
          code: 'K7M2QX',
          server: 's',
          notification: texts,
        );
        await tester.pump(const Duration(milliseconds: 5));
        final second = Completer<CaptureStartResult>();
        cap.consent = second;
        final f2 = c.startCapture(
          code: 'K7M2QX',
          server: 's',
          notification: texts,
        );
        await tester.pump(const Duration(milliseconds: 5));
        expect(cap.stopCalls, 1, reason: 'the old feeder was released');
        second.complete(CaptureStartResult.started);
        await tester.pump(const Duration(milliseconds: 5));
        first.complete(
          CaptureStartResult.started,
        ); // the stale one answers late
        await Future.wait([f1, f2]);
        expect(
          cap.stopCalls,
          1,
          reason: 'the stale feeder must not stop the newer service',
        );
        expect(core.startEncodedCalls, 1);
        await c.stop();
        c.dispose();
        await tester.pump(const Duration(milliseconds: 5));
      },
    );
  });
}
