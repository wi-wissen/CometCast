// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter flow of docs/design/first-run-and-display.md sections 5 and 6 on the new pages and
// the presenter session (join once on Connect, Stop keeps the join, Disconnect leaves), with a
// fake core. The pages' own text and layout rules are in presenter_pages_test.dart.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/launch_intake.dart';
import 'package:cometcast/core/link_target.dart';
import 'package:cometcast/src/presenter/file_picker.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/codec_controller.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/display/display_logic.dart';
import 'package:cometcast/src/presenter/pages.dart' show PresenterDevice;
import 'package:cometcast/src/presenter/presenter_root.dart';

import 'display_support.dart';
import 'fake_core.dart';

class PresenterHarness {
  PresenterHarness(this.core, this.settings, this.store);
  final FakeCoreApi core;
  final AppSettings settings;
  final MemorySettingsStore store;
}

/// The presenter app on the default platform of the test host (a computer).
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
  final store = MemorySettingsStore();
  final settings = AppSettings.fromDefaults(
    core.serverDefaults(),
    serverAddress: 'cast.example.org',
    store: store,
  );
  t.view.physicalSize = size;
  t.view.devicePixelRatio = 1;
  t.platformDispatcher.platformBrightnessTestValue = brightness;
  addTearDown(t.view.reset);
  addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
  await t.pumpWidget(
    CometCastApp(
      locale: locale,
      core: core,
      settings: settings,
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
  return PresenterHarness(core, settings, store);
}

/// [PresenterRoot] alone, so that the device kind, the scanner and the codec are choosable.
Future<PresenterHarness> pumpRoot(
  WidgetTester t, {
  Size size = const Size(400, 800),
  Locale locale = const Locale('en'),
  String? code,
  bool desktop = false,
  PresenterDevice? device,
  ScanQr? scanQr,
  bool windows = false,
  CodecController? codec,
  CoreApi? coreOverride,
  FakeCoreApi? fake,
  Stream<String>? linkStream,
  bool firstLaunch = false,
  VoidCallback? onBeDisplay,
  PickFile? pickFile,
  LinkOutcome? launch,
  Brightness brightness = Brightness.light,
}) async {
  await t.runAsync(loadAppFonts);
  final core = fake ?? FakeCoreApi();
  final store = MemorySettingsStore();
  final settings = AppSettings.fromDefaults(
    core.serverDefaults(),
    serverAddress: 'relay.cometcast.de',
    store: store,
  );
  t.view.physicalSize = size;
  t.view.devicePixelRatio = 1;
  t.platformDispatcher.platformBrightnessTestValue = brightness;
  addTearDown(t.view.reset);
  addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
  await t.pumpWidget(
    MaterialApp(
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: PresenterRoot(
        core: core,
        settings: settings,
        pickFile: pickFile ?? (_) async => '/tmp/slides.pdf',
        scanQr: scanQr,
        desktop: desktop,
        device: device,
        codec: codec,
        links: LinkHandler(core: core, settings: settings),
        linkStream: linkStream,
        firstLaunch: firstLaunch,
        onBeDisplay: onBeDisplay,
        onMiracast: windows ? () {} : null,
        initialCode: code ?? (launch is LinkUse ? launch.target.code : null),
      ),
    ),
  );
  for (var i = 0; i < 4; i++) {
    await t.pump(const Duration(milliseconds: 50));
  }
  return PresenterHarness(core, settings, store);
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

/// Connect with the chip, the room is open: joined at once.
Future<void> toWhat(
  WidgetTester t,
  PresenterHarness h, {
  String? room = 'Meeting room Elbe',
}) async {
  await t.tap(find.text('Connect'));
  await settle(t);
  h.core.presenterEvents.add(PresenterUiEvent.joined(roomName: room));
  await settle(t);
}

Future<void> toLink(WidgetTester t, PresenterHarness h) async {
  await toWhat(t, h);
  await t.tap(find.text('Show a link'));
  await settle(t);
  await t.enterText(find.byKey(const Key('link-url')), 'https://example.org/a');
  await t.tap(find.text('Open on the screen'));
  await settle(t);
}

void main() {
  group('first run (5.3)', () {
    testWidgets('name field, code field, Connect joins at once', (t) async {
      final h = await pumpPresenter(t);
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
      expect(find.text('Your name'), findsOneWidget);
      await connectWith(t, 'k7q m2w');
      // Joined on Connect, once: nothing is presented yet.
      expect(h.core.startPresenterCalls, 1);
      expect(h.core.presenterCode, 'K7QM2W');
      expect(h.core.presenterServer, 'cast.example.org');
      expect(h.core.presentedScreens, isEmpty);
      h.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Meeting room Elbe'),
      );
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
      expect(find.text('Share screen'), findsOneWidget);
      expect(find.text('Whole screen or one window, live'), findsOneWidget);
    });

    testWidgets(
      'an empty code is an inline error, a wrong code is the error page',
      (t) async {
        final h = await pumpPresenter(t);
        await t.tap(find.text('Connect'));
        await settle(t);
        expect(
          find.text('Please enter the code shown large on the screen.'),
          findsOneWidget,
        );
        await connectWith(t, 'OOO 000');
        expect(find.text('This code does not exist right now'), findsOneWidget);
        expect(find.text('Retype code'), findsOneWidget);
        // The wrong code never reaches the server.
        expect(h.core.startPresenterCalls, 0);
        await t.tap(find.text('Retype code'));
        await settle(t);
        expect(find.text('Welcome to Comet Cast'), findsOneWidget);
      },
    );

    testWidgets('a code from a link is a chip', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      expect(find.text('K7Q M2W'), findsOneWidget);
      expect(find.text('taken from your link'), findsOneWidget);
      expect(find.byKey(const Key('code-field')), findsNothing);
      await t.tap(find.text('Connect'));
      await settle(t);
      expect(h.core.presenterCode, 'K7QM2W');
    });

    testWidgets('the name goes to the host with the join', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.enterText(find.byKey(const Key('name-field')), 'Ada');
      await t.tap(find.text('Connect'));
      await settle(t);
      expect(h.core.presenterName, 'Ada');
      expect(h.settings.displayName, 'Ada');
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

  group('waiting (5.4) and what to show (5.5)', () {
    testWidgets('waiting: room label, check number, countdown, withdraw', (
      t,
    ) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.waitingForApproval(
          timeoutSecs: 60,
          check: 42,
          roomName: 'Meeting room Elbe',
        ),
      );
      await settle(t);
      expect(find.text('Waiting for approval'), findsOneWidget);
      expect(find.text('42'), findsOneWidget);
      expect(find.text('Meeting room Elbe'), findsOneWidget);
      expect(find.byKey(const Key('waiting-left')), findsOneWidget);
      await t.tap(find.text('Withdraw request'));
      await settle(t);
      expect(h.core.stopPresenterCalls, greaterThanOrEqualTo(1));
      expect(find.text('Welcome to Comet Cast'), findsOneWidget);
    });

    testWidgets('waiting without a room name shows no label, never the code', (
      t,
    ) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.waitingForApproval(timeoutSecs: 60, check: 7),
      );
      await settle(t);
      expect(find.text('K7Q M2W'), findsNothing);
      expect(find.text('K7QM2W'), findsNothing);
    });

    testWidgets('after the host allowed: the header names the room', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      h.core.presenterEvents
        ..add(
          const PresenterUiEvent.waitingForApproval(timeoutSecs: 60, check: 7),
        )
        ..add(const PresenterUiEvent.joined(roomName: 'Meeting room Elbe'));
      await settle(t);
      expect(find.text('Connected to “Meeting room Elbe”'), findsOneWidget);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });

    testWidgets('without a room name the header says only Connected', (
      t,
    ) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toWhat(t, h, room: null);
      expect(find.text('Connected'), findsOneWidget);
      expect(find.textContaining('K7Q'), findsNothing);
    });

    testWidgets('Disconnect leaves the room', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toWhat(t, h);
      await t.tap(find.text('Disconnect'));
      await settle(t);
      expect(h.core.stopPresenterCalls, greaterThanOrEqualTo(1));
      expect(find.text('Disconnected'), findsOneWidget);
    });
  });

  group('showing a link (5.5, 5.6)', () {
    testWidgets('a link is shown inside the one join; Stop keeps it', (
      t,
    ) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toLink(t, h);
      expect(h.core.startPresenterCalls, 1);
      expect(h.core.presentedContent.single, isA<ContentSource_Link>());
      h.core.presenterEvents
        ..add(
          PresenterUiEvent.contentPrepared(
            name: 'example.org',
            kind: UiContentKind.link,
            size: nullSize,
            host: 'example.org',
          ),
        )
        ..add(const PresenterUiEvent.presenting())
        ..add(const PresenterUiEvent.connectionCode(sas: '123 456'))
        ..add(const PresenterUiEvent.contentSent());
      await settle(t);
      expect(find.text('The screen shows your link'), findsOneWidget);
      expect(find.textContaining('123 456'), findsOneWidget);
      await t.tap(find.text('Stop showing'));
      await settle(t);
      expect(h.core.stopPresentingCalls, 1);
      // Stop keeps the join: back at "What to show", nothing was left or joined again.
      expect(find.text('What would you like to show?'), findsOneWidget);
      expect(h.core.stopPresenterCalls, 0);
      expect(h.core.startPresenterCalls, 1);
      h.core.presenterEvents.add(const PresenterUiEvent.presentingStopped());
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });

    testWidgets('a second thing after Stop needs no second join', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toLink(t, h);
      h.core.presenterEvents.add(const PresenterUiEvent.presenting());
      await settle(t);
      await t.tap(find.text('Stop showing'));
      await settle(t);
      h.core.presenterEvents.add(const PresenterUiEvent.presentingStopped());
      await settle(t);
      await t.tap(find.text('Show a link'));
      await settle(t);
      await t.enterText(
        find.byKey(const Key('link-url')),
        'https://example.org/b',
      );
      await t.tap(find.text('Open on the screen'));
      await settle(t);
      expect(h.core.presentedContent.length, 2);
      expect(h.core.startPresenterCalls, 1);
    });

    testWidgets('a bad link is an inline error', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toWhat(t, h);
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
      expect(h.core.presentedContent, isEmpty);
    });

    testWidgets('a link the core refuses is shown on the link page', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toLink(t, h);
      h.core.presenterEvents.add(
        const PresenterUiEvent.contentRejected(reason: UiSendRejection.badLink),
      );
      await settle(t);
      expect(find.byKey(const Key('link-url')), findsOneWidget);
      expect(
        find.text('Please enter a link that starts with http:// or https://.'),
        findsOneWidget,
      );
    });
  });

  group('showing a file (5.5, 5.6)', () {
    Future<PresenterHarness> toFile(WidgetTester t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toWhat(t, h);
      await t.tap(find.text('Show a photo or file'));
      await settle(t);
      await t.tap(find.text('Choose file'));
      await settle(t);
      return h;
    }

    testWidgets('a PDF: page buttons above Stop, commands go to the core', (
      t,
    ) async {
      final h = await toFile(t);
      expect(h.core.presentedContent.single, isA<ContentSource_File>());
      h.core.presenterEvents
        ..add(
          PresenterUiEvent.contentPrepared(
            kind: UiContentKind.pdf,
            name: 'slides.pdf',
            size: BigInt.from(100),
            pages: 3,
          ),
        )
        ..add(const PresenterUiEvent.presenting())
        ..add(const PresenterUiEvent.contentSent());
      await settle(t);
      expect(find.text('You are showing a file'), findsOneWidget);
      expect(find.text('Previous page'), findsOneWidget);
      // The display has not reported the page: no count line, both buttons work.
      expect(find.byKey(const Key('page-of')), findsNothing);
      await t.tap(find.text('Next page'));
      await settle(t);
      expect(h.core.contentCommands, [const UiContentCmd.nextPage()]);
    });

    testWidgets('Page 1 of 3: the count shows and Previous is disabled', (
      t,
    ) async {
      final h = await toFile(t);
      h.core.presenterEvents
        ..add(
          PresenterUiEvent.contentPrepared(
            kind: UiContentKind.pdf,
            name: 'slides.pdf',
            size: BigInt.from(100),
            pages: 3,
          ),
        )
        ..add(const PresenterUiEvent.presenting())
        ..add(const PresenterUiEvent.page(page: 1, pages: 3));
      await settle(t);
      expect(find.text('Page 1 of 3'), findsOneWidget);
      await t.tap(find.text('Previous page'));
      await settle(t);
      expect(h.core.contentCommands, isEmpty);
      await t.tap(find.text('Next page'));
      await settle(t);
      expect(h.core.contentCommands, [const UiContentCmd.nextPage()]);
      h.core.presenterEvents.add(
        const PresenterUiEvent.page(page: 3, pages: 3),
      );
      await settle(t);
      expect(find.text('Page 3 of 3'), findsOneWidget);
      h.core.contentCommands.clear();
      await t.tap(find.text('Next page'));
      await settle(t);
      expect(h.core.contentCommands, isEmpty);
    });

    testWidgets('the file page shows the name and percent while sending', (
      t,
    ) async {
      final h = await toFile(t);
      h.core.presenterEvents
        ..add(
          PresenterUiEvent.contentPrepared(
            kind: UiContentKind.image,
            name: 'photo.png',
            size: BigInt.from(1000),
          ),
        )
        ..add(
          PresenterUiEvent.progress(
            sent: BigInt.from(500),
            total: BigInt.from(1000),
          ),
        );
      await settle(t);
      expect(find.text('photo.png · 50 %'), findsOneWidget);
    });

    testWidgets('an Office file shows "Converting ..." and no bar', (t) async {
      final h = await toFile(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.converting(name: 'slides.pptx'),
      );
      await settle(t);
      expect(find.text('Converting slides.pptx …'), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsNothing);
      h.core.presenterEvents.add(
        PresenterUiEvent.contentPrepared(
          kind: UiContentKind.pdf,
          name: 'slides.pdf',
          size: BigInt.from(100),
          pages: 2,
        ),
      );
      await settle(t);
      expect(find.text('Converting slides.pptx …'), findsNothing);
    });

    testWidgets('a refused file is an inline error and keeps the join', (
      t,
    ) async {
      final h = await toFile(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.contentRejected(
          reason: UiSendRejection.tooLarge,
        ),
      );
      await settle(t);
      expect(find.byKey(const Key('file-error')), findsOneWidget);
      expect(h.core.stopPresenterCalls, 0);
    });

    testWidgets('cancelling the file dialog changes nothing', (t) async {
      final h = await pumpRoot(
        t,
        code: 'K7QM2W',
        pickFile: (_) async => null,
        desktop: true,
      );
      await toWhat(t, h);
      await t.tap(find.text('Show a photo or file'));
      await settle(t);
      await t.tap(find.text('Choose file'));
      await settle(t);
      expect(h.core.presentedContent, isEmpty);
      expect(find.text('Choose file'), findsOneWidget);
    });
  });

  group('errors (6) by code', () {
    Future<PresenterHarness> afterConnect(WidgetTester t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      return h;
    }

    testWidgets('denied: Ask again counts down 15 s', (t) async {
      final h = await afterConnect(t);
      h.core.presenterEvents
        ..add(
          const PresenterUiEvent.waitingForApproval(timeoutSecs: 60, check: 7),
        )
        ..add(const PresenterUiEvent.error(code: 'join_denied', message: 'x'));
      await settle(t);
      expect(find.text('Not allowed'), findsOneWidget);
      expect(find.textContaining('Ask again in'), findsOneWidget);
    });

    testWidgets('locked while waiting uses the second why text', (t) async {
      final h = await afterConnect(t);
      h.core.presenterEvents
        ..add(
          const PresenterUiEvent.waitingForApproval(timeoutSecs: 60, check: 7),
        )
        ..add(
          const PresenterUiEvent.error(
            code: 'room_locked_waiting',
            message: 'x',
          ),
        );
      await settle(t);
      expect(find.text('Locked right now'), findsOneWidget);
      expect(
        find.text(
          'The host locked the room while you were waiting. Your request was dropped.',
        ),
        findsOneWidget,
      );
    });

    testWidgets('locked at once uses the first why text', (t) async {
      final h = await afterConnect(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'room_locked', message: 'x'),
      );
      await settle(t);
      expect(find.text('Locked right now'), findsOneWidget);
      expect(find.textContaining('while you were waiting'), findsNothing);
    });

    const cases = {
      'rate_limited': 'The server is busy',
      'banned': 'The server is busy',
      'server_full': 'The server is busy',
      'room_full': 'Someone else is connected',
      'approval_timeout': 'No answer',
      'room_not_found': 'This code does not exist right now',
      'connection_lost': 'No internet on this device',
      'peer_failed': 'The screen does not answer',
    };
    for (final e in cases.entries) {
      testWidgets('${e.key} is the page "${e.value}"', (t) async {
        final h = await afterConnect(t);
        h.core.presenterEvents.add(
          PresenterUiEvent.error(code: e.key, message: 'x'),
        );
        await settle(t);
        expect(find.text(e.value), findsOneWidget);
      });
    }

    testWidgets('room_not_found after the host allowed is "expired"', (
      t,
    ) async {
      final h = await afterConnect(t);
      h.core.presenterEvents
        ..add(
          const PresenterUiEvent.waitingForApproval(timeoutSecs: 60, check: 7),
        )
        ..add(
          const PresenterUiEvent.error(code: 'room_not_found', message: 'x'),
        );
      await settle(t);
      expect(find.text('This code is no longer valid'), findsOneWidget);
    });

    testWidgets('proto_mismatch says "app", offers Try again, never Reload', (
      t,
    ) async {
      final h = await afterConnect(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'proto_mismatch', message: 'x'),
      );
      await settle(t);
      expect(find.text('This app is out of date'), findsOneWidget);
      expect(find.text('Update the app, then try again.'), findsOneWidget);
      expect(find.text('Try again'), findsOneWidget);
      expect(find.textContaining('Reload'), findsNothing);
      expect(find.textContaining('page'), findsNothing);
    });

    testWidgets('removed by the host and the display leaving: ended', (
      t,
    ) async {
      final h = await afterConnect(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.ended(code: 'removed_by_host'),
      );
      await settle(t);
      expect(find.text('Presentation ended'), findsOneWidget);
    });

    testWidgets('an unknown code is the other page with the existing text', (
      t,
    ) async {
      final h = await afterConnect(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'forbidden', message: 'no'),
      );
      await settle(t);
      expect(find.text('Something went wrong'), findsOneWidget);
      expect(
        find.text('Try again. If it happens again, tell the host.'),
        findsOneWidget,
      );
    });

    testWidgets('Try again joins once more with the same code', (t) async {
      final h = await afterConnect(t);
      h.core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'peer_failed', message: 'x'),
      );
      await settle(t);
      await t.tap(find.text('Try again'));
      await settle(t);
      expect(h.core.startPresenterCalls, 2);
      expect(h.core.presenterCode, 'K7QM2W');
    });

    testWidgets('a presentation that cannot start keeps the join', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toWhat(t, h);
      h.core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'capture_unavailable', message: 'x'),
      );
      await settle(t);
      expect(find.text('Something went wrong'), findsOneWidget);
      await t.tap(find.text('Try again'));
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
      expect(h.core.startPresenterCalls, 1);
      expect(h.core.stopPresenterCalls, 0);
    });

    testWidgets('capture_denied shows no page at all', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toWhat(t, h);
      h.core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'capture_denied', message: 'x'),
      );
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
    });
  });

  group('disconnected (5.7)', () {
    testWidgets('no body line in the app', (t) async {
      final h = await pumpPresenter(t, code: 'K7QM2W');
      await toWhat(t, h);
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

/// The size of a link has no meaning; the event needs one.
final nullSize = BigInt.zero;
