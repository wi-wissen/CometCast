// SPDX-License-Identifier: GPL-3.0-or-later
// Writes a screenshot of every display state and presenter page (phone, tablet, TV sizes; dark,
// light, OLED black) for the review against the spec. Runs only when COMETCAST_SHOTS names the
// output folder, for example:
//   COMETCAST_SHOTS=/tmp/shots-flutter flutter test test/screenshots_test.dart
// No golden files: nothing is compared.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/codec_controller.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/miracast_page.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/design/icons.dart';
import 'package:cometcast/src/design/tokens.dart';
import 'package:cometcast/src/display/display_logic.dart';
import 'package:cometcast/src/presenter/kit.dart';
import 'package:cometcast/src/presenter/pages.dart';

import 'display_support.dart';
import 'fake_core.dart';
import 'fake_miracast.dart';
import 'nearby_support.dart';
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

  const codecInfo = UiCodecInfo(
    fileName: 'libopenh264-2.6.0-linux64.8.so',
    version: '2.6.0',
    host: 'ciscobinary.openh264.org',
    downloadBytes: 634264,
    autoDownload: false,
    adminDir: null,
  );

  for (final size in {'tv': _tv, 'tablet': _tablet}.entries) {
    for (final look in looks.entries) {
      final p = '${size.key}-${look.key}';
      final tv = size.key == 'tv';

      shotTest('display $p: setup', (t) async {
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

      shotTest('display $p: setup field states', (t) async {
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
        await k(t, LogicalKeyboardKey.select); // step 1
        // The custom field is reached with the arrow keys and opened with OK (a field that is
        // not open takes no text), typed in and emptied again: no chip is selected.
        await k(t, LogicalKeyboardKey.arrowUp);
        await k(t, LogicalKeyboardKey.select);
        await t.enterText(find.byKey(const Key('wz-room')), 'Studio');
        await t.pump(const Duration(milliseconds: 60));
        await shot(t, 'display-$p-setup1-name-typed');
        await t.enterText(find.byKey(const Key('wz-room')), '');
        await t.pump(const Duration(milliseconds: 60));
        await shot(t, 'display-$p-setup1-name-empty');
      });

      shotTest('display $p: setup address field', (t) async {
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
        await k(t, LogicalKeyboardKey.select);
        await k(t, LogicalKeyboardKey.select);
        await k(t, LogicalKeyboardKey.arrowDown);
        await k(t, LogicalKeyboardKey.arrowRight);
        await k(t, LogicalKeyboardKey.select); // step 3
        // "Other address" cycles the found addresses first, then opens the field.
        await k(t, LogicalKeyboardKey.arrowLeft);
        for (
          var i = 0;
          i < 8 && find.byKey(const Key('wz-addr')).evaluate().isEmpty;
          i++
        ) {
          await k(t, LogicalKeyboardKey.select);
        }
        await shot(t, 'display-$p-setup3-field');
        // OK opens the field for typing; Done on a bad address shows the error line.
        await k(t, LogicalKeyboardKey.select);
        await t.enterText(find.byKey(const Key('wz-addr')), 'ws://box.local');
        await t.pump(const Duration(milliseconds: 60));
        await t.testTextInput.receiveAction(TextInputAction.done);
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, 'display-$p-setup3-field-error');
      });

      shotTest('display $p: idle states and dialogs', (t) async {
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
        h.core.receiverEvents.add(
          ReceiverUiEvent.joinRequest(
            id: BigInt.one,
            name: 'Lena Müller',
            check: 42,
          ),
        );
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, 'display-$p-request-single');
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

      shotTest('display $p: connecting, offline, dimmed', (t) async {
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

      shotTest('display $p: offline busy and outdated', (t) async {
        for (final e in {
          'busy': 'rate_limited',
          'outdated': 'proto_mismatch',
        }.entries) {
          final h = await pumpDisplay(
            t,
            tv: tv,
            size: size.value,
            appearance: look.value,
            start: false,
          );
          h.core.receiverEvents.add(
            ReceiverUiEvent.error(
              code: e.value,
              message: 'server error ${e.value}: x',
            ),
          );
          await t.pump(const Duration(milliseconds: 300));
          // The box appears when the attempt after 3 s of silence has failed (spec 4.2); the
          // retry timer has not fired yet.
          await t.pump(const Duration(seconds: 3));
          await shot(t, 'display-$p-idle-offline-${e.key}');
          // A fresh display for the next one (the widget tree is kept otherwise).
          await t.pumpWidget(const SizedBox());
        }
      });

      shotTest('display $p: locked, no toast', (t) async {
        final h = await pumpDisplay(
          t,
          tv: tv,
          size: size.value,
          appearance: look.value,
        );
        h.core.receiverEvents.add(
          const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.locked),
        );
        await t.pump(const Duration(milliseconds: 200));
        await shot(t, 'display-$p-idle-locked');
      });

      shotTest('display $p: dimmed', (t) async {
        await pumpDisplay(t, tv: tv, size: size.value, appearance: look.value);
        // 7 minutes of quiet, then the 2 s fade in real frames.
        await t.pump(const Duration(minutes: 7));
        for (var i = 0; i < 21; i++) {
          await t.pump(const Duration(milliseconds: 100));
        }
        final dim = t.renderObject<RenderAnimatedOpacity>(
          find.byKey(const Key('dim')),
        );
        expect(dim.opacity.value, closeTo(0.6, 0.01));
        await shot(t, 'display-$p-idle-dimmed');
      });

      shotTest('display $p: in-house box', (t) async {
        // Through the app: the server came from a box announcement (a host name and the IPv4
        // address of the announcement), spec 4.2, N40.
        final core = FakeCoreApi()
          ..discovery = const ServerDiscovery(
            servers: [
              ServerCandidate(
                address: 'wss://cometcast.box:443',
                source: 'display-announcement',
                ip: '192.168.1.20',
              ),
            ],
            locked: false,
            announcementsAllowed: true,
            notices: [],
            warnings: [],
          );
        final settings = AppSettings.fromDefaults(core.serverDefaults());
        await settings.discover(core);
        await pumpDisplay(
          t,
          tv: tv,
          size: size.value,
          appearance: look.value,
          core: core,
          settingsOverride: settings,
          server: 'wss://cometcast.box:443',
        );
        await shot(t, 'display-$p-idle-box');
      });

      shotTest('display $p: link card and presenting', (t) async {
        final h = await pumpDisplay(
          t,
          tv: tv,
          size: size.value,
          appearance: look.value,
          canOpen: true,
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
          const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
        );
        await t.pump();
        await deliverFrame(t, h.core, solidFrame(160, 90, [40, 90, 160]));
        await shot(t, 'display-$p-presenting-rest');
        await k(t, LogicalKeyboardKey.arrowDown);
        await t.pump(const Duration(milliseconds: 500));
        await shot(t, 'display-$p-presenting-controls');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.connectionCode(sas: '123 456'),
        );
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, 'display-$p-presenting-controls-sas');
      });

      shotTest('display $p: link card without a browser', (t) async {
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
        await shot(t, 'display-$p-linkcard-nobrowser');
      });

      shotTest('display $p: pill, toast and page error', (t) async {
        final core = _FailingPageCore();
        final h = await pumpDisplay(
          t,
          tv: tv,
          size: size.value,
          appearance: look.value,
          core: core,
        );
        h.core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
        );
        await t.pump();
        await deliverFrame(t, h.core, solidFrame(160, 90, [40, 90, 160]));
        h.core.receiverEvents.add(
          ReceiverUiEvent.contentTransferStarted(
            document: true,
            name: 'slides.pdf',
            size: BigInt.from(1000),
          ),
        );
        await t.pump();
        h.core.receiverEvents.add(
          ReceiverUiEvent.contentTransferProgress(
            received: BigInt.from(420),
            total: BigInt.from(1000),
          ),
        );
        await t.pump();
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, 'display-$p-presenting-pill');
        h.core.receiverEvents.add(
          const ReceiverUiEvent.contentError(
            error: UiContentError.documentRejected,
          ),
        );
        await t.pump();
        await t.pump(const Duration(milliseconds: 100));
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, 'display-$p-presenting-toast');
      });

      shotTest('display $p: PDF page error', (t) async {
        final core = _FailingPageCore();
        final h = await pumpDisplay(
          t,
          tv: tv,
          size: size.value,
          appearance: look.value,
          core: core,
        );
        h.core.receiverEvents
          ..add(const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false))
          ..add(
            const ReceiverUiEvent.contentDocumentOpened(
              name: 'a.pdf',
              pages: 3,
            ),
          )
          ..add(const ReceiverUiEvent.contentFrameReady());
        for (var i = 0; i < 10; i++) {
          await t.runAsync(
            () => Future<void>.delayed(const Duration(milliseconds: 30)),
          );
          await t.pump(const Duration(milliseconds: 50));
        }
        await shot(t, 'display-$p-presenting-page-error');
      });
    }
  }

  // The codec dialog of a desktop display (1920 x 1080) in its four states, the stage hint after
  // "Not now" and the German missing state. Each state is a fresh display: the widget tree is
  // not kept from one to the next, or all shots show the first one.
  for (final look in looks.entries) {
    shotTest('display desktop-${look.key}: codec dialog', (t) async {
      final states = {
        'missing': const UiCodecState.missing(),
        'downloading': const UiCodecState.downloading(progress: 0.45),
        'failed': const UiCodecState.failed(reason: 'network error'),
        'unsupported': const UiCodecState.unsupported(reason: 'no library'),
      };
      Future<DisplayHarness> open(
        UiCodecState state, {
        Locale locale = const Locale('en'),
      }) async {
        await t.pumpWidget(const SizedBox());
        final core = FakeCoreApi()
          ..codecStateResult = state
          ..codecInfoResult = codecInfo;
        final h = await pumpDisplay(
          t,
          tv: false,
          size: _tv,
          appearance: look.value,
          core: core,
          locale: locale,
        );
        h.core.receiverEvents
          ..add(const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false))
          ..add(
            const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
          );
        await t.pump();
        await t.pump(const Duration(milliseconds: 300));
        return h;
      }

      for (final e in states.entries) {
        await open(e.value);
        await shot(t, 'display-desktop-${look.key}-codec-${e.key}');
      }
      // "Not now": the line on the black stage.
      await open(states['missing']!);
      await t.pump(const Duration(milliseconds: 1100));
      await t.tap(find.byKey(const Key('codec-later')));
      await t.pump(const Duration(milliseconds: 300));
      // The input shows the controls above the line; at rest only the line is left.
      await shot(t, 'display-desktop-${look.key}-codec-later-controls');
      await t.pump(const Duration(seconds: 6));
      await shot(t, 'display-desktop-${look.key}-codec-later');
      // The dialog's appearance does not reveal the controls: a shot well after the last input
      // shows no name tag and no End under the scrim.
      await open(states['missing']!);
      await t.pump(const Duration(seconds: 6));
      await shot(t, 'display-desktop-${look.key}-codec-missing-at-rest');
      if (look.key == 'dark') {
        await open(states['missing']!, locale: const Locale('de'));
        await shot(t, 'display-desktop-dark-de-codec-missing');
        await open(states['failed']!, locale: const Locale('de'));
        await shot(t, 'display-desktop-dark-de-codec-failed');
        await open(states['downloading']!, locale: const Locale('de'));
        await shot(t, 'display-desktop-dark-de-codec-downloading');
        await open(states['unsupported']!, locale: const Locale('de'));
        await shot(t, 'display-desktop-dark-de-codec-unsupported');
        await open(states['missing']!, locale: const Locale('de'));
        await t.pump(const Duration(milliseconds: 1100));
        await t.tap(find.byKey(const Key('codec-later')));
        await t.pump(const Duration(seconds: 6));
        await shot(t, 'display-desktop-dark-de-codec-later');
      }
      // The "connected" toast above the scrim: the dialog is open when the sender joins.
      await t.pumpWidget(const SizedBox());
    });
  }

  // German (du), Dark: setup, idle, Settings and the request.
  for (final size in {'tv': _tv, 'tablet': _tablet}.entries) {
    final tv = size.key == 'tv';
    shotTest('display ${size.key}-dark: German', (t) async {
      await pumpDisplay(
        t,
        setupDone: false,
        tv: tv,
        size: size.value,
        locale: const Locale('de'),
      );
      if (!tv) {
        await t.tap(find.text('Dieses Gerät als Bildschirm verwenden'));
        for (var i = 0; i < 5; i++) {
          await t.pump(const Duration(milliseconds: 50));
        }
      }
      await shot(t, 'display-${size.key}-dark-de-setup0-welcome');
      await k(t, LogicalKeyboardKey.select);
      await shot(t, 'display-${size.key}-dark-de-setup1-name');
      await k(t, LogicalKeyboardKey.select);
      await shot(t, 'display-${size.key}-dark-de-setup2-mode');
      await k(t, LogicalKeyboardKey.arrowDown);
      await k(t, LogicalKeyboardKey.arrowRight);
      await k(t, LogicalKeyboardKey.select);
      await shot(t, 'display-${size.key}-dark-de-setup3-address');
      await k(t, LogicalKeyboardKey.select);
      await shot(t, 'display-${size.key}-dark-de-setup4-done');
      // The setup is done: a fresh display, not the same widget tree with the wizard still in it.
      await t.pumpWidget(const SizedBox());
      final h = await pumpDisplay(
        t,
        tv: tv,
        size: size.value,
        locale: const Locale('de'),
        mode: DefaultMode.confirm,
      );
      await shot(t, 'display-${size.key}-dark-de-idle');
      await k(t, LogicalKeyboardKey.select);
      await shot(t, 'display-${size.key}-dark-de-settings');
      await k(t, LogicalKeyboardKey.escape);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
      );
      await t.pump(const Duration(milliseconds: 100));
      await shot(t, 'display-${size.key}-dark-de-request');
    });
  }

  // Presenter pages: phone and tablet, light and dark.
  for (final size in {'phone': _phone, 'tablet': _tablet}.entries) {
    for (final b in [Brightness.light, Brightness.dark]) {
      final p = '${size.key}-${b.name}';
      shotTest('presenter $p', (t) async {
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
          'firstrun-scan',
          FirstRunPage(
            codeController: code,
            nameController: name,
            onConnect: nop,
            onScan: nop,
            scanAgainBody: size.key == 'phone',
          ),
        );
        await page(
          'firstrun-scan-wrong',
          FirstRunPage(
            codeController: code,
            nameController: name,
            onConnect: nop,
            onScan: nop,
            scanWrong: true,
          ),
        );
        if (size.key == 'tablet') {
          await page(
            'firstrun-windows',
            FirstRunPage(
              codeController: code,
              nameController: name,
              onConnect: nop,
              onBeDisplay: nop,
              onMiracast: nop,
            ),
          );
        }
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
          'waiting-room',
          WaitingPage(
            check: 42,
            secondsLeft: 41,
            totalSeconds: 60,
            room: 'Meeting room Elbe',
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
        await page(
          'what-room',
          WhatPage(
            device: size.key == 'tablet'
                ? PresenterDevice.tablet
                : PresenterDevice.phone,
            room: 'Meeting room Elbe',
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
          PresentingPage(
            kind: PresentingKind.screen,
            room: 'Meeting room Elbe',
            sas: '123 456',
            onStop: nop,
          ),
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
        await page(
          'presenting-pdf-full',
          PresentingPage(
            kind: PresentingKind.file,
            pdf: true,
            room: 'Meeting room Elbe',
            sas: '123 456',
            page: 1,
            pages: 3,
            onStop: nop,
            onPrev: nop,
            onNext: nop,
          ),
        );
        await page(
          'presenting-link',
          PresentingPage(
            kind: PresentingKind.link,
            room: 'Meeting room Elbe',
            sas: '123 456',
            onStop: nop,
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
        await page(
          'error-locked-waiting',
          ErrorPage(
            kind: ErrorKind.locked,
            lockedWhile: true,
            onAction: (_) {},
          ),
        );
        await page(
          'error-displayOff-relay',
          ErrorPage(
            kind: ErrorKind.displayOff,
            relayWhy: true,
            onAction: (_) {},
          ),
        );
        // No capture backend: the key text of N33, Back to "What to show".
        await page(
          'error-capture-unavailable',
          ErrorPage(
            kind: ErrorKind.other,
            otherWhy: lookupAppLocalizations(const Locale('en'))
                .errorsCaptureUnavailable,
            useBack: true,
            onAction: (_) {},
          ),
        );
        await page('icons', const _IconSheet());

        // The video codec page in its four states.
        for (final e in {
          'missing': const UiCodecState.missing(),
          'downloading': const UiCodecState.downloading(progress: 0.45),
          'failed': const UiCodecState.failed(reason: 'network error'),
          'unsupported': const UiCodecState.unsupported(reason: 'no library'),
        }.entries) {
          final core = FakeCoreApi()
            ..codecStateResult = e.value
            ..codecInfoResult = codecInfo;
          final ctl = CodecController(
            core: core,
            settings: AppSettings.fromDefaults(core.serverDefaults()),
          );
          await page(
            'codec-${e.key}',
            CodecPage(controller: ctl, onBack: nop, onScreen: nop),
          );
          ctl.dispose();
        }

        // The Miracast page (Windows) and its consent question.
        final miracast = FakeMiracastCore();
        await t.pumpWidget(
          MaterialApp(
            debugShowCheckedModeBanner: false,
            locale: const Locale('en'),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            theme: presenterTheme(Brightness.light),
            darkTheme: presenterTheme(Brightness.dark),
            home: MiracastPage(
              core: miracast,
              initialName: 'Studio PC',
              pollInterval: const Duration(milliseconds: 20),
            ),
          ),
        );
        await t.pump(const Duration(milliseconds: 200));
        await shot(t, 'presenter-$p-miracast');
        await t.tap(find.byKey(const Key('miracastStartButton')));
        await t.pump(const Duration(milliseconds: 200));
        miracast.request(9, 'Pixel 9', pin: '4711');
        await t.pump(const Duration(milliseconds: 200));
        await t.pump(const Duration(milliseconds: 200));
        await shot(t, 'presenter-$p-miracast-consent');
        await t.pumpWidget(const SizedBox());
      });
    }
  }

  // Screens on this network (spec 14): every state of the section on the app start page, phone
  // and laptop, light and dark, English; the main states also in German (du).
  final nearbyScreens = [
    screen('a', name: 'Meeting room Elbe', code: 'K7QM2W'),
    screen(
      'b',
      name: 'Room 204',
      mode: NearbyMode.approval,
      code: 'P4X9TR',
      server: 'wss://cast.musterfirma.de:443',
    ),
    screen('c', name: 'Clubhouse', mode: NearbyMode.locked),
    screen('d', name: '', code: 'R8M3VA'),
  ];
  for (final dev in {
    'phone': (_phone, PresenterDevice.phone, false),
    'laptop': (const Size(1100, 800), PresenterDevice.computer, true),
  }.entries) {
    for (final b in [Brightness.light, Brightness.dark]) {
      final p = 'nearby-${dev.key}-${b.name}';
      final (size, device, desktop) = dev.value;
      Future<Nearby> open(
        WidgetTester t, {
        FakeNearbyPlatform? platform,
        bool allowed = true,
        String? code,
        Locale locale = const Locale('en'),
      }) => pumpNearby(
        t,
        size: size,
        device: device,
        desktop: desktop,
        brightness: b,
        platform:
            platform ?? FakeNearbyPlatform(limited: !desktop, native: !desktop),
        announcementsAllowed: allowed,
        code: code,
        locale: locale,
      );
      shotTest('$p: found, searching, none, paused', (t) async {
        var n = await open(t);
        await shot(t, '$p-searching');
        await found(t, n, nearbyScreens);
        await shot(t, '$p-found');
        await t.tap(find.byKey(const Key('nearby-row-a')));
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, '$p-connecting');
        await done(t, n);

        n = await open(t);
        await t.pump(const Duration(seconds: 8));
        await shot(t, '$p-none');
        if (!desktop) {
          await t.pump(const Duration(seconds: 52));
          await shot(t, '$p-paused');
        }
        await done(t, n);
      });
      shotTest('$p: not asked, denied, no wifi, hidden', (t) async {
        var n = await open(
          t,
          platform: FakeNearbyPlatform(
            native: true,
            perm: NearbyPermission.notAsked,
          ),
        );
        await shot(t, '$p-not-asked');
        await t.tap(find.byKey(const Key('nearby-search')));
        await t.pump(const Duration(milliseconds: 100));
        n.core.search.add(
          const NearbyUiEvent.permission(state: NearbyPermission.denied),
        );
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, '$p-denied');
        await done(t, n);

        n = await open(t, platform: FakeNearbyPlatform(network: false));
        await shot(t, '$p-no-wifi');
        await done(t, n);

        n = await open(t, allowed: false);
        await shot(t, '$p-hidden-policy');
        await done(t, n);

        n = await open(t, code: 'K7QM2W');
        await shot(t, '$p-hidden-chip');
        await done(t, n);
      });
      shotTest('$p: gone line, relay confirmation', (t) async {
        var n = await open(t);
        await found(t, n, nearbyScreens);
        await t.tap(find.byKey(const Key('nearby-row-a')));
        await t.pump(const Duration(milliseconds: 100));
        n.core.presenterEvents.add(
          const PresenterUiEvent.error(code: 'room_not_found', message: 'x'),
        );
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, '$p-gone');
        await done(t, n);

        n = await open(t);
        await found(t, n, nearbyScreens);
        await t.tap(find.byKey(const Key('nearby-row-b')));
        await t.pump(const Duration(milliseconds: 100));
        await shot(t, '$p-relay');
        await done(t, n);
      });
    }
  }
  shotTest('nearby phone-light-de: found and relay (du)', (t) async {
    final n = await pumpNearby(
      t,
      locale: const Locale('de'),
      platform: FakeNearbyPlatform(native: true),
    );
    await found(t, n, [
      screen('a', name: 'Besprechungsraum Elbe', code: 'K7QM2W'),
      screen(
        'b',
        name: 'Raum 204',
        mode: NearbyMode.approval,
        code: 'P4X9TR',
        server: 'wss://cast.musterfirma.de:443',
      ),
      screen('c', name: 'Vereinsheim', mode: NearbyMode.locked),
    ]);
    await shot(t, 'nearby-phone-light-de-found');
    await t.tap(find.byKey(const Key('nearby-row-b')));
    await t.pump(const Duration(milliseconds: 100));
    await shot(t, 'nearby-phone-light-de-relay');
    await done(t, n);
  });
}

/// A core whose pages cannot be taken: the page that cannot be drawn (spec 4.5).
class _FailingPageCore extends FakeCoreApi {
  @override
  Future<ContentFrame?> takeContentFrame() async => throw StateError('no page');
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
