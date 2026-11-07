// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests for the receive screen with a fake core.

import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/annotations.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/window_control.dart';

import 'fake_core.dart';

Future<FakeCoreApi> openReceive(
  WidgetTester tester, {
  String locale = 'en',
}) async {
  final core = FakeCoreApi();
  // Tall window so the whole (lazily built) page is on screen.
  tester.view.physicalSize = const Size(900, 1800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(CometCastApp(locale: Locale(locale), core: core));
  await tester.pumpAndSettle();
  await tester.tap(
    find.text(
      locale == 'en' ? 'Receive on this device' : 'Auf diesem Gerät empfangen',
    ),
  );
  await tester.pumpAndSettle();
  return core;
}

String textOf(WidgetTester tester, String key) =>
    tester.widget<Text>(find.byKey(Key(key))).data!;

void main() {
  videoTests();
  annotationTests();
  testWidgets('server field has the default and the notice is shown', (
    tester,
  ) async {
    await openReceive(tester);
    final field = tester.widget<TextField>(
      find.byKey(const Key('serverField')),
    );
    expect(field.controller!.text, 'relay.cometcast.de:443');
    expect(find.byKey(const Key('receiveNotice')), findsOneWidget);
    expect(find.text('Not started.'), findsOneWidget);
  });

  testWidgets('shows room code, then live stats', (tester) async {
    final core = await openReceive(tester);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(core.receiverServer, 'relay.cometcast.de:443');
    expect(find.text('Connecting to the server …'), findsOneWidget);

    core.receiverEvents.add(
      const ReceiverUiEvent.roomCode(code: 'K7M2QX', joinMode: UiJoinMode.open),
    );
    await tester.pump();
    expect(find.text('K7M2QX'), findsOneWidget);
    expect(find.textContaining('Waiting for a sender'), findsOneWidget);

    core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Guest 1'));
    core.receiverEvents.add(
      const ReceiverUiEvent.connectionState(state: UiConnectionState.connected),
    );
    await tester.pump();
    expect(find.text('Connection: connected'), findsOneWidget);

    core.receiverEvents.add(
      const ReceiverUiEvent.videoTrack(mime: 'video/VP8', codec: 'vp8'),
    );
    core.receiverEvents.add(
      ReceiverUiEvent.stats(
        packets: BigInt.from(120),
        bytes: BigInt.from(48000),
        frames: BigInt.from(15),
        keyframes: BigInt.from(15),
      ),
    );
    await tester.pump();
    expect(textOf(tester, 'statPackets'), '120');
    expect(textOf(tester, 'statBytes'), '48000');
    expect(textOf(tester, 'statFrames'), '15');
    expect(textOf(tester, 'statKeyframes'), '15');
    expect(textOf(tester, 'statCodec'), 'vp8');

    core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
    await tester.pump();
    expect(find.text('The sender has left.'), findsOneWidget);
    // The start button is back so a new room can be created.
    expect(find.byKey(const Key('startButton')), findsOneWidget);
  });

  testWidgets('stop button stops the core and clears the code', (tester) async {
    final core = await openReceive(tester);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    core.receiverEvents.add(
      const ReceiverUiEvent.roomCode(code: 'AB23CD', joinMode: UiJoinMode.open),
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('stopButton')));
    await tester.pump();
    expect(core.stopReceiverCalls, 1);
    expect(find.text('AB23CD'), findsNothing);
    expect(find.text('Not started.'), findsOneWidget);
  });

  testWidgets('a start error is shown with a translated prefix', (
    tester,
  ) async {
    final core = await openReceive(tester);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    core.receiverEvents.addError('signalling connection failed');
    await tester.pump();
    expect(find.textContaining('Error: '), findsOneWidget);
    expect(find.textContaining('signalling connection failed'), findsOneWidget);
  });

  testWidgets('Enter in the server field starts, empty server is rejected', (
    tester,
  ) async {
    final core = await openReceive(tester);
    await tester.enterText(find.byKey(const Key('serverField')), '  ');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    expect(find.byKey(const Key('serverError')), findsOneWidget);
    expect(core.receiverServer, isNull);

    await tester.enterText(
      find.byKey(const Key('serverField')),
      'example.test:9000',
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    expect(core.receiverServer, 'example.test:9000');
  });

  testWidgets('Escape goes back to the home list', (tester) async {
    await openReceive(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Send screen'), findsOneWidget);
  });

  testWidgets('a stream that ends silently shows "Connection ended."', (
    tester,
  ) async {
    final core = await openReceive(tester);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    core.receiverEvents.add(
      const ReceiverUiEvent.roomCode(code: 'AB23CD', joinMode: UiJoinMode.open),
    );
    await tester.pump();
    await core.receiverEvents.close();
    await tester.pump();
    expect(find.text('Connection ended.'), findsOneWidget);
    expect(find.textContaining('Waiting for a sender'), findsNothing);
  });

  testWidgets('status text is a live region', (tester) async {
    await openReceive(tester);
    final handle = tester.ensureSemantics();
    expect(
      tester.getSemantics(find.byKey(const Key('statusText'))),
      matchesSemantics(
        label: 'Not started.',
        isLiveRegion: true,
        hasEnabledState: false,
      ),
    );
    handle.dispose();
  });

  testWidgets('German texts', (tester) async {
    await openReceive(tester, locale: 'de');
    expect(find.text('Empfang starten'), findsOneWidget);
    expect(find.text('Raumcode'), findsOneWidget);
    expect(find.text('Nicht gestartet.'), findsOneWidget);
    expect(find.textContaining('wird nur gezählt'), findsOneWidget);
  });
}

/// A [w] x [h] picture of one colour, as the core would hand it out.
VideoFrame solidFrame(int w, int h, List<int> rgb, {int seq = 1}) {
  final px = Uint8List(w * h * 4);
  for (var i = 0; i < w * h; i++) {
    px.setRange(i * 4, i * 4 + 4, [...rgb, 255]);
  }
  return VideoFrame(seq: BigInt.from(seq), width: w, height: h, rgba: px);
}

/// Hands a picture to the controller the way the core does: `frameReady`, then takeFrame.
/// Decoding the pixels into an image needs the real event loop, hence runAsync.
Future<void> deliverFrame(
  WidgetTester tester,
  FakeCoreApi core,
  VideoFrame frame,
) async {
  core.frames.add(frame);
  core.receiverEvents.add(const ReceiverUiEvent.frameReady());
  // Wait (in real time, up to 5 s) until the picture of this size is on screen.
  for (var i = 0; i < 50; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 100)),
    );
    await tester.pump();
    final raws = find.byType(RawImage).evaluate();
    if (raws.isNotEmpty) {
      final img = (raws.first.widget as RawImage).image;
      if (img != null &&
          img.width == frame.width &&
          img.height == frame.height) {
        return;
      }
    }
  }
}

Future<FakeCoreApi> startAndConnect(WidgetTester tester) async {
  final core = await openReceive(tester);
  await tester.tap(find.byKey(const Key('startButton')));
  await tester.pump();
  core.receiverEvents.add(
    const ReceiverUiEvent.roomCode(code: 'K7M2QX', joinMode: UiJoinMode.open),
  );
  core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Guest 1'));
  core.receiverEvents.add(
    const ReceiverUiEvent.connectionState(state: UiConnectionState.connected),
  );
  await tester.pump();
  return core;
}

void videoTests() {
  testWidgets('waiting state, then the picture replaces the form', (
    tester,
  ) async {
    final core = await startAndConnect(tester);
    // Sender is there, no picture yet: a translated waiting line.
    expect(find.text("Waiting for the sender's screen …"), findsOneWidget);
    expect(find.byKey(const Key('videoView')), findsNothing);

    core.receiverEvents.add(
      const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
    );
    await deliverFrame(tester, core, solidFrame(8, 6, [255, 0, 0]));
    expect(find.byKey(const Key('videoView')), findsOneWidget);
    expect(find.byKey(const Key('startButton')), findsNothing);
    final raw = tester.widget<RawImage>(find.byType(RawImage));
    expect((raw.image!.width, raw.image!.height), (8, 6));
    // The core was told how much room the picture has (physical pixels of the test window).
    await tester.pump();
    expect(core.maxSizes, isNotEmpty);
    expect(core.maxSizes.last.$1, greaterThan(0));

    // A newer picture replaces the old one.
    await deliverFrame(tester, core, solidFrame(4, 2, [0, 0, 255], seq: 2));
    final raw2 = tester.widget<RawImage>(find.byType(RawImage));
    expect((raw2.image!.width, raw2.image!.height), (4, 2));

    // The sender leaves: the picture goes, the status says why.
    core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const Key('videoView')), findsNothing);
    expect(find.text('The sender has left.'), findsOneWidget);
  });

  testWidgets('a track that is not H.264 says so instead of a picture', (
    tester,
  ) async {
    final core = await startAndConnect(tester);
    core.receiverEvents.add(
      const ReceiverUiEvent.videoTrack(mime: 'video/VP8', codec: 'vp8'),
    );
    await tester.pump();
    expect(
      find.text('The sender uses vp8. This version only shows H.264 pictures.'),
      findsOneWidget,
    );
  });

  testWidgets('statistics overlay is optional and shows the decoder counters', (
    tester,
  ) async {
    final core = await startAndConnect(tester);
    await deliverFrame(tester, core, solidFrame(8, 6, [0, 255, 0]));
    expect(find.byKey(const Key('videoStats')), findsNothing);
    core.receiverEvents.add(
      ReceiverUiEvent.videoStats(
        decoded: BigInt.from(42),
        delivered: BigInt.from(40),
        dropped: BigInt.from(2),
        droppedPackets: BigInt.zero,
        decodeErrors: BigInt.zero,
        keyframeRequests: BigInt.from(1),
        width: 8,
        height: 6,
        avgDecodeUs: BigInt.from(2500),
      ),
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('statsButton')));
    await tester.pump();
    final text = tester.widget<Text>(
      find.descendant(
        of: find.byKey(const Key('videoStats')),
        matching: find.byType(Text),
      ),
    );
    expect(text.data, contains('Decoded: 42'));
    expect(text.data, contains('Dropped: 2'));
    expect(text.data, contains('Decode time: 2.5 ms'));
    expect(text.data, contains('Key frame requests: 1'));
    expect(text.data, contains('Picture size: 8 x 6'));
    // The key I hides it again.
    await tester.sendKeyEvent(LogicalKeyboardKey.keyI);
    await tester.pump();
    expect(find.byKey(const Key('videoStats')), findsNothing);
  });

  testWidgets('F11 toggles full screen, Escape leaves it before going back', (
    tester,
  ) async {
    final calls = <Object?>[];
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(const MethodChannel(windowChannelName), (
      call,
    ) async {
      expect(call.method, 'setFullscreen');
      calls.add(call.arguments);
      return null;
    });
    addTearDown(
      () => messenger.setMockMethodCallHandler(
        const MethodChannel(windowChannelName),
        null,
      ),
    );
    final core = await startAndConnect(tester);
    await deliverFrame(tester, core, solidFrame(8, 6, [255, 0, 0]));

    await tester.sendKeyEvent(LogicalKeyboardKey.f11);
    await tester.pump();
    expect(calls, [true]);
    expect(find.byTooltip('Leave full screen (Esc)'), findsOneWidget);

    // First Escape: leave full screen, stay on the picture.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(calls, [true, false]);
    expect(find.byKey(const Key('videoView')), findsOneWidget);
    expect(find.byTooltip('Full screen (F11)'), findsOneWidget);

    // The button does the same as the key.
    await tester.tap(find.byKey(const Key('fullscreenButton')));
    await tester.pump();
    expect(calls, [true, false, true]);

    // Stopping while in full screen leaves it.
    await tester.tap(find.byKey(const Key('stopButton')));
    await tester.pump();
    await tester.pump();
    expect(calls, [true, false, true, false]);
    expect(core.stopReceiverCalls, 1);
    expect(find.byKey(const Key('startButton')), findsOneWidget);
  });

  testWidgets('without a platform handler the window stays windowed', (
    tester,
  ) async {
    final core = await startAndConnect(tester);
    await deliverFrame(tester, core, solidFrame(8, 6, [255, 0, 0]));
    await tester.sendKeyEvent(LogicalKeyboardKey.f11);
    await tester.pump();
    expect(find.byTooltip('Full screen (F11)'), findsOneWidget);
  });

  testWidgets('German texts of the picture view', (tester) async {
    final core = await openReceive(tester, locale: 'de');
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    core.receiverEvents.add(
      const ReceiverUiEvent.roomCode(code: 'K7M2QX', joinMode: UiJoinMode.open),
    );
    core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Guest 1'));
    await tester.pump();
    expect(find.text('Warte auf den Bildschirm des Senders …'), findsOneWidget);
    await deliverFrame(tester, core, solidFrame(8, 6, [255, 0, 0]));
    expect(find.byTooltip('Vollbild (F11)'), findsOneWidget);
    expect(find.byTooltip('Statistik anzeigen (I)'), findsOneWidget);
  });

  testWidgets('the picture and the statistics have screen reader labels', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    final core = await startAndConnect(tester);
    await deliverFrame(tester, core, solidFrame(8, 6, [255, 0, 0]));
    expect(
      find.bySemanticsLabel('Shared screen of the sender'),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const Key('statsButton')));
    await tester.pump();
    expect(find.bySemanticsLabel(RegExp('^Video statistics')), findsOneWidget);
    handle.dispose();
  });

  testWidgets('the server is remembered when receiving starts', (tester) async {
    final core = FakeCoreApi();
    final store = MemorySettingsStore();
    tester.view.physicalSize = const Size(900, 1800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      CometCastApp(
        locale: const Locale('en'),
        core: core,
        settings: AppSettings.fromDefaults(core.serverDefaults(), store: store),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Receive on this device'));
    await tester.pumpAndSettle();
    // A suggestion chip fills the field.
    await tester.tap(find.byKey(const Key('serverSuggestion:127.0.0.1:8443')));
    await tester.pump();
    final field = tester.widget<TextField>(
      find.byKey(const Key('serverField')),
    );
    expect(field.controller!.text, '127.0.0.1:8443');
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(core.receiverServer, '127.0.0.1:8443');
    expect(store.server, '127.0.0.1:8443');
  });

  test(
    'a saved server wins over the list, the environment wins over both',
    () async {
      final core = FakeCoreApi();
      final store = MemorySettingsStore('saved.example:1');
      expect(
        (await AppSettings.load(core: core, store: store)).serverAddress,
        'saved.example:1',
      );
      expect(
        (await AppSettings.load(
          core: core,
          store: store,
          envServer: 'env.example:2',
        )).serverAddress,
        'env.example:2',
      );
      final fresh = await AppSettings.load(
        core: core,
        store: MemorySettingsStore(),
      );
      expect(fresh.serverAddress, 'relay.cometcast.de:443');
      expect(fresh.serverSuggestions.map((s) => s.address), [
        'relay.cometcast.de:443',
        '127.0.0.1:8443',
      ]);
    },
  );
}

// --- the toolbar over the picture (pen, highlighter, eraser, laser, undo, clear, freeze) ---

/// The running picture with its toolbar; returns the core and the annotation state.
Future<(FakeCoreApi, AnnotationController)> videoWithToolbar(
  WidgetTester tester, {
  String locale = 'en',
}) async {
  final core = locale == 'en'
      ? await startAndConnect(tester)
      : await openReceive(tester, locale: locale);
  if (locale != 'en') {
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    core.receiverEvents.add(
      const ReceiverUiEvent.roomCode(code: 'K7M2QX', joinMode: UiJoinMode.open),
    );
    core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Guest 1'));
    await tester.pump();
  }
  core.receiverEvents.add(
    const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
  );
  await deliverFrame(tester, core, solidFrame(8, 6, [255, 0, 0]));
  final layer = tester.widget<AnnotationLayer>(find.byType(AnnotationLayer));
  return (core, layer.controller);
}

Future<void> draw(WidgetTester tester, Offset from, Offset by) async {
  final g = await tester.startGesture(from);
  await g.moveBy(by ~/ 2);
  await g.moveBy(by - by ~/ 2);
  await g.up();
  await tester.pump();
}

void annotationTests() {
  group('toolbar over the picture', () {
    testWidgets('the pen draws a stroke in the chosen colour and size', (
      tester,
    ) async {
      final (_, ann) = await videoWithToolbar(tester);
      expect(find.byKey(const Key('annotationToolbar')), findsOneWidget);
      // Without a tool the picture does not react.
      final box = tester.getRect(find.byKey(const Key('videoView')));
      await draw(tester, box.center, const Offset(60, 0));
      expect(ann.strokes, isEmpty);

      await tester.tap(find.byKey(const Key('toolPen')));
      await tester.tap(find.byKey(const Key('color_blue')));
      await tester.tap(find.byKey(const Key('size_2')));
      await tester.pump();
      await draw(tester, box.center, const Offset(80, 40));
      expect(ann.strokes, hasLength(1));
      final s = ann.strokes.single;
      expect(s.marker, isFalse);
      expect(s.color, AnnotationColor.blue.color);
      expect(s.width, 8);
      expect(s.points.length, greaterThanOrEqualTo(2));
      // Points are normalised to the picture (0..1).
      for (final p in s.points) {
        expect(p.dx, inInclusiveRange(0, 1));
        expect(p.dy, inInclusiveRange(0, 1));
      }
      expect(s.points.first.dx, closeTo(0.5, 0.01));
      expect(s.points.first.dy, closeTo(0.5, 0.01));
    });

    testWidgets(
      'the highlighter is wider, and picking a colour selects the pen',
      (tester) async {
        final (_, ann) = await videoWithToolbar(tester);
        // A colour with no tool switches to the pen.
        await tester.tap(find.byKey(const Key('color_green')));
        await tester.pump();
        expect(ann.tool, AnnotationTool.pen);
        await tester.tap(find.byKey(const Key('toolMarker')));
        await tester.pump();
        final box = tester.getRect(find.byKey(const Key('videoView')));
        await draw(tester, box.center, const Offset(50, 0));
        final s = ann.strokes.single;
        expect(s.marker, isTrue);
        expect(s.width, 20); // medium highlighter
        expect(s.color, AnnotationColor.green.color);
        // Choosing the active tool again goes back to plain viewing.
        await tester.tap(find.byKey(const Key('toolMarker')));
        await tester.pump();
        expect(ann.tool, isNull);
      },
    );

    testWidgets('undo, clear and the eraser', (tester) async {
      final (_, ann) = await videoWithToolbar(tester);
      final box = tester.getRect(find.byKey(const Key('videoView')));
      await tester.tap(find.byKey(const Key('toolPen')));
      await tester.pump();
      await draw(
        tester,
        box.topLeft + const Offset(100, 100),
        const Offset(100, 0),
      );
      await draw(
        tester,
        box.topLeft + const Offset(100, 300),
        const Offset(100, 0),
      );
      expect(ann.strokes, hasLength(2));

      await tester.tap(find.byKey(const Key('toolUndo')));
      await tester.pump();
      expect(ann.strokes, hasLength(1));

      // The eraser removes the stroke it crosses.
      await tester.tap(find.byKey(const Key('toolEraser')));
      await tester.pump();
      await draw(
        tester,
        box.topLeft + const Offset(150, 80),
        const Offset(0, 40),
      );
      expect(ann.strokes, isEmpty);
      // ... and that is one undo step.
      await tester.tap(find.byKey(const Key('toolUndo')));
      await tester.pump();
      expect(ann.strokes, hasLength(1));

      await tester.tap(find.byKey(const Key('toolClear')));
      await tester.pump();
      expect(ann.strokes, isEmpty);
      // Clear can be undone, and is disabled when there is nothing to clear.
      expect(
        tester.widget<IconButton>(find.byKey(const Key('toolClear'))).onPressed,
        isNull,
      );
      await tester.tap(find.byKey(const Key('toolUndo')));
      await tester.pump();
      expect(ann.strokes, hasLength(1));
    });

    testWidgets('the laser pointer follows a mouse and is not stored', (
      tester,
    ) async {
      final (_, ann) = await videoWithToolbar(tester);
      await tester.tap(find.byKey(const Key('toolLaser')));
      await tester.pump();
      expect(find.byKey(const Key('laserDot')), findsNothing);
      final box = tester.getRect(find.byKey(const Key('videoView')));
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      addTearDown(mouse.removePointer);
      await mouse.moveTo(box.center);
      await tester.pump();
      expect(find.byKey(const Key('laserDot')), findsOneWidget);
      final dot = tester.getCenter(find.byKey(const Key('laserDot')));
      expect((dot - box.center).distance, lessThan(1));
      await mouse.moveTo(box.center + const Offset(100, 20));
      await tester.pump();
      expect(
        (tester.getCenter(find.byKey(const Key('laserDot'))) - box.center).dx,
        closeTo(100, 1),
      );
      // Leaving the picture hides the dot; nothing was drawn.
      await mouse.moveTo(const Offset(5, 5));
      await tester.pump();
      expect(find.byKey(const Key('laserDot')), findsNothing);
      expect(ann.strokes, isEmpty);
    });

    testWidgets('freeze keeps the picture while the stream goes on', (
      tester,
    ) async {
      final (core, ann) = await videoWithToolbar(tester);
      await tester.tap(find.byKey(const Key('toolFreeze')));
      await tester.pump();
      expect(ann.frozen, isTrue);
      expect(find.byKey(const Key('frozenHint')), findsOneWidget);
      expect(
        find.text('Frozen. The live stream keeps running in the background.'),
        findsOneWidget,
      );

      // A new, differently sized picture arrives: the old one stays.
      core.frames.add(solidFrame(4, 2, [0, 0, 255], seq: 2));
      core.receiverEvents.add(const ReceiverUiEvent.frameReady());
      for (var i = 0; i < 5; i++) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 100)),
        );
        await tester.pump();
      }
      var raw = tester.widget<RawImage>(find.byType(RawImage));
      expect((raw.image!.width, raw.image!.height), (8, 6));
      // Strokes can be added while frozen.
      await tester.tap(find.byKey(const Key('toolPen')));
      await tester.pump();
      final box = tester.getRect(find.byKey(const Key('videoView')));
      await draw(tester, box.center, const Offset(40, 0));
      expect(ann.strokes, hasLength(1));

      // Unfreezing continues at the live picture.
      await tester.tap(find.byKey(const Key('toolFreeze')));
      await tester.pump();
      expect(ann.frozen, isFalse);
      expect(find.byKey(const Key('frozenHint')), findsNothing);
      raw = tester.widget<RawImage>(find.byType(RawImage));
      expect((raw.image!.width, raw.image!.height), (4, 2));
      expect(ann.strokes, hasLength(1));
    });

    testWidgets('the toolbar collapses and comes back', (tester) async {
      final (_, ann) = await videoWithToolbar(tester);
      await tester.tap(find.byKey(const Key('toolbarCollapse')));
      await tester.pump();
      expect(find.byKey(const Key('annotationToolbar')), findsNothing);
      expect(find.byKey(const Key('toolbarExpand')), findsOneWidget);
      await tester.tap(find.byKey(const Key('toolbarExpand')));
      await tester.pump();
      expect(find.byKey(const Key('annotationToolbar')), findsOneWidget);
      expect(ann.expanded, isTrue);
    });

    testWidgets('annotations end with the picture', (tester) async {
      final (core, ann) = await videoWithToolbar(tester);
      await tester.tap(find.byKey(const Key('toolPen')));
      await tester.pump();
      final box = tester.getRect(find.byKey(const Key('videoView')));
      await draw(tester, box.center, const Offset(40, 0));
      await tester.tap(find.byKey(const Key('toolFreeze')));
      await tester.pump();
      expect(ann.strokes, hasLength(1));
      expect(ann.frozen, isTrue);
      // The sender leaves: the picture goes, and so do strokes and the frozen picture.
      core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
      await tester.pump();
      await tester.pump();
      expect(find.byKey(const Key('videoView')), findsNothing);
      expect(ann.strokes, isEmpty);
      expect(ann.frozen, isFalse);
    });

    testWidgets('German labels and screen reader texts', (tester) async {
      final handle = tester.ensureSemantics();
      await videoWithToolbar(tester, locale: 'de');
      for (final tip in [
        'Stift',
        'Textmarker',
        'Radierer',
        'Laserpointer',
        'Rückgängig',
        'Alle Markierungen löschen',
        'Bild einfrieren (der Live-Stream läuft weiter)',
        'Werkzeugleiste einklappen',
        'Rot',
        'Dünn',
      ]) {
        expect(find.byTooltip(tip), findsOneWidget, reason: tip);
      }
      expect(
        find.bySemanticsLabel('Markierungen (nur auf diesem Gerät sichtbar)'),
        findsOneWidget,
      );
      expect(find.bySemanticsLabel('Bildschirm-Werkzeuge'), findsOneWidget);
      handle.dispose();
    });
  });
}
