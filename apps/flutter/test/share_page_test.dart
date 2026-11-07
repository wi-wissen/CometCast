// SPDX-License-Identifier: GPL-3.0-or-later
// The Android share page and its flows (spec 15.3, fix list F7).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/share_intake.dart';
import 'package:cometcast/src/presenter/share.dart';

import 'nearby_support.dart' show settle;
import 'round5_support.dart';

SharedItem pdf({int size = 1887437}) => SharedItem(
  kind: SharedKind.file,
  path: '/cache/share/a/Quarterly report 2026 final version.pdf',
  name: 'Quarterly report 2026 final version.pdf',
  size: size,
  mime: 'application/pdf',
);

SharedItem docx() => const SharedItem(
  kind: SharedKind.file,
  path: '/cache/share/b/plan.docx',
  name: 'plan.docx',
  size: 400000,
  mime:
      'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
);

const link = SharedItem(
  kind: SharedKind.link,
  url: 'https://example.org/talks/video.mp4',
);

Future<R5> share(
  WidgetTester t,
  SharedItem item, {
  bool office = true,
  Locale locale = const Locale('en'),
  bool withNearby = false,
}) async {
  final intake = FakeShareIntake()..item = item;
  final r = await pumpR5(
    t,
    office: office,
    shareIntake: intake,
    locale: locale,
    withNearby: withNearby,
  );
  await settle(t);
  return r;
}

void main() {
  group('not connected', () {
    testWidgets(
      'heading, card, the start page parts, Show instead of Connect',
      (t) async {
        final r = await share(t, pdf());
        expect(find.text('Show with Comet Cast'), findsOneWidget);
        expect(find.byKey(const Key('share-card')), findsOneWidget);
        // Shortened in the middle, the extension kept.
        expect(find.textContaining('…'), findsOneWidget);
        expect(find.text('PDF · 1.8 MB'), findsOneWidget);
        expect(find.byKey(const Key('name-field')), findsOneWidget);
        expect(find.byKey(const Key('code-field')), findsOneWidget);
        expect(find.byKey(const Key('address-line')), findsOneWidget);
        expect(find.text('Scan QR code'), findsOneWidget);
        expect(find.text('Show'), findsOneWidget);
        expect(find.text('Connect'), findsNothing);
        // No text links of the start page.
        expect(find.byKey(const Key('be-display')), findsNothing);
        await finish(t, r);
      },
    );

    testWidgets('German: decimal comma', (t) async {
      final r = await share(t, pdf(), locale: const Locale('de'));
      expect(find.text('Mit Comet Cast zeigen'), findsOneWidget);
      expect(find.text('PDF · 1,8 MB'), findsOneWidget);
      expect(find.text('Zeigen'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('Show joins and presents at once, no What to show', (t) async {
      final r = await share(t, pdf());
      await t.enterText(find.byKey(const Key('code-field')), 'K7QM2W');
      await tapText(t, 'Show');
      expect(r.core.startPresenterCalls, 1);
      r.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      expect(find.text('What would you like to show?'), findsNothing);
      final src = r.core.presentedContent.single as ContentSource_File;
      expect(src.path, contains('Quarterly report'));
      r.core.presenterEvents
        ..add(
          PresenterUiEvent.contentPrepared(
            name: 'Quarterly report.pdf',
            kind: UiContentKind.pdf,
            size: BigInt.from(100),
            pages: 4,
          ),
        )
        ..add(const PresenterUiEvent.presenting());
      await settle(t);
      expect(find.text('You are showing a file'), findsOneWidget);
      // Stop: "What to show", the app continues normally; the copy is removed.
      await tapText(t, 'Stop showing');
      expect(find.text('What would you like to show?'), findsOneWidget);
      expect(r.intake.discarded, [pdf().path]);
      await finish(t, r);
    });

    testWidgets('Approval on: waiting, then Presenting', (t) async {
      final r = await share(t, pdf());
      await t.enterText(find.byKey(const Key('code-field')), 'K7QM2W');
      await tapText(t, 'Show');
      r.core.presenterEvents.add(
        const PresenterUiEvent.waitingForApproval(timeoutSecs: 60, check: 42),
      );
      await settle(t);
      expect(find.byKey(const Key('waiting-check')), findsOneWidget);
      r.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      expect(r.core.presentedContent, hasLength(1));
      expect(find.text('What would you like to show?'), findsNothing);
      await finish(t, r);
    });

    testWidgets(
      'a failure: Back returns to the share page with the item kept',
      (t) async {
        final r = await share(t, pdf());
        await t.enterText(find.byKey(const Key('code-field')), 'K7QM2W');
        await tapText(t, 'Show');
        r.core.presenterEvents.add(
          const PresenterUiEvent.error(code: 'room_not_found', message: ''),
        );
        await settle(t);
        expect(find.byKey(const Key('error-title')), findsOneWidget);
        await t.tap(find.byKey(const Key('error-retype')));
        await settle(t);
        expect(find.byKey(const Key('share-card')), findsOneWidget);
        expect(find.text('Show with Comet Cast'), findsOneWidget);
        await finish(t, r);
      },
    );

    testWidgets('a link: host and path in mono, Link as the meta line', (
      t,
    ) async {
      final r = await share(t, link);
      expect(find.text('example.org/talks/video.mp4'), findsOneWidget);
      expect(find.text('Link'), findsOneWidget);
      await t.enterText(find.byKey(const Key('code-field')), 'K7QM2W');
      await tapText(t, 'Show');
      r.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      expect(r.core.presentedContent.single, isA<ContentSource_Link>());
      await finish(t, r);
    });

    testWidgets('Office: the meta line names the kind and the conversion', (
      t,
    ) async {
      final r = await share(t, docx());
      expect(find.text('Word · 390.6 KB · shown as a PDF'), findsOneWidget);
      await finish(t, r);
    });
  });

  group('connected', () {
    Future<R5> connected(WidgetTester t, SharedItem item) async {
      final r = await pumpR5(t, office: true, code: 'K7QM2W');
      await tapText(t, 'Connect');
      r.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
      r.intake.arrive(item);
      await settle(t);
      return r;
    }

    testWidgets('Show on {room} presents in one tap', (t) async {
      final r = await connected(t, pdf());
      expect(find.text('Show on “Elbe”'), findsOneWidget);
      expect(find.text('Another screen'), findsOneWidget);
      expect(r.core.presentedContent, isEmpty);
      await tapKey(t, 'share-show');
      expect(r.core.presentedContent, hasLength(1));
      expect(r.core.startPresenterCalls, 1);
      await finish(t, r);
    });

    testWidgets('Another screen leaves the room and shows the start variant', (
      t,
    ) async {
      final r = await connected(t, pdf());
      await tapKey(t, 'share-other');
      expect(r.core.stopPresenterCalls, 1);
      expect(find.byKey(const Key('code-field')), findsOneWidget);
      expect(find.text('Show'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets(
      'while presenting the item replaces what is shown: Stop, then present',
      (t) async {
        final r = await connected(t, pdf());
        r.core.presenterEvents.add(const PresenterUiEvent.presenting());
        await settle(t);
        expect(find.byKey(const Key('presenting-title')), findsOneWidget);
        r.intake.arrive(link);
        await settle(t);
        expect(find.byKey(const Key('share-show')), findsOneWidget);
        await tapKey(t, 'share-show');
        expect(r.core.stopPresentingCalls, 1);
        expect(r.core.presentedContent.last, isA<ContentSource_Link>());
        await finish(t, r);
      },
    );

    testWidgets('Office: while it converts the meta line says so', (t) async {
      final r = await connected(t, docx());
      await tapKey(t, 'share-show');
      r.core.presenterEvents.add(
        const PresenterUiEvent.converting(name: 'plan.docx'),
      );
      await settle(t);
      expect(find.byKey(const Key('share-converting')), findsOneWidget);
      expect(find.byKey(const Key('share-meta')), findsNothing);
      await finish(t, r);
    });
  });

  group('refusals', () {
    testWidgets(
      'too large: the existing text replaces the meta line, only Cancel',
      (t) async {
        final r = await share(
          t,
          const SharedItem(
            kind: SharedKind.tooLarge,
            name: 'big.pdf',
            mime: 'application/pdf',
          ),
        );
        expect(find.byKey(const Key('share-error')), findsOneWidget);
        expect(find.byKey(const Key('share-meta')), findsNothing);
        expect(find.byKey(const Key('share-cancel')), findsOneWidget);
        expect(find.byKey(const Key('share-show')), findsNothing);
        expect(find.byKey(const Key('code-field')), findsNothing);
        await finish(t, r);
      },
    );

    testWidgets('text without a web address: share.noLink', (t) async {
      final r = await share(t, const SharedItem(kind: SharedKind.noLink));
      expect(find.text('This text has no web address in it.'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('Office with the switch off: sendReject.officeOff', (t) async {
      final intake = FakeShareIntake()..item = docx();
      final r = await pumpR5(t, office: true, shareIntake: intake);
      r.settings.officeConvert = false;
      r.intake.arrive(docx());
      await settle(t);
      expect(
        find.text(
          'Converting Office files is turned off in the settings. Pick a PDF instead.',
        ),
        findsOneWidget,
      );
      expect(find.byKey(const Key('share-cancel')), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('Cancel closes the page: back to the app it came from', (
      t,
    ) async {
      final r = await share(t, const SharedItem(kind: SharedKind.noLink));
      await tapKey(t, 'share-cancel');
      expect(r.log, ['leave']);
      await finish(t, r);
    });

    testWidgets('system Back on the share page is Cancel', (t) async {
      final r = await share(t, pdf());
      await t.binding.handlePopRoute();
      await settle(t);
      expect(r.log, ['leave']);
      expect(r.intake.discarded, [pdf().path]);
      await finish(t, r);
    });

    testWidgets('a refusal of the core after the join: the text, only Cancel', (
      t,
    ) async {
      final r = await share(t, pdf());
      await t.enterText(find.byKey(const Key('code-field')), 'K7QM2W');
      await tapText(t, 'Show');
      r.core.presenterEvents
        ..add(const PresenterUiEvent.joined(roomName: 'Elbe'))
        ..add(
          const PresenterUiEvent.contentRejected(
            reason: UiSendRejection.documentUnusable,
          ),
        );
      await settle(t);
      expect(find.byKey(const Key('share-error')), findsOneWidget);
      expect(find.byKey(const Key('share-cancel')), findsOneWidget);
      await finish(t, r);
    });
  });

  test('sizes: one decimal, MB from 1 MiB, decimal comma in German', () {
    expect(formatShareSize(1887437, 'en'), '1.8 MB');
    expect(formatShareSize(1887437, 'de'), '1,8 MB');
    expect(formatShareSize(2048, 'en'), '2.0 KB');
  });

  test('a long name is shortened in the middle keeping the extension', () {
    final s = shortenMiddle('Quarterly report 2026 final version.pdf');
    expect(s, endsWith('ion.pdf'));
    expect(s, contains('…'));
    expect(s.runes.length, lessThanOrEqualTo(28));
    expect(shortenMiddle('short.pdf'), 'short.pdf');
  });

  test('file kinds by name and MIME type', () {
    expect(shareKindOfFile('a.PNG', null)!.$1, ShareKind.image);
    expect(shareKindOfFile('a.pptx', null)!.$2, OfficeKind.powerpoint);
    expect(shareKindOfFile('a.xlsx', null)!.$2, OfficeKind.excel);
    expect(shareKindOfFile('a.zip', 'application/zip'), isNull);
  });
}
