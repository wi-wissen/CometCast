// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests for "Send file or link" with a fake core: picking, checking, progress,
// presenter controls, refusals.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/content_sender_controller.dart';
import 'package:cometcast/main.dart';

import 'fake_core.dart';

class Harness {
  Harness(this.core);

  final FakeCoreApi core;

  /// What the (fake) file dialog answers; null = the person cancelled.
  String? picked = '/home/user/Documents/talk.pdf';
  final pickerLabels = <String>[];
}

Future<void> flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
}

Future<Harness> openSend(WidgetTester tester, {String locale = 'en'}) async {
  final core = FakeCoreApi();
  final h = Harness(core);
  tester.view.physicalSize = const Size(900, 2000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    CometCastApp(
      locale: Locale(locale),
      core: core,
      pickFile: (label) async {
        h.pickerLabels.add(label);
        return h.picked;
      },
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(
    find.text(locale == 'en' ? 'Send file or link' : 'Datei oder Link senden'),
  );
  await tester.pumpAndSettle();
  return h;
}

Future<void> chooseFileAndStart(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('pickFileButton')));
  await flush(tester);
  await tester.enterText(find.byKey(const Key('codeField')), 'k7m2qx');
  await tester.tap(find.byKey(const Key('startButton')));
  await flush(tester);
}

/// Events of a PDF with [pages] pages that was delivered.
void deliverPdf(FakeCoreApi core, {int? pages = 5, int size = 2048}) {
  core.contentSenderEvents
    ..add(
      ContentSenderUiEvent.prepared(
        name: 'talk.pdf',
        kind: UiContentKind.pdf,
        size: BigInt.from(size),
        pages: pages,
      ),
    )
    ..add(const ContentSenderUiEvent.joined())
    ..add(const ContentSenderUiEvent.connected())
    ..add(const ContentSenderUiEvent.sent());
}

void main() {
  testWidgets('the home row opens the screen (no "coming soon")', (
    tester,
  ) async {
    await openSend(tester);
    expect(find.text('Coming soon'), findsNothing);
    expect(find.byKey(const Key('pickFileButton')), findsOneWidget);
    expect(find.byKey(const Key('codeField')), findsOneWidget);
    expect(find.text('No file chosen.'), findsOneWidget);
  });

  group('file', () {
    testWidgets('choose, check the code, send', (tester) async {
      final h = await openSend(tester);
      // Nothing chosen: the person is told, nothing starts.
      await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
      await tester.tap(find.byKey(const Key('startButton')));
      await flush(tester);
      expect(find.text('Please choose a file first.'), findsOneWidget);
      expect(h.core.contentSource, isNull);

      await tester.tap(find.byKey(const Key('pickFileButton')));
      await flush(tester);
      expect(h.pickerLabels, ['Images and PDF']);
      expect(find.text('Chosen: talk.pdf'), findsOneWidget);

      // A wrong room code is refused with the Rust rule.
      await tester.enterText(find.byKey(const Key('codeField')), 'K7');
      await tester.tap(find.byKey(const Key('startButton')));
      await flush(tester);
      expect(h.core.contentSource, isNull);

      await tester.enterText(find.byKey(const Key('codeField')), 'k7m2qx');
      await tester.tap(find.byKey(const Key('startButton')));
      await flush(tester);
      expect(h.core.contentCode, 'K7M2QX');
      expect(h.core.contentServer, 'relay.cometcast.de:443');
      expect(
        h.core.contentSource,
        const ContentSource.file(path: '/home/user/Documents/talk.pdf'),
      );
      expect(find.text('Checking the content …'), findsOneWidget);
    });

    testWidgets('cancelling the file dialog changes nothing', (tester) async {
      final h = await openSend(tester);
      h.picked = null;
      await tester.tap(find.byKey(const Key('pickFileButton')));
      await flush(tester);
      expect(find.text('No file chosen.'), findsOneWidget);
    });

    testWidgets('progress bar, then the PDF is delivered', (tester) async {
      final h = await openSend(tester);
      await chooseFileAndStart(tester);
      h.core.contentSenderEvents
        ..add(
          ContentSenderUiEvent.prepared(
            name: 'talk.pdf',
            kind: UiContentKind.pdf,
            size: BigInt.from(1024),
            pages: 5,
          ),
        )
        ..add(const ContentSenderUiEvent.joined())
        ..add(
          const ContentSenderUiEvent.connectionState(
            state: UiConnectionState.connected,
          ),
        );
      await flush(tester);
      expect(find.text('Connection: connected'), findsOneWidget);

      h.core.contentSenderEvents
        ..add(const ContentSenderUiEvent.connected())
        ..add(
          ContentSenderUiEvent.progress(
            sent: BigInt.from(512),
            total: BigInt.from(1024),
          ),
        );
      await flush(tester);
      final bar = tester.widget<LinearProgressIndicator>(
        find.byKey(const Key('sendProgress')),
      );
      expect(bar.value, 0.5);
      expect(find.text('Sending “talk.pdf” … 50 %'), findsOneWidget);
      // The presenter controls wait for the delivery.
      expect(find.byKey(const Key('presenter')), findsNothing);

      h.core.contentSenderEvents.add(const ContentSenderUiEvent.sent());
      await flush(tester);
      expect(find.byKey(const Key('sendProgress')), findsNothing);
      expect(find.textContaining('“talk.pdf” was sent'), findsOneWidget);
      expect(find.byKey(const Key('presenter')), findsOneWidget);
      expect(find.text('Page 1 of 5'), findsOneWidget);
    });

    testWidgets('stopping during the transfer stops the core', (tester) async {
      final h = await openSend(tester);
      await chooseFileAndStart(tester);
      await tester.tap(find.byKey(const Key('stopButton')));
      await flush(tester);
      expect(h.core.stopSenderCalls, greaterThan(0));
      expect(find.byKey(const Key('startButton')), findsOneWidget);
    });
  });

  group('link', () {
    testWidgets('the link is judged by the core and previewed with its host', (
      tester,
    ) async {
      final h = await openSend(tester);
      await tester.tap(find.text('Link'));
      await flush(tester);
      await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');

      await tester.enterText(
        find.byKey(const Key('linkField')),
        'ftp://files.example/x',
      );
      await tester.tap(find.byKey(const Key('startButton')));
      await flush(tester);
      expect(
        find.text('Please enter a link that starts with http:// or https://.'),
        findsOneWidget,
      );
      expect(h.core.contentSource, isNull);
      expect(find.byKey(const Key('linkPreview')), findsNothing);

      await tester.enterText(
        find.byKey(const Key('linkField')),
        'https://example.org/a?b=1',
      );
      await flush(tester);
      expect(
        find.text('The receiver will be asked to open example.org.'),
        findsOneWidget,
      );
      expect(h.core.linkChecks, contains('https://example.org/a?b=1'));
      await tester.tap(find.byKey(const Key('startButton')));
      await flush(tester);
      expect(
        h.core.contentSource,
        const ContentSource.link(url: 'https://example.org/a?b=1'),
      );

      h.core.contentSenderEvents
        ..add(
          ContentSenderUiEvent.prepared(
            name: 'example.org',
            kind: UiContentKind.link,
            size: BigInt.from(25),
            host: 'example.org',
          ),
        )
        ..add(const ContentSenderUiEvent.connected())
        ..add(const ContentSenderUiEvent.sent());
      await flush(tester);
      expect(
        find.text(
          'The receiver was asked to open example.org. Nothing opens until the person there agrees.',
        ),
        findsOneWidget,
      );
      // No page controls for a link, but the content can be closed on the receiver.
      expect(find.byKey(const Key('presenter')), findsNothing);
      await tester.tap(find.byKey(const Key('stopShowingButton')));
      await flush(tester);
      expect(h.core.contentCommands, [const UiContentCmd.stop()]);
    });
  });

  testWidgets(
    'an image is delivered: no page controls, content can be closed',
    (tester) async {
      final h = await openSend(tester);
      h.picked = '/home/user/Pictures/cat.png';
      await chooseFileAndStart(tester);
      h.core.contentSenderEvents
        ..add(
          ContentSenderUiEvent.prepared(
            name: 'cat.png',
            kind: UiContentKind.image,
            size: BigInt.from(5000),
          ),
        )
        ..add(const ContentSenderUiEvent.connected())
        ..add(const ContentSenderUiEvent.sent());
      await flush(tester);
      expect(find.textContaining('“cat.png” was sent'), findsOneWidget);
      expect(find.byKey(const Key('presenter')), findsNothing);
      expect(find.byKey(const Key('stopShowingButton')), findsOneWidget);

      h.core.contentSenderEvents.add(const ContentSenderUiEvent.stopped());
      await flush(tester);
      expect(
        find.text('The content was closed on the receiver.'),
        findsOneWidget,
      );
      expect(find.byKey(const Key('startButton')), findsOneWidget);
    },
  );

  group('presenter controls', () {
    Future<Harness> delivered(WidgetTester tester, {int? pages = 5}) async {
      final h = await openSend(tester);
      await chooseFileAndStart(tester);
      deliverPdf(h.core, pages: pages);
      await flush(tester);
      return h;
    }

    testWidgets('next, previous and go to page map to the commands', (
      tester,
    ) async {
      final h = await delivered(tester);
      await tester.tap(find.byKey(const Key('nextPageButton')));
      await flush(tester);
      expect(find.text('Page 2 of 5'), findsOneWidget);
      await tester.tap(find.byKey(const Key('nextPageButton')));
      await flush(tester);
      await tester.tap(find.byKey(const Key('prevPageButton')));
      await flush(tester);
      expect(find.text('Page 2 of 5'), findsOneWidget);

      await tester.enterText(find.byKey(const Key('gotoField')), '4');
      await tester.tap(find.byKey(const Key('gotoButton')));
      await flush(tester);
      expect(find.text('Page 4 of 5'), findsOneWidget);

      // Past the end: clamped to the last page (the receiver clamps too).
      await tester.enterText(find.byKey(const Key('gotoField')), '99');
      await tester.tap(find.byKey(const Key('gotoButton')));
      await flush(tester);
      expect(find.text('Page 5 of 5'), findsOneWidget);

      // 0 is not a page.
      await tester.enterText(find.byKey(const Key('gotoField')), '0');
      await tester.tap(find.byKey(const Key('gotoButton')));
      await flush(tester);
      expect(find.text('Enter a page number of 1 or more.'), findsOneWidget);

      expect(h.core.contentCommands, [
        const UiContentCmd.nextPage(),
        const UiContentCmd.nextPage(),
        const UiContentCmd.prevPage(),
        const UiContentCmd.goToPage(page: 4),
        const UiContentCmd.goToPage(page: 5),
      ]);
    });

    testWidgets('the first and last page are the ends', (tester) async {
      final h = await delivered(tester, pages: 2);
      // Previous on the first page: the button is off, the key sends nothing.
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('prevPageButton')))
            .onPressed,
        isNull,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await flush(tester);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await flush(tester);
      expect(find.text('Page 2 of 2'), findsOneWidget);
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('nextPageButton')))
            .onPressed,
        isNull,
      );
      expect(h.core.contentCommands, [const UiContentCmd.nextPage()]);
    });

    testWidgets('arrow keys, Page Up and Page Down turn pages', (tester) async {
      final h = await delivered(tester);
      // No click first: the keys work as soon as the PDF is delivered.
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await tester.sendKeyEvent(LogicalKeyboardKey.pageDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await flush(tester);
      expect(find.text('Page 4 of 5'), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.pageUp);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      await flush(tester);
      expect(find.text('Page 1 of 5'), findsOneWidget);
      expect(h.core.contentCommands, hasLength(6));
      expect(h.core.contentCommands.first, const UiContentCmd.nextPage());
      expect(h.core.contentCommands.last, const UiContentCmd.prevPage());
    });

    testWidgets('without a page count the page is just "Page N"', (
      tester,
    ) async {
      final h = await delivered(tester, pages: null);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
      await flush(tester);
      expect(find.text('Page 2'), findsOneWidget);
      expect(h.core.contentCommands, [const UiContentCmd.nextPage()]);
    });

    testWidgets('a refused command is said calmly and does not move the page', (
      tester,
    ) async {
      final h = await delivered(tester);
      h.core.acceptCommands = false;
      await tester.tap(find.byKey(const Key('nextPageButton')));
      await flush(tester);
      expect(
        find.text('The command was not accepted. Try again.'),
        findsOneWidget,
      );
      expect(find.text('Page 1 of 5'), findsOneWidget);
      h.core.acceptCommands = true;
      await tester.tap(find.byKey(const Key('nextPageButton')));
      await flush(tester);
      expect(find.byKey(const Key('commandFailed')), findsNothing);
      expect(find.text('Page 2 of 5'), findsOneWidget);
    });

    testWidgets('stop showing sends stop; the end is reported', (tester) async {
      final h = await delivered(tester);
      await tester.tap(find.byKey(const Key('stopShowingButton')));
      await flush(tester);
      expect(h.core.contentCommands, [const UiContentCmd.stop()]);
      h.core.contentSenderEvents.add(const ContentSenderUiEvent.stopped());
      await flush(tester);
      expect(find.byKey(const Key('presenter')), findsNothing);
      expect(
        find.text('The content was closed on the receiver.'),
        findsOneWidget,
      );
    });

    testWidgets('the receiver leaving ends the controls', (tester) async {
      final h = await delivered(tester);
      h.core.contentSenderEvents.add(const ContentSenderUiEvent.peerLeft());
      await flush(tester);
      expect(find.byKey(const Key('presenter')), findsNothing);
      expect(find.text('The receiver has left.'), findsOneWidget);
    });
  });

  testWidgets('every refusal is a translated sentence (en and de)', (
    tester,
  ) async {
    for (final locale in ['en', 'de']) {
      for (final reason in UiSendRejection.values) {
        final h = await openSend(tester, locale: locale);
        await chooseFileAndStart(tester);
        h.core.contentSenderEvents.add(
          ContentSenderUiEvent.rejected(reason: reason),
        );
        await flush(tester);
        final text = tester
            .widget<Text>(find.byKey(const Key('statusText')))
            .data!;
        expect(text.endsWith('.'), isTrue, reason: '$locale $reason: $text');
        // Back to a form that can be used again.
        expect(find.byKey(const Key('startButton')), findsOneWidget);
        await tester.pumpWidget(const SizedBox());
      }
    }
  });

  testWidgets('a connection error uses the translated prefix', (tester) async {
    final h = await openSend(tester);
    await chooseFileAndStart(tester);
    h.core.contentSenderEvents.add(
      const ContentSenderUiEvent.error(message: 'server error room_not_found'),
    );
    await flush(tester);
    expect(find.text('Error: server error room_not_found'), findsOneWidget);
  });

  testWidgets('German texts of the presenter', (tester) async {
    final h = await openSend(tester, locale: 'de');
    await chooseFileAndStart(tester);
    deliverPdf(h.core);
    await flush(tester);
    expect(find.text('Präsentationssteuerung'), findsOneWidget);
    expect(find.text('Seite 1 von 5'), findsOneWidget);
    expect(find.text('Anzeige beenden'), findsOneWidget);
  });

  testWidgets('leaving the screen stops the sender', (tester) async {
    final h = await openSend(tester);
    await chooseFileAndStart(tester);
    final before = h.core.stopSenderCalls;
    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(h.core.stopSenderCalls, greaterThan(before));
  });

  test('displayName drops control and bidi characters and truncates', () {
    expect(displayName('a\u202Eb\nc\u0000.pdf'), 'abc.pdf');
    final long = '${'x' * 200}.pdf';
    final shown = displayName(long);
    expect(shown.length, 80);
    expect(shown.endsWith('...'), isTrue);
  });
}
