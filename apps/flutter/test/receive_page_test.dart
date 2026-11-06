// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests for the receive screen with a fake core.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

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

    core.receiverEvents.add(const ReceiverUiEvent.roomCode(code: 'K7M2QX'));
    await tester.pump();
    expect(find.text('K7M2QX'), findsOneWidget);
    expect(find.textContaining('Waiting for a sender'), findsOneWidget);

    core.receiverEvents.add(const ReceiverUiEvent.peerJoined());
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
    core.receiverEvents.add(const ReceiverUiEvent.roomCode(code: 'AB23CD'));
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
    core.receiverEvents.add(const ReceiverUiEvent.roomCode(code: 'AB23CD'));
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
  core.receiverEvents.add(const ReceiverUiEvent.roomCode(code: 'K7M2QX'));
  core.receiverEvents.add(const ReceiverUiEvent.peerJoined());
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
    core.receiverEvents.add(const ReceiverUiEvent.roomCode(code: 'K7M2QX'));
    core.receiverEvents.add(const ReceiverUiEvent.peerJoined());
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
      expect(fresh.serverSuggestions, [
        'relay.cometcast.de:443',
        '127.0.0.1:8443',
      ]);
    },
  );
}
