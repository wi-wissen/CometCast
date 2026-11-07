// SPDX-License-Identifier: GPL-3.0-or-later
// The display screens of docs/design/first-run-and-display.md section 4, with a fake core.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/display/display_logic.dart';

import 'display_support.dart';
import 'solid_frame.dart';

Future<void> key(WidgetTester t, LogicalKeyboardKey k) async {
  await t.sendKeyEvent(k);
  await t.pump(const Duration(milliseconds: 50));
}

Future<void> back(WidgetTester t) async {
  await t.binding.handlePopRoute();
  await t.pump(const Duration(milliseconds: 50));
}

String text(Key k) => (find.byKey(k).evaluate().single.widget as Text).data!;

double controlsOpacity(WidgetTester t) => t
    .widget<AnimatedOpacity>(
      find.ancestor(
        of: find.byKey(const Key('byline')),
        matching: find.byType(AnimatedOpacity),
      ),
    )
    .opacity;

void main() {
  group('setup wizard (4.1)', () {
    testWidgets('welcome, name, mode, address, done with the D-pad', (t) async {
      final h = await pumpDisplay(t, setupDone: false);
      expect(find.text('Set up'), findsOneWidget);
      await key(t, LogicalKeyboardKey.select); // Start has the initial focus
      expect(find.text('What is this room called?'), findsOneWidget);
      expect(find.text('Step 1 of 4'), findsOneWidget);
      expect(find.text('Meeting room Elbe'), findsOneWidget);
      await key(t, LogicalKeyboardKey.select); // Next has the initial focus
      expect(find.text('Who may connect?'), findsOneWidget);
      // Back (Escape / remote Back) goes one step back.
      await key(t, LogicalKeyboardKey.escape);
      expect(find.text('What is this room called?'), findsOneWidget);
      await key(t, LogicalKeyboardKey.select);
      // Open is the initial focus; move to Approval on, choose it, go down to Next.
      await key(t, LogicalKeyboardKey.arrowRight);
      await key(t, LogicalKeyboardKey.select);
      await key(t, LogicalKeyboardKey.arrowDown);
      await key(t, LogicalKeyboardKey.arrowRight);
      await key(t, LogicalKeyboardKey.select);
      expect(find.text('Which address will others use?'), findsOneWidget);
      expect(find.byKey(const Key('wz-domain')), findsOneWidget);
      await key(t, LogicalKeyboardKey.select); // Looks right (initial focus)
      expect(find.text('Done.'), findsOneWidget);
      expect(find.textContaining('Menu key'), findsOneWidget);
      await key(t, LogicalKeyboardKey.select);
      await t.pump(const Duration(milliseconds: 50));
      expect(h.store.setup?.room, 'Meeting room Elbe');
      expect(h.store.setup?.mode, DefaultMode.confirm);
      expect(h.core.receiverServer, isNotNull);
      expect(find.byKey(const Key('settings-open')), findsOneWidget);
    });

    testWidgets('pointer variant of the last text, tablets and computers', (
      t,
    ) async {
      await pumpDisplay(t, setupDone: false, tv: false);
      // A tablet or computer starts as a presenter; the link starts the setup.
      await t.tap(find.text('Use this device as the screen'));
      for (var i = 0; i < 5; i++) {
        await t.pump(const Duration(milliseconds: 50));
      }
      await t.tap(find.text('Set up'));
      await t.pump();
      await t.tap(find.text('Next'));
      await t.pump();
      await t.tap(find.text('Next'));
      await t.pump();
      await t.tap(find.text('Looks right'));
      await t.pump();
      expect(
        find.textContaining('“Settings” at the bottom right'),
        findsOneWidget,
      );
    });
  });

  group('idle (4.2)', () {
    testWidgets('address, code, QR and the Settings button', (t) async {
      final h = await pumpDisplay(t);
      expect(text(const Key('room-name')), 'Meeting room Elbe');
      expect(text(const Key('address')), 'cast.example.org');
      expect(text(const Key('room-code')), 'K7Q M2W');
      expect(find.byKey(const Key('qr')), findsOneWidget);
      expect(find.text('Or scan'), findsOneWidget);
      expect(find.text('Open'), findsOneWidget); // the pill
      expect(find.text('Settings'), findsOneWidget);
      // No wake lock on Idle.
      expect(h.wake.held, isFalse);
    });

    testWidgets('connecting is empty, offline comes after 8 s', (t) async {
      final h = await pumpDisplay(t, start: false);
      expect(text(const Key('room-code')).trim(), isEmpty);
      expect(find.byKey(const Key('offline')), findsNothing);
      await t.pump(const Duration(seconds: 9));
      expect(find.byKey(const Key('offline')), findsOneWidget);
      expect(find.text('No connection to cast.example.org'), findsOneWidget);
      expect(h.core.receiverServer, 'cast.example.org');
    });

    testWidgets('a lost connection shows the box only after 3 s', (t) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.error(message: 'signalling connection closed'),
      );
      await t.pump();
      await t.pump(const Duration(seconds: 1));
      expect(find.byKey(const Key('offline')), findsNothing);
      await t.pump(const Duration(seconds: 3));
      expect(find.byKey(const Key('offline')), findsOneWidget);
    });

    testWidgets('locked: note and caption', (t) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.locked),
      );
      await t.pump();
      expect(find.text('Locked right now.'), findsOneWidget);
      expect(find.text('Locked'), findsNWidgets(2)); // pill and QR caption
    });

    testWidgets('dims after 7 minutes and a key press only restores', (
      t,
    ) async {
      await pumpDisplay(t);
      double dim() =>
          t.widget<AnimatedOpacity>(find.byKey(const Key('dim'))).opacity;
      expect(dim(), 0);
      await t.pump(const Duration(minutes: 7, seconds: 1));
      expect(dim(), .6);
      await key(t, LogicalKeyboardKey.select); // would open Settings
      expect(dim(), 0);
      expect(find.byKey(const Key('settings-close')), findsNothing);
    });
  });

  group('Settings (4.3)', () {
    testWidgets(
      'opened by the focused button, changes at once, closed by Back',
      (t) async {
        final h = await pumpDisplay(t);
        await key(t, LogicalKeyboardKey.select);
        expect(find.byKey(const Key('settings-close')), findsOneWidget);
        expect(find.text('Close (Back key)'), findsOneWidget);
        // Initial focus: the tile of the current mode (Open); Approval on is next to it.
        await key(t, LogicalKeyboardKey.arrowRight);
        await key(t, LogicalKeyboardKey.select);
        expect(h.core.joinModeRequests, [UiJoinMode.confirm]);
        expect(h.store.setup?.mode, DefaultMode.confirm);
        // Appearance: the row below, to Light.
        await key(t, LogicalKeyboardKey.arrowDown);
        await key(t, LogicalKeyboardKey.select);
        expect(h.store.appearance, Appearance.light);
        // Locked is not stored.
        await key(t, LogicalKeyboardKey.arrowUp);
        await key(t, LogicalKeyboardKey.arrowRight);
        await key(t, LogicalKeyboardKey.select);
        expect(h.core.joinModeRequests.last, UiJoinMode.locked);
        expect(h.store.setup?.mode, DefaultMode.confirm);
        await back(t);
        expect(find.byKey(const Key('settings-close')), findsNothing);
      },
    );

    testWidgets('focus never wraps', (t) async {
      await pumpDisplay(t);
      await key(t, LogicalKeyboardKey.select);
      await key(t, LogicalKeyboardKey.arrowLeft); // already on Open
      await key(t, LogicalKeyboardKey.arrowLeft);
      await key(t, LogicalKeyboardKey.select); // Open again: still open
      expect(find.byKey(const Key('settings-close')), findsOneWidget);
    });

    testWidgets('the Menu key opens and closes it', (t) async {
      await pumpDisplay(t);
      await key(t, LogicalKeyboardKey.contextMenu);
      expect(find.byKey(const Key('settings-close')), findsOneWidget);
      await key(t, LogicalKeyboardKey.contextMenu);
      expect(find.byKey(const Key('settings-close')), findsNothing);
    });
  });

  group('join request (4.4)', () {
    testWidgets('guard of 1 s, queue, Yes, No and block', (t) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents
        ..add(
          ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
        )
        ..add(
          ReceiverUiEvent.joinRequest(id: BigInt.two, name: 'Tom', check: 17),
        );
      await t.pump();
      expect(text(const Key('request-name')), '“Lena”');
      expect(text(const Key('request-check')), '42');
      expect(text(const Key('request-queue')), '+1 waiting');
      // Within the guard a press decides nothing.
      await key(t, LogicalKeyboardKey.select);
      expect(h.core.joinDecisions, isEmpty);
      await t.pump(const Duration(seconds: 1));
      await key(t, LogicalKeyboardKey.select);
      expect(h.core.joinDecisions.single.allow, isTrue);
      // The next request takes over, with a new guard.
      expect(text(const Key('request-name')), '“Tom”');
      await key(t, LogicalKeyboardKey.select);
      expect(h.core.joinDecisions.length, 1);
      await t.pump(const Duration(seconds: 1));
      await key(t, LogicalKeyboardKey.arrowRight);
      await key(t, LogicalKeyboardKey.arrowRight);
      await key(t, LogicalKeyboardKey.select); // No and block
      expect(h.core.blockedRequests, [BigInt.two]);
      expect(find.byKey(const Key('request-yes')), findsNothing);
    });

    testWidgets('Back does nothing and a request closes Settings', (t) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      await key(t, LogicalKeyboardKey.select);
      expect(find.byKey(const Key('settings-close')), findsOneWidget);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
      );
      await t.pump();
      expect(find.byKey(const Key('settings-close')), findsNothing);
      await back(t);
      expect(find.byKey(const Key('request-yes')), findsOneWidget);
    });

    testWidgets('long names are cut', (t) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(
          id: BigInt.one,
          name: 'Alexandria Montgomery-Smythe',
          check: 5,
        ),
      );
      await t.pump();
      expect(text(const Key('request-name')), '“Alexandria Montgom…”');
    });

    testWidgets('an expired request leaves with a toast', (t) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
      );
      await t.pump();
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequestCancelled(id: BigInt.one, reason: 'timeout'),
      );
      await t.pump();
      expect(find.text('Request from “Lena” expired'), findsOneWidget);
      await t.pump(const Duration(milliseconds: 3300));
      expect(find.byKey(const Key('toast')), findsNothing);
    });
  });

  group('presenting (4.5)', () {
    testWidgets(
      'quiet at rest, reveal on a key, End, wake lock only while presenting',
      (t) async {
        final h = await pumpDisplay(t);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Lena'),
        );
        await t.pump();
        expect(h.wake.held, isFalse);
        await deliverFrame(t, h.core, solidFrame(16, 9, [255, 0, 0]));
        expect(h.wake.held, isTrue);
        expect(find.byKey(const Key('room-code')), findsNothing);
        expect(controlsOpacity(t), 0);
        // The first key only reveals; it activates nothing.
        await key(t, LogicalKeyboardKey.select);
        expect(h.core.endRoomCalls, 0);
        expect(controlsOpacity(t), 1);
        expect(find.text('“Lena” is showing'), findsOneWidget);
        // Controls hide after 5 s.
        await t.pump(const Duration(seconds: 5, milliseconds: 100));
        expect(controlsOpacity(t), 0);
        // An arrow shows them with End focused; select ends the session.
        await key(t, LogicalKeyboardKey.arrowDown);
        await t.pump(const Duration(milliseconds: 100));
        await key(t, LogicalKeyboardKey.select);
        expect(h.core.endRoomCalls, 1);
        h.core.receiverEvents.add(const ReceiverUiEvent.roomEnded());
        await t.pump();
        expect(find.text('Session ended · new code'), findsOneWidget);
      },
    );

    testWidgets('Back shows the controls, then hides them', (t) async {
      final h = await pumpDisplay(t);
      await deliverFrame(t, h.core, solidFrame(16, 9, [0, 255, 0]));
      await back(t);
      expect(controlsOpacity(t), 1);
      await back(t);
      expect(controlsOpacity(t), 0);
    });
  });

  group('link card (4.5)', () {
    testWidgets('Back = Dismiss', (t) async {
      final h = await pumpDisplay(t, tv: false);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/x',
          host: 'example.org',
        ),
      );
      await t.pump();
      expect(find.text('EXAMPLE.ORG WANTS TO OPEN A LINK'), findsOneWidget);
      await key(t, LogicalKeyboardKey.escape);
      expect(find.byKey(const Key('link-dismiss')), findsNothing);
    });
  });

  group('lifecycle', () {
    testWidgets('going to the background ends the session without a toast', (
      t,
    ) async {
      final h = await pumpDisplay(t);
      t.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
      await t.pump();
      expect(h.core.endRoomCalls, 1);
      h.core.receiverEvents.add(const ReceiverUiEvent.roomEnded());
      await t.pump();
      expect(find.byKey(const Key('toast')), findsNothing);
    });
  });
}
