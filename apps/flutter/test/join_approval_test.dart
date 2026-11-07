// SPDX-License-Identifier: GPL-3.0-or-later
// Join approval with a fake core: the host's panel (mode, requests, hold-to-allow, kick), the
// television profile (D-pad / remote keys only), and the sender's waiting and refusal screens.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/android_platform.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/join_state.dart';
import 'package:cometcast/join_widgets.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/receiver_controller.dart';
import 'package:cometcast/settings.dart';

import 'fake_core.dart';

class _TvPlatform implements AndroidPlatform {
  @override
  Future<bool> isTelevision() async => true;
  @override
  Future<String?> nativeLibraryDir() async => null;
  @override
  Future<void> setKeepScreenOn(bool on) async {}
  @override
  Future<Map<String, Object?>> deviceInfo() async => {'model': 'Test TV'};
}

BigInt id(int n) => BigInt.from(n);

ReceiverUiEvent request(int n, String name, {int check = 40}) =>
    ReceiverUiEvent.joinRequest(id: id(n), name: name, check: check + n);

Future<FakeCoreApi> openReceive(
  WidgetTester tester, {
  bool tv = false,
  String locale = 'en',
  UiJoinMode mode = UiJoinMode.open,
  AppSettings? settings,
}) async {
  final core = FakeCoreApi();
  tester.view.physicalSize = tv
      ? const Size(1920, 1080)
      : const Size(900, 2400);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    CometCastApp(
      locale: Locale(locale),
      core: core,
      tv: tv,
      androidPlatform: tv ? _TvPlatform() : null,
      settings: settings,
    ),
  );
  await tester.pumpAndSettle();
  if (tv) {
    // The TV profile starts receiving by itself after OK on the focused tile.
    await tester.sendKeyEvent(LogicalKeyboardKey.select);
    await tester.pumpAndSettle();
  } else {
    await tester.tap(
      find.text(
        locale == 'en'
            ? 'Receive on this device'
            : 'Auf diesem Gerät empfangen',
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
  }
  core.receiverEvents.add(
    ReceiverUiEvent.roomCode(code: 'K7M2QX', joinMode: mode),
  );
  await tester.pump();
  await tester.pump();
  return core;
}

Future<void> holdAllow(WidgetTester tester, int n, {int ms = 1100}) async {
  final g = await tester.startGesture(
    tester.getCenter(find.byKey(const Key('allowButton')).at(n)),
  );
  // The first frame only starts the ticker; the hold is measured from there.
  await tester.pump();
  await tester.pump(Duration(milliseconds: ms));
  await g.up();
  await tester.pump();
}

void main() {
  group('host panel (desktop)', () {
    testWidgets(
      'mode choice: one press, the server confirms, help text follows',
      (tester) async {
        final core = await openReceive(tester);
        expect(find.byKey(const Key('joinPanel')), findsOneWidget);
        expect(
          find.text('Anyone who knows the room code joins at once.'),
          findsOneWidget,
        );
        await tester.tap(find.byKey(const Key('mode-confirm')));
        await tester.pump();
        expect(core.joinModeRequests, [UiJoinMode.confirm]);
        // Only the server's answer changes what is shown.
        expect(
          find.text('Every join waits until you allow it here.'),
          findsNothing,
        );
        core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.confirm),
        );
        await tester.pump();
        expect(
          find.text('Every join waits until you allow it here.'),
          findsOneWidget,
        );
        await tester.tap(find.byKey(const Key('lockNowButton')));
        expect(core.joinModeRequests.last, UiJoinMode.locked);
      },
    );

    testWidgets('started in the mode that the settings remember', (
      tester,
    ) async {
      final core = FakeCoreApi();
      final settings = AppSettings.fromDefaults(
        core.serverDefaults(),
        joinMode: UiJoinMode.confirm,
      );
      final c = await openReceive(
        tester,
        settings: settings,
        mode: UiJoinMode.confirm,
      );
      expect(c.receiverJoinMode, UiJoinMode.confirm);
    });

    testWidgets(
      'a request is shown with name and check number; nothing takes the focus',
      (tester) async {
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        await tester.tap(find.byKey(const Key('mode-confirm')));
        await tester.pump();
        final before = FocusManager.instance.primaryFocus;
        core.receiverEvents.add(request(1, 'Ana'));
        await tester.pump();
        expect(FocusManager.instance.primaryFocus, before);
        expect(find.text('“Ana”'), findsOneWidget);
        expect(find.byKey(Key('requestCheck-${id(1)}')), findsOneWidget);
        expect(
          tester.widget<Text>(find.byKey(Key('requestCheck-${id(1)}'))).data,
          '41',
        );
        final allow = tester.widget<FilledButton>(
          find.byKey(const Key('allowButton')),
        );
        expect(allow.focusNode!.hasPrimaryFocus, isFalse);
      },
    );

    testWidgets(
      'Allow needs a hold; a short press only shows a hint; Deny is one press',
      (tester) async {
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        core.receiverEvents.add(request(1, 'Ana'));
        core.receiverEvents.add(request(2, 'Ben'));
        await tester.pump();
        // Short press on the first Allow.
        final g = await tester.startGesture(
          tester.getCenter(find.byKey(const Key('allowButton')).first),
        );
        await tester.pump(const Duration(milliseconds: 300));
        await g.up();
        await tester.pump();
        expect(core.joinDecisions, isEmpty);
        expect(find.byKey(const Key('holdHint')), findsOneWidget);
        // A tap (click) never allows.
        await tester.tap(find.byKey(const Key('allowButton')).first);
        await tester.pump();
        expect(core.joinDecisions, isEmpty);
        await tester.pump(const Duration(seconds: 3));
        // A long hold allows.
        await holdAllow(tester, 0);
        expect(core.joinDecisions, [(id: id(1), allow: true)]);
        // Deny: one press.
        await tester.tap(find.byKey(Key('deny-${id(2)}')));
        await tester.pump();
        expect(core.joinDecisions.last, (id: id(2), allow: false));
      },
    );

    testWidgets(
      'after a decision the focus goes to the next Deny, never to Allow',
      (tester) async {
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        core.receiverEvents.add(request(1, 'Ana'));
        core.receiverEvents.add(request(2, 'Ben'));
        await tester.pump();
        await tester.tap(find.byKey(Key('deny-${id(1)}')));
        await tester.pumpAndSettle();
        final deny2 = tester.widget<OutlinedButton>(
          find.byKey(Key('deny-${id(2)}')),
        );
        expect(deny2.focusNode!.hasPrimaryFocus, isTrue);
        // The last one: the focus returns to the mode choice.
        await tester.tap(find.byKey(Key('deny-${id(2)}')));
        await tester.pumpAndSettle();
        final mode = tester.widget<ElevatedButton>(
          find.byKey(const Key('mode-confirm')),
        );
        expect(mode.focusNode!.hasPrimaryFocus, isTrue);
      },
    );

    testWidgets(
      'three requests are shown, the rest counted; Deny all from two on',
      (tester) async {
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        core.receiverEvents.add(request(1, 'A'));
        await tester.pump();
        expect(find.byKey(const Key('denyAllButton')), findsNothing);
        for (var n = 2; n <= 4; n++) {
          core.receiverEvents.add(request(n, 'P$n'));
        }
        await tester.pump();
        expect(find.byKey(const Key('allowButton')), findsNWidgets(3));
        expect(find.text('+1 more waiting'), findsOneWidget);
        await tester.tap(find.byKey(const Key('denyAllButton')));
        await tester.pump();
        expect(core.joinDecisions, [(id: null, allow: false)]);
        expect(find.byKey(const Key('joinNobodyWaiting')), findsOneWidget);
      },
    );

    testWidgets('locking discards waiting requests and says so', (
      tester,
    ) async {
      final core = await openReceive(tester, mode: UiJoinMode.confirm);
      core.receiverEvents.add(request(1, 'A'));
      core.receiverEvents.add(request(2, 'B'));
      await tester.pump();
      core.receiverEvents.add(
        ReceiverUiEvent.joinRequestCancelled(id: id(1), reason: 'locked'),
      );
      core.receiverEvents.add(
        ReceiverUiEvent.joinRequestCancelled(id: id(2), reason: 'locked'),
      );
      core.receiverEvents.add(
        const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.locked),
      );
      await tester.pump();
      expect(
        find.text('Room locked. 2 waiting request(s) were discarded.'),
        findsOneWidget,
      );
      expect(find.byKey(const Key('joinNobodyWaiting')), findsOneWidget);
      await tester.tap(find.byKey(const Key('joinNoteDismiss')));
      await tester.pump();
      expect(find.byKey(const Key('joinNote')), findsNothing);
    });

    testWidgets('the connected sender is listed and can be removed', (
      tester,
    ) async {
      final core = await openReceive(tester);
      core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Cy'));
      await tester.pump();
      expect(find.text('“Cy”'), findsOneWidget);
      await tester.tap(find.byKey(const Key('removePeerButton')));
      expect(core.removePeerCalls, 1);
    });

    testWidgets('names are plain text and are shortened', (tester) async {
      final core = await openReceive(tester, mode: UiJoinMode.confirm);
      core.receiverEvents.add(request(1, '<b>Alexandra-Maria Schmidt</b>'));
      await tester.pump();
      expect(find.text('“<b>Alexandra-Maria…”'), findsOneWidget);
      expect(
        shortJoinName('abcdefghijklmnopqrstuvwxyz'),
        'abcdefghijklmnopqr…',
      );
    });

    testWidgets('German texts', (tester) async {
      final core = await openReceive(
        tester,
        locale: 'de',
        mode: UiJoinMode.confirm,
      );
      core.receiverEvents.add(request(1, 'Ana'));
      await tester.pump();
      expect(find.text('Wer darf beitreten?'), findsOneWidget);
      expect(find.text('Erst fragen'), findsOneWidget);
      expect(find.text('Ablehnen'), findsOneWidget);
      expect(find.textContaining('Halten Sie OK'), findsOneWidget);
    });

    testWidgets(
      'a server that forbids open rooms gets a room that asks first',
      (tester) async {
        final core = await openReceive(tester);
        core.receiverEvents.add(
          const ReceiverUiEvent.error(
            message: 'server error open_rooms_disabled: no',
          ),
        );
        await tester.pump();
        expect(core.receiverJoinMode, UiJoinMode.confirm);
        core.receiverEvents.add(
          const ReceiverUiEvent.roomCode(
            code: 'AB23CD',
            joinMode: UiJoinMode.confirm,
          ),
        );
        await tester.pump();
        expect(
          find.textContaining('does not allow open rooms'),
          findsOneWidget,
        );
      },
    );
  });

  group('television remote (D-pad only)', () {
    testWidgets(
      'a request does not take the focus; OK held on Allow allows, a short OK does not',
      (tester) async {
        final core = await openReceive(
          tester,
          tv: true,
          mode: UiJoinMode.confirm,
        );
        // Move the focus with the D-pad onto the mode choice.
        // Put the focus on the mode choice, then move with the D-pad only.
        tester
            .widget<ElevatedButton>(find.byKey(const Key('mode-confirm')))
            .focusNode!
            .requestFocus();
        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await tester.pump();
        expect(
          tester
              .widget<ElevatedButton>(find.byKey(const Key('mode-locked')))
              .focusNode!
              .hasPrimaryFocus,
          isTrue,
        );
        final focusedBefore = FocusManager.instance.primaryFocus;
        core.receiverEvents.add(request(1, 'Ana'));
        await tester.pump();
        expect(
          FocusManager.instance.primaryFocus,
          focusedBefore,
          reason: 'a new request never grabs the focus',
        );

        // Walk to Deny first (focus order), then to Allow with arrow keys.
        final deny = tester.widget<OutlinedButton>(
          find.byKey(Key('deny-${id(1)}')),
        );
        deny.focusNode!.requestFocus();
        await tester.pump();
        expect(deny.focusNode!.hasPrimaryFocus, isTrue);
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await tester.pump();
        final allow = tester.widget<FilledButton>(
          find.byKey(const Key('allowButton')),
        );
        expect(allow.focusNode!.hasPrimaryFocus, isTrue);

        // A short OK: only a hint.
        await tester.sendKeyDownEvent(LogicalKeyboardKey.select);
        await tester.pump(const Duration(milliseconds: 300));
        await tester.sendKeyUpEvent(LogicalKeyboardKey.select);
        await tester.pump();
        expect(core.joinDecisions, isEmpty);
        expect(find.byKey(const Key('holdHint')), findsOneWidget);

        // OK held about a second allows (key repeats do not restart the hold).
        await tester.sendKeyDownEvent(LogicalKeyboardKey.select);
        await tester.pump(const Duration(milliseconds: 400));
        await tester.sendKeyRepeatEvent(LogicalKeyboardKey.select);
        await tester.pump(const Duration(milliseconds: 400));
        // Not yet: the hold is about a second, measured from the first frame.
        expect(core.joinDecisions, isEmpty);
        await tester.pump(const Duration(milliseconds: 800));
        await tester.sendKeyUpEvent(LogicalKeyboardKey.select);
        await tester.pump();
        expect(core.joinDecisions, [(id: id(1), allow: true)]);
      },
    );

    testWidgets(
      'OK on Deny is one press; the focus then moves to the next Deny',
      (tester) async {
        final core = await openReceive(
          tester,
          tv: true,
          mode: UiJoinMode.confirm,
        );
        core.receiverEvents.add(request(1, 'Ana'));
        core.receiverEvents.add(request(2, 'Ben'));
        await tester.pump();
        tester
            .widget<OutlinedButton>(find.byKey(Key('deny-${id(1)}')))
            .focusNode!
            .requestFocus();
        await tester.pump();
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pumpAndSettle();
        expect(core.joinDecisions, [(id: id(1), allow: false)]);
        expect(
          tester
              .widget<OutlinedButton>(find.byKey(Key('deny-${id(2)}')))
              .focusNode!
              .hasPrimaryFocus,
          isTrue,
        );
      },
    );

    testWidgets('the mode can be changed with the D-pad and OK alone', (
      tester,
    ) async {
      final core = await openReceive(tester, tv: true);
      final locked = tester.widget<ElevatedButton>(
        find.byKey(const Key('mode-locked')),
      );
      locked.focusNode!.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.select);
      await tester.pump();
      expect(core.joinModeRequests, [UiJoinMode.locked]);
      // Arrow keys alone only move the focus.
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
      await tester.pump();
      expect(core.joinModeRequests, [UiJoinMode.locked]);
    });

    testWidgets(
      'while somebody presents: chip and pill; Back closes the panel without allowing',
      (tester) async {
        final core = await openReceive(
          tester,
          tv: true,
          mode: UiJoinMode.confirm,
        );
        core.receiverEvents.add(
          const ReceiverUiEvent.contentLink(
            url: 'https://example.org/',
            host: 'example.org',
          ),
        );
        await tester.pump();
        core.receiverEvents.add(request(1, 'Ana'));
        await tester.pump();
        expect(find.byKey(const Key('joinChip')), findsOneWidget);
        expect(find.text('1 waiting'), findsOneWidget);
        expect(find.byKey(const Key('joinPanel')), findsNothing);
        tester
            .widget<FilledButton>(find.byKey(const Key('joinPill')))
            .focusNode
            ?.requestFocus();
        await tester.tap(find.byKey(const Key('joinPill')));
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('joinPanel')), findsOneWidget);
        // The first thing focused is neutral: the selected mode, not Allow.
        expect(
          tester
              .widget<ElevatedButton>(find.byKey(const Key('mode-confirm')))
              .focusNode!
              .hasPrimaryFocus,
          isTrue,
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('joinPanel')), findsNothing);
        expect(core.joinDecisions, isEmpty);
      },
    );

    testWidgets('hit areas are large on a TV', (tester) async {
      final core = await openReceive(
        tester,
        tv: true,
        mode: UiJoinMode.confirm,
      );
      core.receiverEvents.add(request(1, 'Ana'));
      await tester.pump();
      expect(
        tester.getSize(find.byKey(const Key('mode-open'))).height,
        greaterThanOrEqualTo(64),
      );
      expect(
        tester.getSize(find.byKey(Key('deny-${id(1)}'))).height,
        greaterThanOrEqualTo(60),
      );
      expect(
        tester.getSize(find.byKey(const Key('allowButton'))).height,
        greaterThanOrEqualTo(60),
      );
    });
  });

  group('sender screens', () {
    Future<FakeCoreApi> openSend(
      WidgetTester tester, {
      String locale = 'en',
      AppSettings? settings,
    }) async {
      final core = FakeCoreApi();
      tester.view.physicalSize = const Size(900, 2400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        CometCastApp(locale: Locale(locale), core: core, settings: settings),
      );
      await tester.pumpAndSettle();
      await tester.tap(
        find.text(locale == 'en' ? 'Send screen' : 'Bildschirm senden'),
      );
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
      await tester.enterText(find.byKey(const Key('secondsField')), '5');
      await tester.tap(find.byKey(const Key('startButton')));
      await tester.pump();
      return core;
    }

    testWidgets(
      'waiting for approval: card with the check number and a cancel button',
      (tester) async {
        final core = await openSend(tester);
        core.senderEvents.add(
          const SenderUiEvent.waitingForApproval(timeoutSecs: 60, check: 42),
        );
        await tester.pump();
        expect(find.byKey(const Key('approvalWaitingCard')), findsOneWidget);
        expect(
          tester.widget<Text>(find.byKey(const Key('waitingCheck'))).data,
          '42',
        );
        expect(find.textContaining('Nothing is sent'), findsWidgets);
        await tester.tap(find.byKey(const Key('cancelWaitButton')));
        await tester.pump();
        expect(core.stopSenderCalls, 1);
        expect(find.byKey(const Key('approvalWaitingCard')), findsNothing);
      },
    );

    testWidgets('the display name is passed to the core', (tester) async {
      final core = FakeCoreApi();
      final settings = AppSettings.fromDefaults(
        core.serverDefaults(),
        displayName: 'Alex',
      );
      final c = await openSend(tester, settings: settings);
      expect(c.senderName, 'Alex');
    });

    testWidgets('denied: text, pause before asking again, start disabled', (
      tester,
    ) async {
      var now = DateTime(2026, 1, 1);
      joinClock = () => now;
      addTearDown(() => joinClock = DateTime.now);
      final core = await openSend(tester);
      core.senderEvents.add(
        const SenderUiEvent.waitingForApproval(timeoutSecs: 60, check: 42),
      );
      await tester.pump();
      core.senderEvents.add(
        const SenderUiEvent.joinRefused(code: 'join_denied'),
      );
      await tester.pump();
      expect(find.text('The host did not allow you to join.'), findsOneWidget);
      expect(find.byKey(const Key('retryIn')), findsOneWidget);
      expect(
        tester
            .widget<FilledButton>(find.byKey(const Key('startButton')))
            .onPressed,
        isNull,
      );
      now = now.add(const Duration(seconds: 16));
      await tester.pump(const Duration(seconds: 2));
      expect(
        tester
            .widget<FilledButton>(find.byKey(const Key('startButton')))
            .onPressed,
        isNotNull,
      );
      expect(find.byKey(const Key('retryIn')), findsNothing);
    });

    Future<void> refused(
      WidgetTester tester,
      List<SenderUiEvent> events,
      String text,
    ) async {
      final core = await openSend(tester);
      events.forEach(core.senderEvents.add);
      await tester.pump();
      expect(find.textContaining(text), findsOneWidget);
    }

    testWidgets('locked: the code was right', (tester) async {
      await refused(tester, const [
        SenderUiEvent.joinRefused(code: 'room_locked'),
      ], 'The room code is correct, but the room is locked right now.');
    });

    testWidgets('locked while waiting', (tester) async {
      await refused(tester, const [
        SenderUiEvent.waitingForApproval(timeoutSecs: 60, check: 11),
        SenderUiEvent.joinRefused(code: 'room_locked'),
      ], 'locked while you were waiting');
    });

    testWidgets('timed out', (tester) async {
      await refused(tester, const [
        SenderUiEvent.joinRefused(code: 'approval_timeout'),
      ], 'Nobody answered your request in time.');
    });

    testWidgets('removed by the host', (tester) async {
      await refused(tester, const [
        SenderUiEvent.joined(),
        SenderUiEvent.removedByHost(),
      ], 'The host removed you from the room.');
    });

    testWidgets('German (formal)', (tester) async {
      final core = await openSend(tester, locale: 'de');
      core.senderEvents.add(
        const SenderUiEvent.waitingForApproval(timeoutSecs: 60, check: 42),
      );
      await tester.pump();
      expect(find.textContaining('Ihren Beitritt'), findsWidgets);
      core.senderEvents.add(
        const SenderUiEvent.joinRefused(code: 'join_denied'),
      );
      await tester.pump();
      expect(
        find.text('Der Gastgeber hat Ihren Beitritt nicht erlaubt.'),
        findsOneWidget,
      );
    });
  });

  group('settings', () {
    test(
      'the join mode and the display name are remembered; first run is open',
      () async {
        final store = MemorySettingsStore()
          ..joinMode = 'confirm'
          ..displayName = 'A\u202Eb\u0007c  d';
        final s = await AppSettings.load(core: FakeCoreApi(), store: store);
        expect(s.joinMode, UiJoinMode.confirm);
        expect(s.displayName, 'Abc d');
        s.setJoinMode(UiJoinMode.locked);
        expect(store.joinMode, 'locked');
        final fresh = await AppSettings.load(
          core: FakeCoreApi(),
          store: MemorySettingsStore(),
        );
        expect(fresh.joinMode, UiJoinMode.open);
        expect(fresh.displayName, '');
        expect(cleanDisplayName('x' * 100).length, maxDisplayNameChars);
      },
    );
  });

  group('controller', () {
    test(
      'requests are queued in arrival order and removed when decided',
      () async {
        final core = FakeCoreApi();
        final c = ReceiverController(core: core);
        c.start('s:1', joinMode: UiJoinMode.confirm);
        core.receiverEvents.add(
          const ReceiverUiEvent.roomCode(
            code: 'K7M2QX',
            joinMode: UiJoinMode.confirm,
          ),
        );
        core.receiverEvents.add(request(1, 'Ana'));
        core.receiverEvents.add(request(2, 'Ben'));
        core.receiverEvents.add(request(1, 'Ana')); // duplicate ignored
        await Future<void>.delayed(Duration.zero);
        expect(c.requests.map((r) => r.name), ['Ana', 'Ben']);
        c.decideJoin(id(1), allow: true);
        expect(c.requests.map((r) => r.name), ['Ben']);
        expect(core.joinDecisions, [(id: id(1), allow: true)]);
        c.decideJoin(id(99), allow: true); // unknown: nothing sent
        expect(core.joinDecisions, hasLength(1));
        core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
        await Future<void>.delayed(Duration.zero);
        expect(c.requests, isEmpty);
        c.dispose();
      },
    );
  });
}
