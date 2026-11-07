// SPDX-License-Identifier: GPL-3.0-or-later
// Writes a screenshot of every display state and presenter page (phone, tablet, TV sizes; dark,
// light, OLED black) for the review against the spec. Runs only when COMETCAST_SHOTS names the
// output folder, for example:
//   COMETCAST_SHOTS=/tmp/shots-flutter flutter test test/screenshots_test.dart
// No golden files: nothing is compared.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/src/design/icons.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/display/display_logic.dart';
import 'package:cometcast/src/presenter/kit.dart';
import 'package:cometcast/src/presenter/pages.dart';

import 'display_support.dart';
import 'shot.dart';
import 'solid_frame.dart';

final _on = Platform.environment['COMETCAST_SHOTS'] != null;

const _tv = Size(1920, 1080);
const _tablet = Size(1280, 800);
const _phone = Size(400, 800);

Future<void> k(WidgetTester t, LogicalKeyboardKey key) async {
  await t.sendKeyEvent(key);
  await t.pump(const Duration(milliseconds: 60));
}

void main() {
  if (!_on) {
    test('screenshots are off (set COMETCAST_SHOTS)', () {}, skip: true);
    return;
  }

  final looks = {
    'dark': Appearance.dark,
    'light': Appearance.light,
    'oled': Appearance.oled,
  };

  for (final size in {'tv': _tv, 'tablet': _tablet}.entries) {
    for (final look in looks.entries) {
      final p = '${size.key}-${look.key}';
      final tv = size.key == 'tv';

      testWidgets('display $p: setup', (t) async {
        await pumpDisplay(
          t,
          setupDone: false,
          tv: tv,
          size: size.value,
          appearance: look.value,
        );
        if (!tv) {
          await t.tap(find.text('Use this device as the screen'));
          for (var i = 0; i < 5; i++) {
            await t.pump(const Duration(milliseconds: 50));
          }
        }
        await shot(t, 'display-$p-setup0-welcome');
        await k(t, LogicalKeyboardKey.select);
        await shot(t, 'display-$p-setup1-name');
        await k(t, LogicalKeyboardKey.select);
        await shot(t, 'display-$p-setup2-mode');
        await k(t, LogicalKeyboardKey.arrowDown);
        await k(t, LogicalKeyboardKey.arrowRight);
        await k(t, LogicalKeyboardKey.select);
        await shot(t, 'display-$p-setup3-address');
        await k(t, LogicalKeyboardKey.select);
        await shot(t, 'display-$p-setup4-done');
      });

      testWidgets('display $p: idle states and dialogs', (t) async {
        final h = await pumpDisplay(
          t,
          tv: tv,
          size: size.value,
          appearance: look.value,
          mode: DefaultMode.confirm,
        );
        await shot(t, 'display-$p-idle');
        await k(t, LogicalKeyboardKey.select);
        await shot(t, 'display-$p-settings');
        await k(t, LogicalKeyboardKey.escape);
        h.core.receiverEvents
          ..add(
            ReceiverUiEvent.joinRequest(
              id: BigInt.one,
              name: 'Lena Müller',
              check: 42,
            ),
          )
          ..add(
            ReceiverUiEvent.joinRequest(id: BigInt.two, name: 'Tom', check: 17),
          );
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, 'display-$p-request');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.locked),
        );
        await t.pump(const Duration(milliseconds: 100));
        await t.pump(const Duration(seconds: 2));
        await t.pump(const Duration(milliseconds: 1100));
        await t.tap(find.byKey(const Key('request-no')));
        await t.pump(const Duration(milliseconds: 100));
        await t.tap(find.byKey(const Key('request-no')));
        await t.pump(const Duration(milliseconds: 1100));
        await t.tap(find.byKey(const Key('request-no')), warnIfMissed: false);
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, 'display-$p-idle-locked-toast');
      });

      testWidgets('display $p: connecting, offline, dimmed', (t) async {
        await pumpDisplay(
          t,
          tv: tv,
          size: size.value,
          appearance: look.value,
          start: false,
        );
        await shot(t, 'display-$p-idle-connecting');
        await t.pump(const Duration(seconds: 9));
        await shot(t, 'display-$p-idle-offline');
      });

      testWidgets('display $p: dimmed', (t) async {
        await pumpDisplay(t, tv: tv, size: size.value, appearance: look.value);
        await t.pump(const Duration(minutes: 7, seconds: 3));
        await shot(t, 'display-$p-idle-dimmed');
      });

      testWidgets('display $p: presenting and link card', (t) async {
        final h = await pumpDisplay(
          t,
          tv: tv,
          size: size.value,
          appearance: look.value,
        );
        h.core.receiverEvents.add(
          const ReceiverUiEvent.contentLink(
            url: 'https://example.org/slides',
            host: 'example.org',
          ),
        );
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, 'display-$p-linkcard');
        await k(t, LogicalKeyboardKey.escape);
        h.core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Lena'),
        );
        await t.pump();
        await deliverFrame(t, h.core, solidFrame(160, 90, [40, 90, 160]));
        await shot(t, 'display-$p-presenting-rest');
        await k(t, LogicalKeyboardKey.arrowDown);
        await t.pump(const Duration(milliseconds: 500));
        await shot(t, 'display-$p-presenting-controls');
      });
    }
  }

  // Presenter pages: phone and tablet, light and dark.
  for (final size in {'phone': _phone, 'tablet': _tablet}.entries) {
    for (final b in [Brightness.light, Brightness.dark]) {
      final p = '${size.key}-${b.name}';
      testWidgets('presenter $p', (t) async {
        await t.runAsync(loadAppFonts);
        t.view.physicalSize = size.value;
        t.view.devicePixelRatio = 1;
        t.platformDispatcher.platformBrightnessTestValue = b;
        addTearDown(t.view.reset);
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        final code = TextEditingController();
        final name = TextEditingController(text: 'Lena');
        final url = TextEditingController();
        Future<void> page(String n, Widget w) async {
          await t.pumpWidget(
            MaterialApp(
              debugShowCheckedModeBanner: false,
              locale: const Locale('en'),
              localizationsDelegates: const [
                AppLocalizations.delegate,
                GlobalMaterialLocalizations.delegate,
                GlobalWidgetsLocalizations.delegate,
                GlobalCupertinoLocalizations.delegate,
              ],
              supportedLocales: const [Locale('en'), Locale('de')],
              theme: presenterTheme(Brightness.light),
              darkTheme: presenterTheme(Brightness.dark),
              home: PresenterFrame(
                child: Material(type: MaterialType.transparency, child: w),
              ),
            ),
          );
          await t.pump(const Duration(milliseconds: 100));
          await shot(t, 'presenter-$p-$n');
        }

        void nop() {}
        await page(
          'firstrun-code',
          FirstRunPage(
            codeController: code,
            nameController: name,
            onConnect: nop,
            onBeDisplay: size.key == 'tablet' ? nop : null,
          ),
        );
        await page(
          'firstrun-chip',
          FirstRunPage(
            codeController: code,
            nameController: name,
            onConnect: nop,
            codeChip: 'K7Q M2W',
          ),
        );
        await page(
          'firstrun-empty-error',
          FirstRunPage(
            codeController: code,
            nameController: name,
            onConnect: nop,
            codeEmpty: true,
          ),
        );
        await page(
          'waiting',
          WaitingPage(
            check: 42,
            secondsLeft: 41,
            totalSeconds: 60,
            onWithdraw: nop,
          ),
        );
        await page(
          'what',
          WhatPage(
            device: size.key == 'tablet'
                ? PresenterDevice.computer
                : PresenterDevice.phone,
            onScreen: nop,
            onFile: nop,
            onLink: nop,
            onDisconnect: nop,
          ),
        );
        await page('file', FilePage(onPick: nop, onBack: nop));
        await page(
          'file-progress',
          FilePage(
            onPick: nop,
            onBack: nop,
            progressName: 'slides.pdf',
            progressPercent: 40,
          ),
        );
        await page('link', LinkPage(controller: url, onGo: nop, onBack: nop));
        await page(
          'link-error',
          LinkPage(
            controller: url,
            onGo: nop,
            onBack: nop,
            error: 'Please enter a link that starts with http:// or https://.',
          ),
        );
        await page(
          'presenting-screen',
          PresentingPage(kind: PresentingKind.screen, onStop: nop),
        );
        await page(
          'presenting-pdf',
          PresentingPage(
            kind: PresentingKind.file,
            pdf: true,
            onStop: nop,
            onPrev: nop,
            onNext: nop,
          ),
        );
        await page('bye', ByePage(onAgain: nop));
        await page(
          'relay',
          RelayConfirmPage(
            host: 'relay.example.org',
            onCancel: nop,
            onUse: nop,
          ),
        );
        for (final kind in ErrorKind.values) {
          await page(
            'error-${kind.name}',
            ErrorPage(
              kind: kind,
              onAction: (_) {},
              otherWhy: 'The server refused this request.',
              cooldown: kind == ErrorKind.denied ? 9 : 0,
              canRescan: size.key == 'phone',
            ),
          );
        }
        await page('icons', const _IconSheet());
      });
    }
  }
}

class _IconSheet extends StatelessWidget {
  const _IconSheet();
  @override
  Widget build(BuildContext context) {
    final p = PresenterScope.of(context).tokens;
    return Padding(
      padding: const EdgeInsets.all(24),
      child: Wrap(
        spacing: 16,
        runSpacing: 16,
        children: [
          for (final n in CcIconName.values)
            CcIcon(n, color: p.primary, size: 48),
        ],
      ),
    );
  }
}
