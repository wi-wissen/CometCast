// SPDX-License-Identifier: GPL-3.0-or-later
// A presenter root with everything round 5 adds (settings, About, address line, share, TVs,
// Office), for the tests of fix list round 5 (F2, F3, F5, F6, F7, F10).

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/launch_intake.dart';
import 'package:cometcast/core/share_intake.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/licences.dart';
import 'package:cometcast/nearby_controller.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/presenter/file_picker.dart';
import 'package:cometcast/src/presenter/pages.dart' show PresenterDevice;
import 'package:cometcast/src/presenter/presenter_root.dart';
import 'package:cometcast/tv_controller.dart';

import 'display_support.dart';
import 'fake_core.dart';
import 'nearby_support.dart';

/// A shared item that a test hands to the app.
class FakeShareIntake implements ShareIntake {
  SharedItem? item;
  final discarded = <String>[];
  final _c = StreamController<void>.broadcast();

  @override
  Stream<void> get arrivals => _c.stream;

  void arrive(SharedItem i) {
    item = i;
    _c.add(null);
  }

  @override
  Future<SharedItem?> take() async {
    final i = item;
    item = null;
    return i;
  }

  @override
  Future<void> discard(String path) async => discarded.add(path);
}

class R5 {
  R5({
    required this.core,
    required this.settings,
    required this.store,
    required this.platform,
    required this.nearby,
    required this.tv,
    required this.intake,
    required this.opened,
    required this.picks,
    required this.log,
  });
  final FakeCoreApi core;
  final AppSettings settings;
  final MemorySettingsStore store;
  final FakeNearbyPlatform platform;
  final NearbyController? nearby;
  final TvController? tv;
  final FakeShareIntake intake;
  final List<Uri> opened;
  final List<PickKind> picks;
  final List<String> log;
}

/// A short list so that the tests need no isolate.
LicenceSet fakeLicences() => const LicenceSet(
  featured: [
    LicenceEntry(
      name: 'Cisco OpenH264',
      licence: 'Copyright (c) 2013, Cisco Systems',
      text: 'OpenH264 text\n\nsecond block',
      featured: true,
    ),
    LicenceEntry(
      name: 'Atkinson Hyperlegible',
      licence: 'SIL Open Font License',
      text: 'Atkinson text',
      featured: true,
    ),
  ],
  all: [
    LicenceEntry(
      name: 'tokio 1.40.0',
      licence: 'MIT License',
      text: 'tokio licence text',
      featured: false,
    ),
    LicenceEntry(
      name: 'serde 1.0.0',
      licence: 'MIT License',
      text: 'serde licence text',
      featured: false,
    ),
  ],
);

Future<R5> pumpR5(
  WidgetTester t, {
  Size size = const Size(400, 800),
  Locale locale = const Locale('en'),
  bool desktop = false,
  bool office = false,
  bool withNearby = false,
  bool tvs = false,
  bool locked = false,
  String server = 'relay.cometcast.de',
  String? code,
  FakeShareIntake? shareIntake,
  FakeCoreApi? fake,
  Brightness brightness = Brightness.light,
  PresenterDevice? device,
  List<String>? pickResults,
  bool announcements = true,
}) async {
  await t.runAsync(loadAppFonts);
  final core = fake ?? FakeCoreApi();
  if (office) core.officeWorker = '/tmp/cc-office-worker';
  core.defaults = ServerDefaults(
    servers: core.defaults.servers,
    locked: locked,
    announcementsAllowed: announcements,
    notices: const [],
    devFallback: core.defaults.devFallback,
  );
  final store = MemorySettingsStore();
  final settings = AppSettings.fromDefaults(
    core.defaults,
    serverAddress: server,
    store: store,
  );
  if (tvs) settings.otherTvs = true;
  final platform = FakeNearbyPlatform();
  final nearby = withNearby
      ? NearbyController(
          core: core,
          platform: platform,
          now: () => t.binding.clock.now(),
        )
      : null;
  final tv = desktop && withNearby
      ? TvController(core: core, now: () => t.binding.clock.now())
      : null;
  final intake = shareIntake ?? FakeShareIntake();
  final opened = <Uri>[];
  final picks = <PickKind>[];
  final log = <String>[];
  var pickIndex = 0;
  t.view.physicalSize = size;
  t.view.devicePixelRatio = 1;
  t.platformDispatcher.platformBrightnessTestValue = brightness;
  addTearDown(t.view.reset);
  addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
  await t.pumpWidget(
    MaterialApp(
      debugShowCheckedModeBanner: false,
      locale: locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: PresenterRoot(
        core: core,
        settings: settings,
        pickFile: (_) async => null,
        pickFileFor: (label, kind) async {
          picks.add(kind);
          final r = pickResults;
          if (r == null || pickIndex >= r.length) return null;
          final v = r[pickIndex++];
          return v.isEmpty ? null : v;
        },
        openUrl: (u) async {
          opened.add(u);
          return true;
        },
        shareIntake: intake,
        desktop: desktop,
        device: device,
        scanQr: desktop ? null : () async => null,
        links: LinkHandler(core: core, settings: settings),
        initialCode: code,
        nearbyPlatform: withNearby ? platform : null,
        nearbyController: nearby,
        tvController: tv,
        licenceLoader: () async => fakeLicences(),
        leaveApp: () => log.add('leave'),
      ),
    ),
  );
  for (var i = 0; i < 4; i++) {
    await t.pump(const Duration(milliseconds: 50));
  }
  return R5(
    core: core,
    settings: settings,
    store: store,
    platform: platform,
    nearby: nearby,
    tv: tv,
    intake: intake,
    opened: opened,
    picks: picks,
    log: log,
  );
}

/// The widget tree is replaced and the controllers disposed before the binding checks timers.
Future<void> finish(WidgetTester t, R5 r) async {
  await t.pumpWidget(const SizedBox());
  r.nearby?.dispose();
  r.tv?.dispose();
}

Future<void> tapKey(WidgetTester t, String key) async {
  await t.tap(find.byKey(Key(key)));
  await settle(t);
}

Future<void> tapText(WidgetTester t, String text) async {
  await t.tap(find.text(text));
  await settle(t);
}
