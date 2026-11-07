// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests for received content (image, PDF page, link, errors) with a fake core.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/receiver_controller.dart';

import 'content_support.dart';
import 'fake_core.dart';

void main() {
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
