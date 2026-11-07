// SPDX-License-Identifier: GPL-3.0-or-later
// Office files: the limits line, "Pick my own PDF instead" and the switch (spec 15.6, fix list
// F6).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/presenter/file_picker.dart';

import 'nearby_support.dart' show settle;
import 'round5_support.dart';

Future<void> toWhat(WidgetTester t, R5 r) async {
  await tapText(t, 'Connect');
  r.core.presenterEvents.add(const PresenterUiEvent.joined(roomName: 'Elbe'));
  await settle(t);
}

Future<void> present(
  WidgetTester t,
  R5 r,
  String name,
  UiContentKind kind,
) async {
  r.core.presenterEvents
    ..add(PresenterUiEvent.converting(name: name))
    ..add(
      PresenterUiEvent.contentPrepared(
        name: name,
        kind: kind,
        size: BigInt.from(1000),
        pages: 3,
      ),
    )
    ..add(const PresenterUiEvent.presenting());
  await settle(t);
}

void main() {
  testWidgets('a presented Office file shows the limits line and the way out', (
    t,
  ) async {
    final r = await pumpR5(
      t,
      office: true,
      code: 'K7QM2W',
      pickResults: ['/tmp/report.docx'],
    );
    await toWhat(t, r);
    await tapText(t, 'Show a photo or file');
    await tapText(t, 'Choose file');
    expect(r.picks, [PickKind.imagePdfOffice]);
    expect(r.core.presentedContent.single, isA<ContentSource_File>());
    await present(t, r, 'report.docx', UiContentKind.pdf);
    expect(
      find.text(
        'Converted to PDF. Fonts, headers and footers may differ; animations are missing.',
      ),
      findsOneWidget,
    );
    expect(find.text('Pick my own PDF instead'), findsOneWidget);
    // Above the PDF page row.
    expect(
      t.getTopLeft(find.byKey(const Key('presenting-converted'))).dy,
      lessThan(t.getTopLeft(find.byKey(const Key('next-page'))).dy),
    );
    await finish(t, r);
  });

  testWidgets('not for a PDF or an image', (t) async {
    final r = await pumpR5(
      t,
      office: true,
      code: 'K7QM2W',
      pickResults: ['/tmp/slides.pdf'],
    );
    await toWhat(t, r);
    await tapText(t, 'Show a photo or file');
    await tapText(t, 'Choose file');
    r.core.presenterEvents
      ..add(
        PresenterUiEvent.contentPrepared(
          name: 'slides.pdf',
          kind: UiContentKind.pdf,
          size: BigInt.from(1000),
          pages: 3,
        ),
      )
      ..add(const PresenterUiEvent.presenting());
    await settle(t);
    expect(find.byKey(const Key('presenting-converted')), findsNothing);
    expect(find.text('Pick my own PDF instead'), findsNothing);
    await finish(t, r);
  });

  testWidgets(
    'the link stops presenting and opens the PDF-only picker; cancel returns',
    (t) async {
      final r = await pumpR5(
        t,
        office: true,
        code: 'K7QM2W',
        pickResults: ['/tmp/report.docx', ''],
      );
      await toWhat(t, r);
      await tapText(t, 'Show a photo or file');
      await tapText(t, 'Choose file');
      await present(t, r, 'report.docx', UiContentKind.pdf);
      await tapText(t, 'Pick my own PDF instead');
      expect(r.core.stopPresentingCalls, 1);
      expect(r.picks.last, PickKind.pdf);
      r.core.presenterEvents.add(const PresenterUiEvent.presentingStopped());
      await settle(t);
      // Cancelled: "What to show", still connected.
      expect(find.text('What would you like to show?'), findsOneWidget);
      expect(r.core.stopPresenterCalls, 0);
      await finish(t, r);
    },
  );

  testWidgets('the PDF picked instead is presented at once', (t) async {
    final r = await pumpR5(
      t,
      office: true,
      code: 'K7QM2W',
      pickResults: ['/tmp/report.docx', '/tmp/own.pdf'],
    );
    await toWhat(t, r);
    await tapText(t, 'Show a photo or file');
    await tapText(t, 'Choose file');
    await present(t, r, 'report.docx', UiContentKind.pdf);
    await tapText(t, 'Pick my own PDF instead');
    expect(r.core.presentedContent.length, 2);
    final second = r.core.presentedContent.last as ContentSource_File;
    expect(second.path, '/tmp/own.pdf');
    await finish(t, r);
  });

  testWidgets(
    'switch off: the picker has no Office types and the body says so',
    (t) async {
      final r = await pumpR5(t, office: true, code: 'K7QM2W');
      r.settings.officeConvert = false;
      await toWhat(t, r);
      await tapText(t, 'Show a photo or file');
      expect(
        find.text(
          'A PDF or an image from this device. It appears on the screen as soon as you pick it.',
        ),
        findsOneWidget,
      );
      await tapText(t, 'Choose file');
      expect(r.picks, [PickKind.imagePdf]);
      await finish(t, r);
    },
  );

  testWidgets('switch on with a worker: the body names Office files', (
    t,
  ) async {
    final r = await pumpR5(t, office: true, code: 'K7QM2W');
    await toWhat(t, r);
    await tapText(t, 'Show a photo or file');
    expect(find.textContaining('an Office file'), findsOneWidget);
    await finish(t, r);
  });

  testWidgets('no worker: images and PDF only, whatever the switch says', (
    t,
  ) async {
    final r = await pumpR5(t, code: 'K7QM2W');
    await toWhat(t, r);
    await tapText(t, 'Show a photo or file');
    await tapText(t, 'Choose file');
    expect(r.picks, [PickKind.imagePdf]);
    await finish(t, r);
  });

  test('the dialog types by kind', () {
    final all = contentTypeGroupFor('x', PickKind.imagePdfOffice);
    expect(all.extensions, containsAll(['pdf', 'docx', 'pptx', 'xlsx', 'png']));
    final plain = contentTypeGroupFor('x', PickKind.imagePdf);
    expect(plain.extensions, containsAll(['pdf', 'png']));
    expect(plain.extensions, isNot(contains('docx')));
    expect(plain.mimeTypes!.any((m) => m.contains('openxml')), isFalse);
    expect(contentTypeGroupFor('x', PickKind.pdf).extensions, ['pdf']);
    expect(contentTypeGroupFor('x', PickKind.photo).extensions, [
      'png',
      'jpg',
      'jpeg',
      'webp',
    ]);
  });
}
