// SPDX-License-Identifier: GPL-3.0-or-later
// Widget tests for the H.264 codec card (download consent, progress, failure, offline hint).

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/codec_controller.dart';
import 'package:cometcast/codec_notice.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/settings.dart';

import 'fake_core.dart';

const _info = UiCodecInfo(
  fileName: 'libopenh264-2.6.0-linux64.8.so',
  version: '2.6.0',
  host: 'ciscobinary.openh264.org',
  downloadBytes: 634264,
  autoDownload: false,
  adminDir: null,
);

Future<(FakeCoreApi, CodecController, MemorySettingsStore)> open(
  WidgetTester tester, {
  String locale = 'en',
  UiCodecState state = const UiCodecState.missing(),
  UiCodecInfo info = _info,
  bool consent = false,
}) async {
  final core = FakeCoreApi()
    ..codecStateResult = state
    ..codecInfoResult = info;
  final store = MemorySettingsStore()..codecConsent = consent;
  final settings = AppSettings.fromDefaults(
    core.serverDefaults(),
    store: store,
  );
  final controller = CodecController(
    core: core,
    settings: settings,
    pollInterval: const Duration(milliseconds: 10),
  );
  addTearDown(controller.dispose);
  await tester.pumpWidget(
    MaterialApp(
      locale: Locale(locale),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(body: CodecNotice(controller: controller)),
    ),
  );
  await controller.init();
  await tester.pump();
  return (core, controller, store);
}

void main() {
  testWidgets('hidden when the codec is ready', (tester) async {
    await open(tester, state: const UiCodecState.ready());
    expect(find.byKey(const Key('codecNotice')), findsNothing);
  });

  testWidgets(
    'asks first: explanation, host, file and size, no silent download',
    (tester) async {
      final (core, _, _) = await open(tester);
      expect(find.text('Video codec needed'), findsOneWidget);
      expect(find.textContaining('by Cisco'), findsOneWidget);
      expect(find.textContaining('ciscobinary.openh264.org'), findsOneWidget);
      expect(
        find.textContaining('libopenh264-2.6.0-linux64.8.so'),
        findsWidgets,
      );
      expect(find.textContaining('619 KB'), findsOneWidget);
      expect(core.ensureCodecCalls, 0);
      // The offline hint names the file and the variable.
      expect(find.textContaining('COMETCAST_CODEC_DIR'), findsOneWidget);
    },
  );

  testWidgets('german text uses the formal address', (tester) async {
    await open(tester, locale: 'de');
    expect(find.text('Video-Codec erforderlich'), findsOneWidget);
    expect(find.text('Herunterladen'), findsOneWidget);
    expect(find.textContaining('Bitten Sie Ihre'), findsOneWidget);
  });

  testWidgets('consent button downloads, shows progress, remembers consent', (
    tester,
  ) async {
    final (core, controller, store) = await open(tester);
    core.ensureCodecGate = Completer<void>();
    await tester.tap(find.byKey(const Key('codecDownload')));
    await tester.pump();
    expect(core.ensureCodecCalls, 1);
    expect(store.codecConsent, isTrue);
    expect(find.byKey(const Key('codecProgress')), findsOneWidget);
    // The core reports progress while the download runs.
    core.codecStateResult = const UiCodecState.downloading(progress: 0.5);
    await tester.pump(const Duration(milliseconds: 30));
    expect(find.text('50 %'), findsOneWidget);
    core.ensureCodecGate!.complete();
    await tester.pumpAndSettle();
    expect(controller.ready, isTrue);
    expect(find.byKey(const Key('codecNotice')), findsNothing);
  });

  testWidgets('a failure shows the reason and a retry button', (tester) async {
    final (core, _, _) = await open(tester);
    core.codecStateAfter = const UiCodecState.failed(
      reason: 'network error: x',
    );
    await tester.tap(find.byKey(const Key('codecDownload')));
    await tester.pumpAndSettle();
    expect(find.textContaining('network error: x'), findsOneWidget);
    expect(find.byKey(const Key('codecOffline')), findsOneWidget);
    core.codecStateAfter = const UiCodecState.ready();
    await tester.tap(find.byKey(const Key('codecRetry')));
    await tester.pumpAndSettle();
    expect(core.ensureCodecCalls, 2);
    expect(find.byKey(const Key('codecNotice')), findsNothing);
  });

  testWidgets('remembered consent downloads at once', (tester) async {
    final (core, _, _) = await open(tester, consent: true);
    await tester.pumpAndSettle();
    expect(core.ensureCodecCalls, 1);
  });

  testWidgets('administrator policy skips the question', (tester) async {
    const auto = UiCodecInfo(
      fileName: 'x.so',
      version: '2.6.0',
      host: 'ciscobinary.openh264.org',
      downloadBytes: 1000,
      autoDownload: true,
      adminDir: null,
    );
    final (core, _, store) = await open(tester, info: auto);
    await tester.pumpAndSettle();
    expect(core.ensureCodecCalls, 1);
    // The policy is no consent of the person: nothing is stored.
    expect(store.codecConsent, isFalse);
  });

  testWidgets('shows the administrator folder when one is configured', (
    tester,
  ) async {
    const withDir = UiCodecInfo(
      fileName: 'x.so',
      version: '2.6.0',
      host: 'ciscobinary.openh264.org',
      downloadBytes: 1000,
      autoDownload: false,
      adminDir: '/srv/codecs',
    );
    await open(tester, info: withDir);
    expect(find.textContaining('/srv/codecs'), findsOneWidget);
  });

  testWidgets('unsupported platform: reason, no download button', (
    tester,
  ) async {
    await open(
      tester,
      state: const UiCodecState.unsupported(reason: 'no library for ios'),
    );
    expect(find.byKey(const Key('codecUnsupported')), findsOneWidget);
    expect(find.byKey(const Key('codecDownload')), findsNothing);
  });

  Future<FakeCoreApi> openScreen(WidgetTester tester, String entry) async {
    final core = FakeCoreApi()
      ..codecStateResult = const UiCodecState.missing()
      ..codecInfoResult = _info;
    tester.view.physicalSize = const Size(900, 1800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      CometCastApp(locale: const Locale('en'), core: core),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text(entry));
    await tester.pumpAndSettle();
    return core;
  }

  testWidgets('send screen: sharing waits for the codec', (tester) async {
    final core = await openScreen(tester, 'Send screen');
    // Not asked while the screen is not shared (the test video needs no codec).
    expect(find.byKey(const Key('codecNotice')), findsNothing);
    await tester.tap(find.byKey(const Key('shareScreenSwitch')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('codecNotice')), findsOneWidget);
    await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(core.senderScreen, isNull);
    expect(core.senderCode, isNull);
    // After the download the same start works.
    await tester.tap(find.byKey(const Key('codecDownload')));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('codecNotice')), findsNothing);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(core.senderScreen, isNotNull);
  });

  testWidgets('receive screen shows the card but can still start', (
    tester,
  ) async {
    final core = await openScreen(tester, 'Receive on this device');
    expect(find.byKey(const Key('codecNotice')), findsOneWidget);
    await tester.tap(find.byKey(const Key('startButton')));
    await tester.pump();
    expect(core.receiverServer, isNotNull);
  });
}
