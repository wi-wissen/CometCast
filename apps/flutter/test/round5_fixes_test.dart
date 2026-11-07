// SPDX-License-Identifier: GPL-3.0-or-later
// Fix list round 5-2 (spec 15.14, N97-N118): the items that are testable without a device.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/share_intake.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/src/branding_info.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/presenter/file_picker.dart';
import 'package:cometcast/src/presenter/nearby.dart' show NearbyCard;

import 'nearby_support.dart' show settle;
import 'round5_support.dart';
import 'share_page_test.dart' show docx, pdf;
import 'tv_test.dart' show airplay, cast, connectTv, desktop, see;
import 'ways_support.dart';

void main() {
  tearDown(() => BrandingInfo.current = const BrandingInfo.generated());

  group('U1 state lines of a switch row (N97)', () {
    testWidgets('Locked wins over failed: both rows say "Paused"', (t) async {
      final h = await ways(t);
      await openSettings(t);
      await toggle(t, 'airplay');
      await rxEvent(t, h, const AirplayUiEvent.failed(code: 'no_dnssd'));
      expect(
        find.text(
          'Could not start. Switch it off and on again, or restart the app.',
        ),
        findsOneWidget,
      );
      h.core.receiverEvents.add(
        const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.locked),
      );
      await pump(t, 100);
      expect(find.text('Paused while Locked is on.'), findsOneWidget);
      expect(find.textContaining('Could not start'), findsNothing);
    });

    testWidgets('the German failed text is short and the same in both sets', (
      t,
    ) async {
      final de = await AppLocalizations.delegate.load(const Locale('de'));
      expect(
        de.settingsReceiveFailed,
        'Startet nicht. Aus- und wieder einschalten oder App neu starten.',
      );
    });

    testWidgets('the track takes the knob colour within 120 ms', (t) async {
      await ways(t);
      await openSettings(t);
      final box = find.descendant(
        of: find.byKey(const Key('settings-way-airplay')),
        matching: find.byWidgetPredicate(
          (w) =>
              w is AnimatedContainer &&
              w.constraints?.maxWidth == 44 &&
              w.constraints?.maxHeight == 24,
        ),
      );
      Color? fill() {
        final c = t.widget<AnimatedContainer>(box);
        return (c.decoration as BoxDecoration?)?.color;
      }

      final before = fill();
      await toggle(t, 'airplay');
      await pump(t, 200);
      expect(fill(), isNot(before));
      final tokens = DisplayTokens.of(Appearance.dark, Brightness.dark);
      expect(fill(), tokens.acc);
    });
  });

  group('U13 one request queue (N111)', () {
    testWidgets(
      'an AirPlay request, then a Comet Cast request: AirPlay first',
      (t) async {
        final h = await ways(t);
        await openSettings(t);
        await toggle(t, 'airplay');
        await t.tap(find.byKey(const Key('settings-close')));
        await pump(t);
        await rxEvent(t, h, const AirplayUiEvent.advertising());
        h.core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.confirm),
        );
        await pump(t, 100);
        await rxEvent(t, h, const AirplayUiEvent.request());
        h.core.receiverEvents.add(
          ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
        );
        await pump(t, 100);
        expect(find.text('An AirPlay device'), findsOneWidget);
        expect(find.text('“Lena”'), findsNothing);
        await pump(t, 1100);
        await t.tap(find.byKey(const Key('request-yes')));
        await pump(t, 100);
        expect(h.core.airplayCalls, contains('answer:true'));
        // The Comet Cast request is next.
        expect(find.text('An AirPlay device'), findsNothing);
        expect(find.text('“Lena”'), findsOneWidget);
      },
    );

    testWidgets(
      'a Comet Cast request, then an AirPlay request: Comet Cast first',
      (t) async {
        final h = await ways(t);
        await openSettings(t);
        await toggle(t, 'airplay');
        await t.tap(find.byKey(const Key('settings-close')));
        await pump(t);
        await rxEvent(t, h, const AirplayUiEvent.advertising());
        h.core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.confirm),
        );
        await pump(t, 100);
        h.core.receiverEvents.add(
          ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
        );
        await pump(t, 100);
        await rxEvent(t, h, const AirplayUiEvent.request());
        expect(find.text('“Lena”'), findsOneWidget);
        expect(find.text('An AirPlay device'), findsNothing);
        await pump(t, 1100);
        await t.tap(find.byKey(const Key('request-no')));
        await pump(t, 100);
        expect(find.text('An AirPlay device'), findsOneWidget);
      },
    );
  });

  group('U5 PIN dialogs (N110)', () {
    testWidgets('the PIN starts at the body text\'s left edge', (t) async {
      final h = await ways(t);
      await openSettings(t);
      await toggle(t, 'airplay');
      await t.tap(find.byKey(const Key('settings-close')));
      await pump(t);
      await rxEvent(t, h, const AirplayUiEvent.advertising());
      await rxEvent(t, h, const AirplayUiEvent.pin(pin: '4821'));
      final body = t.getTopLeft(find.byKey(const Key('pin-body'))).dx;
      final pin = t.getTopLeft(find.byKey(const Key('pin-value'))).dx;
      expect(pin, body);
    });
  });

  group('U11 publisher and legal links (N108)', () {
    testWidgets('empty branding: no publisher, no Privacy, no Imprint', (
      t,
    ) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      expect(find.byKey(const Key('about-publisher')), findsNothing);
      expect(find.byKey(const Key('about-privacy')), findsNothing);
      expect(find.byKey(const Key('about-imprint')), findsNothing);
      expect(find.byKey(const Key('about-source')), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('official branding: "Publisher: {name}", Privacy, Imprint', (
      t,
    ) async {
      BrandingInfo.current = BrandingInfo.official;
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      expect(find.text('Publisher: Schulverwalter GmbH'), findsOneWidget);
      expect(find.byKey(const Key('about-privacy')), findsOneWidget);
      expect(find.byKey(const Key('about-imprint')), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('German: "Herausgeber: {name}"', (t) async {
      BrandingInfo.current = BrandingInfo.official;
      final r = await pumpR5(t, office: true, locale: const Locale('de'));
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      expect(find.text('Herausgeber: Schulverwalter GmbH'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('a publisher without URLs: only the publisher line', (t) async {
      BrandingInfo.current = const BrandingInfo(
        publisherName: 'Example e.V.',
        sourceUrl: 'https://git.example/me/cast',
      );
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      expect(find.text('Publisher: Example e.V.'), findsOneWidget);
      expect(find.byKey(const Key('about-privacy')), findsNothing);
      expect(find.byKey(const Key('about-imprint')), findsNothing);
      await finish(t, r);
    });

    testWidgets(
      'display About: publisher line, web line only with a privacy URL',
      (t) async {
        BrandingInfo.current = BrandingInfo.official;
        await ways(t);
        await openSettings(t);
        await t.tap(find.byKey(const Key('settings-about')));
        await pump(t);
        expect(find.text('Publisher: Schulverwalter GmbH'), findsOneWidget);
        expect(find.byKey(const Key('about-web')), findsOneWidget);
        await t.pumpWidget(const SizedBox());
        BrandingInfo.current = const BrandingInfo(
          publisherName: 'X',
          imprintUrlDe: 'https://x.example/imprint',
          sourceUrl: 'https://github.com/wi-wissen/CometCast',
        );
        await ways(t);
        await openSettings(t);
        await t.tap(find.byKey(const Key('settings-about')));
        await pump(t);
        expect(find.byKey(const Key('about-web')), findsNothing);
      },
    );
  });

  group('U6 and U7 About (N102, N104)', () {
    testWidgets('"Flutter engine" has the subtitle "Several licences"', (
      t,
    ) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      await t.enterText(find.byKey(const Key('licence-search')), 'flutter');
      await settle(t);
      expect(find.text('Several licences'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('phone: Back sits at the bottom, also when nothing is found', (
      t,
    ) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      await t.enterText(find.byKey(const Key('licence-search')), 'zzzzzz');
      await settle(t);
      expect(find.byKey(const Key('about-none')), findsOneWidget);
      final back = t.getBottomLeft(find.byKey(const Key('about-back')));
      expect(back.dy, greaterThan(700));
      await finish(t, r);
    });

    testWidgets('phone: the licence text page has Back at the bottom', (
      t,
    ) async {
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      await t.tap(find.text('Atkinson Hyperlegible'));
      await settle(t);
      expect(find.byKey(const Key('licence-title')), findsOneWidget);
      expect(
        t.getBottomLeft(find.byKey(const Key('licence-back'))).dy,
        greaterThan(700),
      );
      await finish(t, r);
    });

    testWidgets('the About links stand at the content\'s left edge', (t) async {
      BrandingInfo.current = BrandingInfo.official;
      final r = await pumpR5(t, office: true);
      await tapKey(t, 'app-settings-open');
      await tapKey(t, 'about-open');
      final card = t
          .getTopLeft(find.text('Free software under the GPL-3.0'))
          .dx;
      for (final k in ['about-source', 'about-privacy', 'about-imprint']) {
        expect(
          t.getTopLeft(find.byKey(Key(k))).dx,
          lessThan(card + 20),
          reason: k,
        );
      }
      await finish(t, r);
    });
  });

  group('U9 share page converting and refusal (N113)', () {
    Future<R5> connected(WidgetTester t, SharedItem item) async {
      final r = await pumpR5(t, office: true, code: 'K7QM2W');
      await tapText(t, 'Connect');
      r.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      r.intake.arrive(item);
      await settle(t);
      return r;
    }

    testWidgets('both buttons are disabled while converting', (t) async {
      final r = await connected(t, docx());
      await tapKey(t, 'share-show');
      r.core.presenterEvents.add(
        const PresenterUiEvent.converting(name: 'plan.docx'),
      );
      await settle(t);
      expect(find.byKey(const Key('share-converting')), findsOneWidget);
      final presented = r.core.presentedContent.length;
      await t.tap(find.byKey(const Key('share-show')), warnIfMissed: false);
      await t.tap(find.byKey(const Key('share-other')), warnIfMissed: false);
      await settle(t);
      expect(r.core.presentedContent.length, presented);
      expect(r.core.stopPresenterCalls, 0);
      await finish(t, r);
    });

    testWidgets('the icon disc keeps its place with a long refusal text', (
      t,
    ) async {
      double offset() =>
          t.getTopLeft(find.byKey(const Key('share-disc'))).dy -
          t.getTopLeft(find.byKey(const Key('share-card'))).dy;
      final r = await connected(t, pdf());
      final normal = offset();
      expect(normal, 15); // 1 px border + 14 padding
      await finish(t, r);
      // Office with the switch off: a refusal of several lines under the title.
      final r2 = await pumpR5(t, office: true, code: 'K7QM2W');
      r2.settings.officeConvert = false;
      await tapText(t, 'Connect');
      r2.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      r2.intake.arrive(docx());
      await settle(t);
      expect(find.byKey(const Key('share-error')), findsOneWidget);
      expect(
        t.getSize(find.byKey(const Key('share-card'))).height,
        greaterThan(84),
      );
      expect(offset(), normal);
      await finish(t, r2);
    });
  });

  group('U10 a lost TV (N114)', () {
    testWidgets('the heading says the TV is no longer connected', (t) async {
      final r = await desktop(t);
      await see(t, r, [cast('cast:1')]);
      await connectTv(t, r, 'cast:1', 'Living room TV');
      r.core.tvSessions.last.add(
        const PresenterUiEvent.error(code: 'tv_lost', message: ''),
      );
      await settle(t);
      expect(
        find.text('“Living room TV” is no longer connected'),
        findsOneWidget,
      );
      expect(find.text('The connection to the TV was lost.'), findsOneWidget);
      await finish(t, r);
    });
  });

  group('U12 accessible name and picker filters (N103, N109)', () {
    testWidgets('a Beta TV row reads "name, way, Beta"', (t) async {
      final handle = t.ensureSemantics();
      final r = await desktop(t);
      await see(t, r, [airplay('airplay:1')]);
      expect(
        t.getSemantics(find.byKey(const Key('tv-row-airplay:1'))).label,
        '“Apple TV”, AirPlay, Beta',
      );
      await finish(t, r);
      handle.dispose();
    });

    testWidgets('the root passes a named filter for every kind', (t) async {
      // Office on, a document is picked: the full set.
      final r = await pumpR5(t, office: true, code: 'K7QM2W');
      await tapText(t, 'Connect');
      r.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      await tapKey(t, 'what-file');
      await tapText(t, 'Choose file');
      expect(r.picks, [PickKind.imagePdfOffice]);
      expect(r.pickLabels, ['Images, PDF and Office files']);
      // The switch off: images and PDF only.
      r.settings.officeConvert = false;
      await settle(t);
      await tapText(t, 'Choose file');
      expect(r.picks.last, PickKind.imagePdf);
      expect(r.pickLabels.last, 'Images and PDF');
      await finish(t, r);
    });

    testWidgets('photos for a TV and PDF only have names too', (t) async {
      final r = await desktop(t);
      await see(t, r, [airplay('airplay:1')]);
      await connectTv(t, r, 'airplay:1', 'Apple TV');
      await tapKey(t, 'what-photo');
      await tapText(t, 'Choose file');
      expect(r.picks, [PickKind.photo]);
      expect(r.pickLabels, ['Photos']);
      await finish(t, r);
      final de = await AppLocalizations.delegate.load(const Locale('de'));
      expect(de.sendContentPickerPhotos, 'Fotos');
      expect(de.sendContentPickerPdf, 'PDF');
      expect(de.sendContentPickerTypesNoOffice, 'Bilder und PDF');
    });
  });

  group('U14 share Cancel (N112)', () {
    testWidgets('Back (connected) returns to the caller, the join stays', (
      t,
    ) async {
      final r = await pumpR5(t, office: true, code: 'K7QM2W', leaveApp: false);
      await tapText(t, 'Connect');
      r.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      r.intake.arrive(pdf());
      await settle(t);
      await t.binding.handlePopRoute();
      await settle(t);
      expect(r.intake.returned, 1);
      expect(r.core.stopPresenterCalls, 0);
      expect(find.text('What would you like to show?'), findsOneWidget);
      await finish(t, r);
    });

    testWidgets('Cancel on a refusal returns to the caller too', (t) async {
      final intake = FakeShareIntake()
        ..item = const SharedItem(kind: SharedKind.noLink);
      final r = await pumpR5(t, shareIntake: intake, leaveApp: false);
      await settle(t);
      await tapKey(t, 'share-cancel');
      expect(intake.returned, 1);
      await finish(t, r);
    });
  });

  group('U2 and U3 first-run layout (N98, N99)', () {
    testWidgets('laptop: the brand row spans both columns', (t) async {
      final r = await pumpR5(
        t,
        size: const Size(1100, 800),
        desktop: true,
        withNearby: true,
        tvs: true,
        locale: const Locale('de'),
      );
      await settle(t);
      final brand = t.getRect(find.byKey(const Key('brand')));
      final settings = t.getRect(find.byKey(const Key('app-settings-open')));
      final card = t.getRect(find.byType(NearbyCard));
      expect(settings.right, closeTo(card.right, 0.5));
      expect(brand.right, closeTo(card.right, 0.5));
      // "Comet Cast" on one line.
      final title = t.getSize(find.text('Comet Cast').first);
      expect(title.height, lessThan(40));
      await finish(t, r);
    });

    testWidgets('the address line is a box 44 high from the code field', (
      t,
    ) async {
      final r = await pumpR5(t);
      final field = t.getRect(find.byKey(const Key('code-field')));
      final box = t.getRect(find.byKey(const Key('address-line-box')));
      expect(box.height, 44);
      // The code field's widget may stand inside a column with its own bottom space.
      expect(box.top - field.bottom, lessThan(30));
      final text = t.getRect(find.byKey(const Key('address-line')));
      expect(text.center.dy, closeTo(box.center.dy, 1));
      await finish(t, r);
    });

    testWidgets('link and policy variants have the text at the same y', (
      t,
    ) async {
      final r = await pumpR5(t);
      final y1 = t.getRect(find.byKey(const Key('address-line'))).center.dy;
      await finish(t, r);
      final r2 = await pumpR5(t, locked: true);
      final y2 = t.getRect(find.byKey(const Key('address-line'))).center.dy;
      expect(y2, closeTo(y1, 0.5));
      await finish(t, r2);
    });
  });

  group('U4 licence viewer marking (N100)', () {
    Future<void> viewer(WidgetTester t, Appearance look) async {
      await ways(t, appearance: look);
      await openSettings(t);
      await t.tap(find.byKey(const Key('settings-about')));
      await pump(t);
      await t.tap(find.byKey(const Key('about-licences')));
      await pump(t, 100);
      await pump(t, 100);
    }

    BoxDecoration rowDecoration(WidgetTester t, String key) {
      final box = find.descendant(
        of: find.byKey(ValueKey('licence-row-$key')),
        matching: find.byWidgetPredicate(
          (w) =>
              w is Container &&
              w.decoration is BoxDecoration &&
              (w.decoration as BoxDecoration).border != null,
        ),
      );
      return t.widget<Container>(box.first).decoration! as BoxDecoration;
    }

    testWidgets('OLED black: the current row has a 2 px accent border', (
      t,
    ) async {
      await viewer(t, Appearance.oled);
      final tok = DisplayTokens.of(Appearance.oled, Brightness.dark);
      final d = rowDecoration(t, 'f:Cisco OpenH264');
      final border = d.border! as Border;
      expect(border.top.width, 2);
      expect(border.top.color, tok.acc);
      // The other rows keep the border space but it is clear.
      final other =
          rowDecoration(t, 'f:Atkinson Hyperlegible').border! as Border;
      expect(other.top.width, 2);
      expect(other.top.color, Colors.transparent);
    });

    testWidgets('the pane text does not move when the pane gains focus', (
      t,
    ) async {
      await viewer(t, Appearance.dark);
      final before = t.getTopLeft(find.byKey(const Key('licence-text')));
      await key(t, LogicalKeyboardKey.arrowRight);
      expect(focusLabel(), 'licence:pane');
      expect(t.getTopLeft(find.byKey(const Key('licence-text'))), before);
      expect(find.byKey(const Key('licence-pane-ring')), findsOneWidget);
    });
  });
}
