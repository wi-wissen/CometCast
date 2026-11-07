// SPDX-License-Identifier: GPL-3.0-or-later
// The H.264 codec controller behind the display dialog (spec 4.7) and the presenter page (5.8):
// consent, progress, failure and retry, a download that somebody else runs, an administrator's
// policy. The card of the old UI is gone; these rules are what both new surfaces stand on.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/codec_controller.dart';
import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/settings.dart';

import 'fake_core.dart';

const info = UiCodecInfo(
  fileName: 'libopenh264-2.6.0-linux64.8.so',
  version: '2.6.0',
  host: 'ciscobinary.openh264.org',
  downloadBytes: 634264,
  autoDownload: false,
  adminDir: null,
);

Future<void> tick([int ms = 40]) =>
    Future<void>.delayed(Duration(milliseconds: ms));

Future<(FakeCoreApi, CodecController, MemorySettingsStore)> open({
  UiCodecState state = const UiCodecState.missing(),
  UiCodecInfo i = info,
  String? consent,
}) async {
  final core = FakeCoreApi()
    ..codecStateResult = state
    ..codecInfoResult = i;
  final store = MemorySettingsStore()..codecConsent = consent;
  final c = CodecController(
    core: core,
    settings: AppSettings.fromDefaults(core.serverDefaults(), store: store),
    pollInterval: const Duration(milliseconds: 10),
  );
  addTearDown(c.dispose);
  if (state is UiCodecState_Downloading) {
    unawaited(c.init());
  } else {
    await c.init();
  }
  return (core, c, store);
}

void main() {
  test('size text: kilobytes and megabytes', () {
    expect(codecSizeText(634264), '619 KB');
    expect(codecSizeText(2 * 1024 * 1024), '2.0 MB');
  });

  test('asks first: missing, needs consent, no silent download', () async {
    final (core, c, _) = await open();
    expect(c.ready, isFalse);
    expect(c.needsConsent, isTrue);
    expect(core.ensureCodecCalls, 0);
  });

  test(
    'Download downloads, shows the progress, remembers the consent',
    () async {
      final (core, c, store) = await open();
      core.ensureCodecGate = Completer<void>();
      unawaited(c.download());
      await tick();
      expect(core.ensureCodecCalls, 1);
      expect(store.codecConsent, 'ciscobinary.openh264.org|2.6.0');
      expect(c.state, isA<UiCodecState_Downloading>());
      core.codecStateResult = const UiCodecState.downloading(progress: 0.5);
      await tick();
      expect((c.state as UiCodecState_Downloading).progress, 0.5);
      core.ensureCodecGate!.complete();
      await tick();
      expect(c.ready, isTrue);
    },
  );

  test('a failure keeps the reason and a retry can succeed', () async {
    final (core, c, _) = await open();
    core.codecStateAfter = const UiCodecState.failed(
      reason: 'network error: x',
    );
    await c.download();
    expect((c.state as UiCodecState_Failed).reason, 'network error: x');
    core.codecStateAfter = const UiCodecState.ready();
    await c.download();
    expect(core.ensureCodecCalls, 2);
    expect(c.ready, isTrue);
  });

  test('remembered consent downloads at once', () async {
    final (core, _, _) = await open(consent: 'ciscobinary.openh264.org|2.6.0');
    await tick();
    expect(core.ensureCodecCalls, 1);
  });

  test('consent for another version or host asks again', () async {
    for (final old in [
      'ciscobinary.openh264.org|2.5.0',
      'other.example|2.6.0',
      // The boolean of the first release is never read back as consent.
      'true',
    ]) {
      final (core, c, _) = await open(consent: old);
      await tick();
      expect(core.ensureCodecCalls, 0, reason: old);
      expect(c.needsConsent, isTrue, reason: old);
    }
  });

  test(
    'an administrator policy skips the question and stores nothing',
    () async {
      const auto = UiCodecInfo(
        fileName: 'x.so',
        version: '2.6.0',
        host: 'ciscobinary.openh264.org',
        downloadBytes: 1000,
        autoDownload: true,
        adminDir: null,
      );
      final (core, c, store) = await open(i: auto);
      await tick();
      expect(core.ensureCodecCalls, 1);
      expect(c.needsConsent, isFalse);
      expect(store.codecConsent, isNull);
    },
  );

  test('a download owned by someone else is followed to the end', () async {
    final (core, c, _) = await open();
    core.codecStateAfter = const UiCodecState.downloading(progress: 0.2);
    unawaited(c.download());
    await tick();
    expect(c.state, isA<UiCodecState_Downloading>());
    core.codecStateResult = const UiCodecState.downloading(progress: 0.7);
    await tick();
    expect((c.state as UiCodecState_Downloading).progress, 0.7);
    expect(c.ready, isFalse);
    core.codecStateResult = const UiCodecState.ready();
    await tick();
    expect(c.ready, isTrue);
  });

  test('a shared download that fails ends in the failed state', () async {
    final (core, c, _) = await open();
    core.codecStateAfter = const UiCodecState.downloading(progress: 0);
    unawaited(c.download());
    await tick();
    core.codecStateResult = const UiCodecState.failed(reason: 'disk full');
    await tick();
    expect(c.state, isA<UiCodecState_Failed>());
  });

  test('init() follows a download that is already running', () async {
    final (core, c, _) = await open(
      state: const UiCodecState.downloading(progress: 0.3),
    );
    await tick();
    expect(core.ensureCodecCalls, 0);
    core.codecStateResult = const UiCodecState.downloading(progress: 0.9);
    await tick();
    expect((c.state as UiCodecState_Downloading).progress, 0.9);
    core.codecStateResult = const UiCodecState.ready();
    await tick();
    expect(c.ready, isTrue);
  });

  test(
    'an unsupported platform stays unsupported, nothing to download',
    () async {
      final (core, c, _) = await open(
        state: const UiCodecState.unsupported(reason: 'no library for ios'),
      );
      expect(c.state, isA<UiCodecState_Unsupported>());
      expect(c.needsConsent, isFalse);
      expect(core.ensureCodecCalls, 0);
    },
  );

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

  test(
    'a hardware decoder that fails makes the codec needed after all',
    () async {
      final core = FakeCoreApi()
        ..codecStateResult = const UiCodecState.missing()
        ..codecInfoResult = info;
      final store = MemorySettingsStore();
      final c = CodecController(
        core: core,
        settings: AppSettings.fromDefaults(core.serverDefaults(), store: store),
        needed: false,
      );
      addTearDown(c.dispose);
      expect(c.needed, isFalse);
      await c.require();
      expect(c.needed, isTrue);
      expect(c.ready, isFalse);
      // Without the person's consent nothing is downloaded.
      expect(core.ensureCodecCalls, 0);
    },
  );
}
