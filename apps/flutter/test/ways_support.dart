// SPDX-License-Identifier: GPL-3.0-or-later
// Helpers of the display tests of round 5 (AirPlay, Miracast, About) and of their screenshots.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/window_control.dart' show RaiseWindow;

import 'display_support.dart';
import 'fake_core.dart';
import 'fake_miracast.dart';
import 'round5_support.dart' show fakeLicences;

Future<void> pump(WidgetTester t, [int ms = 50]) =>
    t.pump(Duration(milliseconds: ms));

Future<DisplayHarness> ways(
  WidgetTester t, {
  bool airplay = true,
  FakeMiracastCore? miracast,
  Appearance appearance = Appearance.dark,
  Locale locale = const Locale('en'),
  bool tv = false,
  String server = 'cast.example.org',
  RaiseWindow? raise,
}) async {
  final core = FakeCoreApi()..airplayAvailable = airplay;
  final h = await pumpDisplay(
    t,
    tv: tv,
    core: core,
    miracast: miracast,
    offerWays: true,
    appearance: appearance,
    locale: locale,
    server: server,
    licenceLoader: () async => fakeLicences(),
    raise: raise,
  );
  await pump(t, 100);
  return h;
}

Future<void> openSettings(WidgetTester t) async {
  await t.tap(find.byKey(const Key('settings-open')));
  await pump(t);
}

Future<void> toggle(WidgetTester t, String way) async {
  await t.tap(find.byKey(Key('settings-way-$way')));
  await pump(t, 100);
}

String? focusLabel() => FocusManager.instance.primaryFocus?.debugLabel;

Future<void> key(WidgetTester t, LogicalKeyboardKey k) async {
  await t.sendKeyEvent(k);
  await pump(t);
}

Future<void> rxEvent(WidgetTester t, DisplayHarness h, AirplayUiEvent e) async {
  h.core.airplayControllers.last.add(e);
  await pump(t);
}
