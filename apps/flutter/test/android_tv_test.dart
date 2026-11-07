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

const _codecInfo = UiCodecInfo(
  fileName: 'libopenh264-2.6.0-android-arm64.8.so',
  version: '2.6.0',
  host: 'ciscobinary.openh264.org',
  downloadBytes: 634264,
  autoDownload: false,
  adminDir: null,
);

void main() {
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

  group('television profile', () {
    testWidgets(
      'home shows only Receive, focused, plus the receiver-only note',
      (tester) async {
        await openTvReceive(tester, FakePlatform());
        expect(find.text('Send screen'), findsNothing);
        expect(find.text('Send file or link'), findsNothing);
        expect(find.byKey(const Key('tvReceiverOnlyNotice')), findsOneWidget);
        final focused = FocusManager.instance.primaryFocus?.context;
        expect(
          focused?.findAncestorWidgetOfExactType<ListTile>()?.key,
          const Key('tvReceiveTile'),
        );
      },
    );

    testWidgets('home note is translated', (tester) async {
      await openTvReceive(tester, FakePlatform(), locale: 'de');
      expect(find.textContaining('empfängt nur'), findsOneWidget);
    });

    testWidgets(
      'select on the focused tile opens Receive and starts by itself',
      (tester) async {
        final platform = FakePlatform();
        final core = await openTvReceive(tester, platform);
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pumpAndSettle();
        // Started without pressing "Start".
        expect(core.receiverServer, 'relay.cometcast.de:443');
        expect(platform.awake, [true]);

        core.receiverEvents.add(
          const ReceiverUiEvent.roomCode(
            code: 'K7M2QX',
            joinMode: UiJoinMode.open,
          ),
        );
        await tester.pump();
        final code = tester.widget<SelectableText>(
          find.byKey(const Key('roomCode')),
        );
        // Very large (10-foot) and no copy button.
        expect(code.style!.fontSize, greaterThanOrEqualTo(120));
        expect(find.byKey(const Key('copyCodeButton')), findsNothing);
      },
    );

    testWidgets(
      'hardware decoder: no codec card and no download, even if OpenH264 is missing',
      (tester) async {
        final core = FakeCoreApi()
          ..codecStateResult = const UiCodecState.missing();
        tester.view.physicalSize = const Size(1920, 1080);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        await tester.pumpWidget(
          CometCastApp(
            core: core,
            tv: true,
            androidPlatform: FakePlatform(),
            videoDecoder: FakeVideoDecoder(),
          ),
        );
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('roomCode')), findsOneWidget);
        expect(find.byKey(const Key('codecNotice')), findsNothing);
        expect(core.ensureCodecCalls, 0);
      },
    );

    testWidgets(
      'software decoder: the codec card is shown while it is missing',
      (tester) async {
        final core = FakeCoreApi()
          ..codecStateResult = const UiCodecState.missing()
          ..codecInfoResult = const UiCodecInfo(
            fileName: 'libopenh264-2.6.0-android-arm64.8.so',
            version: '2.6.0',
            host: 'ciscobinary.openh264.org',
            downloadBytes: 634264,
            autoDownload: false,
            adminDir: null,
          );
        tester.view.physicalSize = const Size(1920, 1080);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        await tester.pumpWidget(
          CometCastApp(core: core, tv: true, androidPlatform: FakePlatform()),
        );
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('codecNotice')), findsOneWidget);
      },
    );

    testWidgets('keep-screen-on is released when the receive view closes', (
      tester,
    ) async {
      final platform = FakePlatform();
      await openTvReceive(tester, platform);
      await tester.sendKeyEvent(LogicalKeyboardKey.select);
      await tester.pumpAndSettle();
      expect(platform.awake, [true]);
      // Back key of the remote.
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(platform.awake, [true, false]);
    });

    testWidgets('no pointer-only toolbar over the live picture on a TV', (
      tester,
    ) async {
      final core = await openTvReceive(tester, FakePlatform());
      await tester.sendKeyEvent(LogicalKeyboardKey.select);
      await tester.pumpAndSettle();
      core.receiverEvents.add(
        const ReceiverUiEvent.roomCode(
          code: 'K7M2QX',
          joinMode: UiJoinMode.open,
        ),
      );
      core.receiverEvents.add(
        const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
      );
      core.frames.add(
        VideoFrame(seq: BigInt.one, width: 2, height: 2, rgba: Uint8List(16)),
      );
      core.receiverEvents.add(const ReceiverUiEvent.frameReady());
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 50));
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      await tester.pump();
      expect(find.byKey(const Key('videoView')), findsOneWidget);
      // The stop button of the video toolbar is there (focusable), the drawing toolbar is not.
      expect(find.byKey(const Key('stopButton')), findsOneWidget);
      expect(find.byTooltip('Pen'), findsNothing);
    });
  });

  group('hardware decoder path', () {
    testWidgets(
      'hardware failure: the codec card appears, nothing is downloaded before consent',
      (tester) async {
        final decoder = FakeVideoDecoder();
        final core = FakeCoreApi()
          ..codecStateResult = const UiCodecState.missing()
          ..codecInfoResult = _codecInfo;
        tester.view.physicalSize = const Size(1920, 1080);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.reset);
        await tester.pumpWidget(
          CometCastApp(
            core: core,
            tv: true,
            androidPlatform: FakePlatform(),
            videoDecoder: decoder,
          ),
        );
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('codecNotice')), findsNothing);
        decoder.controller.add(const VideoDecoderError('codec died'));
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('codecNotice')), findsOneWidget);
        expect(find.byKey(const Key('codecDownload')), findsOneWidget);
        expect(find.byKey(const Key('hardwareError')), findsOneWidget);
        expect(find.textContaining('video decoder of this device'), findsOne);
        expect(core.ensureCodecCalls, 0);
        await tester.tap(find.byKey(const Key('codecDownload')));
        await tester.pumpAndSettle();
        expect(core.ensureCodecCalls, 1);
      },
    );

    testWidgets('hardware failure: German text and an unsupported codec', (
      tester,
    ) async {
      final decoder = FakeVideoDecoder();
      final core = FakeCoreApi()
        ..codecStateResult = const UiCodecState.unsupported(reason: 'no arch');
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        CometCastApp(
          locale: const Locale('de'),
          core: core,
          tv: true,
          androidPlatform: FakePlatform(),
          videoDecoder: decoder,
        ),
      );
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.select);
      await tester.pumpAndSettle();
      decoder.controller.add(const VideoDecoderError('codec died'));
      await tester.pumpAndSettle();
      expect(find.textContaining('Videodecoder dieses Geräts'), findsOne);
      expect(find.byKey(const Key('codecUnsupported')), findsOneWidget);
      expect(core.ensureCodecCalls, 0);
    });

    testWidgets('hardware failure is sticky: new sessions use software', (
      tester,
    ) async {
      final decoder = FakeVideoDecoder();
      final core = FakeCoreApi()
        ..codecStateResult = const UiCodecState.missing()
        ..codecInfoResult = _codecInfo;
      tester.view.physicalSize = const Size(1920, 1080);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      Future<void> open() async {
        await tester.pumpWidget(
          CometCastApp(
            key: UniqueKey(),
            core: core,
            tv: true,
            androidPlatform: FakePlatform(),
            videoDecoder: decoder,
          ),
        );
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pumpAndSettle();
      }

      await open();
      expect(decoder.startCalls, 1);
      decoder.controller.add(const VideoDecoderError('codec died'));
      await tester.pumpAndSettle();
      // A new receive view (new session): no hardware attempt, card from the start.
      await open();
      expect(decoder.startCalls, 1);
      expect(core.passthrough, isFalse);
      expect(find.byKey(const Key('codecNotice')), findsOneWidget);
      expect(find.byKey(const Key('hardwareError')), findsOneWidget);
      expect(core.ensureCodecCalls, 0);
    });

    testWidgets(
      'pass-through, texture, units reach the decoder, size shows it',
      (tester) async {
        final decoder = FakeVideoDecoder();
        final core = await openTvReceive(
          tester,
          FakePlatform(),
          decoder: decoder,
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pumpAndSettle();
        expect(core.passthrough, isTrue);
        expect(decoder.startCalls, 1);

        core.receiverEvents.add(
          const ReceiverUiEvent.roomCode(
            code: 'K7M2QX',
            joinMode: UiJoinMode.open,
          ),
        );
        core.receiverEvents.add(
          const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
        );
        core.encodedUnits.addAll([unit(1, key: true), unit(2)]);
        core.receiverEvents.add(const ReceiverUiEvent.encodedReady());
        await tester.pump();
        await tester.pump();
        expect(decoder.queued.map((u) => u.ptsUs), [1000, 2000]);
        expect(decoder.queued.first.key, isTrue);

        // No picture is shown before the decoder says how large it is.
        expect(find.byKey(const Key('videoTexture')), findsNothing);
        decoder.controller.add(const VideoSize(1280, 720));
        await tester.pump();
        await tester.pump();
        final tex = tester.widget<Texture>(
          find.byKey(const Key('videoTexture')),
        );
        expect(tex.textureId, 7);
        // The aspect ratio comes from the decoder's size.
        expect(
          tester
              .widget<AspectRatio>(find.byType(AspectRatio).first)
              .aspectRatio,
          closeTo(16 / 9, 0.001),
        );
      },
    );

    testWidgets('a key frame request of the decoder reaches the core', (
      tester,
    ) async {
      final decoder = FakeVideoDecoder();
      final core = await openTvReceive(
        tester,
        FakePlatform(),
        decoder: decoder,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.select);
      await tester.pumpAndSettle();
      decoder.controller.add(const VideoNeedKeyFrame());
      await tester.pump();
      expect(core.videoKeyframeRequests, 1);
    });

    testWidgets('a failing decoder falls back to the software decoder', (
      tester,
    ) async {
      final decoder = FakeVideoDecoder();
      final core = await openTvReceive(
        tester,
        FakePlatform(),
        decoder: decoder,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.select);
      await tester.pumpAndSettle();
      expect(core.passthrough, isTrue);
      decoder.controller.add(const VideoDecoderError('codec died'));
      await tester.pump();
      expect(core.passthrough, isFalse);
      expect(decoder.stopCalls, greaterThanOrEqualTo(1));
    });

    testWidgets('a decoder that cannot start falls back at once', (
      tester,
    ) async {
      final decoder = FakeVideoDecoder()..startError = StateError('no codec');
      final core = await openTvReceive(
        tester,
        FakePlatform(),
        decoder: decoder,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.select);
      await tester.pumpAndSettle();
      expect(core.passthrough, isFalse);
    });

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
