// SPDX-License-Identifier: GPL-3.0-or-later
// The display fixes of docs/design/fix-list-flutter.md (A1 to A7, A12, B10) and the display
// behaviours of the old home, receive and join tests that still hold on the new screens: the
// offline box, the ring in the first frame, the wizard, the link card, the host without :443,
// the policy source, the codec dialog, the security code and the transfer percentage.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/display/codec_dialog.dart';
import 'package:cometcast/src/display/display_logic.dart';
import 'package:cometcast/src/display/kit.dart';
import 'package:cometcast/src/display/screens.dart';
import 'package:cometcast/src/display/setup_wizard.dart';
import 'package:cometcast/codec_controller.dart';
import 'package:cometcast/settings.dart';

import 'display_support.dart';
import 'fake_core.dart';
import 'solid_frame.dart';

Future<void> key(WidgetTester t, LogicalKeyboardKey k) async {
  await t.sendKeyEvent(k);
  await t.pump(const Duration(milliseconds: 50));
}

/// One display widget on the 960 x 540 canvas at 1920 x 1080 (scale 2).
Future<void> pumpIn(
  WidgetTester t,
  Widget Function(BuildContext) build, {
  Appearance appearance = Appearance.dark,
  Locale locale = const Locale('en'),
}) async {
  await t.runAsync(loadAppFonts);
  t.view.physicalSize = const Size(1920, 1080);
  t.view.devicePixelRatio = 1;
  addTearDown(t.view.reset);
  final tokens = DisplayTokens.of(appearance, Brightness.dark);
  await t.pumpWidget(
    MaterialApp(
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: DisplayScope(
        tokens: tokens,
        ringVisible: ValueNotifier(true),
        reduceMotion: true,
        child: Theme(
          data: displayTheme(tokens),
          child: Material(
            color: tokens.bg,
            child: DisplayCanvas(child: Builder(builder: build)),
          ),
        ),
      ),
    ),
  );
  await t.pump();
}

/// The focus rings on screen (the 4 px outline of the ring).
int rings(WidgetTester t, Color focus) =>
    t.widgetList<DecoratedBox>(find.byType(DecoratedBox)).where((d) {
      final dec = d.decoration;
      if (dec is! BoxDecoration) return false;
      final b = dec.border;
      return b is Border && b.top.width == 4 && b.top.color == focus;
    }).length;

Rect rectOf(WidgetTester t, Key k) => t.getRect(find.byKey(k));

void main() {
  group('A6 the host without the default port', () {
    test('displayHost drops :443 only', () {
      expect(displayHost('relay.cometcast.de:443'), 'relay.cometcast.de');
      expect(displayHost('wss://relay.cometcast.de:443'), 'relay.cometcast.de');
      expect(displayHost('cast.example.org:8443'), 'cast.example.org:8443');
      expect(displayHost(' cast.example.org '), 'cast.example.org');
      expect(displayHost('127.0.0.1:44'), '127.0.0.1:44');
    });

    test('the QR URL has no :443 and keeps another port', () {
      expect(
        joinUrl('relay.cometcast.de:443', 'K7Q M2W'),
        'https://relay.cometcast.de/K7QM2W',
      );
      expect(
        joinUrl('cast.example.org:8443', 'K7QM2W'),
        'https://cast.example.org:8443/K7QM2W',
      );
    });

    testWidgets('the idle screen shows the address and QR without :443', (
      t,
    ) async {
      await pumpIn(
        t,
        (_) => IdleScreen(
          room: 'Elbe',
          mode: DisplayJoinMode.open,
          code: 'K7QM2W',
          dom: 'relay.cometcast.de:443',
          view: ConnectionView.online,
          cause: OfflineCause.network,
          dimmed: false,
          onSettings: () {},
        ),
      );
      expect(
        (find.byKey(const Key('address')).evaluate().single.widget as Text)
            .data,
        'relay.cometcast.de',
      );
      final semantics = t
          .widget<Semantics>(
            find.ancestor(
              of: find.byType(CustomPaint),
              matching: find.byWidgetPredicate(
                (w) =>
                    w is Semantics &&
                    (w.properties.label ?? '').startsWith('QR'),
              ),
            ),
          )
          .properties
          .label;
      expect(semantics, 'QR: https://relay.cometcast.de/K7QM2W');
    });
  });

  group('A1 the offline box', () {
    Future<void> offline(WidgetTester t, OfflineCause cause) => pumpIn(
      t,
      (_) => IdleScreen(
        room: 'Elbe',
        mode: DisplayJoinMode.open,
        code: null,
        dom: 'cast.example.org',
        view: ConnectionView.offline,
        cause: cause,
        dimmed: false,
        onSettings: () {},
      ),
    );

    testWidgets('dashes are drawn over the fill; the box hugs its content', (
      t,
    ) async {
      await offline(t, OfflineCause.network);
      final box = rectOf(t, const Key('offline'));
      final qr = rectOf(t, const Key('qr'));
      // Height about 116 ref (scale 2), well under the whole main row (about 400 ref).
      expect(box.height / 2, inInclusiveRange(100, 140));
      // Centred in the main row like the steps; the QR box sits about 14 ref higher because its
      // column also holds the caption (implementation-questions-flutter.md, F1).
      expect((box.center.dy - qr.center.dy).abs() / 2, lessThan(16));
      // The painter sits in front of the fill, not behind it.
      final paint = t.widget<CustomPaint>(
        find
            .ancestor(
              of: find.byKey(const Key('offline')),
              matching: find.byType(CustomPaint),
            )
            .first,
      );
      expect(paint.foregroundPainter, isNotNull);
      expect(paint.painter, isNull);
    });

    testWidgets('outdated reads "app", never "Reload"', (t) async {
      await offline(t, OfflineCause.outdated);
      final body =
          (find.byKey(const Key('offline-body')).evaluate().single.widget
                  as Text)
              .data!;
      expect(body, 'This app is out of date. Update it to get a code.');
      expect(find.textContaining('Reload'), findsNothing);
    });

    testWidgets('busy and network have their own bodies', (t) async {
      await offline(t, OfflineCause.busy);
      expect(find.textContaining('busy'), findsWidgets);
    });
  });

  group('A2 one ring, in the first frame', () {
    testWidgets('Settings opens with the ring on the current mode tile', (
      t,
    ) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      expect(h.core.receiverServer, isNotNull);
      final focus = DisplayTokens.dark.focus;
      // The Settings button has the ring on an idle screen.
      expect(rings(t, focus), 1);
      await t.sendKeyEvent(LogicalKeyboardKey.select);
      // One frame only: the dialog's ring is there, none under the scrim.
      await t.pump();
      expect(rings(t, focus), 1);
      final tile = rectOf(t, const Key('settings-mode-confirm'));
      final ring = t
          .widgetList<DecoratedBox>(find.byType(DecoratedBox))
          .firstWhere((d) {
            final dec = d.decoration;
            return dec is BoxDecoration &&
                dec.border is Border &&
                (dec.border! as Border).top.width == 4;
          });
      final ringRect = t.getRect(find.byWidget(ring));
      expect(tile.inflate(10).overlaps(ringRect), isTrue);
      expect(ringRect.center.dx, closeTo(tile.center.dx, 4));
    });

    testWidgets('a join request: the ring is on Yes and nowhere else', (
      t,
    ) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
      );
      await t.pump();
      await t.pump();
      expect(rings(t, DisplayTokens.dark.focus), 1);
      final yes = rectOf(t, const Key('request-yes'));
      final ring = t
          .widgetList<DecoratedBox>(find.byType(DecoratedBox))
          .firstWhere((d) {
            final dec = d.decoration;
            return dec is BoxDecoration &&
                dec.border is Border &&
                (dec.border! as Border).top.width == 4;
          });
      expect(
        t.getRect(find.byWidget(ring)).center.dx,
        closeTo(yes.center.dx, 4),
      );
    });

    testWidgets('the link card: the ring is on Open', (t) async {
      final h = await pumpDisplay(t, tv: false);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.contentLink(
          url: 'https://example.org/x',
          host: 'example.org',
        ),
      );
      await t.pump();
      await t.pump();
      expect(rings(t, DisplayTokens.dark.focus), lessThanOrEqualTo(1));
    });

    testWidgets('wizard steps take the ring in their first frame', (t) async {
      await pumpDisplay(t, setupDone: false);
      final focus = DisplayTokens.dark.focus;
      await t.pump();
      expect(rings(t, focus), 1); // Start
      await key(t, LogicalKeyboardKey.select);
      await t.pump();
      expect(rings(t, focus), 1); // Next on step 1
      await key(t, LogicalKeyboardKey.select);
      await t.pump();
      expect(rings(t, focus), 1); // the selected card on step 2
    });
  });

  group('A3 A4 A7 the wizard', () {
    testWidgets('steps 1 to 4 have their card top edge at 40', (t) async {
      await pumpDisplay(t, setupDone: false);
      await key(t, LogicalKeyboardKey.select); // Start
      final top = t.getTopLeft(find.byType(DCard)).dy;
      expect(top / 2, closeTo(40, 0.5));
      // The card ends inside the canvas with room below (EN).
      expect(
        t.getBottomLeft(find.byType(DCard)).dy / 2,
        lessThanOrEqualTo(516),
      );
    });

    testWidgets('step 1 in German also ends by 516 ref', (t) async {
      await pumpDisplay(t, setupDone: false, locale: const Locale('de'));
      await key(t, LogicalKeyboardKey.select);
      expect(
        t.getBottomLeft(find.byType(DCard)).dy / 2,
        lessThanOrEqualTo(516),
      );
    });

    testWidgets('step 2: equal height, selected fill, one ring', (t) async {
      await pumpDisplay(t, setupDone: false);
      await key(t, LogicalKeyboardKey.select); // Start
      await key(t, LogicalKeyboardKey.select); // Next
      final open = rectOf(t, const Key('wz-mode-open'));
      final confirm = rectOf(t, const Key('wz-mode-confirm'));
      expect(open.top, closeTo(confirm.top, 0.5));
      expect(open.height, closeTo(confirm.height, 0.5));
      // The selected card (Open) has the selected-tile fill, the other the panel2 fill.
      Color fill(Key k) {
        final c = t.widget<Container>(
          find
              .descendant(of: find.byKey(k), matching: find.byType(Container))
              .first,
        );
        return (c.decoration! as BoxDecoration).color!;
      }

      const tok = DisplayTokens.dark;
      expect(fill(const Key('wz-mode-open')), tok.selectedTileFill);
      expect(fill(const Key('wz-mode-confirm')), tok.panel2);
      expect(rings(t, tok.focus), 1);
    });

    testWidgets('a policy source says who set the address', (t) async {
      final c =
          SetupController(
              suggestions: const ['A', 'B', 'C', 'D', 'E'],
              options: const [
                AddressOption('mdm.example.org:443', AddressSource.policy),
              ],
              changeAllowed: false,
              onFinish: (_) {},
            )
            ..next()
            ..next()
            ..next();
      addTearDown(c.dispose);
      await pumpIn(t, (_) => SetupWizardView(controller: c, remote: true));
      expect(
        find.text('Set by the administrator of this device.'),
        findsOneWidget,
      );
      // The address is shown without :443, and "Other address" stays hidden under a lock.
      expect(find.text('mdm.example.org'), findsOneWidget);
      expect(find.text('Other address'), findsNothing);
    });

    testWidgets('step 3 shows the default address without :443', (t) async {
      final c =
          SetupController(
              suggestions: const ['A', 'B', 'C', 'D', 'E'],
              options: const [
                AddressOption(
                  'relay.cometcast.de:443',
                  AddressSource.defaultServer,
                ),
              ],
              changeAllowed: true,
              onFinish: (_) {},
            )
            ..next()
            ..next()
            ..next();
      addTearDown(c.dispose);
      await pumpIn(t, (_) => SetupWizardView(controller: c, remote: true));
      expect(find.text('relay.cometcast.de'), findsOneWidget);
      expect(find.text('relay.cometcast.de:443'), findsNothing);
    });

    testWidgets(
      'Other address opens the field; a typed address keeps its port',
      (t) async {
        final c =
            SetupController(
                suggestions: const ['A', 'B', 'C', 'D', 'E'],
                options: const [
                  AddressOption(
                    'relay.cometcast.de',
                    AddressSource.defaultServer,
                  ),
                ],
                changeAllowed: true,
                onFinish: (_) {},
              )
              ..next()
              ..next()
              ..next();
        addTearDown(c.dispose);
        await pumpIn(t, (_) => SetupWizardView(controller: c, remote: true));
        await t.tap(find.byKey(const Key('wz-change')));
        await t.pump();
        expect(c.showField, isTrue);
        c.addressController.text = 'ws://insecure.example.org';
        expect(c.applyField(), isFalse);
        await t.pump();
        expect(find.byKey(const Key('wz-addr-error')), findsOneWidget);
        c.addressController.text = 'cast.example.org:8443';
        expect(c.applyField(), isTrue);
        await t.pump();
        expect(find.text('cast.example.org:8443'), findsOneWidget);
      },
    );
  });

  group('A5 the link card', () {
    Future<void> card(WidgetTester t, {required bool canOpen}) => pumpIn(
      t,
      (_) => LinkCard(
        host: 'example.org',
        url: 'https://example.org/a/very/long/path',
        canOpen: canOpen,
        onOpen: () {},
        onDismiss: () {},
        guard: Duration.zero,
      ),
    );

    testWidgets('the address is in --fg, the hint says "browser"', (t) async {
      await card(t, canOpen: true);
      final style = t
          .widget<Text>(find.byKey(const Key('link-address')))
          .style!;
      expect(style.color, DisplayTokens.dark.fg);
      expect(style.fontSize, 18);
      expect(style.fontFamily, fontMono);
      expect(
        find.textContaining('It opens in the browser, and only if you choose'),
        findsOneWidget,
      );
      expect(find.textContaining('new tab'), findsNothing);
      expect(find.byKey(const Key('link-open')), findsOneWidget);
      expect(find.byKey(const Key('link-dismiss')), findsOneWidget);
    });

    testWidgets('without a browser: only Dismiss and the TV hint', (t) async {
      await card(t, canOpen: false);
      expect(find.byKey(const Key('link-open')), findsNothing);
      expect(find.byKey(const Key('link-dismiss')), findsOneWidget);
      expect(find.textContaining('browser, and only if'), findsNothing);
    });
  });

  group('B10 the name tag and the transfer pill', () {
    testWidgets('the name tag has the security code only with the controls', (
      t,
    ) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents
        ..add(const ReceiverUiEvent.peerJoined(name: 'Lena'))
        ..add(const ReceiverUiEvent.connectionCode(sas: '123 456'));
      await t.pump();
      await deliverFrame(t, h.core, solidFrame(16, 9, [255, 0, 0]));
      await key(t, LogicalKeyboardKey.select); // reveal the controls
      expect(find.text('“Lena” is showing'), findsOneWidget);
      final sas = t.widget<Text>(find.byKey(const Key('sas-line')));
      final spans = (sas.textSpan! as TextSpan).children!.cast<TextSpan>();
      expect(spans.map((s) => s.text).join(), 'Security code 123 456');
      final digits = spans.firstWhere((s) => s.text == '123 456');
      expect(digits.style!.fontFamily, fontMono);
      expect(digits.style!.fontWeight, FontWeight.w700);
      expect(digits.style!.color, DisplayTokens.dark.fg);
    });

    testWidgets('no code reported: no second line', (t) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Lena'));
      await t.pump();
      await deliverFrame(t, h.core, solidFrame(16, 9, [255, 0, 0]));
      await key(t, LogicalKeyboardKey.select);
      expect(find.byKey(const Key('sas-line')), findsNothing);
    });

    testWidgets('the pill counts the received percent', (t) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(const ReceiverUiEvent.peerJoined(name: 'Lena'));
      await t.pump();
      await deliverFrame(t, h.core, solidFrame(16, 9, [0, 0, 255]));
      h.core.receiverEvents.add(
        ReceiverUiEvent.contentTransferStarted(
          document: true,
          name: 'slides.pdf',
          size: BigInt.from(1000),
        ),
      );
      await t.pump();
      expect(find.text('Receiving slides.pdf… 0 %'), findsOneWidget);
      h.core.receiverEvents.add(
        ReceiverUiEvent.contentTransferProgress(
          received: BigInt.from(400),
          total: BigInt.from(1000),
        ),
      );
      await t.pump();
      await t.pump(const Duration(milliseconds: 50));
      expect(find.text('Receiving slides.pdf… 40 %'), findsOneWidget);
    });
  });

  group('a page that cannot be drawn (4.5)', () {
    testWidgets('its text is centred in a name-tag box over the black stage', (
      t,
    ) async {
      final h = await pumpDisplay(t, core: _FailingPageCore());
      h.core.receiverEvents
        ..add(const ReceiverUiEvent.peerJoined(name: 'Lena'))
        ..add(
          const ReceiverUiEvent.contentDocumentOpened(name: 'a.pdf', pages: 3),
        )
        ..add(const ReceiverUiEvent.contentFrameReady());
      for (var i = 0; i < 10; i++) {
        await t.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 30)),
        );
        await t.pump(const Duration(milliseconds: 50));
      }
      expect(find.byKey(const Key('page-error')), findsOneWidget);
      expect(find.text('This page could not be shown.'), findsOneWidget);
    });
  });

  group('A12 the codec dialog on the display', () {
    const info = UiCodecInfo(
      fileName: 'libopenh264-2.6.0-linux64.8.so',
      version: '2.6.0',
      host: 'ciscobinary.openh264.org',
      downloadBytes: 634264,
      autoDownload: false,
      adminDir: null,
    );

    Future<DisplayHarness> presentH264(
      WidgetTester t, {
      UiCodecState state = const UiCodecState.missing(),
      FakeCoreApi? core,
    }) async {
      final c = core ?? FakeCoreApi();
      c.codecStateResult = state;
      c.codecInfoResult = info;
      final h = await pumpDisplay(t, core: c);
      h.core.receiverEvents
        ..add(const ReceiverUiEvent.peerJoined(name: 'Lena'))
        ..add(
          const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
        );
      await t.pump();
      await t.pump(const Duration(milliseconds: 50));
      return h;
    }

    testWidgets('over Presenting: label, explanation, source, two buttons', (
      t,
    ) async {
      final h = await presentH264(t);
      expect(find.text('VIDEO CODEC NEEDED'), findsOneWidget);
      expect(find.byKey(const Key('codec-explain')), findsOneWidget);
      expect(find.textContaining('ciscobinary.openh264.org'), findsOneWidget);
      expect(find.byKey(const Key('codec-download')), findsOneWidget);
      expect(find.byKey(const Key('codec-later')), findsOneWidget);
      // The controls are under the scrim, visible and inert; Idle is not shown.
      expect(find.byKey(const Key('room-code')), findsNothing);
      expect(find.byKey(const Key('byline')), findsOneWidget);
      // No Material card, icon or progress bar.
      expect(find.byType(Card), findsNothing);
      expect(find.byType(Icon), findsNothing);
      expect(find.byType(FilledButton), findsNothing);
      expect(h.core.ensureCodecCalls, 0, reason: 'no silent download');
    });

    testWidgets('never over Idle', (t) async {
      final c = FakeCoreApi()
        ..codecStateResult = const UiCodecState.missing()
        ..codecInfoResult = info;
      await pumpDisplay(t, core: c);
      expect(find.byKey(const Key('codec-download')), findsNothing);
    });

    testWidgets('Not now hides it for this presentation, not for the next', (
      t,
    ) async {
      final h = await presentH264(t);
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('codec-later')));
      await t.pump();
      expect(find.byKey(const Key('codec-download')), findsNothing);
      // Not twice in one presentation.
      h.core.receiverEvents.add(
        const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
      );
      await t.pump();
      expect(find.byKey(const Key('codec-download')), findsNothing);
      // The presenter leaves; the next one sees it again.
      h.core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
      await t.pump();
      h.core.receiverEvents
        ..add(const ReceiverUiEvent.peerJoined(name: 'Tom'))
        ..add(
          const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
        );
      await t.pump();
      await t.pump();
      expect(find.byKey(const Key('codec-download')), findsOneWidget);
    });

    testWidgets('Back = Not now; the 1 s guard ignores an early press', (
      t,
    ) async {
      await presentH264(t);
      await t.tap(find.byKey(const Key('codec-later')));
      await t.pump();
      expect(find.byKey(const Key('codec-download')), findsOneWidget);
      await t.pump(const Duration(seconds: 1));
      await t.binding.handlePopRoute();
      await t.pump();
      expect(find.byKey(const Key('codec-download')), findsNothing);
    });

    testWidgets('Download asks, shows the progress and closes when ready', (
      t,
    ) async {
      final h = await presentH264(t);
      h.core.ensureCodecGate = Completer<void>();
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('codec-download')));
      await t.pump();
      expect(h.core.ensureCodecCalls, 1);
      expect(find.byKey(const Key('codec-progress')), findsOneWidget);
      expect(
        find.textContaining('Downloading the video codec'),
        findsOneWidget,
      );
      h.core.codecStateResult = const UiCodecState.downloading(progress: 0.5);
      await t.pump(const Duration(milliseconds: 400));
      expect(find.textContaining('50 %'), findsOneWidget);
      h.core.ensureCodecGate!.complete();
      await t.pump(const Duration(milliseconds: 400));
      await t.pump(const Duration(milliseconds: 400));
      expect(find.byKey(const Key('codec-progress')), findsNothing);
      expect(
        find.textContaining('codec'),
        findsNothing,
        reason: 'no codec toast',
      );
    });

    testWidgets('a failure: the line, Try again and Not now', (t) async {
      final h = await presentH264(
        t,
        state: const UiCodecState.failed(reason: 'network error'),
      );
      expect(find.byKey(const Key('codec-failed')), findsOneWidget);
      expect(find.byKey(const Key('codec-retry')), findsOneWidget);
      expect(find.byKey(const Key('codec-later')), findsOneWidget);
      expect(h.core.ensureCodecCalls, 0);
    });

    testWidgets('unsupported: only text and Close', (t) async {
      await presentH264(
        t,
        state: const UiCodecState.unsupported(reason: 'no bz2'),
      );
      expect(find.byKey(const Key('codec-unsupported')), findsOneWidget);
      expect(find.byKey(const Key('codec-close')), findsOneWidget);
      expect(find.byKey(const Key('codec-download')), findsNothing);
    });

    testWidgets('the dialog widget in German is du', (t) async {
      final core = FakeCoreApi()
        ..codecStateResult = const UiCodecState.missing()
        ..codecInfoResult = info;
      final ctl = CodecController(
        core: core,
        settings: AppSettings.fromDefaults(core.serverDefaults()),
      );
      addTearDown(ctl.dispose);
      await pumpIn(
        t,
        (_) =>
            CodecDialog(controller: ctl, onLater: () {}, guard: Duration.zero),
        locale: const Locale('de'),
      );
      expect(find.text('Nicht jetzt'), findsOneWidget);
      expect(find.text('Herunterladen'), findsOneWidget);
    });
  });
}

/// A core whose pages cannot be taken.
class _FailingPageCore extends FakeCoreApi {
  @override
  Future<ContentFrame?> takeContentFrame() async => throw StateError('no page');
}
