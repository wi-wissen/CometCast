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
  String? consent,
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
  if (state is UiCodecState_Downloading) {
    // init() follows the download until it ends; the test ends it.
    unawaited(controller.init());
  } else {
    await controller.init();
  }
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
    expect(store.codecConsent, 'ciscobinary.openh264.org|2.6.0');
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
    final (core, _, _) = await open(
      tester,
      consent: 'ciscobinary.openh264.org|2.6.0',
    );
    await tester.pumpAndSettle();
    expect(core.ensureCodecCalls, 1);
  });

  testWidgets('consent for another version or host asks again', (tester) async {
    for (final old in [
      'ciscobinary.openh264.org|2.5.0',
      'other.example|2.6.0',
      // The boolean of the first release is never read back as consent.
      'true',
    ]) {
      final (core, controller, _) = await open(tester, consent: old);
      await tester.pumpAndSettle();
      expect(core.ensureCodecCalls, 0, reason: old);
      expect(controller.needsConsent, isTrue, reason: old);
      expect(find.byKey(const Key('codecDownload')), findsOneWidget);
    }
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
    expect(store.codecConsent, isNull);
  });

  testWidgets('a download owned by someone else is followed to the end', (
    tester,
  ) async {
    // ensureCodec answers "downloading" at once (another controller or process runs it).
    final (core, controller, _) = await open(tester);
    core.codecStateAfter = const UiCodecState.downloading(progress: 0.2);
    await tester.tap(find.byKey(const Key('codecDownload')));
    await tester.pump(const Duration(milliseconds: 30));
    expect(core.ensureCodecCalls, 1);
    expect(find.byKey(const Key('codecProgress')), findsOneWidget);
    core.codecStateResult = const UiCodecState.downloading(progress: 0.7);
    await tester.pump(const Duration(milliseconds: 30));
    expect(find.text('70 %'), findsOneWidget);
    expect(controller.ready, isFalse);
    core.codecStateResult = const UiCodecState.ready();
    await tester.pump(const Duration(milliseconds: 30));
    await tester.pumpAndSettle();
    expect(controller.ready, isTrue);
    expect(find.byKey(const Key('codecNotice')), findsNothing);
  });

  testWidgets('a shared download that fails ends in the failed card', (
    tester,
  ) async {
    final (core, controller, _) = await open(tester);
    core.codecStateAfter = const UiCodecState.downloading(progress: 0);
    await tester.tap(find.byKey(const Key('codecDownload')));
    await tester.pump(const Duration(milliseconds: 30));
    core.codecStateResult = const UiCodecState.failed(reason: 'disk full');
    await tester.pump(const Duration(milliseconds: 30));
    await tester.pumpAndSettle();
    expect(controller.state, isA<UiCodecState_Failed>());
    expect(find.byKey(const Key('codecRetry')), findsOneWidget);
  });

  testWidgets('init() follows a download that is already running', (
    tester,
  ) async {
    final (core, controller, _) = await open(
      tester,
      state: const UiCodecState.downloading(progress: 0.3),
    );
    // Not started by us: no ensureCodec call, but the card shows the progress.
    expect(core.ensureCodecCalls, 0);
    expect(find.byKey(const Key('codecProgress')), findsOneWidget);
    core.codecStateResult = const UiCodecState.downloading(progress: 0.9);
    await tester.pump(const Duration(milliseconds: 30));
    expect(find.text('90 %'), findsOneWidget);
    core.codecStateResult = const UiCodecState.ready();
    await tester.pump(const Duration(milliseconds: 30));
    await tester.pumpAndSettle();
    expect(controller.ready, isTrue);
    expect(find.byKey(const Key('codecNotice')), findsNothing);
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

  Future<FakeCoreApi> openScreen(
    WidgetTester tester,
    String entry, {
    UiCodecState state = const UiCodecState.missing(),
  }) async {
    final core = FakeCoreApi()
      ..codecStateResult = state
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

  testWidgets('send screen follows a download that another owner runs', (
    tester,
  ) async {
    final core = await openScreen(
      tester,
      'Send screen',
      state: const UiCodecState.downloading(progress: 0.4),
    );
    await tester.tap(find.byKey(const Key('shareScreenSwitch')));
    await tester.pump(const Duration(milliseconds: 50));
    expect(find.byKey(const Key('codecProgress')), findsOneWidget);
    expect(core.ensureCodecCalls, 0);
    core.codecStateResult = const UiCodecState.ready();
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('codecNotice')), findsNothing);
  });

  testWidgets('receive screen follows a download that another owner runs', (
    tester,
  ) async {
    final core = await openScreen(
      tester,
      'Receive on this device',
      state: const UiCodecState.downloading(progress: 0.4),
    );
    expect(find.byKey(const Key('codecProgress')), findsOneWidget);
    core.codecStateResult = const UiCodecState.ready();
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('codecNotice')), findsNothing);
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

  test(
    'not needed (hardware decoder, Android sender): ready, nothing asked',
    () async {
      final core = FakeCoreApi()
        ..codecStateResult = const UiCodecState.missing();
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      final c = CodecController(core: core, settings: settings, needed: false);
      addTearDown(c.dispose);
      await c.init();
      expect(c.ready, isTrue);
      expect(c.needsConsent, isFalse);
      expect(core.ensureCodecCalls, 0);
    },
  );
}
