// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests for the send screen with a fake core.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/main.dart';

import 'fake_core.dart';

Future<FakeCoreApi> openSend(WidgetTester tester,
    {String locale = 'en'}) async {
  final core = FakeCoreApi();
  // Tall window so the whole (lazily built) page is on screen.
  tester.view.physicalSize = const Size(900, 1800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(CometCastApp(locale: Locale(locale), core: core));
  await tester.pumpAndSettle();
  await tester
      .tap(find.text(locale == 'en' ? 'Send screen' : 'Bildschirm senden'));
  await tester.pumpAndSettle();
  return core;
}

void main() {
  testWidgets('shows the no-capture notice', (tester) async {
    await openSend(tester);
    expect(find.byKey(const Key('sendNotice')), findsOneWidget);
    expect(find.textContaining('not your screen'), findsOneWidget);
  });

  testWidgets('rejects a code with a character outside the alphabet',
      (tester) async {
    final core = await openSend(tester);
    await tester.enterText(find.byKey(const Key('codeField')), 'k7m2qo');
    await tester.pump();
    // Typed lowercase is shown as capitals; 'O' is not allowed.
    expect(find.text('K7M2QO'), findsOneWidget);
    expect(find.textContaining('"O" is not allowed'), findsOneWidget);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(core.senderCode, isNull);
  });

  testWidgets('too short code is reported on start', (tester) async {
    final core = await openSend(tester);
    await tester.enterText(find.byKey(const Key('codeField')), 'K7M');
    await tester.pump();
    expect(find.textContaining('exactly 6 characters'), findsNothing);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(find.textContaining('exactly 6 characters'), findsOneWidget);
    expect(core.senderCode, isNull);
  });

  testWidgets('valid code starts the sender and shows progress',
      (tester) async {
    final core = await openSend(tester);
    await tester.enterText(find.byKey(const Key('codeField')), ' k7m2qx ');
    await tester.enterText(find.byKey(const Key('secondsField')), '12');
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(core.senderCode, 'K7M2QX');
    expect(core.senderServer, '127.0.0.1:8443');
    expect(core.senderSeconds, 12);

    core.senderEvents.add(const SenderUiEvent.joined());
    await tester.pump();
    expect(find.text('Joined the room. The connection is being set up …'),
        findsOneWidget);
    core.senderEvents.add(SenderUiEvent.streaming(framesSent: BigInt.from(30)));
    await tester.pump();
    expect(find.text('Sending the test video: 30 frames sent'), findsOneWidget);
    core.senderEvents.add(SenderUiEvent.finished(framesSent: BigInt.from(180)));
    await tester.pump();
    expect(find.text('Finished: 180 frames sent'), findsOneWidget);
  });

  testWidgets('invalid duration is rejected', (tester) async {
    final core = await openSend(tester);
    await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
    await tester.enterText(find.byKey(const Key('secondsField')), '0');
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(find.text('Enter a whole number from 1 to 3600.'), findsOneWidget);
    expect(core.senderCode, isNull);
  });

  testWidgets('an unknown room error from the core is shown', (tester) async {
    final core = await openSend(tester);
    await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    core.senderEvents.addError('server error room_not_found: no such room');
    await tester.pump();
    expect(find.textContaining('room_not_found'), findsOneWidget);
  });

  testWidgets('German texts', (tester) async {
    await openSend(tester, locale: 'de');
    expect(find.text('Raumcode des Empfängers'), findsOneWidget);
    expect(find.text('Senden starten'), findsOneWidget);
    expect(find.textContaining('nicht Ihren Bildschirm'), findsOneWidget);
    await tester.enterText(find.byKey(const Key('codeField')), 'AB0');
    await tester.pump();
    expect(find.textContaining('„0“ ist in einem Raumcode nicht erlaubt'),
        findsOneWidget);
  });
}
