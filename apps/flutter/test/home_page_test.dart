// SPDX-License-Identifier: GPL-3.0-or-later
// Widget test: starts the app in English and German and checks the texts.
// Run with `flutter test` (after `flutter gen-l10n`).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/main.dart';

import 'fake_core.dart';

void main() {
  testWidgets('shows English texts', (tester) async {
    await tester.pumpWidget(
      CometCastApp(locale: const Locale('en'), core: FakeCoreApi()),
    );
    await tester.pumpAndSettle();
    expect(find.text('Comet Cast'), findsOneWidget);
    expect(find.text('Send screen'), findsOneWidget);
    expect(find.text('Preview build (v0.1)'), findsOneWidget);
  });

  testWidgets('shows German texts', (tester) async {
    await tester.pumpWidget(
      CometCastApp(locale: const Locale('de'), core: FakeCoreApi()),
    );
    await tester.pumpAndSettle();
    expect(find.text('Comet Cast'), findsOneWidget);
    expect(find.text('Bildschirm senden'), findsOneWidget);
    expect(find.text('Vorschau-Version (v0.1)'), findsOneWidget);
  });

  testWidgets('shows the app icon', (tester) async {
    await tester.pumpWidget(
      CometCastApp(locale: const Locale('en'), core: FakeCoreApi()),
    );
    await tester.pumpAndSettle();
    expect(find.byType(Image), findsOneWidget);
  });
}
