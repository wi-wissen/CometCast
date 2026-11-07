// SPDX-License-Identifier: GPL-3.0-or-later
// Android platform pieces without a device: fake platform channels, a fake hardware decoder and a
// fake core. Covers
//   * the platform wrapper and the worker set-up (`configureAndroidWorkers`),
//   * the television profile: receiver-only home screen, D-pad, auto-start, big room code, no
//     pointer-only controls, keep-screen-on,
//   * the hardware decoder path of the receiver controller (pass-through, texture, fallback).
// None of this proves anything about a real TV or MediaCodec (docs/android.md).

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/android_platform.dart';
import 'package:cometcast/core/android_video.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/receiver_controller.dart';

import 'fake_core.dart';

class FakePlatform implements AndroidPlatform {
  final awake = <bool>[];
  String? libDir = '/data/app/x/lib/arm64';

  @override
  Future<bool> isTelevision() async => true;

  @override
  Future<String?> nativeLibraryDir() async => libDir;

  @override
  Future<void> setKeepScreenOn(bool on) async => awake.add(on);

  @override
  Future<Map<String, Object?>> deviceInfo() async => {'model': 'Test TV'};
}

class FakeVideoDecoder implements AndroidVideoDecoder {
  final controller = StreamController<VideoDecoderEvent>.broadcast();
  Completer<int> startGate = Completer()..complete(7);
  Object? startError;
  int startCalls = 0;
  int stopCalls = 0;
  final queued = <({Uint8List data, int ptsUs, bool key})>[];

  @override
  Stream<VideoDecoderEvent> get events => controller.stream;

  @override
  Future<int> start() {
    startCalls++;
    final e = startError;
    if (e != null) return Future.error(e);
    return startGate.future;
  }

  @override
  void queue(Uint8List data, {required int ptsUs, required bool key}) =>
      queued.add((data: data, ptsUs: ptsUs, key: key));

  @override
  Future<void> stop() async => stopCalls++;
}

EncodedVideoUnit unit(int tag, {bool key = false}) => EncodedVideoUnit(
  data: Uint8List.fromList([0, 0, 0, 1, tag]),
  isKey: key,
  ptsUs: BigInt.from(tag * 1000),
);

Future<FakeCoreApi> openTvReceive(
  WidgetTester tester,
  FakePlatform platform, {
  FakeVideoDecoder? decoder,
  String locale = 'en',
}) async {
  final core = FakeCoreApi();
  tester.view.physicalSize = const Size(1920, 1080);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    CometCastApp(
      locale: Locale(locale),
      core: core,
      tv: true,
      androidPlatform: platform,
      videoDecoder: decoder,
      deviceName: 'Test TV',
    ),
  );
  await tester.pumpAndSettle();
  return core;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(ReceiverController.resetHardwareBroken);
  tearDown(ReceiverController.resetHardwareBroken);

  group('platform wrapper', () {
    const channel = MethodChannel('de.cometcast/platform');

    tearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null),
    );

    test('talks to the platform channel', () async {
      final calls = <MethodCall>[];
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (call) async {
            calls.add(call);
            switch (call.method) {
              case 'isTelevision':
                return true;
              case 'nativeLibraryDir':
                return '/lib';
              case 'deviceInfo':
                return {'model': 'Streamer', 'sdkInt': 34};
            }
            return null;
          });
      const p = ChannelAndroidPlatform();
      expect(await p.isTelevision(), isTrue);
      expect(await p.nativeLibraryDir(), '/lib');
      await p.setKeepScreenOn(true);
      expect((await p.deviceInfo())['model'], 'Streamer');
      expect(calls.map((c) => c.method), [
        'isTelevision',
        'nativeLibraryDir',
        'setKeepScreenOn',
        'deviceInfo',
      ]);
      expect(calls[2].arguments, true);
    });
  });

  group('workers as native libraries', () {
    test('both are given to the core when the files exist', () async {
      final core = FakeCoreApi();
      await configureAndroidWorkers(core, FakePlatform(), exists: (_) => true);
      expect(core.docsWorker, '/data/app/x/lib/arm64/libcc_docs_worker.so');
      expect(core.officeWorker, '/data/app/x/lib/arm64/libcc_office_worker.so');
    });

    test(
      'a missing file is not given (the core then reports no worker)',
      () async {
        final core = FakeCoreApi();
        await configureAndroidWorkers(
          core,
          FakePlatform(),
          exists: (p) => p.endsWith('libcc_docs_worker.so'),
        );
        expect(core.docsWorker, isNotNull);
        expect(core.officeWorker, isNull);
      },
    );

    test('no library directory leaves everything as it is', () async {
      final core = FakeCoreApi();
      await configureAndroidWorkers(
        core,
        FakePlatform()..libDir = null,
        exists: (_) => true,
      );
      expect(core.docsWorker, isNull);
      expect(core.officeWorker, isNull);
    });
  });

  group('television profile', () {});

  group('hardware decoder path', () {
    test('controller: stop releases decoder and texture', () async {
      final decoder = FakeVideoDecoder();
      final core = FakeCoreApi();
      final c = ReceiverController(
        core: core,
        hardwareDecoder: decoder,
        setFullscreen: (_) async => false,
      );
      c.start('s:1');
      await Future<void>.delayed(Duration.zero);
      expect(c.textureId, 7);
      await c.stop();
      expect(c.textureId, isNull);
      expect(core.passthrough, isFalse);
      expect(decoder.stopCalls, greaterThanOrEqualTo(1));
      c.dispose();
    });

    test('without a hardware decoder the core stays in software mode', () {
      final core = FakeCoreApi();
      final c = ReceiverController(
        core: core,
        setFullscreen: (_) async => false,
      );
      c.start('s:1');
      expect(core.passthrough, isFalse);
      c.dispose();
    });
  });

  group('video channel wrapper', () {
    const method = MethodChannel('de.cometcast/video');

    test('start returns the texture id, queue passes the unit', () async {
      final calls = <MethodCall>[];
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(method, (call) async {
            calls.add(call);
            return call.method == 'start' ? {'textureId': 42} : true;
          });
      addTearDown(
        () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(method, null),
      );
      const d = ChannelAndroidVideoDecoder();
      expect(await d.start(), 42);
      d.queue(Uint8List.fromList([1, 2]), ptsUs: 5, key: true);
      await d.stop();
      expect(calls.map((c) => c.method), ['start', 'queue', 'stop']);
      final args = calls[1].arguments as Map;
      expect(args['ptsUs'], 5);
      expect(args['key'], true);
    });

    test('events are parsed, unknown ones ignored', () {
      expect(
        parseVideoEvent({'type': 'size', 'width': 3, 'height': 2}),
        isA<VideoSize>(),
      );
      expect(
        parseVideoEvent({'type': 'size', 'width': 0, 'height': 2}),
        isNull,
      );
      expect(
        parseVideoEvent({'type': 'needKeyFrame'}),
        isA<VideoNeedKeyFrame>(),
      );
      expect(
        (parseVideoEvent({'type': 'error', 'message': 'x'})!
                as VideoDecoderError)
            .message,
        'x',
      );
      expect(parseVideoEvent({'type': 'bogus'}), isNull);
      expect(parseVideoEvent('nope'), isNull);
    });
  });
}
