// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests: the screens can be operated with arrow keys and select only (TV remote / D-pad).
// This simulates key events; it does NOT prove anything about a real Android TV remote.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/main.dart';

import 'fake_core.dart';

/// The title of the home-list tile that currently has the focus (or null).
String? focusedTileTitle() {
  final ctx = FocusManager.instance.primaryFocus?.context;
  final tile = ctx?.findAncestorWidgetOfExactType<ListTile>();
  final title = tile?.title;
  return title is Text ? title.data : null;
}

/// Whether the focused widget is inside the button with [key].
bool focusIsInside(WidgetTester tester, Key key) {
  final ctx = FocusManager.instance.primaryFocus?.context;
  if (ctx == null) return false;
  var inside = false;
  ctx.visitAncestorElements((e) {
    if (e.widget.key == key) {
      inside = true;
      return false;
    }
    return true;
  });
  return inside;
}

Future<void> press(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyEvent(key);
  await tester.pump();
}

void main() {
  testWidgets('home: arrow keys move the focus, select opens the screen', (
    tester,
  ) async {
    await tester.pumpWidget(
      CometCastApp(locale: const Locale('en'), core: FakeCoreApi()),
    );
    await tester.pumpAndSettle();

    // The first row has the focus at start, so a remote has something to act on.
    expect(focusedTileTitle(), 'Send screen');

    await press(tester, LogicalKeyboardKey.arrowDown);
    expect(focusedTileTitle(), 'Send file or link');
    await press(tester, LogicalKeyboardKey.arrowDown);
    expect(focusedTileTitle(), 'Receive on this device');
    await press(tester, LogicalKeyboardKey.arrowUp);
    expect(focusedTileTitle(), 'Send file or link');
    await press(tester, LogicalKeyboardKey.arrowDown);

    // Select (the centre button of a TV remote) activates the focused row.
    await press(tester, LogicalKeyboardKey.select);
    await tester.pumpAndSettle();
    expect(find.text('Receive on this device'), findsWidgets);
    expect(find.byKey(const Key('startButton')), findsOneWidget);
  });

  testWidgets('home: the focused row shows a visible frame', (tester) async {
    await tester.pumpWidget(
      CometCastApp(locale: const Locale('en'), core: FakeCoreApi()),
    );
    await tester.pumpAndSettle();
    final frames = tester
        .widgetList<DecoratedBox>(find.byKey(const Key('focusRing')))
        .map((b) => (b.decoration as BoxDecoration).border! as Border)
        .map((b) => b.top.color)
        .toList();
    expect(frames, hasLength(3));
    // Exactly one of the three frames is drawn (opaque), the others are transparent.
    expect(frames.where((c) => c.a > 0).length, 1);
    expect(frames.first.a, greaterThan(0));
  });

  testWidgets('receive screen: arrow down goes to the button, select starts', (
    tester,
  ) async {
    final core = FakeCoreApi();
    await tester.pumpWidget(
      CometCastApp(locale: const Locale('en'), core: core),
    );
    await tester.pumpAndSettle();
    await press(tester, LogicalKeyboardKey.arrowDown);
    await press(tester, LogicalKeyboardKey.arrowDown);
    await press(tester, LogicalKeyboardKey.select);
    await tester.pumpAndSettle();

    // The server field has the focus (autofocus); arrow down leaves it for the start button.
    expect(focusIsInside(tester, const Key('serverField')), isTrue);
    // The two suggested servers (chips) come before the start button.
    for (final key in const [
      Key('serverSuggestion:relay.cometcast.de:443'),
      Key('serverSuggestion:127.0.0.1:8443'),
      Key('startButton'),
    ]) {
      await press(tester, LogicalKeyboardKey.arrowDown);
      expect(focusIsInside(tester, key), isTrue, reason: '$key should be next');
    }
    for (final key in const [
      Key('serverSuggestion:127.0.0.1:8443'),
      Key('serverSuggestion:relay.cometcast.de:443'),
      Key('serverField'),
    ]) {
      await press(tester, LogicalKeyboardKey.arrowUp);
      expect(focusIsInside(tester, key), isTrue, reason: '$key should be next');
    }
  });

  testWidgets('send screen: arrow keys walk through all fields to the button', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(900, 1800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      CometCastApp(locale: const Locale('en'), core: FakeCoreApi()),
    );
    await tester.pumpAndSettle();
    await press(
      tester,
      LogicalKeyboardKey.select,
    ); // "Send screen" has the focus at start
    await tester.pumpAndSettle();

    // Visit order from the top: server field, the two suggested servers (chips), the
    // "Share this screen" switch, code field, name field, duration field, start button.
    final order = <Key>[
      const Key('serverField'),
      const Key('serverSuggestion:relay.cometcast.de:443'),
      const Key('serverSuggestion:127.0.0.1:8443'),
      const Key('shareScreenSwitch'),
      const Key('codeField'),
      const Key('displayNameField'),
      const Key('secondsField'),
      const Key('startButton'),
    ];
    // The code field has autofocus; go up to the server field first.
    for (var i = 0; i < 4; i++) {
      await press(tester, LogicalKeyboardKey.arrowUp);
    }
    expect(focusIsInside(tester, order[0]), isTrue);
    for (final key in order.skip(1)) {
      await press(tester, LogicalKeyboardKey.arrowDown);
      expect(focusIsInside(tester, key), isTrue, reason: '$key should be next');
    }
  });
}
