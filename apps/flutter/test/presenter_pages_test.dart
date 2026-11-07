// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter pages themselves (fix list A9 to A16, B10): order and placeholder of the first-run
// page, the scan result, the PDF page row and the disabled look, the screen row by device kind,
// the file page, the outdated page in the apps, the relay page and the codec page. Plain widgets,
// no core.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/codec_controller.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/presenter/kit.dart';
import 'package:cometcast/src/presenter/pages.dart';

import 'display_support.dart';
import 'fake_core.dart';

Future<void> pumpPage(
  WidgetTester t,
  Widget page, {
  Size size = const Size(400, 800),
  Locale locale = const Locale('en'),
  Brightness brightness = Brightness.light,
}) async {
  await t.runAsync(loadAppFonts);
  t.view.physicalSize = size;
  t.view.devicePixelRatio = 1;
  t.platformDispatcher.platformBrightnessTestValue = brightness;
  addTearDown(t.view.reset);
  addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
  await t.pumpWidget(
    MaterialApp(
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: PresenterFrame(
        child: Material(type: MaterialType.transparency, child: page),
      ),
    ),
  );
  await t.pump();
}

void nop() {}

FirstRunPage firstRun({
  TextEditingController? code,
  String? chip,
  bool chipFromLink = true,
  VoidCallback? onScan,
  bool scanWrong = false,
  bool scanAgainBody = false,
  VoidCallback? onBeDisplay,
  VoidCallback? onMiracast,
}) => FirstRunPage(
  codeController: code ?? TextEditingController(),
  nameController: TextEditingController(),
  codeChip: chip,
  chipFromLink: chipFromLink,
  onScan: onScan,
  scanWrong: scanWrong,
  scanAgainBody: scanAgainBody,
  onBeDisplay: onBeDisplay,
  onMiracast: onMiracast,
  onConnect: nop,
);

double top(WidgetTester t, Finder f) => t.getTopLeft(f).dy;

void main() {
  group('A9 the first-run page', () {
    testWidgets('order: brand, heading, name, scan, code, Connect, links', (
      t,
    ) async {
      await pumpPage(
        t,
        firstRun(onScan: nop, onBeDisplay: nop, onMiracast: nop),
        size: const Size(1024, 900),
      );
      final order = [
        find.byKey(const Key('brand')),
        find.text('Welcome to Comet Cast'),
        find.byKey(const Key('name-field')),
        find.byKey(const Key('scan-button')),
        find.byKey(const Key('code-field')),
        find.text('Connect'),
        find.byKey(const Key('be-display')),
        find.byKey(const Key('miracast-link')),
      ];
      final ys = [for (final f in order) top(t, f)];
      for (var i = 1; i < ys.length; i++) {
        expect(ys[i], greaterThan(ys[i - 1]), reason: 'item $i');
      }
      // The two text links are stacked, the display link first (decision N18).
      expect(find.text('Use this device as the screen'), findsOneWidget);
      expect(find.text('Receive Miracast on this PC'), findsOneWidget);
    });

    testWidgets('the brand row has the logo and the product name', (t) async {
      await pumpPage(t, firstRun());
      expect(find.text('Comet Cast'), findsOneWidget);
      final logo = t.getSize(find.byType(SvgPicture));
      expect(logo.width, closeTo(28, 0.01));
    });

    testWidgets('the empty code field shows XXX XXX, typing removes it', (
      t,
    ) async {
      final c = TextEditingController();
      await pumpPage(t, firstRun(code: c));
      final hint = find.byKey(const Key('field-hint'));
      expect(hint, findsOneWidget);
      final style = t.widget<Text>(hint).style!;
      expect(t.widget<Text>(hint).data, 'XXX XXX');
      expect(style.color, PresenterTokens.of(Brightness.light).fg3);
      expect(style.fontSize, 22);
      expect(style.fontFamily, fontMono);
      await t.enterText(find.byKey(const Key('code-field')), 'K7Q');
      await t.pump();
      expect(hint, findsNothing);
    });

    testWidgets('the placeholder also shows in the dark appearance', (t) async {
      await pumpPage(t, firstRun(), brightness: Brightness.dark);
      expect(find.byKey(const Key('field-hint')), findsOneWidget);
    });

    testWidgets('a phone without links: Connect is the last item', (t) async {
      await pumpPage(t, firstRun());
      expect(find.byKey(const Key('be-display')), findsNothing);
      expect(find.byKey(const Key('miracast-link')), findsNothing);
      expect(find.byKey(const Key('scan-button')), findsNothing);
    });

    testWidgets('the chip replaces the scan button and the field', (t) async {
      await pumpPage(t, firstRun(chip: 'K7Q M2W', onScan: nop));
      expect(find.byKey(const Key('code-chip')), findsOneWidget);
      expect(find.text('taken from your link'), findsOneWidget);
      expect(find.byKey(const Key('code-field')), findsNothing);
      expect(find.byKey(const Key('scan-button')), findsNothing);
    });

    testWidgets('a scanned chip says so', (t) async {
      await pumpPage(
        t,
        firstRun(chip: 'K7Q M2W', chipFromLink: false, onScan: nop),
      );
      expect(find.text('scanned'), findsOneWidget);
    });

    testWidgets('scan wrong: the line under the scan button', (t) async {
      await pumpPage(t, firstRun(onScan: nop, scanWrong: true));
      expect(
        find.text('This QR code is not from a Comet Cast screen.'),
        findsOneWidget,
      );
      expect(
        top(t, find.byKey(const Key('scan-wrong'))),
        greaterThan(top(t, find.byKey(const Key('scan-button')))),
      );
      final s = t.widget<Text>(find.byKey(const Key('scan-wrong'))).style!;
      expect(s.fontSize, 13);
      expect(s.fontWeight, FontWeight.w700);
      expect(s.color, PresenterTokens.of(Brightness.light).error);
    });

    testWidgets('scan-again text only on the first launch without a code', (
      t,
    ) async {
      await pumpPage(t, firstRun(onScan: nop, scanAgainBody: true));
      expect(find.byKey(const Key('scan-again-body')), findsOneWidget);
      await pumpPage(t, firstRun(onScan: nop));
      expect(find.byKey(const Key('scan-again-body')), findsNothing);
    });
  });

  group('A10 the PDF page row', () {
    PresentingPage pdf({int? page, int? pages}) => PresentingPage(
      kind: PresentingKind.file,
      pdf: true,
      page: page,
      pages: pages,
      room: 'Meeting room Elbe',
      sas: '123 456',
      onPrev: nop,
      onNext: nop,
      onStop: nop,
    );

    testWidgets('phone: half width each, side by side above Stop', (t) async {
      await pumpPage(t, pdf());
      final prev = t.getRect(find.byKey(const Key('prev-page')));
      final next = t.getRect(find.byKey(const Key('next-page')));
      expect(prev.top, next.top);
      expect((prev.width - next.width).abs(), lessThan(1));
      expect(prev.width, greaterThan(150));
      expect(next.left - prev.right, closeTo(8, 0.5));
      expect(prev.bottom, lessThan(top(t, find.text('Stop showing'))));
    });

    testWidgets('laptop: content width, left-aligned, 8 apart', (t) async {
      await pumpPage(t, pdf(), size: const Size(1280, 800));
      final prev = t.getRect(find.byKey(const Key('prev-page')));
      final next = t.getRect(find.byKey(const Key('next-page')));
      final stop = t.getRect(find.text('Stop showing'));
      expect(prev.width, lessThan(260));
      expect(prev.width, greaterThanOrEqualTo(160));
      expect(next.left - prev.right, closeTo(8, 0.5));
      expect(prev.left, closeTo(stop.left - 24, 30));
    });

    testWidgets('page 1 of 3: the line shows and Previous is disabled', (
      t,
    ) async {
      await pumpPage(t, pdf(page: 1, pages: 3));
      expect(find.text('Page 1 of 3'), findsOneWidget);
      final tokens = PresenterTokens.of(Brightness.light);
      final box = t.widget<Container>(
        find
            .descendant(
              of: find.byKey(const Key('prev-page')),
              matching: find.byType(Container),
            )
            .first,
      );
      final dec = box.decoration! as BoxDecoration;
      expect(dec.color, tokens.disabled);
      final label = t.widget<Text>(find.text('Previous page'));
      expect(label.style!.color, tokens.onDisabled);
      expect(label.style!.fontSize, 16);
      expect(label.style!.fontWeight, FontWeight.w500);
      // Next stays a normal secondary button.
      final next = t.widget<Container>(
        find
            .descendant(
              of: find.byKey(const Key('next-page')),
              matching: find.byType(Container),
            )
            .first,
      );
      expect((next.decoration! as BoxDecoration).color, tokens.surface);
    });

    testWidgets('the last page disables Next', (t) async {
      await pumpPage(t, pdf(page: 3, pages: 3));
      final tokens = PresenterTokens.of(Brightness.light);
      final next = t.widget<Container>(
        find
            .descendant(
              of: find.byKey(const Key('next-page')),
              matching: find.byType(Container),
            )
            .first,
      );
      expect((next.decoration! as BoxDecoration).color, tokens.disabled);
    });

    testWidgets('before the display reported: no line, both enabled', (
      t,
    ) async {
      await pumpPage(t, pdf());
      expect(find.byKey(const Key('page-of')), findsNothing);
      final tokens = PresenterTokens.of(Brightness.light);
      for (final k in const [Key('prev-page'), Key('next-page')]) {
        final c = t.widget<Container>(
          find
              .descendant(of: find.byKey(k), matching: find.byType(Container))
              .first,
        );
        expect((c.decoration! as BoxDecoration).color, tokens.surface);
      }
    });

    testWidgets('where and the security code, digits in mono 700', (t) async {
      await pumpPage(t, pdf());
      expect(find.text('On “Meeting room Elbe”.'), findsOneWidget);
      final rich = t.widget<Text>(find.byKey(const Key('presenting-sas')));
      final spans = (rich.textSpan! as TextSpan).children!.cast<TextSpan>();
      final digits = spans.firstWhere((s) => s.text == '123 456');
      expect(digits.style!.fontFamily, fontMono);
      expect(digits.style!.fontWeight, FontWeight.w700);
      expect(digits.style!.color, PresenterTokens.of(Brightness.light).fg);
      expect(
        spans.map((s) => s.text).join(),
        startsWith('Security code 123 456'),
      );
    });

    testWidgets('no room name, no code: neither line', (t) async {
      await pumpPage(
        t,
        const PresentingPage(kind: PresentingKind.screen, onStop: nop),
      );
      expect(find.byKey(const Key('presenting-where')), findsNothing);
      expect(find.byKey(const Key('presenting-sas')), findsNothing);
    });
  });

  group('A11 the screen row by device kind', () {
    for (final e in {
      PresenterDevice.phone: 'Your whole phone display, live',
      PresenterDevice.tablet: 'Your whole tablet display, live',
      PresenterDevice.computer: 'Whole screen or one window, live',
    }.entries) {
      testWidgets('${e.key.name}: ${e.value}', (t) async {
        await pumpPage(
          t,
          WhatPage(
            device: e.key,
            onScreen: nop,
            onFile: nop,
            onLink: nop,
            onDisconnect: nop,
          ),
        );
        expect(find.text(e.value), findsOneWidget);
      });
    }

    testWidgets('a tablet in the wide layout still says tablet', (t) async {
      await pumpPage(
        t,
        WhatPage(
          device: PresenterDevice.tablet,
          onScreen: nop,
          onFile: nop,
          onLink: nop,
          onDisconnect: nop,
        ),
        size: const Size(1280, 800),
      );
      expect(find.text('Your whole tablet display, live'), findsOneWidget);
    });

    testWidgets('the header: tick disc and the room, or only Connected', (
      t,
    ) async {
      await pumpPage(
        t,
        WhatPage(
          device: PresenterDevice.phone,
          room: 'Meeting room Elbe',
          onScreen: nop,
          onFile: nop,
          onLink: nop,
          onDisconnect: nop,
        ),
      );
      expect(find.text('Connected to “Meeting room Elbe”'), findsOneWidget);
      final disc = t.getSize(
        find.descendant(
          of: find.byType(Row).first,
          matching: find.byType(PIconDisc),
        ),
      );
      expect(disc, const Size(40, 40));
      expect(
        top(t, find.byKey(const Key('connected-to'))),
        lessThan(top(t, find.text('What would you like to show?'))),
      );
    });
  });

  group('A14 the file page', () {
    testWidgets('desktops name Office files', (t) async {
      await pumpPage(t, FilePage(onPick: nop, onBack: nop, office: true));
      expect(find.textContaining('Office file'), findsOneWidget);
    });

    testWidgets('without conversion the browser text stays', (t) async {
      await pumpPage(t, FilePage(onPick: nop, onBack: nop));
      expect(find.textContaining('Office file'), findsNothing);
    });

    testWidgets('converting: one small line, no bar, no second pick', (
      t,
    ) async {
      await pumpPage(
        t,
        FilePage(
          onPick: nop,
          onBack: nop,
          office: true,
          convertingName: 'slides.pptx',
        ),
      );
      expect(find.text('Converting slides.pptx …'), findsOneWidget);
      expect(find.byType(PProgressBar), findsNothing);
    });

    testWidgets('sending: name, percent and a bar', (t) async {
      await pumpPage(
        t,
        FilePage(
          onPick: nop,
          onBack: nop,
          progressName: 'a.pdf',
          progressPercent: 30,
        ),
      );
      expect(find.text('a.pdf · 30 %'), findsOneWidget);
      expect(find.byType(PProgressBar), findsOneWidget);
    });
  });

  group('A15 A16 the apps never say page or Reload', () {
    testWidgets('outdated: app texts and Try again', (t) async {
      var action = ErrorAction.retype;
      await pumpPage(
        t,
        ErrorPage(kind: ErrorKind.outdated, onAction: (a) => action = a),
      );
      expect(find.text('This app is out of date'), findsOneWidget);
      expect(find.text('Update the app, then try again.'), findsOneWidget);
      expect(
        find.text('The server was updated in the meantime.'),
        findsOneWidget,
      );
      expect(find.textContaining('Reload'), findsNothing);
      await t.tap(find.text('Try again'));
      expect(action, ErrorAction.retry);
    });

    testWidgets('outdated in German is du', (t) async {
      await pumpPage(
        t,
        ErrorPage(kind: ErrorKind.outdated, onAction: (_) {}),
        locale: const Locale('de'),
      );
      expect(find.text('Diese App ist veraltet'), findsOneWidget);
      expect(find.textContaining('Aktualisiere die App'), findsOneWidget);
    });

    testWidgets('the relay page: "this app", the host and Cancel first', (
      t,
    ) async {
      await pumpPage(
        t,
        RelayConfirmPage(host: 'relay.example.org', onCancel: nop, onUse: nop),
      );
      expect(
        find.textContaining('A link asks this app to use the relay'),
        findsOneWidget,
      );
      expect(find.byKey(const Key('relay-host')), findsOneWidget);
      expect(
        top(t, find.byKey(const Key('relay-cancel'))),
        lessThanOrEqualTo(top(t, find.byKey(const Key('relay-use')))),
      );
      // Cancel has the initial focus (decision 48).
      final focused = FocusManager.instance.primaryFocus!.context!;
      expect(
        focused.findAncestorWidgetOfExactType<PSecondaryButton>()?.key,
        const Key('relay-cancel'),
      );
    });

    testWidgets('the relay page in German is du', (t) async {
      await pumpPage(
        t,
        RelayConfirmPage(host: 'relay.example.org', onCancel: nop, onUse: nop),
        locale: const Locale('de'),
      );
      expect(find.textContaining('Mach nur weiter'), findsOneWidget);
    });
  });

  group('every error page (6)', () {
    for (final kind in ErrorKind.values) {
      testWidgets('${kind.name}: icon, heading, why, now, a button', (t) async {
        await pumpPage(
          t,
          ErrorPage(
            kind: kind,
            canRescan: true,
            otherWhy: kind == ErrorKind.other ? 'Because.' : null,
            onAction: (_) {},
          ),
        );
        expect(find.byKey(const Key('error-title')), findsOneWidget);
        expect(find.byKey(const Key('error-why')), findsOneWidget);
        expect(find.byKey(const Key('error-now')), findsOneWidget);
        expect(find.byType(PIconDisc), findsOneWidget);
        expect(find.byType(PPrimaryButton), findsOneWidget);
      });
    }

    testWidgets('Scan again only where the scanner exists', (t) async {
      await pumpPage(
        t,
        ErrorPage(kind: ErrorKind.code, canRescan: true, onAction: (_) {}),
      );
      expect(find.byKey(const Key('error-rescan')), findsOneWidget);
      await pumpPage(t, ErrorPage(kind: ErrorKind.code, onAction: (_) {}));
      expect(find.byKey(const Key('error-rescan')), findsNothing);
    });
  });

  group('A12 the codec page', () {
    const info = UiCodecInfo(
      fileName: 'libopenh264-2.6.0-linux64.8.so',
      version: '2.6.0',
      host: 'ciscobinary.openh264.org',
      downloadBytes: 634264,
      autoDownload: false,
      adminDir: null,
    );

    Future<(FakeCoreApi, CodecController)> pumpCodec(
      WidgetTester t,
      UiCodecState state, {
      VoidCallback? onScreen,
      Locale locale = const Locale('en'),
    }) async {
      final core = FakeCoreApi()
        ..codecStateResult = state
        ..codecInfoResult = info;
      final ctl = CodecController(
        core: core,
        settings: AppSettings.fromDefaults(core.serverDefaults()),
        pollInterval: const Duration(milliseconds: 10),
      );
      addTearDown(ctl.dispose);
      await pumpPage(
        t,
        CodecPage(controller: ctl, onBack: nop, onScreen: onScreen ?? nop),
        size: const Size(1280, 800),
        locale: locale,
      );
      return (core, ctl);
    }

    void noMaterial(WidgetTester t) {
      expect(find.byType(Card), findsNothing);
      expect(find.byType(FilledButton), findsNothing);
      expect(find.byType(LinearProgressIndicator), findsNothing);
      expect(find.byType(Icon), findsNothing);
    }

    testWidgets('asks first: Download primary, Back secondary, no download', (
      t,
    ) async {
      final (core, _) = await pumpCodec(t, const UiCodecState.missing());
      expect(find.text('Video codec needed'), findsOneWidget);
      expect(find.byKey(const Key('codec-download')), findsOneWidget);
      expect(find.byKey(const Key('codec-back')), findsOneWidget);
      expect(find.textContaining('ciscobinary.openh264.org'), findsOneWidget);
      expect(find.textContaining('619 KB'), findsOneWidget);
      expect(find.textContaining('COMETCAST_CODEC_DIR'), findsOneWidget);
      expect(core.ensureCodecCalls, 0);
      noMaterial(t);
      // Download is the primary (left), Back follows (laptop row, primary first).
      expect(
        top(t, find.byKey(const Key('codec-download'))),
        top(t, find.byKey(const Key('codec-back'))),
      );
      expect(
        t.getTopLeft(find.byKey(const Key('codec-download'))).dx,
        lessThan(t.getTopLeft(find.byKey(const Key('codec-back'))).dx),
      );
    });

    testWidgets('Download starts the download and remembers the consent', (
      t,
    ) async {
      final (core, _) = await pumpCodec(t, const UiCodecState.missing());
      core.ensureCodecGate = Completer<void>();
      await t.tap(find.byKey(const Key('codec-download')));
      await t.pump();
      expect(core.ensureCodecCalls, 1);
      // Downloading: the primary is disabled with its own label, a progress line shows.
      expect(find.text('Downloading the video codec …'), findsOneWidget);
      core.codecStateResult = const UiCodecState.downloading(progress: 0.5);
      await t.pump(const Duration(milliseconds: 30));
      expect(find.byKey(const Key('codec-progress-text')), findsOneWidget);
      expect(find.text('50 %'), findsOneWidget);
      expect(find.byType(PProgressBar), findsOneWidget);
      // Back stays enabled while the download goes on.
      expect(find.byKey(const Key('codec-back')), findsOneWidget);
      noMaterial(t);
      core.ensureCodecGate!.complete();
      await t.pumpAndSettle();
    });

    testWidgets('failed: the inline error and the retry button', (t) async {
      await pumpCodec(t, const UiCodecState.failed(reason: 'network error'));
      expect(find.byKey(const Key('codec-failed')), findsOneWidget);
      final style = t
          .widget<Text>(find.byKey(const Key('codec-failed')))
          .style!;
      expect(style.color, PresenterTokens.of(Brightness.light).error);
      expect(find.byKey(const Key('codec-retry')), findsOneWidget);
      noMaterial(t);
    });

    testWidgets('ready: the primary is the screen button', (t) async {
      var opened = 0;
      await pumpCodec(t, const UiCodecState.ready(), onScreen: () => opened++);
      expect(find.byKey(const Key('codec-screen')), findsOneWidget);
      expect(find.text('Share screen'), findsOneWidget);
      await t.tap(find.byKey(const Key('codec-screen')));
      expect(opened, 1);
    });

    testWidgets('unsupported: the other page with the codec text', (t) async {
      await pumpCodec(t, const UiCodecState.unsupported(reason: 'no bz2'));
      expect(find.text('Something went wrong'), findsOneWidget);
      expect(
        find.textContaining('This system has no video codec to download'),
        findsOneWidget,
      );
      noMaterial(t);
    });

    testWidgets('German is du', (t) async {
      await pumpCodec(
        t,
        const UiCodecState.missing(),
        locale: const Locale('de'),
      );
      expect(find.text('Herunterladen'), findsOneWidget);
    });
  });

  group('the kit', () {
    testWidgets(
      'a disabled primary stays readable, a secondary has no border',
      (t) async {
        await pumpPage(
          t,
          const PPage(
            actions: [
              PPrimaryButton(label: 'Go', onPressed: null),
              PSecondaryButton(label: 'Back', onPressed: null),
            ],
            children: [],
          ),
        );
        final tokens = PresenterTokens.of(Brightness.light);
        final sec = t.widget<Container>(
          find
              .descendant(
                of: find.byType(PSecondaryButton),
                matching: find.byType(Container),
              )
              .first,
        );
        final dec = sec.decoration! as BoxDecoration;
        expect(dec.color, tokens.disabled);
        expect((dec.border! as Border).top.color, Colors.transparent);
      },
    );
  });
}
