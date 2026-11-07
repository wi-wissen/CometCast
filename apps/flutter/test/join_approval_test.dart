// SPDX-License-Identifier: GPL-3.0-or-later
// Join approval with a fake core: the host's panel (mode, requests, hold-to-allow, kick), the
// television profile (D-pad / remote keys only), and the sender's waiting and refusal screens.

import 'package:flutter/gestures.dart' show PointerDeviceKind;
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

/// A test clock for the join screens (the guard of 700 ms and the countdown use it).
class FakeClock {
  DateTime now = DateTime(2026, 1, 1, 12);
  void install(WidgetTester tester) {
    joinClock = () => now;
    addTearDown(() => joinClock = DateTime.now);
  }

  /// Moves the test clock and the widget clock together.
  Future<void> advance(WidgetTester tester, Duration d) async {
    now = now.add(d);
    await tester.pump(d);
  }
}

void main() {
  group('request takeover and host menu (desktop)', () {
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
      'a request takes over the display as a large dialog; No has the focus',
      (tester) async {
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        expect(find.byKey(const Key('joinTakeover')), findsNothing);
        core.receiverEvents.add(request(1, 'Ana'));
        await tester.pump();
        await tester.pump();
        expect(find.byKey(const Key('joinTakeover')), findsOneWidget);
        expect(find.text('Someone wants to join'), findsOneWidget);
        expect(find.text('\u201CAna\u201D'), findsOneWidget);
        expect(
          tester.widget<Text>(find.byKey(Key('requestCheck-${id(1)}'))).data,
          '41',
        );
        // The safe choice has the focus, never "Yes".
        final no = tester.widget<OutlinedButton>(
          find.byKey(Key('deny-${id(1)}')),
        );
        expect(no.focusNode!.hasPrimaryFocus, isTrue);
      },
    );

    testWidgets(
      'presses right after the request appeared are ignored (700 ms guard)',
      (tester) async {
        final clock = FakeClock()..install(tester);
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        core.receiverEvents.add(request(1, 'Ana'));
        await tester.pump();
        await clock.advance(tester, const Duration(milliseconds: 300));
        await tester.tap(find.byKey(Key('allow-${id(1)}')));
        await tester.tap(find.byKey(Key('deny-${id(1)}')));
        await tester.tap(find.byKey(Key('denyBlock-${id(1)}')));
        await tester.pump();
        expect(core.joinDecisions, isEmpty);
        expect(core.blockedRequests, isEmpty);
        await clock.advance(tester, const Duration(milliseconds: 500));
        await tester.tap(find.byKey(Key('allow-${id(1)}')));
        await tester.pump();
        expect(core.joinDecisions, [(id: id(1), allow: true)]);
      },
    );

    testWidgets(
      'No, Yes and "No and block" go to the core; after a decision the next No is focused',
      (tester) async {
        final clock = FakeClock()..install(tester);
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        core.receiverEvents.add(request(1, 'Ana'));
        core.receiverEvents.add(request(2, 'Ben'));
        core.receiverEvents.add(request(3, 'Cy'));
        await tester.pump();
        await clock.advance(tester, const Duration(seconds: 1));
        await tester.tap(find.byKey(Key('deny-${id(1)}')));
        await tester.pumpAndSettle();
        expect(core.joinDecisions, [(id: id(1), allow: false)]);
        expect(
          tester
              .widget<OutlinedButton>(find.byKey(Key('deny-${id(2)}')))
              .focusNode!
              .hasPrimaryFocus,
          isTrue,
        );
        await tester.tap(find.byKey(Key('denyBlock-${id(2)}')));
        await tester.pumpAndSettle();
        expect(core.blockedRequests, [id(2)]);
        expect(core.joinDecisions, hasLength(1));
        await tester.tap(find.byKey(Key('allow-${id(3)}')));
        await tester.pumpAndSettle();
        expect(core.joinDecisions.last, (id: id(3), allow: true));
      },
    );

    testWidgets(
      'Later (and Escape) hide the takeover; the request stays in the menu',
      (tester) async {
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        core.receiverEvents.add(request(1, 'Ana'));
        await tester.pump();
        await tester.pump();
        await tester.tap(find.byKey(const Key('laterButton')));
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('joinTakeover')), findsNothing);
        expect(core.joinDecisions, isEmpty);
        // It is listed in the menu with the same buttons.
        expect(find.byKey(Key('listed-${id(1)}')), findsOneWidget);
        core.receiverEvents.add(request(2, 'Ben'));
        await tester.pump();
        await tester.pump();
        expect(find.byKey(const Key('joinTakeover')), findsOneWidget);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('joinTakeover')), findsNothing);
        expect(core.joinDecisions, isEmpty);
      },
    );

    testWidgets('more requests are counted on the takeover; Deny all', (
      tester,
    ) async {
      final clock = FakeClock()..install(tester);
      final core = await openReceive(tester, mode: UiJoinMode.confirm);
      for (var n = 1; n <= 3; n++) {
        core.receiverEvents.add(request(n, 'P$n'));
      }
      await tester.pump();
      await tester.pump();
      expect(find.text('+2 more waiting'), findsOneWidget);
      await clock.advance(tester, const Duration(seconds: 1));
      await tester.tap(find.byKey(const Key('denyAllButton')));
      await tester.pumpAndSettle();
      expect(core.joinDecisions, [(id: null, allow: false)]);
      expect(find.byKey(const Key('joinTakeover')), findsNothing);
    });

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
      await tester.pump();
      expect(
        find.text('Room locked. 2 waiting request(s) were discarded.'),
        findsOneWidget,
      );
      expect(find.byKey(const Key('joinNobodyWaiting')), findsOneWidget);
      expect(find.byKey(const Key('joinTakeover')), findsNothing);
      await tester.tap(find.byKey(const Key('joinNoteDismiss')));
      await tester.pump();
      expect(find.byKey(const Key('joinNote')), findsNothing);
    });

    testWidgets(
      'the connected sender: end the presentation, end and block, and the block list',
      (tester) async {
        final core = await openReceive(tester);
        core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Cy'));
        await tester.pump();
        await tester.pump();
        expect(find.text('\u201CCy\u201D'), findsOneWidget);
        await tester.tap(find.byKey(const Key('removePeerButton')));
        await tester.tap(find.byKey(const Key('removeBlockButton')));
        expect(core.removePeerBlock, [false, true]);
        core.receiverEvents.add(ReceiverUiEvent.blocked(id: id(7), name: 'Cy'));
        await tester.pump();
        expect(find.byKey(Key('blocked-${id(7)}')), findsOneWidget);
        await tester.tap(find.byKey(Key('unblock-${id(7)}')));
        expect(core.unblocked, [id(7)]);
        core.receiverEvents.add(ReceiverUiEvent.unblocked(id: id(7)));
        await tester.pump();
        expect(find.byKey(Key('blocked-${id(7)}')), findsNothing);
      },
    );

    testWidgets(
      'End session needs a second press, then a new room starts in the same mode',
      (tester) async {
        final core = await openReceive(tester, mode: UiJoinMode.confirm);
        await tester.tap(find.byKey(const Key('endSessionButton')));
        await tester.pump();
        expect(core.endRoomCalls, 0);
        expect(find.text('Press again to end the session'), findsOneWidget);
        await tester.tap(find.byKey(const Key('endSessionButton')));
        await tester.pump();
        expect(core.endRoomCalls, 1);
        // The core ends the stream with roomEnded; the controller asks for a new room.
        core.receiverServer = null;
        core.receiverEvents.add(const ReceiverUiEvent.roomEnded());
        await tester.pump();
        expect(core.receiverServer, isNotNull);
        expect(core.receiverJoinMode, UiJoinMode.confirm);
        core.receiverEvents.add(
          const ReceiverUiEvent.roomCode(
            code: 'NEW234',
            joinMode: UiJoinMode.confirm,
          ),
        );
        await tester.pump();
        expect(find.text('NEW234'), findsOneWidget);
      },
    );

    testWidgets('names are plain text and are shortened', (tester) async {
      final core = await openReceive(tester, mode: UiJoinMode.confirm);
      core.receiverEvents.add(request(1, '<b>Alexandra-Maria Schmidt</b>'));
      await tester.pump();
      await tester.pump();
      expect(find.text('\u201C<b>Alexandra-Maria\u2026\u201D'), findsOneWidget);
      expect(
        shortJoinName('abcdefghijklmnopqrstuvwxyz'),
        'abcdefghijklmnopqr\u2026',
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
      await tester.pump();
      expect(find.text('Jemand möchte beitreten'), findsOneWidget);
      expect(find.text('Nein'), findsOneWidget);
      expect(find.text('Ja'), findsOneWidget);
      expect(find.text('Nein und blockieren'), findsOneWidget);
      expect(find.text('Wer darf beitreten?'), findsOneWidget);
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

  group('while somebody presents', () {
    Future<FakeCoreApi> presenting(
      WidgetTester tester, {
      bool tv = false,
    }) async {
      final core = await openReceive(tester, tv: tv, mode: UiJoinMode.confirm);
      core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/',
          host: 'example.org',
        ),
      );
      await tester.pump();
      await tester.pump();
      return core;
    }

    testWidgets('the join chip appears when something moves and fades out', (
      tester,
    ) async {
      await presenting(tester);
      double opacity() => tester
          .widget<AnimatedOpacity>(find.byKey(const Key('joinChipFade')))
          .opacity;
      expect(opacity(), 0);
      final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await gesture.addPointer(location: const Offset(100, 100));
      await gesture.moveTo(const Offset(120, 120));
      await tester.pump();
      expect(opacity(), 1);
      await tester.pump(const Duration(seconds: 7));
      expect(opacity(), 0);
      // Any key reveals it as well.
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
      expect(opacity(), 1);
      await tester.pump(const Duration(seconds: 7));
    });

    testWidgets(
      'a request shows as a banner; Back closes the menu without deciding',
      (tester) async {
        final core = await presenting(tester, tv: true);
        core.receiverEvents.add(request(1, 'Ana'));
        await tester.pump();
        await tester.pump();
        expect(find.byKey(const Key('joinTakeover')), findsOneWidget);
        expect(find.text('Someone wants to join'), findsOneWidget);
        // Escape (Back) hides it without deciding.
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('joinTakeover')), findsNothing);
        expect(core.joinDecisions, isEmpty);
        // The chip opens the menu in a dialog; its first focus is neutral; Back closes it.
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pump();
        await tester.tap(find.byKey(const Key('joinChip')));
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('joinPanel')), findsOneWidget);
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
  });

  group('television remote (D-pad only)', () {
    testWidgets(
      'a request takes over; OK on the focused No answers; the guard blocks an early OK',
      (tester) async {
        final clock = FakeClock()..install(tester);
        final core = await openReceive(
          tester,
          tv: true,
          mode: UiJoinMode.confirm,
        );
        core.receiverEvents.add(request(1, 'Ana'));
        await tester.pump();
        await tester.pump();
        expect(
          tester
              .widget<OutlinedButton>(find.byKey(Key('deny-${id(1)}')))
              .focusNode!
              .hasPrimaryFocus,
          isTrue,
        );
        // An OK press right after the request appeared does nothing.
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pump();
        expect(core.joinDecisions, isEmpty);
        await clock.advance(tester, const Duration(milliseconds: 800));
        // Move right to Yes with the D-pad, then OK.
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
        await tester.pump();
        expect(
          tester
              .widget<FilledButton>(find.byKey(Key('allow-${id(1)}')))
              .focusNode
              ?.hasPrimaryFocus,
          isNot(false),
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.select);
        await tester.pumpAndSettle();
        expect(core.joinDecisions, [(id: id(1), allow: true)]);
      },
    );

    testWidgets('OK on No is one press; the focus then moves to the next No', (
      tester,
    ) async {
      final clock = FakeClock()..install(tester);
      final core = await openReceive(
        tester,
        tv: true,
        mode: UiJoinMode.confirm,
      );
      core.receiverEvents.add(request(1, 'Ana'));
      core.receiverEvents.add(request(2, 'Ben'));
      await tester.pump();
      await tester.pump();
      await clock.advance(tester, const Duration(seconds: 1));
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
    });

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

    testWidgets('hit areas are large on a TV', (tester) async {
      final clock = FakeClock()..install(tester);
      final core = await openReceive(
        tester,
        tv: true,
        mode: UiJoinMode.confirm,
      );
      core.receiverEvents.add(request(1, 'Ana'));
      await tester.pump();
      await tester.pump();
      await clock.advance(tester, const Duration(seconds: 1));
      expect(
        tester.getSize(find.byKey(const Key('mode-open'))).height,
        greaterThanOrEqualTo(64),
      );
      expect(
        tester.getSize(find.byKey(Key('deny-${id(1)}'))).height,
        greaterThanOrEqualTo(60),
      );
      expect(
        tester.getSize(find.byKey(Key('allow-${id(1)}'))).height,
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
