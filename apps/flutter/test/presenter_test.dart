// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter pages of docs/design/first-run-and-display.md sections 5 and 6, with a fake core.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/display/display_logic.dart';

import 'display_support.dart';
import 'fake_core.dart';

class PresenterHarness {
  PresenterHarness(this.core);
  final FakeCoreApi core;
}

Future<PresenterHarness> pumpPresenter(
  WidgetTester t, {
  Size size = const Size(400, 800),
  Locale locale = const Locale('en'),
  String? code,
  Brightness brightness = Brightness.light,
  bool capable = false,
}) async {
  await t.runAsync(loadAppFonts);
  final core = FakeCoreApi();
  t.view.physicalSize = size;
  t.view.devicePixelRatio = 1;
  t.platformDispatcher.platformBrightnessTestValue = brightness;
  addTearDown(t.view.reset);
  addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
  await t.pumpWidget(
    CometCastApp(
      locale: locale,
      core: core,
      settings: AppSettings.fromDefaults(
        core.serverDefaults(),
        serverAddress: 'cast.example.org',
      ),
      displayStore: MemoryDisplayStore(),
      forceDisplayCapable: capable,
      initialCode: code,
      pickFile: (_) async => '/tmp/slides.pdf',
      openUrl: (_) async => true,
    ),
  );
  for (var i = 0; i < 4; i++) {
    await t.pump(const Duration(milliseconds: 50));
  }
  return PresenterHarness(core);
}

Future<void> settle(WidgetTester t) async {
  for (var i = 0; i < 3; i++) {
    await t.pump(const Duration(milliseconds: 50));
  }
}

Future<void> connectWith(WidgetTester t, String code) async {
  await t.enterText(find.byKey(const Key('code-field')), code);
  await t.pump();
  await t.tap(find.text('Connect'));
  await settle(t);
}

void main() {
  group('first run (5.3)', () {
    testWidgets('code field, name field, Connect goes to What to show', (
      t,
    ) async {
      await pumpPresenter(t);
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
      expect(find.text('Your name'), findsOneWidget);
      await connectWith(t, 'k7q m2w');
      expect(find.text('What would you like to show?'), findsOneWidget);
      expect(find.text('Share screen'), findsOneWidget);
      expect(find.text('Whole screen or one window, live'), findsOneWidget);
    });

    testWidgets(
      'an empty code is an inline error, a wrong code is the error page',
      (t) async {
        await pumpPresenter(t);
        await t.tap(find.text('Connect'));
        await settle(t);
        expect(
          find.text('Please enter the code shown large on the screen.'),
          findsOneWidget,
        );
        await connectWith(t, 'OOO 000');
        expect(find.text('This code does not exist right now'), findsOneWidget);
        expect(find.text('Retype code'), findsOneWidget);
        await t.tap(find.text('Retype code'));
        await settle(t);
        expect(find.text('Welcome to Comet Cast'), findsOneWidget);
      },
    );

    testWidgets('a code from a link is a chip', (t) async {
      await pumpPresenter(t, code: 'K7QM2W');
      expect(find.text('K7Q M2W'), findsOneWidget);
      expect(find.text('taken from your link'), findsOneWidget);
      expect(find.byKey(const Key('code-field')), findsNothing);
      await t.tap(find.text('Connect'));
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });

    testWidgets('tablets and computers end the page with the display link', (
      t,
    ) async {
      await pumpPresenter(t, size: const Size(1024, 768), capable: true);
      expect(find.text('Use this device as the screen'), findsOneWidget);
    });

    testWidgets('a phone has no display link', (t) async {
      await pumpPresenter(t);
      expect(find.text('Use this device as the screen'), findsNothing);
    });
  });

  group('showing a link (5.5, 5.6)', () {
    testWidgets('link page, waiting with the check number, presenting, stop', (
      t,
    ) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      await t.tap(find.text('Show a link'));
      await settle(t);
      await t.enterText(
        find.byKey(const Key('link-url')),
        'https://example.org/a',
      );
      await t.tap(find.text('Open on the screen'));
      await settle(t);
      expect(h.core.contentCode, 'K7QM2W');
      expect(h.core.contentServer, 'cast.example.org');
      h.core.contentSenderEvents.add(
        const ContentSenderUiEvent.waitingForApproval(
          timeoutSecs: 60,
          check: 42,
        ),
      );
      await settle(t);
      expect(find.text('Waiting for approval'), findsOneWidget);
      expect(find.text('42'), findsOneWidget);
      expect(find.text('Withdraw request'), findsOneWidget);
      h.core.contentSenderEvents
        ..add(const ContentSenderUiEvent.joined())
        ..add(const ContentSenderUiEvent.connected())
        ..add(const ContentSenderUiEvent.sent());
      await settle(t);
      expect(find.text('The screen shows your link'), findsOneWidget);
      await t.tap(find.text('Stop showing'));
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });

    testWidgets('a bad link is an inline error', (t) async {
      await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      await t.tap(find.text('Show a link'));
      await settle(t);
      await t.enterText(find.byKey(const Key('link-url')), 'nonsense');
      await t.tap(find.text('Open on the screen'));
      await settle(t);
      expect(find.byKey(const Key('link-url')), findsOneWidget);
      expect(
        find.text('Please enter a link that starts with http:// or https://.'),
        findsOneWidget,
      );
    });

    testWidgets('withdrawing goes back to the first page', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      await t.tap(find.text('Show a link'));
      await settle(t);
      await t.enterText(
        find.byKey(const Key('link-url')),
        'https://example.org/a',
      );
      await t.tap(find.text('Open on the screen'));
      await settle(t);
      h.core.contentSenderEvents.add(
        const ContentSenderUiEvent.waitingForApproval(
          timeoutSecs: 60,
          check: 7,
        ),
      );
      await settle(t);
      await t.tap(find.text('Withdraw request'));
      await settle(t);
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
    });
  });

  group('showing a file (5.5, 5.6)', () {
    testWidgets('a PDF has page buttons above Stop', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      await t.tap(find.text('Show a photo or file'));
      await settle(t);
      expect(find.text('Choose file'), findsOneWidget);
      await t.tap(find.text('Choose file'));
      await settle(t);
      expect(h.core.contentSource, isA<ContentSource_File>());
      h.core.contentSenderEvents
        ..add(
          ContentSenderUiEvent.prepared(
            kind: UiContentKind.pdf,
            name: 'slides.pdf',
            size: BigInt.from(100),
            pages: 3,
          ),
        )
        ..add(const ContentSenderUiEvent.joined())
        ..add(const ContentSenderUiEvent.connected())
        ..add(const ContentSenderUiEvent.sent());
      await settle(t);
      expect(find.text('You are showing a file'), findsOneWidget);
      expect(find.text('Previous page'), findsOneWidget);
      await t.tap(find.text('Next page'));
      await settle(t);
      expect(h.core.contentCommands, [const UiContentCmd.nextPage()]);
    });
  });

  group('errors (6)', () {
    Future<PresenterHarness> toLinkSent(WidgetTester t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      await t.tap(find.text('Show a link'));
      await settle(t);
      await t.enterText(
        find.byKey(const Key('link-url')),
        'https://example.org/a',
      );
      await t.tap(find.text('Open on the screen'));
      await settle(t);
      return h;
    }

    testWidgets('denied: Ask again counts down 15 s', (t) async {
      final h = await toLinkSent(t);
      h.core.contentSenderEvents
        ..add(
          const ContentSenderUiEvent.waitingForApproval(
            timeoutSecs: 60,
            check: 7,
          ),
        )
        ..add(const ContentSenderUiEvent.joinRefused(code: 'join_denied'));
      await settle(t);
      expect(find.text('Not allowed'), findsOneWidget);
      expect(find.textContaining('Ask again in'), findsOneWidget);
    });

    testWidgets('locked while waiting uses the second why text', (t) async {
      final h = await toLinkSent(t);
      h.core.contentSenderEvents
        ..add(
          const ContentSenderUiEvent.waitingForApproval(
            timeoutSecs: 60,
            check: 7,
          ),
        )
        ..add(const ContentSenderUiEvent.joinRefused(code: 'room_locked'));
      await settle(t);
      expect(find.text('Locked right now'), findsOneWidget);
      expect(
        find.text(
          'The host locked the room while you were waiting. Your request was dropped.',
        ),
        findsOneWidget,
      );
    });

    const cases = {
      'server error rate_limited: slow down': 'The server is busy',
      'server error room_full: x': 'Someone else is connected',
      'server error proto_mismatch: x': 'This page is out of date',
      'server error approval_timeout: x': 'No answer',
      'server error removed_by_host: x': 'Presentation ended',
      'server error room_not_found: x': 'This code does not exist right now',
    };
    for (final e in cases.entries) {
      testWidgets('${e.key} is the page "${e.value}"', (t) async {
        final h = await toLinkSent(t);
        h.core.contentSenderEvents.add(
          ContentSenderUiEvent.error(message: e.key),
        );
        await settle(t);
        expect(find.text(e.value), findsOneWidget);
      });
    }

    testWidgets(
      'an unknown server code is the other page with the existing text',
      (t) async {
        final h = await toLinkSent(t);
        h.core.contentSenderEvents.add(
          const ContentSenderUiEvent.error(
            message: 'server error forbidden: no',
          ),
        );
        await settle(t);
        expect(find.text('Something went wrong'), findsOneWidget);
        expect(
          find.text('Try again. If it happens again, tell the host.'),
          findsOneWidget,
        );
      },
    );

    testWidgets('no network is the no internet page', (t) async {
      final h = await toLinkSent(t);
      h.core.contentSenderEvents.add(
        const ContentSenderUiEvent.error(
          message: 'signalling connection failed: dns',
        ),
      );
      await settle(t);
      expect(find.text('No internet on this device'), findsOneWidget);
    });

    testWidgets('the display ended the session', (t) async {
      final h = await toLinkSent(t);
      h.core.contentSenderEvents
        ..add(const ContentSenderUiEvent.joined())
        ..add(const ContentSenderUiEvent.connected())
        ..add(const ContentSenderUiEvent.sent())
        ..add(const ContentSenderUiEvent.peerLeft());
      await settle(t);
      expect(find.text('Presentation ended'), findsOneWidget);
    });
  });

  group('disconnected (5.7)', () {
    testWidgets('no body line in the app', (t) async {
      await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      await t.tap(find.text('Disconnect'));
      await settle(t);
      expect(find.text('Disconnected'), findsOneWidget);
      expect(find.textContaining('close this tab'), findsNothing);
      await t.tap(find.text('Connect again'));
      await settle(t);
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
    });
  });

  group('German', () {
    testWidgets('the shipped German is du', (t) async {
      await pumpPresenter(t, locale: const Locale('de'));
      expect(find.text('Dein Name'), findsOneWidget);
    });
  });
}
