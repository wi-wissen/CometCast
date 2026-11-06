// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests for the receive screen with a fake core.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/main.dart';

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
  testWidgets('server field has the default and the notice is shown', (
    tester,
  ) async {
    await openReceive(tester);
    final field = tester.widget<TextField>(
      find.byKey(const Key('serverField')),
    );
    expect(field.controller!.text, '127.0.0.1:8443');
    expect(find.byKey(const Key('receiveNotice')), findsOneWidget);
    expect(find.text('Not started.'), findsOneWidget);
  });

  testWidgets('shows room code, then live stats', (tester) async {
    final core = await openReceive(tester);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(core.receiverServer, '127.0.0.1:8443');
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
    expect(
      find.textContaining('zählt das eingehende Video nur'),
      findsOneWidget,
    );
  });
}
