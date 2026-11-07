// SPDX-License-Identifier: GPL-3.0-or-later
// Links, install referrer and relay confirmation in the app (fix list B8, B9).

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/app_links.dart';
import 'package:cometcast/core/launch_intake.dart';
import 'package:cometcast/core/link_target.dart';
import 'package:cometcast/main.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/src/display/display_logic.dart';

import 'display_support.dart';
import 'fake_core.dart';

class FakeLinks implements AppLinks {
  FakeLinks({this.initial, this.referrer});
  final String? initial;
  final String? referrer;
  int referrerReads = 0;
  final _c = StreamController<String>.broadcast();

  @override
  Future<String?> initialLink() async => initial;

  @override
  Stream<String> get links => _c.stream;

  @override
  Future<String?> installReferrer() async {
    referrerReads++;
    return referrer;
  }
}

AppSettings settingsFor(FakeCoreApi core, MemorySettingsStore store) =>
    AppSettings.fromDefaults(
      core.serverDefaults(),
      serverAddress: 'relay.cometcast.de',
      store: store,
    );

void main() {
  group('what starts the app', () {
    late FakeCoreApi core;
    late MemorySettingsStore store;
    late AppSettings settings;
    late LinkHandler handler;

    setUp(() {
      core = FakeCoreApi();
      store = MemorySettingsStore();
      settings = settingsFor(core, store);
      handler = LinkHandler(core: core, settings: settings);
    });

    test('a link beats the referrer', () async {
      final links = FakeLinks(
        initial: 'https://relay.cometcast.de/K7QM2W',
        referrer: 'code=ABCDEF',
      );
      final o = await resolveLaunch(
        links: links,
        handler: handler,
        store: store,
      );
      expect(o, isA<LinkUse>());
      expect((o as LinkUse).target.code, 'K7QM2W');
      expect(links.referrerReads, 0);
    });

    test('the referrer is read on the first launch only', () async {
      final links = FakeLinks(referrer: 'code=K7QM2W');
      final first = await resolveLaunch(
        links: links,
        handler: handler,
        store: store,
      );
      expect((first as LinkUse).target.code, 'K7QM2W');
      expect(store.referrerRead, isTrue);
      final second = await resolveLaunch(
        links: links,
        handler: handler,
        store: store,
      );
      expect(second, isA<LinkNothing>());
      expect(links.referrerReads, 1);
    });

    test(
      'no referrer and no link: nothing, and the referrer is not asked again',
      () async {
        final links = FakeLinks();
        final o = await resolveLaunch(
          links: links,
          handler: handler,
          store: store,
        );
        expect(o, isA<LinkNothing>());
        expect(store.referrerRead, isTrue);
      },
    );

    test(
      'an unknown server asks first; nothing is stored before the decision',
      () async {
        final links = FakeLinks(
          initial: 'https://relay.cometcast.de/K7QM2W?server=cast.example.org',
        );
        final o = await resolveLaunch(
          links: links,
          handler: handler,
          store: store,
        );
        expect(o, isA<LinkConfirm>());
        expect(settings.confirmedServers, isEmpty);
        expect(
          store.server,
          isNull,
          reason: 'a link never changes the last used server',
        );
        expect(settings.serverAddress, 'relay.cometcast.de');
      },
    );

    test('Use remembers the server as confirmed (persisted) and takes it for this run', () async {
      await handler.confirmAndUse('cast.example.org');
      expect(settings.confirmedServers, {'cast.example.org'});
      expect(store.confirmedServers, ['cast.example.org']);
      expect(settings.serverAddress, 'cast.example.org');
      expect(store.server, isNull, reason: 'saved only after a join succeeded');
      // Next time it is used at once.
      expect(
        handler.fromUrl(
          'https://relay.cometcast.de/K7QM2W?server=cast.example.org',
        ),
        isA<LinkUse>(),
      );
    });

    test('a scanned foreign QR code is nothing', () {
      expect(handler.fromUrl('https://example.org/hello'), isA<LinkNothing>());
      expect(handler.fromUrl('WIFI:S:x;T:WPA;P:y;;'), isA<LinkNothing>());
      expect(
        handler.fromUrl('https://relay.cometcast.de/K7QM2W'),
        isA<LinkUse>(),
      );
    });
  });

  group('the first-run page', () {
    Future<(FakeCoreApi, AppSettings)> pump(
      WidgetTester t,
      LinkOutcome launch,
    ) async {
      await t.runAsync(loadAppFonts);
      final core = FakeCoreApi();
      final store = MemorySettingsStore();
      final settings = settingsFor(core, store);
      t.view.physicalSize = const Size(400, 800);
      t.view.devicePixelRatio = 1;
      addTearDown(t.view.reset);
      await t.pumpWidget(
        CometCastApp(
          locale: const Locale('en'),
          core: core,
          settings: settings,
          displayStore: MemoryDisplayStore(),
          forceDisplayCapable: false,
          launch: launch,
          links: LinkHandler(core: core, settings: settings),
          pickFile: (_) async => null,
          openUrl: (_) async => true,
        ),
      );
      for (var i = 0; i < 4; i++) {
        await t.pump(const Duration(milliseconds: 50));
      }
      return (core, settings);
    }

    testWidgets('a known server: the chip with the code', (t) async {
      await pump(
        t,
        const LinkUse(LinkTarget(code: 'K7QM2W', server: 'relay.cometcast.de')),
      );
      expect(find.text('K7Q M2W'), findsOneWidget);
      expect(find.byKey(const Key('relay-host')), findsNothing);
    });

    testWidgets(
      'an unknown server: the relay page first; Cancel keeps the chip and the default server',
      (t) async {
        final (_, settings) = await pump(
          t,
          const LinkConfirm(
            LinkTarget(code: 'K7QM2W', server: 'cast.example.org'),
          ),
        );
        expect(find.byKey(const Key('relay-host')), findsOneWidget);
        expect(find.text('cast.example.org'), findsOneWidget);
        await t.tap(find.byKey(const Key('relay-cancel')));
        await t.pump(const Duration(milliseconds: 100));
        expect(find.byKey(const Key('relay-host')), findsNothing);
        expect(find.text('K7Q M2W'), findsOneWidget);
        expect(settings.serverAddress, 'relay.cometcast.de');
        expect(settings.confirmedServers, isEmpty);
      },
    );

    testWidgets('Use takes the server and remembers it as confirmed', (
      t,
    ) async {
      final (_, settings) = await pump(
        t,
        const LinkConfirm(
          LinkTarget(code: 'K7QM2W', server: 'cast.example.org'),
        ),
      );
      await t.tap(find.byKey(const Key('relay-use')));
      await t.pump(const Duration(milliseconds: 100));
      await t.pump(const Duration(milliseconds: 100));
      expect(settings.confirmedServers, {'cast.example.org'});
      expect(settings.serverAddress, 'cast.example.org');
      expect(find.text('K7Q M2W'), findsOneWidget);
    });
  });
}
