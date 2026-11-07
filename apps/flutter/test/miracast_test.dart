// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the Windows Miracast receive screen with a fake core (no Windows, no DLL):
// JSON parsing, library lookup, the state machine, the consent dialog, the video view, the
// translated hints and the platform guard of the home screen.

import 'dart:async';
import 'dart:io' show Platform;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/miracast.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/miracast_controller.dart';
import 'package:cometcast/miracast_page.dart';
import 'package:cometcast/src/display/miracast_view.dart';
import 'package:cometcast/src/presenter/kit.dart';

import 'fake_miracast.dart';

void main() {
  group('JSON of the C interface', () {
    test('a full state is parsed', () {
      final s = MiracastState.fromJson({
        'v': 1,
        'status': 'connection_pending',
        'issue': null,
        'generation': 3,
        'pending': {'id': 7, 'name': 'Pixel', 'pin': '4711'},
        'connected': null,
        'last_error': 'x',
        'rejected': 2,
        'failed': 1,
        'frames': {
          'published': 10,
          'taken': 8,
          'dropped': 2,
          'failed': 0,
          'width': 1280,
          'height': 720,
        },
        'feature': 'installed',
        'adapter': 'supported',
      });
      expect(s.status, MiracastStatus.connectionPending);
      expect(
        s.pending,
        const MiracastRequest(id: 7, name: 'Pixel', pin: '4711'),
      );
      expect(s.rejected, 2);
      expect(s.frames.dropped, 2);
      expect(s.frames.width, 1280);
      expect(s.feature, 'installed');
      expect(s.generation, 3);
    });

    test('unknown words are failures, never success', () {
      expect(parseMiracastStatus('banana'), MiracastStatus.failed);
      expect(parseMiracastStatus(null), MiracastStatus.failed);
      expect(parseMiracastIssue('banana'), MiracastIssue.other);
      expect(parseMiracastIssue(null), isNull);
      final s = MiracastState.fromJson({
        'status': 'listening',
        'pending': 'nonsense',
      });
      expect(s.pending, isNull);
    });

    test('every status and issue word of the Rust side is known', () {
      const statuses = {
        'off': MiracastStatus.off,
        'starting': MiracastStatus.starting,
        'stopping': MiracastStatus.stopping,
        'listening': MiracastStatus.listening,
        'connection_pending': MiracastStatus.connectionPending,
        'connected': MiracastStatus.connected,
        'unavailable': MiracastStatus.unavailable,
        'disabled_by_policy': MiracastStatus.disabledByPolicy,
        'unsupported': MiracastStatus.unsupported,
        'failed': MiracastStatus.failed,
      };
      statuses.forEach((k, v) => expect(parseMiracastStatus(k), v));
      const issues = {
        'not_windows': MiracastIssue.notWindows,
        'feature_missing': MiracastIssue.featureMissing,
        'adapter_unsupported': MiracastIssue.adapterUnsupported,
        'adapter_not_optimized': MiracastIssue.adapterNotOptimized,
        'disabled_by_policy': MiracastIssue.disabledByPolicy,
        'temporarily_unavailable': MiracastIssue.temporarilyUnavailable,
        'access_denied': MiracastIssue.accessDenied,
        'invalid_settings': MiracastIssue.invalidSettings,
        'other': MiracastIssue.other,
      };
      issues.forEach((k, v) => expect(parseMiracastIssue(k), v));
    });

    test('pairing names match the Rust side', () {
      expect(MiracastPairing.confirm.wire, 'confirm');
      expect(MiracastPairing.pinIfRequested.wire, 'pin_if_requested');
      expect(MiracastPairing.pinRequired.wire, 'pin_required');
    });

    test('the probe is parsed', () {
      final p = MiracastProbe.fromJson({
        'v': 1,
        'feature': 'unknown',
        'adapter': 'not_supported',
        'receiver_available': true,
        'issue': 'adapter_unsupported',
      });
      expect(p.issue, MiracastIssue.adapterUnsupported);
      expect(p.adapter, 'not_supported');
      expect(p.receiverAvailable, isTrue);
    });
  });

  group('library lookup', () {
    test('candidates: next to the exe, then lib', () {
      expect(miracastLibraryCandidates(exeDir: r'C:\app', separator: r'\'), [
        r'C:\app\cc_miracast_ffi.dll',
        r'C:\app\lib\cc_miracast_ffi.dll',
      ]);
    });

    test('the environment variable wins, then the first existing file', () {
      expect(
        resolveMiracastLibraryPath(
          environment: {miracastLibraryEnvVar: 'X:/custom.dll'},
          executableDir: '/app',
          separator: '/',
          fileExists: (_) => true,
        ),
        'X:/custom.dll',
      );
      expect(
        resolveMiracastLibraryPath(
          environment: {},
          executableDir: '/app',
          separator: '/',
          fileExists: (p) => p == '/app/lib/cc_miracast_ffi.dll',
        ),
        '/app/lib/cc_miracast_ffi.dll',
      );
      expect(
        resolveMiracastLibraryPath(
          environment: {},
          executableDir: '/app',
          separator: '/',
          fileExists: (_) => false,
        ),
        'cc_miracast_ffi.dll',
      );
    });

    test(
      'a library that cannot be loaded is reported as library problem',
      () async {
        final core = FfiMiracastCore(opener: () => throw StateError('no file'));
        final probe = await core.probe();
        expect(probe.issue, MiracastIssue.libraryMissing);
        await expectLater(
          core.state(),
          throwsA(
            isA<MiracastCallException>().having(
              (e) => e.isLibraryProblem,
              'library problem',
              true,
            ),
          ),
        );
      },
    );
  });

  group('MiracastController', () {
    late FakeMiracastCore core;
    late MiracastController c;

    setUp(() {
      core = FakeMiracastCore();
      c = MiracastController(core: core, displayName: '  My PC  ');
    });
    tearDown(() => c.dispose());

    test('idle at first, the probe hint shows only while idle', () async {
      expect(c.phase, MiracastPhase.idle);
      core.probeResult = const MiracastProbe(
        issue: MiracastIssue.featureMissing,
      );
      await c.init();
      expect(c.issue, MiracastIssue.featureMissing);
      expect(c.phase, MiracastPhase.idle);
    });

    test(
      'start passes the trimmed name and the pairing, then listens',
      () async {
        c.pairing = MiracastPairing.pinRequired;
        await c.start();
        expect(core.startedName, 'My PC');
        expect(core.startedPairing, MiracastPairing.pinRequired);
        expect(c.phase, MiracastPhase.listening);
      },
    );

    test('starting stays visible until the core says otherwise', () async {
      core.listenOnStart = false;
      await c.start();
      expect(c.phase, MiracastPhase.starting);
      core.listening();
      await c.poll();
      expect(c.phase, MiracastPhase.listening);
    });

    test('a request moves to consent; the answer is sent once', () async {
      await c.start();
      core.request(5, 'Pixel', pin: '1234');
      await c.poll();
      expect(c.phase, MiracastPhase.consent);
      expect(c.pending?.name, 'Pixel');
      expect(c.pending?.pin, '1234');
      expect(await c.respond(99, true), isFalse, reason: 'wrong id');
      expect(core.responses, isEmpty);
      expect(await c.respond(5, true), isTrue);
      expect(await c.respond(5, true), isFalse, reason: 'already answered');
      expect(core.responses, [(5, true)]);
    });

    test('a request that timed out is not answered', () async {
      await c.start();
      core.request(5, 'Pixel');
      await c.poll();
      core.requestGone();
      await c.poll();
      expect(c.pending, isNull);
      expect(await c.respond(5, true), isFalse);
      expect(core.responses, isEmpty);
    });

    test('decline goes back to listening', () async {
      await c.start();
      core.request(5, 'Pixel');
      await c.poll();
      await c.respond(5, false);
      expect(core.responses, [(5, false)]);
      expect(c.phase, MiracastPhase.listening);
    });

    test('pictures are only exposed while connected (privacy)', () async {
      await c.start();
      core.addFrame(1);
      core.request(5, 'Pixel');
      await c.poll();
      expect(c.frame, isNull, reason: 'consent pending');
      expect(
        core.pollFrameCalls,
        0,
        reason: 'no picture is even fetched before consent',
      );
      core.connect('Pixel');
      await c.poll();
      expect(c.phase, MiracastPhase.connected);
      expect(c.frame?.seq, 1);
      expect(c.framesSeen, 1);
      await c.disconnect();
      expect(core.calls, contains('disconnect'));
      expect(c.frame, isNull);
      expect(c.phase, MiracastPhase.listening);
    });

    test('the sender leaves: back to listening, no picture', () async {
      await c.start();
      core.connect('Pixel');
      core.addFrame(1);
      await c.poll();
      expect(c.frame, isNotNull);
      core.listening();
      await c.poll();
      expect(c.frame, isNull);
      expect(c.phase, MiracastPhase.listening);
    });

    test('start errors become visible problems', () async {
      core.startError = const MiracastCallException(-2);
      await c.start();
      expect(c.phase, MiracastPhase.problem);
      expect(c.issue, MiracastIssue.invalidSettings);
      // And a new start is allowed afterwards.
      await c.start();
      expect(c.phase, MiracastPhase.listening);
    });

    test('a missing library is a problem with its own hint', () async {
      core.startError = const MiracastCallException(0, 'no dll');
      await c.start();
      expect(c.phase, MiracastPhase.problem);
      expect(c.issue, MiracastIssue.libraryMissing);
    });

    test('busy (already running) is taken over, not an error', () async {
      core.startError = const MiracastCallException(-1);
      core.listening();
      await c.start();
      expect(c.phase, MiracastPhase.listening);
      expect(c.issue, isNull);
    });

    test('statuses map to phases and problems', () async {
      await c.start();
      for (final (status, phase) in [
        (MiracastStatus.unsupported, MiracastPhase.problem),
        (MiracastStatus.unavailable, MiracastPhase.problem),
        (MiracastStatus.disabledByPolicy, MiracastPhase.problem),
        (MiracastStatus.failed, MiracastPhase.problem),
        (MiracastStatus.stopping, MiracastPhase.stopping),
        (MiracastStatus.listening, MiracastPhase.listening),
      ]) {
        core.current = MiracastState(
          status: status,
          generation: core.generation,
        );
        await c.poll();
        expect(c.phase, phase, reason: '$status');
      }
    });

    test(
      'an adapter that is not optimised is a warning while listening',
      () async {
        await c.start();
        core.current = MiracastState(
          status: MiracastStatus.listening,
          generation: core.generation,
          adapter: 'not_optimized',
        );
        await c.poll();
        expect(c.phase, MiracastPhase.listening);
        expect(c.issue, MiracastIssue.adapterNotOptimized);
      },
    );

    test('a failing poll is skipped, a library problem is shown', () async {
      await c.start();
      core.stateError = StateError('x');
      await c.poll();
      expect(c.phase, MiracastPhase.listening);
      core.stateError = const MiracastCallException(0);
      await c.poll();
      expect(c.phase, MiracastPhase.problem);
    });

    test('stop returns to idle and stops the core', () async {
      await c.start();
      await c.stop();
      expect(c.phase, MiracastPhase.idle);
      expect(core.calls.last, 'stop');
    });

    test('dispose stops the receiver', () async {
      await c.start();
      final other = MiracastController(core: core);
      core.calls.clear();
      other.dispose();
      expect(core.calls, ['stop']);
    });

    test('polling by timer fetches the state and the pictures', () async {
      final fast = MiracastController(
        core: core,
        pollInterval: const Duration(milliseconds: 5),
        statePollInterval: const Duration(milliseconds: 10),
      );
      await fast.start();
      core.connect('Pixel');
      core.addFrame(1);
      for (var i = 0; i < 50 && fast.phase != MiracastPhase.connected; i++) {
        await Future<void>.delayed(const Duration(milliseconds: 10));
      }
      core.addFrame(2);
      for (var i = 0; i < 50 && fast.frame?.seq != 2; i++) {
        await Future<void>.delayed(const Duration(milliseconds: 10));
      }
      expect(fast.frame?.seq, 2);
      fast.dispose();
    });
  });

  group('screen', () {
    Future<void> pumpDirect(
      WidgetTester tester,
      FakeMiracastCore core, {
      Locale locale = const Locale('en'),
      FrameDecoder? decoder,
    }) async {
      // Tall window: the screen is a lazily built list, everything must be on screen.
      tester.view.physicalSize = const Size(900, 2400);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        MaterialApp(
          locale: locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: const [Locale('en'), Locale('de')],
          home: MiracastPage(
            core: core,
            initialName: 'Test PC',
            decoder: decoder ?? ((f) => Completer<ui.Image>().future),
            pollInterval: const Duration(milliseconds: 20),
          ),
        ),
      );
      await tester.pumpAndSettle();
    }

    Future<void> close(WidgetTester tester) async {
      await tester.pumpWidget(const SizedBox());
      await tester.pump();
    }

    testWidgets('start shows the waiting text with the name', (tester) async {
      final core = FakeMiracastCore();
      await pumpDirect(tester, core);
      expect(find.text('Not started.'), findsOneWidget);
      await tester.tap(find.byKey(const Key('miracastStartButton')));
      await tester.pump(const Duration(milliseconds: 200));
      expect(core.startedName, 'Test PC');
      expect(find.textContaining('choose "Test PC"'), findsOneWidget);
      expect(find.byKey(const Key('miracastStopButton')), findsOneWidget);
      await close(tester);
      expect(
        core.calls.last,
        'stop',
        reason: 'leaving the screen stops receiving',
      );
    });

    testWidgets('consent dialog: name, PIN, Decline is focused first', (
      tester,
    ) async {
      final core = FakeMiracastCore();
      await pumpDirect(tester, core);
      await tester.tap(find.byKey(const Key('miracastStartButton')));
      await tester.pump(const Duration(milliseconds: 200));
      core.request(9, 'Pixel 9', pin: '4711');
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pump();
      expect(find.byKey(const Key('miracastConsentDialog')), findsOneWidget);
      expect(
        find.textContaining('"Pixel 9" wants to show its screen'),
        findsOneWidget,
      );
      expect(
        find.text(
          'PIN: 4711. Enter it on the sending device if it asks for it.',
        ),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('miracastVideo')),
        findsNothing,
        reason: 'nothing before consent',
      );
      // The first focus is on Decline: an accidental Enter must not share a screen.
      final focused = FocusManager.instance.primaryFocus!.context!;
      expect(
        focused.findAncestorWidgetOfExactType<PSecondaryButton>()?.key,
        const Key('miracastDeny'),
      );
      await tester.tap(find.byKey(const Key('miracastDeny')));
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pumpAndSettle();
      expect(core.responses, [(9, false)]);
      expect(find.byKey(const Key('miracastConsentDialog')), findsNothing);
      await close(tester);
    });

    testWidgets('Allow answers yes, then the video view appears', (
      tester,
    ) async {
      final core = FakeMiracastCore();
      final decoded = <int>[];
      await pumpDirect(
        tester,
        core,
        decoder: (f) {
          decoded.add(f.seq);
          return Completer<ui.Image>().future;
        },
      );
      await tester.tap(find.byKey(const Key('miracastStartButton')));
      await tester.pump(const Duration(milliseconds: 200));
      core.request(9, 'Pixel 9');
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pump();
      await tester.tap(find.byKey(const Key('miracastAllow')));
      await tester.pump();
      expect(core.responses, [(9, true)]);
      core.connect('Pixel 9');
      core.addFrame(1);
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('miracastVideo')), findsOneWidget);
      expect(find.text('Receiving the screen of "Pixel 9".'), findsOneWidget);
      expect(find.byKey(const Key('miracastDisconnectButton')), findsOneWidget);
      expect(decoded, [1]);
      await tester.tap(find.byKey(const Key('miracastDisconnectButton')));
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('miracastVideo')), findsNothing);
      await close(tester);
    });

    testWidgets('the dialog closes by itself when the request times out', (
      tester,
    ) async {
      final core = FakeMiracastCore();
      await pumpDirect(tester, core);
      await tester.tap(find.byKey(const Key('miracastStartButton')));
      await tester.pump(const Duration(milliseconds: 200));
      core.request(3, 'Laptop');
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pump();
      expect(find.byKey(const Key('miracastConsentDialog')), findsOneWidget);
      core.requestGone();
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('miracastConsentDialog')), findsNothing);
      expect(core.responses, isEmpty);
      await close(tester);
    });

    testWidgets(
      'a hostile name is shown as plain text of one line of the dialog',
      (tester) async {
        final core = FakeMiracastCore();
        await pumpDirect(tester, core);
        await tester.tap(find.byKey(const Key('miracastStartButton')));
        await tester.pump(const Duration(milliseconds: 200));
        // The Rust side already cleaned the name; the screen shows it inside quotes in the body.
        core.request(3, 'Laptop Accept?');
        await tester.pump(const Duration(milliseconds: 200));
        await tester.pump();
        final body = tester.widget<Text>(
          find.byKey(const Key('miracastConsentBody')),
        );
        expect(body.data, contains('"Laptop Accept?" wants to show'));
        await close(tester);
      },
    );

    testWidgets(
      'the feature hint names the Settings path and the DISM capability',
      (tester) async {
        final core = FakeMiracastCore()
          ..probeResult = const MiracastProbe(
            issue: MiracastIssue.featureMissing,
          );
        await pumpDirect(tester, core);
        final hint = tester.widget<Text>(find.byKey(const Key('miracastHint')));
        expect(hint.data, contains('Optional features'));
        expect(hint.data, contains('App.WirelessDisplay.Connect~~~~0.0.1.0'));
        await close(tester);
      },
    );

    testWidgets('German texts use the informal address (du is shipped)', (
      tester,
    ) async {
      final core = FakeMiracastCore()
        ..probeResult = const MiracastProbe(
          issue: MiracastIssue.featureMissing,
        );
      await pumpDirect(tester, core, locale: const Locale('de'));
      expect(find.text('Nicht gestartet.'), findsOneWidget);
      expect(find.text('Empfang starten'), findsOneWidget);
      final hint = tester.widget<Text>(find.byKey(const Key('miracastHint')));
      expect(hint.data, contains('Starte danach diese App neu'));
      await close(tester);
    });

    testWidgets('a missing library shows its hint instead of crashing', (
      tester,
    ) async {
      final core = FakeMiracastCore()
        ..probeResult = const MiracastProbe(
          issue: MiracastIssue.libraryMissing,
        );
      await pumpDirect(tester, core);
      expect(find.byKey(const Key('miracastHint')), findsOneWidget);
      expect(find.textContaining('cc_miracast_ffi.dll'), findsWidgets);
      await close(tester);
    });

    testWidgets('a second sender that was turned away is mentioned', (
      tester,
    ) async {
      final core = FakeMiracastCore();
      await pumpDirect(tester, core);
      await tester.tap(find.byKey(const Key('miracastStartButton')));
      await tester.pump(const Duration(milliseconds: 200));
      core.current = MiracastState(
        status: MiracastStatus.listening,
        generation: core.generation,
        rejected: 2,
      );
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.byKey(const Key('miracastRejectedNote')), findsOneWidget);
      await close(tester);
    });

    test('the platform guard follows the platform', () {
      expect(miracastSupportedHere(), Platform.isWindows);
    });
  });

  group('the display role (native-apps.md 21.4)', () {
    test('a PIN is always required', () {
      expect(displayMiracastPairing, MiracastPairing.pinRequired);
      expect(displayMiracastPairing.wire, 'pin_required');
    });

    test('the diagnosis maps to the four state lines', () {
      expect(
        miracastStateLine(MiracastIssue.adapterUnsupported),
        MiracastStateLine.noHw,
      );
      expect(
        miracastStateLine(MiracastIssue.featureMissing),
        MiracastStateLine.noFeature,
      );
      expect(
        miracastStateLine(MiracastIssue.disabledByPolicy),
        MiracastStateLine.policy,
      );
      // Every other issue is a plain failure.
      for (final i in MiracastIssue.values.where(
        (i) => !{
          MiracastIssue.adapterUnsupported,
          MiracastIssue.featureMissing,
          MiracastIssue.disabledByPolicy,
        }.contains(i),
      )) {
        expect(miracastStateLine(i), MiracastStateLine.failed, reason: '$i');
      }
    });
  });
}
