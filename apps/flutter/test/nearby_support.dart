// SPDX-License-Identifier: GPL-3.0-or-later
// A fake platform part and helpers for the tests of "screens on this network" (spec 14).

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/launch_intake.dart';
import 'package:cometcast/core/nearby_platform.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/nearby_controller.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/presenter/pages.dart' show PresenterDevice;
import 'package:cometcast/src/presenter/presenter_root.dart';

import 'display_support.dart';
import 'fake_core.dart';

class FakeNearbyPlatform implements NearbyPlatform {
  FakeNearbyPlatform({
    this.native = false,
    this.limited = true,
    this.network = true,
    this.perm = NearbyPermission.granted,
  });

  final bool native;
  final bool limited;
  bool network;
  NearbyPermission perm;
  int startCalls = 0;
  int stopCalls = 0;
  int settingsCalls = 0;
  final _changes = StreamController<bool>.broadcast();

  void networkChanged(bool v) {
    network = v;
    _changes.add(v);
  }

  @override
  bool get nativeBrowse => native;

  @override
  bool get limitedTime => limited;

  @override
  Future<NearbyPermission> permission() async => perm;

  @override
  Future<void> startBrowse() async => startCalls++;

  @override
  Future<void> stopBrowse() async => stopCalls++;

  @override
  Future<bool> hasLocalNetwork() async => network;

  @override
  Stream<bool> get networkChanges => _changes.stream;

  @override
  Future<bool> openSettings() async {
    settingsCalls++;
    return true;
  }
}

NearbyScreenInfo screen(
  String id, {
  String name = 'Room 204',
  String server = 'wss://relay.cometcast.de:443',
  NearbyMode mode = NearbyMode.open,
  String? code = 'K7QM2W',
}) => NearbyScreenInfo(
  id: id,
  name: name,
  server: server,
  mode: mode,
  code: mode == NearbyMode.locked ? null : code,
);

class Nearby {
  Nearby(this.core, this.settings, this.store, this.platform, this.controller);
  final FakeCoreApi core;
  final AppSettings settings;
  final MemorySettingsStore store;
  final FakeNearbyPlatform platform;
  final NearbyController controller;
}

Future<void> settle(WidgetTester t, [int n = 4]) async {
  for (var i = 0; i < n; i++) {
    await t.pump(const Duration(milliseconds: 50));
  }
}

Future<Nearby> pumpNearby(
  WidgetTester t, {
  Size size = const Size(400, 800),
  Locale locale = const Locale('en'),
  PresenterDevice device = PresenterDevice.phone,
  bool desktop = false,
  FakeNearbyPlatform? platform,
  FakeCoreApi? fake,
  String? code,
  bool locked = false,
  bool announcementsAllowed = true,
  bool withPlatform = true,
  Brightness brightness = Brightness.light,
  bool? beDisplay,
  bool miracast = false,
}) async {
  await t.runAsync(loadAppFonts);
  final core = fake ?? FakeCoreApi();
  final store = MemorySettingsStore();
  core.defaults = ServerDefaults(
    servers: core.defaults.servers,
    locked: locked,
    announcementsAllowed: announcementsAllowed,
    notices: const [],
    devFallback: core.defaults.devFallback,
  );
  final settings = AppSettings.fromDefaults(
    core.defaults,
    serverAddress: 'relay.cometcast.de',
    store: store,
  );
  final fakePlatform = platform ?? FakeNearbyPlatform();
  final controller = NearbyController(
    core: core,
    platform: fakePlatform,
    now: () => t.binding.clock.now(),
  );
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
        desktop: desktop,
        device: device,
        scanQr: desktop ? null : () async => null,
        links: LinkHandler(core: core, settings: settings),
        initialCode: code,
        // A computer always has `app.beDisplay` (C1); tablets too.
        onBeDisplay: (beDisplay ?? desktop) ? () {} : null,
        onMiracast: miracast ? () {} : null,
        nearbyPlatform: withPlatform ? fakePlatform : null,
        nearbyController: withPlatform ? controller : null,
      ),
    ),
  );
  await settle(t);
  return Nearby(core, settings, store, fakePlatform, controller);
}

Future<void> found(WidgetTester t, Nearby n, List<NearbyScreenInfo> s) async {
  n.core.search.add(const NearbyUiEvent.searchStarted());
  for (final x in s) {
    n.core.search.add(NearbyUiEvent.added(screen: x));
  }
  await settle(t);
  // Past the 500 ms of the tap guard.
  await t.pump(const Duration(milliseconds: 600));
}

/// The widget tree is replaced and the controller disposed before the binding checks timers.
Future<void> done(WidgetTester t, Nearby n) async {
  await t.pumpWidget(const SizedBox());
  n.controller.dispose();
}
