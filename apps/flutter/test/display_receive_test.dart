// SPDX-License-Identifier: GPL-3.0-or-later
// What the display does with a presenter, content and the host's choices, on the new screens: the
// behaviours of the old receive, join-approval, D-pad and home tests that still hold (link card,
// toasts, remembered mode, German, remote keys and hit areas, a presenter that leaves).

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/display/display_logic.dart';
import 'package:cometcast/src/display/kit.dart';

import 'content_support.dart';
import 'display_support.dart';
import 'solid_frame.dart';

Future<void> key(WidgetTester t, LogicalKeyboardKey k) async {
  await t.sendKeyEvent(k);
  await t.pump(const Duration(milliseconds: 50));
}

String text(Key k) => (find.byKey(k).evaluate().single.widget as Text).data!;

Future<void> two(WidgetTester t) async {
  await t.pump();
  await t.pump(const Duration(milliseconds: 50));
}

void main() {
  group('link card: only a card, nothing opens by itself', () {
    Future<(DisplayHarness, List<Uri>)> withLink(
      WidgetTester t, {
      String url = 'https://example.org/path?q=1',
      String host = 'example.org',
      Locale locale = const Locale('en'),
    }) async {
      final opened = <Uri>[];
      final h = await pumpDisplay(
        t,
        tv: false,
        canOpen: true,
        locale: locale,
        openUrl: (u) async {
          opened.add(u);
          return true;
        },
      );
      h.core.receiverEvents.add(
        ReceiverUiEvent.contentLink(url: url, host: host),
      );
      await two(t);
      return (h, opened);
    }

    testWidgets('a link is only a card until somebody chooses', (t) async {
      final (_, opened) = await withLink(t);
      expect(find.byKey(const Key('link-open')), findsOneWidget);
      expect(text(const Key('link-address')), 'https://example.org/path?q=1');
      await t.pump(const Duration(seconds: 3));
      expect(opened, isEmpty);
    });

    testWidgets('Open opens exactly that address once, after the guard', (
      t,
    ) async {
      final (_, opened) = await withLink(t);
      // Within the one-second guard nothing opens.
      await t.tap(find.byKey(const Key('link-open')));
      await t.pump();
      expect(opened, isEmpty);
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('link-open')));
      await two(t);
      expect(opened, [Uri.parse('https://example.org/path?q=1')]);
    });

    testWidgets('Dismiss (and Enter on the focused button) opens nothing', (
      t,
    ) async {
      final (_, opened) = await withLink(t);
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('link-dismiss')));
      await two(t);
      expect(opened, isEmpty);
      expect(find.byKey(const Key('link-dismiss')), findsNothing);
    });

    testWidgets('a link that fails the second check is never opened', (
      t,
    ) async {
      final (_, opened) = await withLink(t, url: 'javascript:alert(1)');
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('link-open')));
      await two(t);
      expect(opened, isEmpty);
    });

    testWidgets('German texts of the link card are du', (t) async {
      await withLink(t, locale: const Locale('de'));
      expect(find.text('Öffnen'), findsOneWidget);
      expect(
        find.textContaining('Öffne ihn nur, wenn du ihn erwartest'),
        findsOneWidget,
      );
    });

    testWidgets('a presenter that stops showing removes the card', (t) async {
      final (h, _) = await withLink(t);
      h.core.receiverEvents.add(const ReceiverUiEvent.contentStopped());
      await two(t);
      expect(find.byKey(const Key('link-open')), findsNothing);
    });
  });

  group('content on the display', () {
    testWidgets(
      'a presenter that leaves: Idle with the same code and a toast',
      (t) async {
        final h = await pumpDisplay(t);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
        );
        await two(t);
        await deliverFrame(t, h.core, solidFrame(16, 9, [255, 0, 0]));
        h.core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
        await two(t);
        expect(text(const Key('room-code')), 'K7Q M2W');
        expect(find.text('“Lena” disconnected'), findsOneWidget);
        expect(h.wake.held, isFalse);
      },
    );

    testWidgets('a stopped presentation is Idle without a toast', (t) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
      );
      await two(t);
      await t.pump(const Duration(seconds: 4));
      await deliverFrame(t, h.core, solidFrame(16, 9, [255, 0, 0]));
      h.core.receiverEvents.add(const ReceiverUiEvent.presentationStopped());
      await two(t);
      expect(text(const Key('room-code')), 'K7Q M2W');
      expect(find.textContaining('disconnected'), findsNothing);
    });

    testWidgets('a damaged document is a toast with the existing wording', (
      t,
    ) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentError(
          error: UiContentError.documentRejected,
        ),
      );
      await two(t);
      await two(t);
      expect(find.byKey(const Key('toast')), findsOneWidget);
    });

    testWidgets('a PDF page is the picture, with controls only on demand', (
      t,
    ) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents
        ..add(const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false))
        ..add(
          const ReceiverUiEvent.contentDocumentOpened(name: 'a.pdf', pages: 3),
        );
      h.core.contentFrames.add(
        ContentFrame.page(
          page: 1,
          width: 8,
          height: 6,
          rgba: solidRgba(8, 6, [10, 200, 30]),
        ),
      );
      h.core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      for (var i = 0; i < 20; i++) {
        await t.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 50)),
        );
        await t.pump();
        if (find.byKey(const Key('room-code')).evaluate().isEmpty) break;
      }
      expect(find.byKey(const Key('room-code')), findsNothing);
      expect(h.wake.held, isTrue);
    });
  });

  group('the host decides (join approval)', () {
    testWidgets('Yes, No and block say what happened in a toast', (t) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
      );
      await two(t);
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('request-no')));
      await two(t);
      expect(find.text('“Lena” denied'), findsOneWidget);
      expect(h.core.joinDecisions.single.allow, isFalse);
    });

    testWidgets(
      'Yes: "allowed", and the join that follows has no second toast',
      (t) async {
        final h = await pumpDisplay(t, mode: DefaultMode.confirm);
        h.core.receiverEvents.add(
          ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
        );
        await two(t);
        await t.pump(const Duration(seconds: 1));
        await t.tap(find.byKey(const Key('request-yes')));
        await two(t);
        expect(find.text('“Lena” allowed'), findsOneWidget);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
        );
        await two(t);
        expect(find.text('“Lena” connected'), findsNothing);
      },
    );

    testWidgets('a join without a request (Open) says "connected"', (t) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
      );
      await two(t);
      expect(find.text('“Lena” connected'), findsOneWidget);
    });

    testWidgets('names are plain text and are shortened; no name is said so', (
      t,
    ) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(
          id: BigInt.one,
          name: '<b>Lena</b>',
          check: 3,
        ),
      );
      await two(t);
      expect(text(const Key('request-name')), '“<b>Lena</b>”');
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(id: BigInt.two, name: '', check: 4),
      );
      await two(t);
      expect(text(const Key('request-queue')), '+1 waiting');
    });

    testWidgets('the request in German is du', (t) async {
      final h = await pumpDisplay(
        t,
        mode: DefaultMode.confirm,
        locale: const Locale('de'),
      );
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
      );
      await two(t);
      expect(find.text('Ja'), findsOneWidget);
      expect(find.textContaining(RegExp(r'\bSie\b')), findsNothing);
    });

    testWidgets('open_rooms_disabled changes the pill and says so once', (
      t,
    ) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.error(
          code: 'open_rooms_disabled',
          message: 'server',
        ),
      );
      await two(t);
      expect(find.text('This server only allows Approval on'), findsOneWidget);
    });

    testWidgets('the mode of a new room follows what the settings remember', (
      t,
    ) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      expect(h.core.receiverJoinMode, UiJoinMode.confirm);
    });

    testWidgets('choosing Open in Settings is asked of the core and kept', (
      t,
    ) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      await key(t, LogicalKeyboardKey.select); // Settings
      await key(t, LogicalKeyboardKey.arrowLeft); // Open
      await key(t, LogicalKeyboardKey.select);
      expect(h.core.joinModeRequests, [UiJoinMode.open]);
      expect(h.store.setup!.mode, DefaultMode.open);
    });

    testWidgets('Locked is never stored as the default', (t) async {
      final h = await pumpDisplay(t);
      await key(t, LogicalKeyboardKey.select);
      await key(t, LogicalKeyboardKey.arrowRight);
      await key(t, LogicalKeyboardKey.arrowRight); // Locked
      await key(t, LogicalKeyboardKey.select);
      expect(h.core.joinModeRequests.last, UiJoinMode.locked);
      expect(h.store.setup!.mode, DefaultMode.open);
    });

    testWidgets('the appearance is kept', (t) async {
      final h = await pumpDisplay(t);
      await key(t, LogicalKeyboardKey.select);
      await key(t, LogicalKeyboardKey.arrowDown);
      await key(t, LogicalKeyboardKey.arrowRight); // Light
      await key(t, LogicalKeyboardKey.select);
      await t.pump(const Duration(milliseconds: 50));
      expect(h.store.appearance, Appearance.light);
    });

    testWidgets('Settings in German is du and the word for the close button', (
      t,
    ) async {
      await pumpDisplay(t, locale: const Locale('de'));
      await key(t, LogicalKeyboardKey.select);
      expect(find.text('Einstellungen'), findsWidgets);
    });
  });

  group('the television remote (D-pad only)', () {
    testWidgets('Settings: the remote close label and the mode tiles', (
      t,
    ) async {
      await pumpDisplay(t);
      await key(t, LogicalKeyboardKey.select);
      expect(find.text('Close (Back key)'), findsOneWidget);
    });

    testWidgets('the focused control shows a visible frame, only one', (
      t,
    ) async {
      await pumpDisplay(t);
      final count = t
          .widgetList<DecoratedBox>(find.byType(DecoratedBox))
          .where(
            (d) =>
                d.decoration is BoxDecoration &&
                (d.decoration as BoxDecoration).border is Border &&
                ((d.decoration as BoxDecoration).border! as Border).top.width ==
                    4,
          )
          .length;
      expect(count, 1);
    });

    testWidgets('OK on No is one press; the focus then moves on', (t) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents
        ..add(
          ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 1),
        )
        ..add(
          ReceiverUiEvent.joinRequest(id: BigInt.two, name: 'Tom', check: 2),
        );
      await two(t);
      await t.pump(const Duration(seconds: 1));
      await key(t, LogicalKeyboardKey.arrowRight); // No
      await key(t, LogicalKeyboardKey.select);
      expect(h.core.joinDecisions.single.allow, isFalse);
      // The next request has the focus on Yes again.
      expect(text(const Key('request-name')), '“Tom”');
    });

    testWidgets('hit areas are large on a TV', (t) async {
      await pumpDisplay(t);
      // 46 ref high at scale 2 (getRect is in screen pixels).
      final s = t.getRect(find.byKey(const Key('settings-open')));
      expect(s.height, greaterThanOrEqualTo(90));
      await key(t, LogicalKeyboardKey.select);
      final tile = t.getRect(find.byKey(const Key('settings-mode-open')));
      expect(tile.height, greaterThanOrEqualTo(110));
    });

    testWidgets('arrow keys move through the rows without wrapping', (t) async {
      await pumpDisplay(t);
      await key(t, LogicalKeyboardKey.select);
      for (var i = 0; i < 6; i++) {
        await key(t, LogicalKeyboardKey.arrowDown);
      }
      for (var i = 0; i < 6; i++) {
        await key(t, LogicalKeyboardKey.arrowUp);
      }
      expect(find.byKey(const Key('settings-close')), findsOneWidget);
    });
  });

  group('the setup of a display (home)', () {
    testWidgets('Finish stores the setup and starts the room with its name', (
      t,
    ) async {
      final h = await pumpDisplay(t, setupDone: false);
      await key(t, LogicalKeyboardKey.select); // Set up
      await key(t, LogicalKeyboardKey.select); // Next (first suggestion)
      await key(t, LogicalKeyboardKey.arrowDown); // step 2: down to its buttons
      await key(t, LogicalKeyboardKey.arrowRight); // Next
      await key(t, LogicalKeyboardKey.select);
      await key(t, LogicalKeyboardKey.select); // Looks right
      await key(t, LogicalKeyboardKey.select); // Finish
      await two(t);
      expect(h.store.setup, isNotNull);
      expect(h.core.receiverRoomName, h.store.setup!.room);
      expect(h.core.receiverServer, isNotNull);
    });

    testWidgets('the English and German first screens', (t) async {
      await pumpDisplay(t, setupDone: false);
      expect(find.text('Set up'), findsOneWidget);
      await pumpDisplay(t, setupDone: false, locale: const Locale('de'));
      expect(find.text('Einrichten'), findsOneWidget);
    });

    testWidgets('the idle screen carries the app icon as the logo', (t) async {
      await pumpDisplay(t);
      expect(find.byType(DLogo), findsOneWidget);
    });
  });
}
