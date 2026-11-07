// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests for received content (image, PDF page, link, errors) with a fake core.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/annotations.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/receiver_controller.dart';

import 'content_support.dart';
import 'fake_core.dart';

class Harness {
  Harness(this.core, this.opened, this.openResult);

  final FakeCoreApi core;

  /// Links the app asked the system to open.
  final List<Uri> opened;
  final List<bool> openResult;
}

Future<Harness> startReceiving(
  WidgetTester tester, {
  String locale = 'en',
  Size size = const Size(900, 1200),
  double dpr = 1,
}) async {
  final core = FakeCoreApi();
  final opened = <Uri>[];
  final result = <bool>[true];
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = dpr;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    CometCastApp(
      locale: Locale(locale),
      core: core,
      openUrl: (u) async {
        opened.add(u);
        return result.last;
      },
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(
    find.text(
      locale == 'en' ? 'Receive on this device' : 'Auf diesem Gerät empfangen',
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const Key('startButton')));
  await flush(tester);
  core.receiverEvents.add(
    const ReceiverUiEvent.roomCode(code: 'K7M2QX', joinMode: UiJoinMode.open),
  );
  core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Guest 1'));
  core.receiverEvents.add(
    const ReceiverUiEvent.connectionState(state: UiConnectionState.connected),
  );
  await flush(tester);
  return Harness(core, opened, result);
}

Future<void> showPage(
  WidgetTester tester,
  FakeCoreApi core,
  int page,
  List<int> rgb, {
  int w = 8,
  int h = 6,
}) async {
  core.contentFrames.add(
    ContentFrame.page(
      page: page,
      width: w,
      height: h,
      rgba: solidRgba(w, h, rgb),
    ),
  );
  core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
  await pumpUntilFound(tester, find.text('Page $page of 3'));
}

/// Two frames: an event added to a stream reaches the controller in a microtask, and the
/// screen needs its frame after that.
Future<void> flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('the receiver asks the core for content', (tester) async {
    final h = await startReceiving(tester);
    expect(h.core.receiverContent, isTrue);
  });

  testWidgets(
    'a received image is shown with a close button; closing goes back',
    (tester) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        ReceiverUiEvent.contentTransferStarted(
          document: false,
          name: 'photo.png',
          size: BigInt.from(1000),
        ),
      );
      await flush(tester);
      // While it arrives the status says so.
      expect(find.text('Receiving “photo.png” …'), findsOneWidget);

      h.core.contentFrames.add(
        ContentFrame.image(
          name: 'photo.png',
          mime: 'image/png',
          bytes: makePng(20, 10, [255, 0, 0]),
        ),
      );
      h.core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      expect(
        await pumpUntilFound(tester, find.byKey(const Key('contentImage'))),
        isTrue,
      );
      expect(find.byKey(const Key('contentCloseButton')), findsOneWidget);
      // The form is gone, the picture has the room.
      expect(find.byKey(const Key('startButton')), findsNothing);

      await tester.tap(find.byKey(const Key('contentCloseButton')));
      await flush(tester);
      expect(find.byKey(const Key('contentImage')), findsNothing);
      // Back to waiting; the receiver is still running.
      expect(find.text('K7M2QX'), findsOneWidget);
      expect(h.core.stopReceiverCalls, 0);
    },
  );

  testWidgets('an image that cannot be decoded gives a calm note', (
    tester,
  ) async {
    final h = await startReceiving(tester);
    h.core.contentFrames.add(
      ContentFrame.image(
        name: 'bad.png',
        mime: 'image/png',
        bytes: Uint8List.fromList(List.filled(64, 7)),
      ),
    );
    h.core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
    expect(
      await pumpUntilFound(tester, find.byKey(const Key('contentNote'))),
      isTrue,
    );
    expect(find.text('This image cannot be shown.'), findsOneWidget);
    expect(find.byKey(const Key('contentImage')), findsNothing);
  });

  testWidgets(
    'a PDF page is shown with a page indicator; pages follow the sender',
    (tester) async {
      final h = await startReceiving(
        tester,
        size: const Size(900, 1200),
        dpr: 2,
      );
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'talk.pdf', pages: 3),
      );
      await flush(tester);
      // Opening: the first page is not here yet.
      expect(find.text('Opening “talk.pdf” …'), findsOneWidget);
      expect(find.byKey(const Key('pageIndicator')), findsNothing);

      await showPage(tester, h.core, 1, [255, 0, 0]);
      expect(find.byKey(const Key('pageIndicator')), findsOneWidget);
      expect(await firstPixel(tester, const Key('pageView')), [255, 0, 0]);
      // The core was told the width of the view in physical pixels (the window is 900 wide,
      // 450 logical at a pixel ratio of 2).
      await flush(tester);
      expect(h.core.contentWidths.last, 900);

      // The sender turns the page: new indicator, different pixels.
      await showPage(tester, h.core, 2, [0, 0, 255]);
      expect(find.text('Page 1 of 3'), findsNothing);
      expect(await firstPixel(tester, const Key('pageView')), [0, 0, 255]);

      // The receiver's person cannot turn pages: keys do nothing.
      for (final k in [
        LogicalKeyboardKey.arrowRight,
        LogicalKeyboardKey.pageDown,
        LogicalKeyboardKey.arrowLeft,
      ]) {
        await tester.sendKeyEvent(k);
        await flush(tester);
      }
      expect(find.text('Page 2 of 3'), findsOneWidget);
      expect(h.core.contentFrames, isEmpty);
    },
  );

  testWidgets('a resize reports the new width to the core', (tester) async {
    final h = await startReceiving(tester, size: const Size(900, 1200));
    h.core.receiverEvents.add(
      const ReceiverUiEvent.contentDocumentOpened(name: 'a.pdf', pages: 3),
    );
    await showPage(tester, h.core, 1, [1, 2, 3]);
    await flush(tester);
    expect(h.core.contentWidths.last, 900);
    tester.view.physicalSize = const Size(640, 1200);
    await flush(tester);
    await flush(tester);
    expect(h.core.contentWidths.last, 640);
  });

  testWidgets(
    'stop from the sender closes the page; late pages of a closed document are ignored',
    (tester) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'a.pdf', pages: 3),
      );
      await showPage(tester, h.core, 1, [9, 9, 9]);
      h.core.receiverEvents.add(const ReceiverUiEvent.contentStopped());
      await flush(tester);
      expect(find.byKey(const Key('pageIndicator')), findsNothing);
      expect(
        find.byKey(const Key('startButton')),
        findsNothing,
      ); // running: stop button
      expect(find.text('K7M2QX'), findsOneWidget);

      // Open again, close by the person, then a late page arrives: ignored.
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'b.pdf', pages: 3),
      );
      await showPage(tester, h.core, 1, [5, 5, 5]);
      await tester.tap(find.byKey(const Key('contentCloseButton')));
      await flush(tester);
      h.core.contentFrames.add(
        ContentFrame.page(
          page: 2,
          width: 4,
          height: 4,
          rgba: solidRgba(4, 4, [1, 1, 1]),
        ),
      );
      h.core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 300)),
      );
      await flush(tester);
      expect(find.byKey(const Key('pageIndicator')), findsNothing);
    },
  );

  group('toolbar over content', () {
    testWidgets('a page can be marked; a page turn clears the marks, and '
        'there is no freeze for a page', (tester) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'talk.pdf', pages: 3),
      );
      await flush(tester);
      await showPage(tester, h.core, 1, [255, 0, 0]);
      expect(find.byKey(const Key('annotationToolbar')), findsOneWidget);
      // Freezing a still page makes no sense: the button is off.
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('toolFreeze')))
            .onPressed,
        isNull,
      );
      final ann = tester
          .widget<AnnotationLayer>(find.byType(AnnotationLayer))
          .controller;
      await tester.tap(find.byKey(const Key('toolPen')));
      await tester.pump();
      final box = tester.getRect(find.byKey(const Key('pageView')));
      final g = await tester.startGesture(box.center);
      await g.moveBy(const Offset(30, 10));
      await g.moveBy(const Offset(30, 10));
      await g.up();
      await tester.pump();
      expect(ann.strokes, hasLength(1));
      // The drawing sits on the page: the layer has exactly the size of the page.
      expect(
        tester.getSize(find.byType(AnnotationLayer)),
        tester.getSize(find.byKey(const Key('pageView'))),
      );

      // The sender turns the page: the marks belonged to page 1.
      await showPage(tester, h.core, 2, [0, 0, 255]);
      final ann2 = tester
          .widget<AnnotationLayer>(find.byType(AnnotationLayer))
          .controller;
      expect(ann2.strokes, isEmpty);
      // The tool stays selected for the next page.
      expect(ann2.tool, AnnotationTool.pen);
    });

    testWidgets('the same page drawn again (a resize) keeps the marks; another '
        'document with the same name and page does not', (tester) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'talk.pdf', pages: 3),
      );
      await flush(tester);
      await showPage(tester, h.core, 1, [255, 0, 0]);
      final ann = tester
          .widget<AnnotationLayer>(find.byType(AnnotationLayer))
          .controller;
      await tester.tap(find.byKey(const Key('toolPen')));
      await tester.pump();
      final g = await tester.startGesture(
        tester.getRect(find.byKey(const Key('pageView'))).center,
      );
      await g.moveBy(const Offset(30, 10));
      await g.up();
      await tester.pump();
      expect(ann.strokes, hasLength(1));

      // Page 1 again, in a new picture of another size (what a resize produces): a new
      // ReceivedDocument object for the same page. The marks stay.
      final before = tester.widget<RawImage>(find.byType(RawImage).first).image;
      h.core.contentFrames.add(
        ContentFrame.page(
          page: 1,
          width: 16,
          height: 12,
          rgba: solidRgba(16, 12, [0, 255, 0]),
        ),
      );
      h.core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      expect(
        await pumpUntil(
          tester,
          () => !identical(
            tester.widget<RawImage>(find.byType(RawImage).first).image,
            before,
          ),
        ),
        isTrue,
        reason: 'the page was drawn again',
      );
      expect(find.text('Page 1 of 3'), findsOneWidget);
      expect(
        tester.widget<AnnotationLayer>(find.byType(AnnotationLayer)).controller,
        same(ann),
      );
      expect(ann.strokes, hasLength(1));

      // A different document with the same name, again at page 1: not the same page.
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'talk.pdf', pages: 3),
      );
      await flush(tester);
      await showPage(tester, h.core, 1, [0, 0, 255]);
      expect(ann.strokes, isEmpty);
    });

    testWidgets('a second finger does not draw while the first one is down', (
      tester,
    ) async {
      final h = await startReceiving(tester);
      h.core.contentFrames.add(
        ContentFrame.image(
          name: 'photo.png',
          mime: 'image/png',
          bytes: makePng(20, 10, [255, 0, 0]),
        ),
      );
      h.core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      expect(
        await pumpUntilFound(tester, find.byKey(const Key('contentImage'))),
        isTrue,
      );
      final ann = tester
          .widget<AnnotationLayer>(find.byType(AnnotationLayer))
          .controller;
      await tester.tap(find.byKey(const Key('toolPen')));
      await tester.pump();
      final r = tester.getRect(find.byType(AnnotationLayer));
      final first = await tester.startGesture(r.center, pointer: 1);
      final second = await tester.startGesture(
        r.center + const Offset(40, 0),
        pointer: 2,
      );
      await second.moveBy(const Offset(0, 30));
      await first.moveBy(const Offset(20, 20));
      await second.up();
      await tester.pump();
      // One stroke, drawn by the first finger only (start and one move).
      expect(ann.strokes, hasLength(1));
      expect(ann.strokes.single.points, hasLength(2));
      await first.up();
      await tester.pump();
      expect(ann.currentStroke, isNull);
      // After both are up a new stroke can start.
      final again = await tester.startGesture(r.center, pointer: 3);
      await again.moveBy(const Offset(10, 10));
      await again.up();
      await tester.pump();
      expect(ann.strokes, hasLength(2));
    });

    testWidgets('an image can be marked; a link card has no toolbar', (
      tester,
    ) async {
      final h = await startReceiving(tester);
      h.core.contentFrames.add(
        ContentFrame.image(
          name: 'photo.png',
          mime: 'image/png',
          bytes: makePng(20, 10, [255, 0, 0]),
        ),
      );
      h.core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      expect(
        await pumpUntilFound(tester, find.byKey(const Key('contentImage'))),
        isTrue,
      );
      expect(find.byKey(const Key('annotationToolbar')), findsOneWidget);
      expect(find.byType(AnnotationLayer), findsOneWidget);
      // 20 x 10 picture: the layer keeps that aspect ratio.
      final size = tester.getSize(find.byType(AnnotationLayer));
      expect(size.width / size.height, closeTo(2, 0.01));

      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/',
          host: 'example.org',
        ),
      );
      await flush(tester);
      expect(find.byKey(const Key('annotationToolbar')), findsNothing);
      expect(find.byType(AnnotationLayer), findsNothing);
    });
  });

  group('links', () {
    testWidgets('a link is only a card: nothing opens without a click', (
      tester,
    ) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://xn--bcher-kva.example/path?q=1',
          host: 'xn--bcher-kva.example',
        ),
      );
      await flush(tester);
      await tester.pump(const Duration(seconds: 2));
      // The punycode host is what the card says; the full address is visible too.
      expect(
        find.text('The sender wants to open xn--bcher-kva.example'),
        findsOneWidget,
      );
      expect(
        find.text('https://xn--bcher-kva.example/path?q=1'),
        findsOneWidget,
      );
      expect(find.text('Open'), findsOneWidget);
      expect(find.text('Dismiss'), findsOneWidget);
      expect(h.opened, isEmpty, reason: 'never opened automatically');
      expect(
        h.core.linkChecks,
        isEmpty,
        reason: 'checked right before opening',
      );
    });

    testWidgets('Dismiss (and Enter on the focused button) opens nothing', (
      tester,
    ) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/',
          host: 'example.org',
        ),
      );
      await flush(tester);
      // The safe button has the initial focus: a stray Enter does not open the link.
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await flush(tester);
      expect(find.byKey(const Key('linkCard')), findsNothing);
      expect(h.opened, isEmpty);

      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/',
          host: 'example.org',
        ),
      );
      await flush(tester);
      await tester.tap(find.byKey(const Key('linkDismissButton')));
      await flush(tester);
      expect(find.byKey(const Key('linkCard')), findsNothing);
      expect(h.opened, isEmpty);
    });

    testWidgets('Open opens exactly that address once, after the click', (
      tester,
    ) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/a?b=1',
          host: 'example.org',
        ),
      );
      await flush(tester);
      await tester.tap(find.byKey(const Key('linkOpenButton')));
      await flush(tester);
      await flush(tester);
      expect(h.opened, [Uri.parse('https://example.org/a?b=1')]);
      // The address was checked again by the core before it was opened.
      expect(h.core.linkChecks, ['https://example.org/a?b=1']);
      expect(find.byKey(const Key('linkCard')), findsNothing);
      expect(find.byKey(const Key('contentNote')), findsNothing);
    });

    testWidgets('a link that fails the second check is never opened', (
      tester,
    ) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'javascript:alert(1)',
          host: 'x',
        ),
      );
      await flush(tester);
      await tester.tap(find.byKey(const Key('linkOpenButton')));
      await flush(tester);
      expect(h.opened, isEmpty);
      expect(find.text('The link could not be opened.'), findsOneWidget);
    });

    testWidgets('when the system refuses, a calm note appears', (tester) async {
      final h = await startReceiving(tester);
      h.openResult.add(false);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/',
          host: 'example.org',
        ),
      );
      await flush(tester);
      await tester.tap(find.byKey(const Key('linkOpenButton')));
      await flush(tester);
      await flush(tester);
      expect(h.opened, hasLength(1));
      expect(find.text('The link could not be opened.'), findsOneWidget);
    });

    testWidgets('German texts of the link card', (tester) async {
      final h = await startReceiving(tester, locale: 'de');
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/',
          host: 'example.org',
        ),
      );
      await flush(tester);
      expect(
        find.text('Der Absender möchte example.org öffnen'),
        findsOneWidget,
      );
      expect(find.text('Öffnen'), findsOneWidget);
      expect(find.text('Verwerfen'), findsOneWidget);
    });
  });

  testWidgets(
    'the latest content wins: page, then link, then image, then stop',
    (tester) async {
      final h = await startReceiving(tester);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'a.pdf', pages: 3),
      );
      await showPage(tester, h.core, 1, [1, 1, 1]);
      expect(find.byKey(const Key('pageView')), findsOneWidget);

      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/',
          host: 'example.org',
        ),
      );
      await flush(tester);
      expect(find.byKey(const Key('pageView')), findsNothing);
      expect(find.byKey(const Key('linkCard')), findsOneWidget);

      h.core.contentFrames.add(
        ContentFrame.image(
          name: 'i.png',
          mime: 'image/png',
          bytes: makePng(4, 4, [0, 255, 0]),
        ),
      );
      h.core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      expect(
        await pumpUntilFound(tester, find.byKey(const Key('contentImage'))),
        isTrue,
      );
      expect(find.byKey(const Key('linkCard')), findsNothing);

      h.core.receiverEvents.add(const ReceiverUiEvent.contentStopped());
      await flush(tester);
      await flush(tester);
      expect(find.byKey(const Key('contentImage')), findsNothing);
      expect(find.text('K7M2QX'), findsOneWidget);
    },
  );

  testWidgets(
    'every error code is a translated, non-alarming note (en and de)',
    (tester) async {
      for (final locale in ['en', 'de']) {
        final h = await startReceiving(tester, locale: locale);
        for (final e in UiContentError.values) {
          h.core.receiverEvents.add(ReceiverUiEvent.contentError(error: e));
          await flush(tester);
          final text = tester
              .widget<Text>(find.byKey(const Key('contentNoteText')))
              .data!;
          expect(text, isNotEmpty, reason: '$e');
          // A translated sentence, not a code or a raw name.
          expect(text, isNot(contains(e.name)), reason: '$e');
          expect(text, isNot(contains('Exception')), reason: '$e');
          expect(text.endsWith('.'), isTrue, reason: '$e: $text');
        }
        // The note can be hidden.
        await tester.tap(find.byKey(const Key('contentNoteHide')));
        await flush(tester);
        expect(find.byKey(const Key('contentNote')), findsNothing);
        await tester.pumpWidget(const SizedBox());
      }
    },
  );

  testWidgets('specific notes: rejected PDF, worker missing, timeout', (
    tester,
  ) async {
    final h = await startReceiving(tester);
    h.core.receiverEvents.add(
      const ReceiverUiEvent.contentDocumentOpened(name: 'a.pdf', pages: 2),
    );
    await flush(tester);
    h.core.receiverEvents.add(
      const ReceiverUiEvent.contentError(
        error: UiContentError.documentRejected,
      ),
    );
    await flush(tester);
    // The refused document is closed (the core does the same).
    expect(find.text('Opening “a.pdf” …'), findsNothing);
    expect(find.textContaining('This PDF cannot be shown'), findsOneWidget);

    h.core.receiverEvents.add(
      const ReceiverUiEvent.contentError(
        error: UiContentError.workerUnavailable,
      ),
    );
    await flush(tester);
    expect(find.textContaining('PDF component was not found'), findsOneWidget);

    h.core.receiverEvents.add(
      const ReceiverUiEvent.contentError(error: UiContentError.transferTimeout),
    );
    await flush(tester);
    expect(find.text('A transfer stalled and was cancelled.'), findsOneWidget);

    h.core.receiverEvents.add(
      const ReceiverUiEvent.contentError(error: UiContentError.tooLarge),
    );
    await flush(tester);
    expect(
      find.text('The file is larger than this app accepts.'),
      findsOneWidget,
    );
  });

  testWidgets('the sender leaving removes the content', (tester) async {
    final h = await startReceiving(tester);
    h.core.receiverEvents.add(
      const ReceiverUiEvent.contentLink(
        url: 'https://example.org/',
        host: 'example.org',
      ),
    );
    await flush(tester);
    h.core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
    await flush(tester);
    expect(find.byKey(const Key('linkCard')), findsNothing);
    expect(find.text('The sender has left.'), findsOneWidget);
  });

  group('controller', () {
    testWidgets('an image with too many pixels is not decoded', (tester) async {
      final core = FakeCoreApi();
      final c = ReceiverController(core: core, maxImagePixels: 100);
      addTearDown(c.dispose);
      c.start('s:1');
      core.contentFrames.add(
        ContentFrame.image(
          name: 'big.png',
          mime: 'image/png',
          bytes: makePng(20, 20, [1, 2, 3]),
        ),
      );
      core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      expect(await pumpUntil(tester, () => c.note != null), isTrue);
      expect(c.note, ContentNote.imageTooBig);
      expect(c.shown, isNull);

      // Within the limit it is accepted.
      core.contentFrames.add(
        ContentFrame.image(
          name: 'ok.png',
          mime: 'image/png',
          bytes: makePng(10, 10, [1, 2, 3]),
        ),
      );
      core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      expect(await pumpUntil(tester, () => c.shown is ReceivedImage), isTrue);
      expect(c.note, isNull);
    });

    testWidgets('a picture fetched for replaced content is dropped', (
      tester,
    ) async {
      final core = FakeCoreApi();
      final c = ReceiverController(core: core);
      addTearDown(c.dispose);
      c.start('s:1');
      core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'a.pdf', pages: 2),
      );
      core.contentFrames.add(
        ContentFrame.page(
          page: 1,
          width: 4,
          height: 4,
          rgba: solidRgba(4, 4, [1, 1, 1]),
        ),
      );
      core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      // A link replaces the document while the page is still being fetched.
      core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/',
          host: 'example.org',
        ),
      );
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 300)),
      );
      expect(c.shown, isA<ReceivedLink>());
    });

    testWidgets('the first page of a new document is shown when it is taken '
        'while the document opens', (tester) async {
      final core = _SlowTakeCore();
      final c = ReceiverController(core: core);
      addTearDown(c.dispose);
      c.start('s:1');
      core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'a.pdf', pages: 2),
      );
      core.receiverEvents.add(const ReceiverUiEvent.contentFrameReady());
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      expect(core.pending, isNotNull);
      // A new document opens while the take is still running; its page 1 is what the take
      // returns (the core cleared its mailbox for the new document).
      core.receiverEvents.add(
        const ReceiverUiEvent.contentDocumentOpened(name: 'b.pdf', pages: 3),
      );
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 50)),
      );
      core.pending!.complete(
        ContentFrame.page(
          page: 1,
          width: 4,
          height: 4,
          rgba: solidRgba(4, 4, [1, 1, 1]),
        ),
      );
      await pumpUntil(tester, () {
        final d = c.shown;
        return d is ReceivedDocument && d.image != null;
      });
      final shown = c.shown;
      expect(shown, isA<ReceivedDocument>());
      expect((shown! as ReceivedDocument).name, 'b.pdf');
      expect((shown as ReceivedDocument).image, isNotNull);
    });
  });
}

/// A core whose `takeContentFrame` waits until the test completes it.
class _SlowTakeCore extends FakeCoreApi {
  Completer<ContentFrame?>? pending;

  @override
  Future<ContentFrame?> takeContentFrame() {
    pending = Completer<ContentFrame?>();
    return pending!.future;
  }
}
