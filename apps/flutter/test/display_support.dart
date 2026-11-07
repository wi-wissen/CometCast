// SPDX-License-Identifier: GPL-3.0-or-later
// Shared helpers of the display and presenter tests.

import 'package:flutter/services.dart';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/receiver_controller.dart' show UrlOpener;
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/display/display_logic.dart';
import 'package:cometcast/src/display/wake_lock.dart';

import 'fake_core.dart';

bool _fontsLoaded = false;

/// The tests draw with the bundled Atkinson fonts, not the test font, so that layout and
/// screenshots match the app (flutter test loads app fonts only on request).
Future<void> loadAppFonts() async {
  if (_fontsLoaded) return;
  _fontsLoaded = true;
  const families = {
    'AtkinsonHyperlegibleNext': ['400', '500', '700'],
    'AtkinsonHyperlegibleNextExt': ['400', '500', '700'],
    'AtkinsonHyperlegibleMono': ['500', '700'],
  };
  for (final e in families.entries) {
    final loader = FontLoader(e.key);
    for (final w in e.value) {
      loader.addFont(rootBundle.load('assets/fonts/${e.key}-$w.ttf'));
    }
    await loader.load();
  }
}

class DisplayHarness {
  DisplayHarness(this.core, this.store, this.wake, this.settings);
  final FakeCoreApi core;
  final MemoryDisplayStore store;
  final FakeWakeLock wake;
  final AppSettings settings;
}

/// The whole app as a display (a TV, or a tablet/computer whose setup is done).
Future<DisplayHarness> pumpDisplay(
  WidgetTester tester, {
  bool tv = true,
  bool setupDone = true,
  Locale locale = const Locale('en'),
  Size size = const Size(1920, 1080),
  Appearance appearance = Appearance.dark,
  DefaultMode mode = DefaultMode.open,
  FakeCoreApi? core,
  bool start = true,
  UrlOpener? openUrl,
  bool canOpen = false,
  AppSettings? settingsOverride,
}) async {
  await tester.runAsync(loadAppFonts);
  final c = core ?? FakeCoreApi();
  final store = MemoryDisplayStore(
    setup: setupDone
        ? DisplaySetup(
            room: 'Meeting room Elbe',
            mode: mode,
            server: 'cast.example.org',
          )
        : null,
    appearance: appearance,
  );
  final wake = FakeWakeLock();
  final settings =
      settingsOverride ?? AppSettings.fromDefaults(c.serverDefaults());
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    CometCastApp(
      locale: locale,
      core: c,
      settings: settings,
      tv: tv,
      displayStore: store,
      wakeLock: wake,
      forceDisplayCapable: true,
      openUrl: openUrl ?? (_) async => true,
      canOpenLinks: () async => canOpen,
    ),
  );
  for (var i = 0; i < 40; i++) {
    await tester.pump(const Duration(milliseconds: 25));
    if (c.receiverServer != null || !setupDone) break;
  }
  if (start && setupDone) {
    c.receiverEvents.add(
      ReceiverUiEvent.roomCode(
        code: 'K7QM2W',
        joinMode: mode == DefaultMode.open
            ? UiJoinMode.open
            : UiJoinMode.confirm,
      ),
    );
    await tester.pump();
  }
  return DisplayHarness(c, store, wake, settings);
}

/// Hands a picture to the receiver and waits until it is on screen.
Future<void> deliverFrame(
  WidgetTester tester,
  FakeCoreApi core,
  VideoFrame frame,
) async {
  core.frames.add(frame);
  core.receiverEvents.add(const ReceiverUiEvent.frameReady());
  for (var i = 0; i < 50; i++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 100)),
    );
    await tester.pump();
    final raws = find.byType(RawImage).evaluate();
    if (raws.isNotEmpty) {
      final img = (raws.first.widget as RawImage).image;
      if (img != null && img.width == frame.width) return;
    }
  }
}
