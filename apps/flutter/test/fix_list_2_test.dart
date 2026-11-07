// SPDX-License-Identifier: GPL-3.0-or-later
// Fix list 2 (docs/design/fix-list-flutter-2.md), part A: the codec dialog in its four states
// (sizes, scrim rules, stage hint), the transfer pill, the wizard row and field error, the
// first-run links, the Miracast page and its consent line, the ring of the emptied name field.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/miracast_page.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/presenter/kit.dart';
import 'package:cometcast/src/presenter/pages.dart';

import 'display_support.dart';
import 'fake_core.dart';
import 'fake_miracast.dart';
import 'solid_frame.dart';

const _info = UiCodecInfo(
  fileName: 'libopenh264-2.6.0-linux64.8.so',
  version: '2.6.0',
  host: 'ciscobinary.openh264.org',
  downloadBytes: 634264,
  autoDownload: false,
  adminDir: null,
);

Future<DisplayHarness> _presentH264(
  WidgetTester t,
  UiCodecState state, {
  Locale locale = const Locale('en'),
}) async {
  final c = FakeCoreApi()
    ..codecStateResult = state
    ..codecInfoResult = _info;
  final h = await pumpDisplay(t, tv: false, core: c, locale: locale);
  h.core.receiverEvents
    ..add(const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false))
    ..add(const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'));
  await t.pump();
  await t.pump(const Duration(milliseconds: 300));
  return h;
}

Rect _card(WidgetTester t) => t.getRect(find.byKey(const Key('codec-card')));

void main() {
  group('A1, A2: the codec dialog', () {
    final states = {
      'missing': const UiCodecState.missing(),
      'downloading': const UiCodecState.downloading(progress: 0.45),
      'failed': const UiCodecState.failed(reason: 'network error'),
      'unsupported': const UiCodecState.unsupported(reason: 'no library'),
    };

    testWidgets('each state draws its own content', (t) async {
      await _presentH264(t, states['missing']!);
      expect(find.byKey(const Key('codec-download')), findsOneWidget);
      expect(find.byKey(const Key('codec-later')), findsOneWidget);
      expect(find.byKey(const Key('codec-explain')), findsOneWidget);
      expect(find.byKey(const Key('codec-source')), findsOneWidget);
      expect(find.byKey(const Key('codec-offline')), findsOneWidget);
      expect(find.byKey(const Key('codec-failed')), findsNothing);

      await t.pumpWidget(const SizedBox());
      await _presentH264(t, states['downloading']!);
      expect(find.byKey(const Key('codec-explain')), findsOneWidget);
      expect(find.text('Downloading the video codec … 45 %'), findsOneWidget);
      expect(find.byKey(const Key('codec-download')), findsNothing);
      // [Not now] only, with the focus.
      expect(find.byKey(const Key('codec-later')), findsOneWidget);
      // The bar: 6 ref high, the fill 45 % of the track.
      final bar = t.getRect(find.byKey(const Key('codec-progress')));
      expect(bar.height, 12, reason: '6 ref at 1920 x 1080');
      expect(find.byKey(const Key('codec-fill')), findsOneWidget);
      final fill = t.getRect(find.byKey(const Key('codec-fill')));
      expect(fill.height, greaterThan(0));
      expect(fill.width / (bar.width - 2), closeTo(0.45, 0.02));

      await t.pumpWidget(const SizedBox());
      await _presentH264(t, states['failed']!);
      // The key's text only, no reason, no red.
      expect(
        find.text('The video codec could not be downloaded.'),
        findsOneWidget,
      );
      expect(find.textContaining('network error'), findsNothing);
      expect(find.byKey(const Key('codec-retry')), findsOneWidget);
      expect(find.byKey(const Key('codec-later')), findsOneWidget);
      expect(find.byKey(const Key('codec-offline')), findsOneWidget);
      expect(find.byKey(const Key('codec-explain')), findsOneWidget);

      await t.pumpWidget(const SizedBox());
      await _presentH264(t, states['unsupported']!);
      expect(
        find.text(
          'This system cannot use the video codec, so shared screens cannot be shown or sent here.',
        ),
        findsOneWidget,
      );
      expect(find.textContaining('no library'), findsNothing);
      expect(find.byKey(const Key('codec-close')), findsOneWidget);
      expect(find.byKey(const Key('codec-explain')), findsNothing);
      expect(find.byKey(const Key('codec-source')), findsNothing);
      expect(find.byKey(const Key('codec-offline')), findsNothing);
      expect(find.byKey(const Key('codec-later')), findsNothing);
    });

    for (final locale in [const Locale('en'), const Locale('de')]) {
      testWidgets(
        '780 wide, top edge never above 66 ref (${locale.languageCode})',
        (t) async {
          for (final e in states.entries) {
            await t.pumpWidget(const SizedBox());
            await _presentH264(t, e.value, locale: locale);
            final r = _card(t);
            expect(r.width, 1560, reason: '${e.key}: 780 ref');
            expect(
              r.top,
              greaterThanOrEqualTo(132),
              reason: '${e.key}: 66 ref',
            );
            // The toast (top 14, 40 high) does not touch the card.
            final toast = t.getRect(find.byKey(const Key('toast')));
            expect(toast.bottom, lessThan(r.top), reason: e.key);
          }
        },
      );
    }

    testWidgets('its appearance does not reveal the name tag and End', (
      t,
    ) async {
      await _presentH264(t, states['missing']!);
      await t.pump(const Duration(seconds: 6));
      final tag = t.widget<AnimatedOpacity>(
        find.ancestor(
          of: find.byKey(const Key('byline')),
          matching: find.byType(AnimatedOpacity),
        ),
      );
      expect(tag.opacity, 0);
    });

    testWidgets('the explain, source and note have the sizes of 4.7', (
      t,
    ) async {
      await _presentH264(t, states['missing']!);
      TextStyle style(String k) => t.widget<Text>(find.byKey(Key(k))).style!;
      expect(style('codec-explain').fontSize, 16);
      expect(style('codec-explain').height, 1.4);
      expect(style('codec-source').fontSize, 15);
      expect(style('codec-source').height, 1.3);
      expect(style('codec-offline').fontSize, 14);
      expect(style('codec-offline').height, 1.4);
    });
  });

  group('A3: codec.stageHint after Not now', () {
    testWidgets('the line on black, at rest; it goes with the picture', (
      t,
    ) async {
      final h = await _presentH264(t, const UiCodecState.missing());
      expect(find.byKey(const Key('codec-stage-hint')), findsNothing);
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('codec-later')));
      await t.pump(const Duration(seconds: 6));
      expect(
        find.text('No picture yet: this screen needs the video codec.'),
        findsOneWidget,
      );
      final text = t.widget<Text>(find.byKey(const Key('codec-stage-hint')));
      expect(text.style!.fontSize, 15);
      expect(text.style!.color, DisplayTokens.onBlack);
      // The presenter stops: the hint goes (Idle).
      h.core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
      await t.pump();
      await t.pump();
      expect(find.byKey(const Key('codec-stage-hint')), findsNothing);
    });

    testWidgets('German text', (t) async {
      await _presentH264(
        t,
        const UiCodecState.missing(),
        locale: const Locale('de'),
      );
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('codec-later')));
      await t.pump(const Duration(seconds: 6));
      expect(
        find.text('Noch kein Bild: Dieser Bildschirm braucht den Video-Codec.'),
        findsOneWidget,
      );
    });
  });

  group('A4: the transfer pill', () {
    testWidgets(
      'content width, at most 340, bottom 12, name shortened in the middle',
      (t) async {
        final h = await pumpDisplay(t, tv: true);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
        );
        await t.pump();
        await deliverFrame(t, h.core, solidFrame(160, 90, [40, 90, 160]));
        h.core.receiverEvents
          ..add(
            ReceiverUiEvent.contentTransferStarted(
              document: true,
              name: 'slides.pdf',
              size: BigInt.from(1000),
            ),
          )
          ..add(
            ReceiverUiEvent.contentTransferProgress(
              received: BigInt.from(420),
              total: BigInt.from(1000),
            ),
          );
        await t.pump();
        await t.pump(const Duration(milliseconds: 100));
        var r = t.getRect(find.byKey(const Key('transfer-pill')));
        expect(r.width, lessThan(500), reason: 'content width, not the canvas');
        expect(r.center.dx, closeTo(960, 1));
        expect(
          1080 - r.bottom,
          closeTo(24, 1),
          reason: '12 ref from the bottom',
        );
        expect(find.text('Receiving slides.pdf… 42 %'), findsOneWidget);

        h.core.receiverEvents
          ..add(
            ReceiverUiEvent.contentTransferStarted(
              document: true,
              name: 'A very long presentation file name for the quarterly review 2026.pdf',
              size: BigInt.from(1000),
            ),
          )
          ..add(
            ReceiverUiEvent.contentTransferProgress(
              received: BigInt.from(420),
              total: BigInt.from(1000),
            ),
          );
        await t.pump();
        await t.pump(const Duration(milliseconds: 100));
        r = t.getRect(find.byKey(const Key('transfer-pill')));
        expect(r.width, lessThanOrEqualTo(680), reason: '340 ref');
        final line = t.widget<Text>(
          find.byKey(const Key('transfer-pill-text')),
        );
        expect(
          line.data,
          endsWith('… 42 %'),
          reason: 'the percentage is never cut',
        );
        expect(line.data, contains('…'));
        expect(line.data, startsWith('Receiving A very'));
        expect(
          line.data,
          contains('.pdf'),
          reason: 'the end of the name stays',
        );
      },
    );
  });

  group('A5: wizard step 3', () {
    Future<void> toStep3(WidgetTester t) async {
      await pumpDisplay(t, setupDone: false, tv: true);
      Future<void> k(LogicalKeyboardKey key) async {
        await t.sendKeyEvent(key);
        await t.pump(const Duration(milliseconds: 60));
      }

      await k(LogicalKeyboardKey.select);
      await k(LogicalKeyboardKey.select);
      await k(LogicalKeyboardKey.arrowDown);
      await k(LogicalKeyboardKey.arrowRight);
      await k(LogicalKeyboardKey.select);
    }

    testWidgets('the row is right-aligned to the content edge', (t) async {
      await toStep3(t);
      final card = t.getRect(find.byKey(const Key('wz-ok'))).right;
      // Content edge of the 780 card: 34 ref padding, centred on 1920.
      const edge = 960 + 780 - 34 * 2;
      expect(card, closeTo(edge, 2));
      final back = t.getRect(find.byKey(const Key('wz-back')));
      final change = t.getRect(find.byKey(const Key('wz-change')));
      expect(back.left, lessThan(change.left));
      expect(change.right, lessThan(card));
    });

    testWidgets('a bad typed address shows the value and the error line', (
      t,
    ) async {
      await toStep3(t);
      await t.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
      await t.pump(const Duration(milliseconds: 60));
      for (
        var i = 0;
        i < 8 && find.byKey(const Key('wz-addr')).evaluate().isEmpty;
        i++
      ) {
        await t.sendKeyEvent(LogicalKeyboardKey.select);
        await t.pump(const Duration(milliseconds: 60));
      }
      await t.sendKeyEvent(
        LogicalKeyboardKey.select,
      ); // opens the field for typing
      await t.pump(const Duration(milliseconds: 60));
      await t.enterText(find.byKey(const Key('wz-addr')), 'ws://box.local');
      await t.pump(const Duration(milliseconds: 60));
      await t.testTextInput.receiveAction(TextInputAction.done);
      await t.pump(const Duration(milliseconds: 100));
      expect(find.text('ws://box.local'), findsOneWidget);
      expect(find.byKey(const Key('wz-addr-error')), findsOneWidget);
    });
  });

  group('A11: the emptied room name field keeps its ring', () {
    testWidgets('one ring, no scale', (t) async {
      final h = await pumpDisplay(t, setupDone: false, tv: true);
      expect(h, isNotNull);
      Future<void> k(LogicalKeyboardKey key) async {
        await t.sendKeyEvent(key);
        await t.pump(const Duration(milliseconds: 60));
      }

      await k(LogicalKeyboardKey.select);
      await k(LogicalKeyboardKey.arrowUp);
      await k(LogicalKeyboardKey.select);
      await t.enterText(find.byKey(const Key('wz-room')), 'Studio');
      await t.pump(const Duration(milliseconds: 60));
      await t.enterText(find.byKey(const Key('wz-room')), '');
      await t.pump(const Duration(milliseconds: 60));
      final rings = find.byWidgetPredicate(
        (w) =>
            w is DecoratedBox &&
            w.decoration is BoxDecoration &&
            (w.decoration as BoxDecoration).border is Border &&
            ((w.decoration as BoxDecoration).border! as Border).top.width == 4,
      );
      expect(rings, findsOneWidget);
    });
  });

  group('A6: the links under Connect', () {
    Future<void> pumpFirstRun(
      WidgetTester t,
      Size size, {
      required bool laptop,
    }) async {
      await loadAppFonts();
      t.view.physicalSize = size;
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.reset);
      await t.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: presenterTheme(Brightness.light),
          home: PresenterFrame(
            child: Material(
              type: MaterialType.transparency,
              child: FirstRunPage(
                codeController: TextEditingController(),
                nameController: TextEditingController(text: 'Lena'),
                onConnect: () {},
                onBeDisplay: () {},
                onMiracast: () {},
              ),
            ),
          ),
        ),
      );
      await t.pump(const Duration(milliseconds: 100));
    }

    testWidgets('laptop: 44 apart, left-aligned, 12 below Connect', (t) async {
      await t.runAsync(loadAppFonts);
      await pumpFirstRun(t, const Size(1280, 800), laptop: true);
      final connect = t.getRect(find.widgetWithText(PPrimaryButton, 'Connect'));
      final a = t.getRect(find.byKey(const Key('be-display')));
      final b = t.getRect(find.byKey(const Key('miracast-link')));
      expect(b.top - a.top, closeTo(44, 1), reason: 'baselines 44 apart');
      // The text is centred in a 44 high area that starts 12 below Connect.
      expect(a.center.dy - connect.bottom, closeTo(12 + 22, 1.5));
      expect(
        a.left,
        closeTo(b.left, 1),
        reason: 'left-aligned with each other',
      );
      expect(a.left, closeTo(connect.left, 1), reason: 'and with Connect');
    });

    testWidgets('phone: centred, 8 below Connect', (t) async {
      await t.runAsync(loadAppFonts);
      await pumpFirstRun(t, const Size(400, 800), laptop: false);
      final connect = t.getRect(find.widgetWithText(PPrimaryButton, 'Connect'));
      final a = t.getRect(find.byKey(const Key('be-display')));
      final b = t.getRect(find.byKey(const Key('miracast-link')));
      expect(a.center.dx, closeTo(200, 1));
      expect(b.center.dx, closeTo(200, 1));
      expect(b.top - a.top, closeTo(44, 1));
      expect(a.center.dy - connect.bottom, closeTo(8 + 22, 1.5));
    });
  });

  group('A7, A8: the Miracast page', () {
    Future<FakeMiracastCore> pumpMiracast(WidgetTester t, Size size) async {
      await t.runAsync(loadAppFonts);
      t.view.physicalSize = size;
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.reset);
      final core = FakeMiracastCore();
      await t.pumpWidget(
        MaterialApp(
          locale: const Locale('en'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          theme: presenterTheme(Brightness.light),
          home: MiracastPage(
            core: core,
            initialName: 'Studio PC',
            pollInterval: const Duration(milliseconds: 20),
          ),
        ),
      );
      await t.pump(const Duration(milliseconds: 200));
      return core;
    }

    testWidgets(
      'laptop: Start receiving and Back in one row, 24 below the last note',
      (t) async {
        await pumpMiracast(t, const Size(1280, 800));
        final start = t.getRect(find.byKey(const Key('miracastStartButton')));
        final back = t.getRect(find.byKey(const Key('miracastBack')));
        expect(back.center.dy, closeTo(start.center.dy, 1));
        expect(back.left, greaterThan(start.right));
        expect(back.width, greaterThanOrEqualTo(160));
        final note = t.getRect(
          find.text(
            'Protected content (for example copy-protected video) may appear black. Picture quality and delay depend on the Wi-Fi adapter and have not been measured yet.',
          ),
        );
        expect(start.top - note.bottom, closeTo(24, 1.5));
      },
    );

    testWidgets(
      'phone: the actions stay at the bottom, 24 free below the last note',
      (t) async {
        await pumpMiracast(t, const Size(400, 640));
        final start = t.getRect(find.byKey(const Key('miracastStartButton')));
        final back = t.getRect(find.byKey(const Key('miracastBack')));
        expect(
          back.bottom,
          closeTo(640 - 22, 1),
          reason: 'pinned in the bottom area',
        );
        expect(back.top, greaterThan(start.bottom));
        // Scrolled to the end: at least 24 between the last note and the action area.
        await t.drag(
          find.byType(SingleChildScrollView).first,
          const Offset(0, -2000),
        );
        await t.pump(const Duration(milliseconds: 300));
        final note = t.getRect(find.textContaining('Protected content'));
        expect(start.top - note.bottom, greaterThanOrEqualTo(24 - 0.5));
      },
    );

    testWidgets(
      'consent: the sentence is body text, only the digits are mono 700',
      (t) async {
        final core = await pumpMiracast(t, const Size(400, 800));
        await t.tap(find.byKey(const Key('miracastStartButton')));
        await t.pump(const Duration(milliseconds: 200));
        core.request(9, 'Pixel 9', pin: '4711');
        await t.pump(const Duration(milliseconds: 200));
        await t.pump(const Duration(milliseconds: 200));
        final rich = t.widget<Text>(
          find.byKey(const Key('miracastConsentPin')),
        );
        final span = rich.textSpan! as TextSpan;
        expect(rich.style!.fontSize, 15);
        final parts = span.children!.cast<TextSpan>();
        expect(
          parts.map((p) => p.text).join(),
          'PIN: 4711. Enter it on the sending device if it asks for it.',
        );
        final digits = parts.singleWhere((p) => p.text == '4711');
        expect(digits.style!.fontWeight, FontWeight.w700);
        expect(digits.style!.fontFamily, contains('Mono'));
        expect(
          parts.where((p) => p.text != '4711').every((p) => p.style == null),
          isTrue,
        );
        // [Decline, Allow] unchanged, the initial focus is Decline.
        final deny = t.getRect(find.byKey(const Key('miracastDeny')));
        final allow = t.getRect(find.byKey(const Key('miracastAllow')));
        expect(deny.top < allow.top || deny.left < allow.left, isTrue);
      },
    );
  });
}
