// SPDX-License-Identifier: GPL-3.0-or-later
// Pause from either side (docs/design/pause.md) on the native screens: the display's own Pause
// control, the badge that says who paused, the frozen picture; the presenter's Pause / Resume
// button and its banner. A fake core stands in for the Rust side.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/src/design/icons.dart';
import 'package:cometcast/src/presenter/kit.dart';
import 'package:cometcast/src/presenter/pages.dart';
import 'package:cometcast/presenter_session_controller.dart' show PauseBanner;

import 'display_support.dart';
import 'presenter_test.dart' show pumpRoot, settle, toWhat;
import 'solid_frame.dart';

Future<void> key(WidgetTester t, LogicalKeyboardKey k) async {
  await t.sendKeyEvent(k);
  await t.pump(const Duration(milliseconds: 50));
}

Future<void> two(WidgetTester t) async {
  await t.pump();
  await t.pump(const Duration(milliseconds: 50));
}

String badge(WidgetTester t) {
  final box = find.byKey(const Key('frozen-badge'));
  return (find
              .descendant(of: box, matching: find.byType(Text))
              .evaluate()
              .single
              .widget
          as Text)
      .data!;
}

double badgeOpacity(WidgetTester t) => t
    .widget<AnimatedOpacity>(
      find.ancestor(
        of: find.byKey(const Key('frozen-badge')),
        matching: find.byType(AnimatedOpacity),
      ),
    )
    .opacity;

int shownWidth(WidgetTester t) =>
    (find.byType(RawImage).evaluate().first.widget as RawImage).image!.width;

/// A display with a live picture on show.
Future<DisplayHarness> live(WidgetTester t) async {
  final h = await pumpDisplay(t);
  h.core.receiverEvents.add(
    const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
  );
  await t.pump();
  await deliverFrame(t, h.core, solidFrame(16, 9, [255, 0, 0]));
  return h;
}

void main() {
  group('the display', () {
    testWidgets(
      'no badge while the picture is live; the Pause control is offered',
      (t) async {
        await live(t);
        expect(find.byKey(const Key('frozen-badge')), findsNothing);
        await key(t, LogicalKeyboardKey.select); // reveals the controls
        expect(find.byKey(const Key('pause')), findsOneWidget);
        expect(find.text('Pause'), findsOneWidget);
      },
    );

    testWidgets('pressing Pause sets the display flag; Resume clears it', (
      t,
    ) async {
      final h = await live(t);
      await key(t, LogicalKeyboardKey.select);
      await t.tap(find.byKey(const Key('pause')));
      await two(t);
      expect(h.core.displayPauseRequests, [true]);
      expect(find.text('Resume'), findsOneWidget);
      expect(
        badge(t),
        'Frozen. The live stream keeps running in the background.',
      );
      await t.tap(find.byKey(const Key('pause')));
      await two(t);
      expect(h.core.displayPauseRequests, [true, false]);
      expect(find.byKey(const Key('frozen-badge')), findsNothing);
      expect(find.text('Pause'), findsOneWidget);
    });

    testWidgets(
      'the badge is a state: it shows at rest and steps aside while the controls are shown',
      (t) async {
        final h = await live(t);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.pauseChanged(presenter: true, display: false),
        );
        await two(t);
        expect(badgeOpacity(t), 1);
        await key(t, LogicalKeyboardKey.select);
        await t.pump(const Duration(milliseconds: 500));
        expect(badgeOpacity(t), 0);
        await t.pump(const Duration(seconds: 5, milliseconds: 100));
        await t.pump(const Duration(milliseconds: 500));
        expect(badgeOpacity(t), 1);
      },
    );

    testWidgets(
      'the presenter paused: its text; both: the presenter text; the presenter resumes: the display text',
      (t) async {
        final h = await live(t);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.pauseChanged(presenter: true, display: false),
        );
        await two(t);
        expect(badge(t), 'Paused by the presenter');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.pauseChanged(presenter: true, display: true),
        );
        await two(t);
        expect(badge(t), 'Paused by the presenter');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.pauseChanged(presenter: false, display: true),
        );
        await two(t);
        expect(
          badge(t),
          'Frozen. The live stream keeps running in the background.',
        );
        h.core.receiverEvents.add(
          const ReceiverUiEvent.pauseChanged(presenter: false, display: false),
        );
        await two(t);
        expect(find.byKey(const Key('frozen-badge')), findsNothing);
      },
    );

    testWidgets(
      'both flags: the display clearing its own keeps the picture frozen',
      (t) async {
        final h = await live(t);
        // The fake core echoes with the presenter's flag it is told about, like the real one.
        h.core
          ..presenterPaused = true
          ..displayPaused = true;
        h.core.receiverEvents.add(
          const ReceiverUiEvent.pauseChanged(presenter: true, display: true),
        );
        await two(t);
        await key(t, LogicalKeyboardKey.select);
        // The control shows Resume (the display's own flag is set); pressing it clears only that.
        expect(find.text('Resume'), findsOneWidget);
        await t.tap(find.byKey(const Key('pause')));
        await two(t);
        expect(h.core.displayPauseRequests, [false]);
        expect(badge(t), 'Paused by the presenter');
      },
    );

    testWidgets(
      'a frozen picture stays; the newest one that came meanwhile shows when it is live again',
      (t) async {
        final h = await live(t);
        expect(shownWidth(t), 16);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.pauseChanged(presenter: false, display: true),
        );
        await two(t);
        h.core.frames.add(solidFrame(32, 18, [0, 255, 0]));
        h.core.receiverEvents.add(const ReceiverUiEvent.frameReady());
        for (var i = 0; i < 5; i++) {
          await t.runAsync(
            () => Future<void>.delayed(const Duration(milliseconds: 50)),
          );
          await t.pump();
        }
        expect(shownWidth(t), 16, reason: 'frozen: the last picture stays');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.pauseChanged(presenter: false, display: false),
        );
        for (var i = 0; i < 20 && shownWidth(t) != 32; i++) {
          await t.runAsync(
            () => Future<void>.delayed(const Duration(milliseconds: 50)),
          );
          await t.pump();
        }
        expect(shownWidth(t), 32, reason: 'live again: the held picture shows');
      },
    );

    testWidgets('D-pad: Left from End reaches Pause, select presses it', (
      t,
    ) async {
      final h = await live(t);
      await key(t, LogicalKeyboardKey.arrowDown); // reveals, focuses End
      await t.pump(const Duration(milliseconds: 100));
      await key(t, LogicalKeyboardKey.arrowLeft);
      await key(t, LogicalKeyboardKey.select);
      expect(h.core.displayPauseRequests, [true]);
      expect(h.core.endRoomCalls, 0);
    });

    testWidgets(
      'the Play/Pause key of a remote toggles the display flag without revealing the controls',
      (t) async {
        final h = await live(t);
        await key(t, LogicalKeyboardKey.mediaPlayPause);
        await two(t);
        expect(h.core.displayPauseRequests, [true]);
        expect(
          badge(t),
          'Frozen. The live stream keeps running in the background.',
        );
        await key(t, LogicalKeyboardKey.mediaPlayPause);
        await two(t);
        expect(h.core.displayPauseRequests, [true, false]);
      },
    );

    testWidgets('the end of the presentation clears the flags', (t) async {
      final h = await live(t);
      h.core.receiverEvents
        ..add(
          const ReceiverUiEvent.pauseChanged(presenter: true, display: true),
        )
        ..add(const ReceiverUiEvent.presentationStopped());
      await two(t);
      expect(find.byKey(const Key('frozen-badge')), findsNothing);
    });

    testWidgets(
      'no Pause control for a link card (only the live screen can be paused)',
      (t) async {
        final h = await live(t);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.contentLink(
            url: 'https://example.org/x',
            host: 'example.org',
          ),
        );
        await two(t);
        await key(t, LogicalKeyboardKey.select);
        expect(find.byKey(const Key('pause')), findsNothing);
      },
    );

    testWidgets('German text of the badge', (t) async {
      final h = await pumpDisplay(t, locale: const Locale('de'));
      h.core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
      );
      await t.pump();
      await deliverFrame(t, h.core, solidFrame(16, 9, [255, 0, 0]));
      h.core.receiverEvents.add(
        const ReceiverUiEvent.pauseChanged(presenter: true, display: false),
      );
      await two(t);
      expect(badge(t), 'Vom Vortragenden pausiert');
    });
  });

  group('the presenter page', () {
    Future<void> pumpPage(
      WidgetTester t, {
      void Function(bool)? onPause,
      bool paused = false,
      PauseBanner? banner,
      PresentingKind kind = PresentingKind.screen,
      Locale locale = const Locale('en'),
    }) async {
      await t.runAsync(loadAppFonts);
      t.view.physicalSize = const Size(400, 800);
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.reset);
      await t.pumpWidget(
        MaterialApp(
          locale: locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: PresenterFrame(
            child: Material(
              type: MaterialType.transparency,
              child: PresentingPage(
                kind: kind,
                onStop: () {},
                onPause: onPause,
                paused: paused,
                banner: banner,
              ),
            ),
          ),
        ),
      );
      await t.pump();
    }

    testWidgets('a Pause button; pressing it asks to pause', (t) async {
      bool? asked;
      await pumpPage(t, onPause: (p) => asked = p);
      expect(find.text('Pause'), findsOneWidget);
      expect(find.byKey(const Key('pause-banner')), findsNothing);
      await t.tap(find.byKey(const Key('pause-button')));
      expect(asked, true);
    });

    testWidgets('paused: the button reads Resume and asks to resume', (
      t,
    ) async {
      bool? asked;
      await pumpPage(t, onPause: (p) => asked = p, paused: true);
      expect(find.text('Resume'), findsOneWidget);
      await t.tap(find.byKey(const Key('pause-button')));
      expect(asked, false);
    });

    testWidgets('the banners of both cases', (t) async {
      await pumpPage(
        t,
        onPause: (_) {},
        paused: true,
        banner: PauseBanner.presenter,
      );
      expect(
        find.text('Paused: the audience sees the last picture.'),
        findsOneWidget,
      );
      await pumpPage(t, onPause: (_) {}, banner: PauseBanner.display);
      expect(find.text('The display has frozen the picture.'), findsOneWidget);
    });

    testWidgets('files, links and TVs have no pause control', (t) async {
      await pumpPage(t, kind: PresentingKind.file);
      expect(find.byKey(const Key('pause-button')), findsNothing);
    });

    testWidgets('German', (t) async {
      await pumpPage(
        t,
        onPause: (_) {},
        paused: true,
        banner: PauseBanner.presenter,
        locale: const Locale('de'),
      );
      expect(find.text('Fortsetzen'), findsOneWidget);
      expect(
        find.text('Pausiert: Das Publikum sieht das letzte Bild.'),
        findsOneWidget,
      );
    });
  });

  group('the presenter session', () {
    testWidgets('Pause stops at once, the echo confirms it, Resume clears it', (
      t,
    ) async {
      final h = await pumpRoot(t, code: 'K7QM2W', desktop: true);
      await toWhat(t, h);
      await t.tap(find.byKey(const Key('what-screen')));
      await settle(t);
      h.core.presenterEvents.add(const PresenterUiEvent.presenting());
      await settle(t);
      expect(find.byKey(const Key('pause-button')), findsOneWidget);
      // Before the display answers: the button already reads Resume, no banner yet (pending).
      h.core.presenterPauseEcho = null;
      await t.tap(find.byKey(const Key('pause-button')));
      await settle(t);
      expect(h.core.presenterPauseRequests, [true]);
      expect(find.text('Resume'), findsOneWidget);
      expect(find.byKey(const Key('pause-banner')), findsNothing);
      // The display's echo confirms it.
      h.core.presenterEvents.add(
        const PresenterUiEvent.pauseState(presenter: true, display: false),
      );
      await settle(t);
      expect(
        find.text('Paused: the audience sees the last picture.'),
        findsOneWidget,
      );
      await t.tap(find.byKey(const Key('pause-button')));
      await settle(t);
      expect(h.core.presenterPauseRequests, [true, false]);
      expect(find.byKey(const Key('pause-banner')), findsNothing);
      expect(find.text('Pause'), findsOneWidget);
    });

    testWidgets(
      'only the display froze: its banner, the button stays Pause; both: the presenter text',
      (t) async {
        final h = await pumpRoot(t, code: 'K7QM2W', desktop: true);
        await toWhat(t, h);
        await t.tap(find.byKey(const Key('what-screen')));
        await settle(t);
        h.core.presenterEvents
          ..add(const PresenterUiEvent.presenting())
          ..add(
            const PresenterUiEvent.pauseState(presenter: false, display: true),
          );
        await settle(t);
        expect(
          find.text('The display has frozen the picture.'),
          findsOneWidget,
        );
        expect(find.text('Pause'), findsOneWidget);
        await t.tap(find.byKey(const Key('pause-button')));
        await settle(t);
        expect(
          find.text('Paused: the audience sees the last picture.'),
          findsOneWidget,
        );
      },
    );

    testWidgets(
      'a link has no pause button, and stopping clears the flags for the next screen',
      (t) async {
        final h = await pumpRoot(t, code: 'K7QM2W', desktop: true);
        await toWhat(t, h);
        await t.tap(find.byKey(const Key('what-screen')));
        await settle(t);
        h.core.presenterEvents.add(const PresenterUiEvent.presenting());
        await settle(t);
        await t.tap(find.byKey(const Key('pause-button')));
        await settle(t);
        h.core.presenterEvents.add(const PresenterUiEvent.presentingStopped());
        await settle(t);
        await t.tap(find.byKey(const Key('what-screen')));
        await settle(t);
        h.core.presenterEvents.add(const PresenterUiEvent.presenting());
        await settle(t);
        expect(
          find.text('Pause'),
          findsOneWidget,
          reason: 'the new screen starts live',
        );
        expect(find.byKey(const Key('pause-banner')), findsNothing);
      },
    );
  });

  test('the pause icons exist', () {
    expect(CcIconName.values, containsAll([CcIconName.pause, CcIconName.play]));
  });
}
