// SPDX-License-Identifier: GPL-3.0-or-later
// The server field with discovery (sources, lock, notes, progress, cancel), the announcement
// of the receiver and the list of nearby receivers. All with the fake core: no machine policy
// and no network are involved.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/discovery.dart';
import 'package:cometcast/l10n/app_localizations.dart';
import 'package:cometcast/receive_page.dart';
import 'package:cometcast/send_page.dart';
import 'package:cometcast/settings.dart';
import 'package:cometcast/widgets.dart';

import 'fake_core.dart';

const announcedLabel = 'Announced by a display on this network, not verified';

const _policyOnly = ServerDefaults(
  servers: [ServerCandidate(address: 'wss://mdm.example:443', source: 'mdm')],
  locked: true,
  announcementsAllowed: false,
  notices: [],
  devFallback: '127.0.0.1:8443',
);

ServerDiscovery _found(
  List<(String, String)> servers, {
  bool locked = false,
  bool announcementsAllowed = true,
  List<DiscoveryWarning> notices = const [],
  List<String> warnings = const [],
}) => ServerDiscovery(
  servers: [
    for (final (address, source) in servers)
      ServerCandidate(address: address, source: source),
  ],
  locked: locked,
  announcementsAllowed: announcementsAllowed,
  notices: notices,
  warnings: warnings,
);

/// Opens [page] built from the core and settings in a navigator (so that it can be popped).
Future<void> openPage(
  WidgetTester tester, {
  required FakeCoreApi core,
  required AppSettings settings,
  required Widget Function(FakeCoreApi, AppSettings) page,
  String locale = 'en',
}) async {
  tester.view.physicalSize = const Size(900, 2400);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      locale: Locale(locale),
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: const [Locale('en'), Locale('de')],
      home: Builder(
        builder: (context) => Scaffold(
          body: Center(
            child: TextButton(
              key: const Key('open'),
              onPressed: () => Navigator.of(context).push(
                MaterialPageRoute<void>(builder: (_) => page(core, settings)),
              ),
              child: const Text('open'),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const Key('open')));
  await tester.pumpAndSettle();
}

Future<void> openReceive(
  WidgetTester tester,
  FakeCoreApi core,
  AppSettings settings, {
  String locale = 'en',
}) => openPage(
  tester,
  core: core,
  settings: settings,
  locale: locale,
  page: (c, s) => ReceivePage(
    core: c,
    settings: s,
    receiverName: () => 'Room 12',
    setFullscreen: (_) async => true,
  ),
);

Future<void> openSend(
  WidgetTester tester,
  FakeCoreApi core,
  AppSettings settings, {
  String locale = 'en',
}) => openPage(
  tester,
  core: core,
  settings: settings,
  locale: locale,
  page: (c, s) => SendPage(core: c, settings: s),
);

String fieldText(WidgetTester tester) => tester
    .widget<TextField>(find.byKey(const Key('serverField')))
    .controller!
    .text;

TextField serverField(WidgetTester tester) =>
    tester.widget<TextField>(find.byKey(const Key('serverField')));

String chipSource(WidgetTester tester, String address) =>
    tester.widget<Text>(find.byKey(Key('serverSource:$address'))).data!;

Future<void> goBack(WidgetTester tester) async {
  final NavigatorState nav = tester.state(find.byType(Navigator));
  nav.pop();
  await tester.pumpAndSettle();
}

void main() {
  group('sources and order', () {
    testWidgets(
      'policy first, then DNS, announcement, default; each with its label',
      (tester) async {
        final core = FakeCoreApi()
          ..defaults = const ServerDefaults(
            servers: [
              ServerCandidate(address: 'wss://mdm.example:443', source: 'mdm'),
              ServerCandidate(
                address: 'relay.cometcast.de:443',
                source: 'default-list',
              ),
            ],
            locked: false,
            announcementsAllowed: true,
            notices: [],
            devFallback: '127.0.0.1:8443',
          )
          ..discovery = _found([
            ('wss://mdm.example:443', 'mdm'),
            ('srv.example.org:8443', 'dns-srv'),
            ('10.0.0.7:8443', 'display-announcement'),
            ('relay.cometcast.de:443', 'default-list'),
          ]);
        final settings = AppSettings.fromDefaults(core.serverDefaults());
        await openReceive(tester, core, settings);

        expect(fieldText(tester), 'wss://mdm.example:443');
        expect(settings.serverSuggestions.map((s) => s.address), [
          'wss://mdm.example:443',
          'srv.example.org:8443',
          '10.0.0.7:8443',
          'relay.cometcast.de:443',
          '127.0.0.1:8443',
        ]);
        expect(
          chipSource(tester, 'wss://mdm.example:443'),
          'Set by your administrator',
        );
        expect(
          chipSource(tester, 'srv.example.org:8443'),
          'Found in the network (DNS)',
        );
        expect(chipSource(tester, '10.0.0.7:8443'), announcedLabel);
        expect(chipSource(tester, 'relay.cometcast.de:443'), 'Default list');
        expect(
          chipSource(tester, '127.0.0.1:8443'),
          'This computer (development)',
        );
      },
    );

    testWidgets(
      'without a policy the DNS server fills the field, a tap on a chip changes it',
      (tester) async {
        final core = FakeCoreApi()
          ..discovery = _found([
            ('srv.example.org:8443', 'dns-srv'),
            ('relay.cometcast.de:443', 'default-list'),
          ]);
        final settings = AppSettings.fromDefaults(core.serverDefaults());
        // Before the answer: the offline list.
        expect(settings.serverAddress, 'relay.cometcast.de:443');
        await openReceive(tester, core, settings);
        expect(fieldText(tester), 'srv.example.org:8443');
        await tester.tap(
          find.byKey(const Key('serverSuggestion:relay.cometcast.de:443')),
        );
        await tester.pump();
        expect(fieldText(tester), 'relay.cometcast.de:443');
        expect(settings.serverAddress, 'relay.cometcast.de:443');
      },
    );

    testWidgets(
      'an announced server is only a suggestion after the saved one',
      (tester) async {
        final core = FakeCoreApi()
          ..discovery = _found([
            ('10.0.0.7:8443', 'display-announcement'),
            ('relay.cometcast.de:443', 'default-list'),
          ]);
        final settings = await AppSettings.load(
          core: core,
          store: MemorySettingsStore('mine.example:1'),
        );
        await openReceive(tester, core, settings);
        expect(fieldText(tester), 'mine.example:1');
        expect(
          settings.serverSuggestions.first.kind,
          ServerSourceKind.lastUsed,
        );
        expect(chipSource(tester, 'mine.example:1'), 'Last used');
        expect(chipSource(tester, '10.0.0.7:8443'), announcedLabel);
      },
    );

    testWidgets(
      'a policy server beats the saved one, which stays a suggestion',
      (tester) async {
        final core = FakeCoreApi()
          ..defaults = const ServerDefaults(
            servers: [
              ServerCandidate(address: 'wss://mdm.example:443', source: 'mdm'),
            ],
            locked: false,
            announcementsAllowed: true,
            notices: [],
            devFallback: '127.0.0.1:8443',
          );
        final settings = await AppSettings.load(
          core: core,
          store: MemorySettingsStore('mine.example:1'),
        );
        expect(settings.serverAddress, 'wss://mdm.example:443');
        expect(settings.serverSuggestions.map((s) => s.address).toList(), [
          'wss://mdm.example:443',
          'mine.example:1',
          '127.0.0.1:8443',
        ]);
      },
    );

    testWidgets('an empty network result still leaves the development server', (
      tester,
    ) async {
      final core = FakeCoreApi()
        ..defaults = const ServerDefaults(
          servers: [],
          locked: false,
          announcementsAllowed: true,
          notices: [],
          devFallback: '127.0.0.1:8443',
        )
        ..discovery = _found([]);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      expect(fieldText(tester), '127.0.0.1:8443');
      expect(
        find.byKey(const Key('serverSuggestion:127.0.0.1:8443')),
        findsOneWidget,
      );
      expect(find.byKey(const Key('serverLockedNote')), findsNothing);
    });

    testWidgets('a source name from a newer core gets a neutral label', (
      tester,
    ) async {
      final core = FakeCoreApi()
        ..discovery = _found([('x.example:1', 'future-source')]);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      expect(chipSource(tester, 'x.example:1'), 'Other source');
    });
  });

  group('locked by a policy', () {
    testWidgets(
      'the field is read-only, explained, and only policy servers are offered',
      (tester) async {
        final core = FakeCoreApi()..defaults = _policyOnly;
        final store = MemorySettingsStore('mine.example:1');
        final settings = await AppSettings.load(
          core: core,
          store: store,
          envServer: 'env.example:2',
        );
        // Neither the saved nor the development server counts under a lock.
        expect(settings.serverAddress, 'wss://mdm.example:443');
        await openReceive(tester, core, settings);
        expect(serverField(tester).readOnly, isTrue);
        expect(
          find.text('Your administrator has set the server.'),
          findsOneWidget,
        );
        expect(settings.serverSuggestions.map((s) => s.address), [
          'wss://mdm.example:443',
        ]);
        expect(
          find.byKey(const Key('serverSuggestion:127.0.0.1:8443')),
          findsNothing,
        );

        // Typing does nothing; the model refuses as well.
        await tester.enterText(
          find.byKey(const Key('serverField')),
          'evil.example:1',
        );
        await tester.pump();
        settings.serverAddress = 'evil.example:1';
        expect(settings.serverAddress, 'wss://mdm.example:443');

        // Receiving uses the policy server and does not save it as "last used".
        await tester.tap(find.byKey(const Key('startButton')));
        await tester.pump();
        expect(core.receiverServer, 'wss://mdm.example:443');
        expect(store.server, 'mine.example:1');
      },
    );

    testWidgets('the explanation is translated', (tester) async {
      final core = FakeCoreApi()..defaults = _policyOnly;
      await openReceive(
        tester,
        core,
        AppSettings.fromDefaults(core.serverDefaults()),
        locale: 'de',
      );
      expect(
        find.text('Ihre IT-Administration hat den Server festgelegt.'),
        findsOneWidget,
      );
    });

    testWidgets(
      'the lock found by the network search applies even if the offline answer had none',
      (tester) async {
        final core = FakeCoreApi()
          ..discovery = _found(
            [('wss://mdm.example:443', 'mdm')],
            locked: true,
            announcementsAllowed: false,
          );
        final settings = AppSettings.fromDefaults(
          core.serverDefaults(),
          serverAddress: 'typed.example:1',
        );
        await openReceive(tester, core, settings);
        expect(fieldText(tester), 'wss://mdm.example:443');
        expect(serverField(tester).readOnly, isTrue);
      },
    );

    testWidgets(
      'an unusable policy: empty read-only field, calm note, start refused',
      (tester) async {
        final core = FakeCoreApi()
          ..defaults = const ServerDefaults(
            servers: [],
            locked: true,
            announcementsAllowed: false,
            notices: [DiscoveryWarning.policyUnusable],
            devFallback: '127.0.0.1:8443',
          );
        await openReceive(
          tester,
          core,
          AppSettings.fromDefaults(core.serverDefaults()),
        );
        expect(fieldText(tester), '');
        expect(serverField(tester).readOnly, isTrue);
        expect(
          find.text(
            'The policy file of your administrator could not be read, so no network server is offered.',
          ),
          findsOneWidget,
        );
        await tester.tap(find.byKey(const Key('startButton')));
        await tester.pump();
        expect(core.receiverServer, isNull);
        expect(find.byKey(const Key('serverError')), findsOneWidget);
      },
    );

    testWidgets('the send screen also locks the field', (tester) async {
      final core = FakeCoreApi()..defaults = _policyOnly;
      await openSend(
        tester,
        core,
        AppSettings.fromDefaults(core.serverDefaults()),
      );
      expect(serverField(tester).readOnly, isTrue);
      expect(find.byKey(const Key('serverLockedNote')), findsOneWidget);
    });
  });

  group('notes', () {
    final cases = <DiscoveryWarning, String>{
      DiscoveryWarning.policyUnusable: 'The policy file of your administrator could not be read, so no network server is offered.',
      DiscoveryWarning.policyNamesNoServer: 'Your administrator has not named a server. Please ask them for help.',
      DiscoveryWarning.dnsTimeout: 'The network did not answer the question for a server in time. The other sources are still used.',
      DiscoveryWarning.dnsFailed: 'The network could not be asked for a server (DNS). The other sources are still used.',
      DiscoveryWarning.dnsServiceUnavailable:
          'This network says that no server is provided here.',
      DiscoveryWarning.entriesIgnored: 'Some entries from the network were ignored because they did not look usable.',
      DiscoveryWarning.announcementFailed:
          'Display devices in the network could not be searched.',
      DiscoveryWarning.other: 'Not every source of servers could be asked.',
    };

    for (final entry in cases.entries) {
      testWidgets('code ${entry.key.name} has its own translated sentence', (
        tester,
      ) async {
        final core = FakeCoreApi()
          ..discovery = _found(
            [('relay.cometcast.de:443', 'default-list')],
            notices: [entry.key],
            warnings: ['RAW <b>secret</b> text from the network'],
          );
        await openReceive(
          tester,
          core,
          AppSettings.fromDefaults(core.serverDefaults()),
        );
        expect(
          find.byKey(Key('discoveryNotice:${entry.key.name}')),
          findsOneWidget,
        );
        expect(find.text(entry.value), findsOneWidget);
        // Raw text of the core is never shown.
        expect(find.textContaining('RAW'), findsNothing);
        expect(find.textContaining('secret'), findsNothing);
      });
    }

    test('every code has a German sentence too', () async {
      final de = await AppLocalizations.delegate.load(const Locale('de'));
      final en = await AppLocalizations.delegate.load(const Locale('en'));
      final texts = <String>{};
      for (final w in DiscoveryWarning.values) {
        final t = discoveryNoticeText(de, w);
        expect(t, isNotEmpty);
        expect(t, isNot(discoveryNoticeText(en, w)));
        texts.add(t);
      }
      expect(texts.length, DiscoveryWarning.values.length);
    });

    testWidgets('a failing search is a calm note, not a crash', (tester) async {
      final core = FakeCoreApi()..discoveryThrows = true;
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      expect(find.byKey(const Key('discoveryNotice:other')), findsOneWidget);
      expect(tester.takeException(), isNull);
      // The offline list stays usable.
      expect(fieldText(tester), 'relay.cometcast.de:443');
    });

    testWidgets(
      'notes from the policy file (offline) show before the search ends',
      (tester) async {
        final core = FakeCoreApi()
          ..defaults = const ServerDefaults(
            servers: [],
            locked: true,
            announcementsAllowed: false,
            notices: [DiscoveryWarning.policyNamesNoServer],
            devFallback: '127.0.0.1:8443',
          )
          ..discoveryGate = Completer<void>();
        await openReceive(
          tester,
          core,
          AppSettings.fromDefaults(core.serverDefaults()),
        );
        expect(
          find.byKey(const Key('discoveryNotice:policyNamesNoServer')),
          findsOneWidget,
        );
      },
    );
  });

  group('progress and cancel', () {
    testWidgets(
      'a hint shows while the search runs and the screen stays usable',
      (tester) async {
        final gate = Completer<void>();
        final core = FakeCoreApi()
          ..discoveryGate = gate
          ..discovery = _found([('srv.example.org:8443', 'dns-srv')]);
        final settings = AppSettings.fromDefaults(core.serverDefaults());
        await openPage(
          tester,
          core: core,
          settings: settings,
          page: (c, s) =>
              ReceivePage(core: c, settings: s, receiverName: () => 'R'),
        );
        expect(
          find.text('Looking for servers in the network …'),
          findsOneWidget,
        );
        expect(settings.discovering, isTrue);
        // The offline pre-fill is there, the person can use it at once.
        expect(fieldText(tester), 'relay.cometcast.de:443');
        await tester.tap(find.byKey(const Key('startButton')));
        await tester.pump();
        expect(core.receiverServer, 'relay.cometcast.de:443');

        gate.complete();
        await tester.pumpAndSettle();
        expect(find.byKey(const Key('discoveryProgress')), findsNothing);
        expect(settings.discovering, isFalse);
        expect(
          settings.serverSuggestions.map((s) => s.address),
          contains('srv.example.org:8443'),
        );
      },
    );

    testWidgets('what the person typed is not overwritten by a late answer', (
      tester,
    ) async {
      final gate = Completer<void>();
      final core = FakeCoreApi()
        ..discoveryGate = gate
        ..discovery = _found([('srv.example.org:8443', 'dns-srv')]);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      await tester.enterText(
        find.byKey(const Key('serverField')),
        'mine.example:9',
      );
      await tester.pump();
      gate.complete();
      await tester.pumpAndSettle();
      expect(fieldText(tester), 'mine.example:9');
      expect(
        settings.serverSuggestions.map((s) => s.address),
        contains('srv.example.org:8443'),
      );
    });

    testWidgets('closing the screen cancels: a late answer changes nothing', (
      tester,
    ) async {
      final gate = Completer<void>();
      final core = FakeCoreApi()
        ..discoveryGate = gate
        ..discovery = _found([
          ('srv.example.org:8443', 'dns-srv'),
        ], locked: true);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      expect(settings.discovering, isTrue);
      await goBack(tester);
      expect(settings.discovering, isFalse);
      gate.complete();
      await tester.pumpAndSettle();
      expect(settings.locked, isFalse);
      expect(settings.serverAddress, 'relay.cometcast.de:443');
      expect(tester.takeException(), isNull);
    });

    testWidgets('a newer search replaces a running one', (tester) async {
      final first = Completer<void>();
      final core = FakeCoreApi()
        ..discoveryGate = first
        ..discovery = _found([('old.example:1', 'dns-srv')]);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      final a = settings.discover(core);
      core
        ..discoveryGate = null
        ..discovery = _found([('new.example:1', 'dns-srv')]);
      await settings.discover(core);
      first.complete();
      await a;
      expect(
        settings.serverSuggestions.map((s) => s.address),
        contains('new.example:1'),
      );
      expect(
        settings.serverSuggestions.map((s) => s.address),
        isNot(contains('old.example:1')),
      );
    });
  });

  group('announcing the receiver', () {
    testWidgets(
      'announces name and server while the view is open and stops on close',
      (tester) async {
        final core = FakeCoreApi();
        final settings = AppSettings.fromDefaults(core.serverDefaults());
        await openReceive(tester, core, settings);
        expect(core.advertiseCalls, hasLength(1));
        expect(core.advertiseCalls.single.name, 'Room 12');
        expect(core.advertiseCalls.single.server, 'relay.cometcast.de:443');
        expect(
          find.text('This device is visible to senders nearby as “Room 12”.'),
          findsOneWidget,
        );
        expect(core.stopAdvertisingCalls, 0);

        await goBack(tester);
        expect(core.stopAdvertisingCalls, 1);
      },
    );

    testWidgets('starting with another server announces that one', (
      tester,
    ) async {
      final core = FakeCoreApi();
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      await tester.enterText(
        find.byKey(const Key('serverField')),
        'mine.example:9',
      );
      await tester.tap(find.byKey(const Key('startButton')));
      await tester.pumpAndSettle();
      expect(core.advertiseCalls.map((c) => c.server), [
        'relay.cometcast.de:443',
        'mine.example:9',
      ]);
      // Starting again with the same server does not announce again.
      await tester.tap(find.byKey(const Key('stopButton')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('startButton')));
      await tester.pumpAndSettle();
      expect(core.advertiseCalls, hasLength(2));
    });

    testWidgets(
      'a refused server (loopback) is left out, the receiver is still announced',
      (tester) async {
        final core = FakeCoreApi()
          ..advertiseOutcomes = [
            AdvertiseOutcome.refused,
            AdvertiseOutcome.started,
          ];
        final settings = AppSettings(serverAddress: '127.0.0.1:8443');
        await openReceive(tester, core, settings);
        expect(core.advertiseCalls.map((c) => c.server), [
          '127.0.0.1:8443',
          null,
        ]);
        expect(find.byKey(const Key('advertisedNote')), findsOneWidget);
      },
    );

    testWidgets('when announcing is not possible nothing alarming is shown', (
      tester,
    ) async {
      final core = FakeCoreApi()..advertiseOutcomes = [AdvertiseOutcome.failed];
      await openReceive(
        tester,
        core,
        AppSettings.fromDefaults(core.serverDefaults()),
      );
      expect(find.byKey(const Key('advertisedNote')), findsNothing);
      expect(find.byKey(const Key('startButton')), findsOneWidget);
    });
  });

  group('nearby receivers', () {
    testWidgets(
      'lists name and server; a tap takes the server and moves to the code field',
      (tester) async {
        final core = FakeCoreApi();
        final settings = AppSettings.fromDefaults(core.serverDefaults());
        await openSend(tester, core, settings);
        expect(core.watchCalls, 1);
        expect(find.text('Nearby receivers'), findsOneWidget);
        expect(find.byKey(const Key('nearbyEmpty')), findsOneWidget);

        core.nearbyEvents!
          ..add(
            const NearbyReceiverEvent.found(
              id: 'room-12',
              name: 'Room 12',
              server: '10.0.0.7:8443',
            ),
          )
          ..add(const NearbyReceiverEvent.found(id: 'lab', name: 'Lab TV'));
        await tester.pumpAndSettle();
        expect(find.text('Room 12'), findsOneWidget);
        expect(find.text('Server: 10.0.0.7:8443'), findsOneWidget);
        expect(find.text('No server announced'), findsOneWidget);
        expect(find.byKey(const Key('nearbyHint')), findsOneWidget);

        await tester.tap(find.byKey(const Key('nearbyReceiver:room-12')));
        await tester.pumpAndSettle();
        expect(fieldText(tester), '10.0.0.7:8443');
        expect(settings.serverAddress, '10.0.0.7:8443');
        // The room code is not announced: the person types it.
        final code = tester.widget<TextField>(
          find.byKey(const Key('codeField')),
        );
        expect(code.focusNode!.hasFocus, isTrue);
        expect(code.controller!.text, isEmpty);

        // A receiver without a server keeps the current one.
        await tester.tap(find.byKey(const Key('nearbyReceiver:lab')));
        await tester.pumpAndSettle();
        expect(fieldText(tester), '10.0.0.7:8443');

        core.nearbyEvents!.add(const NearbyReceiverEvent.lost(id: 'room-12'));
        await tester.pumpAndSettle();
        expect(find.text('Room 12'), findsNothing);
      },
    );

    testWidgets('the chosen server is used to send', (tester) async {
      final core = FakeCoreApi();
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openSend(tester, core, settings);
      core.nearbyEvents!.add(
        const NearbyReceiverEvent.found(
          id: 'room-12',
          name: 'Room 12',
          server: '10.0.0.7:8443',
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('nearbyReceiver:room-12')));
      await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
      await tester.tap(find.byKey(const Key('startButton')));
      await tester.pumpAndSettle();
      expect(core.senderServer, '10.0.0.7:8443');
      expect(core.senderCode, 'K7M2QX');
    });

    testWidgets('closing the screen stops listening', (tester) async {
      final core = FakeCoreApi();
      await openSend(
        tester,
        core,
        AppSettings.fromDefaults(core.serverDefaults()),
      );
      await goBack(tester);
      expect(core.nearbyCancelled, isTrue);
      expect(core.stopWatchingCalls, 1);
    });

    testWidgets('a locked policy shows no list and does not listen', (
      tester,
    ) async {
      final core = FakeCoreApi()..defaults = _policyOnly;
      await openSend(
        tester,
        core,
        AppSettings.fromDefaults(core.serverDefaults()),
      );
      expect(core.watchCalls, 0);
      expect(find.byKey(const Key('nearbyReceivers')), findsNothing);
    });

    testWidgets('a policy that forbids announcements shows no list', (
      tester,
    ) async {
      final core = FakeCoreApi()
        ..defaults = const ServerDefaults(
          servers: [
            ServerCandidate(address: 'wss://mdm.example:443', source: 'mdm'),
          ],
          locked: false,
          announcementsAllowed: false,
          notices: [],
          devFallback: '127.0.0.1:8443',
        );
      await openSend(
        tester,
        core,
        AppSettings.fromDefaults(core.serverDefaults()),
      );
      expect(core.watchCalls, 0);
      expect(find.byKey(const Key('nearbyReceivers')), findsNothing);
    });

    testWidgets('when listening is not possible a calm note is shown', (
      tester,
    ) async {
      final core = FakeCoreApi();
      await openSend(
        tester,
        core,
        AppSettings.fromDefaults(core.serverDefaults()),
      );
      core.nearbyEvents!.add(const NearbyReceiverEvent.unavailable());
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('nearbyUnavailable')), findsOneWidget);
    });

    test(
      'a flood of announcements evicts the oldest, newer ones still appear',
      () async {
        final core = FakeCoreApi();
        final nearby = NearbyReceivers(core)..start();
        await Future<void>.delayed(Duration.zero);
        for (var i = 0; i < 100; i++) {
          core.nearbyEvents!.add(
            NearbyReceiverEvent.found(id: 'r$i', name: 'R$i'),
          );
        }
        await Future<void>.delayed(Duration.zero);
        expect(nearby.receivers, hasLength(NearbyReceivers.maxShown));
        expect(nearby.truncated, isTrue);
        // The newest are kept, the oldest are gone.
        expect(nearby.receivers.first.id, 'r68');
        expect(nearby.receivers.last.id, 'r99');
        // One that is known can still be updated.
        core.nearbyEvents!.add(
          const NearbyReceiverEvent.found(id: 'r99', name: 'New'),
        );
        await Future<void>.delayed(Duration.zero);
        expect(nearby.receivers.last.name, 'New');
        // Room again: the note goes away.
        core.nearbyEvents!.add(const NearbyReceiverEvent.lost(id: 'r99'));
        await Future<void>.delayed(Duration.zero);
        expect(nearby.truncated, isFalse);
        nearby.dispose();
      },
    );
  });

  group('review fixes', () {
    testWidgets('a refused answer after the view closed starts nothing new', (
      tester,
    ) async {
      final core = _GatedCore()..firstGate = Completer();
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      await goBack(tester);
      // The stop waits for the start that is still running.
      expect(core.stopAdvertisingCalls, 0);
      core.firstGate!.complete(AdvertiseOutcome.refused);
      await tester.pumpAndSettle();
      expect(core.advertiseCalls, hasLength(1));
      expect(core.stopAdvertisingCalls, 1);
    });

    testWidgets('stop is queued behind the pending start (order of calls)', (
      tester,
    ) async {
      final core = _GatedCore()..firstGate = Completer();
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      await goBack(tester);
      core.firstGate!.complete(AdvertiseOutcome.started);
      await tester.pumpAndSettle();
      expect(core.order, ['start', 'stop']);
    });

    for (final forbid in ['announcements', 'lock']) {
      testWidgets('no announcement when the policy has $forbid', (
        tester,
      ) async {
        final core = FakeCoreApi()
          ..defaults = ServerDefaults(
            servers: const [
              ServerCandidate(address: 'wss://mdm.example:443', source: 'mdm'),
            ],
            locked: forbid == 'lock',
            announcementsAllowed: forbid != 'announcements',
            notices: const [],
            devFallback: '127.0.0.1:8443',
          );
        final settings = AppSettings.fromDefaults(core.serverDefaults());
        await openReceive(tester, core, settings);
        expect(core.advertiseCalls, isEmpty);
        expect(find.byKey(const Key('advertisedNote')), findsNothing);
      });
    }

    testWidgets('a ban learned late stops a running announcement', (
      tester,
    ) async {
      final core = FakeCoreApi()
        ..discoveryGate = Completer()
        ..discovery = _found([
          ('wss://mdm.example:443', 'mdm'),
        ], announcementsAllowed: false);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openReceive(tester, core, settings);
      expect(core.advertiseCalls, hasLength(1));
      expect(find.byKey(const Key('advertisedNote')), findsOneWidget);
      core.discoveryGate!.complete();
      await tester.pumpAndSettle();
      expect(core.stopAdvertisingCalls, 1);
      expect(find.byKey(const Key('advertisedNote')), findsNothing);
    });

    testWidgets(
      'a late announced server does not replace the default pre-fill',
      (tester) async {
        final core = FakeCoreApi()
          ..discoveryGate = Completer()
          ..discovery = _found([
            ('ws://192.168.1.66:8443', 'display-announcement'),
            ('relay.cometcast.de:443', 'default-list'),
          ]);
        final settings = AppSettings.fromDefaults(core.serverDefaults());
        await openSend(tester, core, settings);
        await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
        core.discoveryGate!.complete();
        await tester.pumpAndSettle();
        expect(settings.serverAddress, 'relay.cometcast.de:443');
        expect(fieldText(tester), 'relay.cometcast.de:443');
        // It is offered, labelled, and not selected.
        expect(chipSource(tester, 'ws://192.168.1.66:8443'), announcedLabel);
        expect(find.byKey(const Key('serverFieldSource')), findsNothing);
      },
    );

    test('only announced servers: nothing is pre-filled from them', () {
      final settings = AppSettings(
        serverSuggestions: const [
          ServerSuggestion('10.0.0.7:8443', ServerSourceKind.announced),
        ],
      );
      expect(settings.serverAddress, fallbackServerAddress);
    });

    testWidgets('a tapped chip shows its source next to the field', (
      tester,
    ) async {
      final core = FakeCoreApi()
        ..discovery = _found([
          ('srv.example.org:8443', 'dns-srv'),
          ('10.0.0.7:8443', 'display-announcement'),
          ('relay.cometcast.de:443', 'default-list'),
        ]);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openSend(tester, core, settings);
      expect(find.byKey(const Key('serverFieldSource')), findsNothing);
      await tester.tap(find.byKey(const Key('serverSuggestion:10.0.0.7:8443')));
      await tester.pump();
      expect(
        tester.widget<Text>(find.byKey(const Key('serverFieldSource'))).data,
        'Source of this address: $announcedLabel',
      );
      await tester.tap(
        find.byKey(const Key('serverSuggestion:srv.example.org:8443')),
      );
      await tester.pump();
      expect(
        tester.widget<Text>(find.byKey(const Key('serverFieldSource'))).data,
        'Source of this address: Found in the network (DNS)',
      );
      // Typing removes the label: the text is the person's own.
      await tester.enterText(find.byKey(const Key('serverField')), 'mine:1');
      await tester.pump();
      expect(find.byKey(const Key('serverFieldSource')), findsNothing);
    });

    testWidgets('German label of the source', (tester) async {
      final core = FakeCoreApi()
        ..discovery = _found([('10.0.0.7:8443', 'display-announcement')]);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openSend(tester, core, settings, locale: 'de');
      await tester.tap(find.byKey(const Key('serverSuggestion:10.0.0.7:8443')));
      await tester.pump();
      expect(
        tester.widget<Text>(find.byKey(const Key('serverFieldSource'))).data,
        'Quelle dieser Adresse: Von einem Anzeigegerät in diesem Netzwerk '
        'angekündigt, nicht überprüft',
      );
    });

    testWidgets(
      'an announced server is labelled in the tile and never saved as last used',
      (tester) async {
        final core = FakeCoreApi();
        final store = MemorySettingsStore();
        final settings = AppSettings.fromDefaults(
          core.serverDefaults(),
          store: store,
        );
        await openSend(tester, core, settings);
        core.nearbyEvents!.add(
          const NearbyReceiverEvent.found(
            id: 'x',
            name: 'TV',
            server: 'wss://evil.example:443',
          ),
        );
        await tester.pumpAndSettle();
        expect(
          tester.widget<Text>(find.byKey(const Key('nearbyUnverified:x'))).data,
          announcedLabel,
        );
        await tester.tap(find.byKey(const Key('nearbyReceiver:x')));
        await tester.pump();
        expect(
          tester.widget<Text>(find.byKey(const Key('serverFieldSource'))).data,
          'Source of this address: $announcedLabel',
        );
        await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
        await tester.tap(find.byKey(const Key('startButton')));
        await tester.pumpAndSettle();
        expect(core.senderServer, 'wss://evil.example:443');
        expect(store.server, isNull);
        // The next start is not pre-filled from it.
        final next = await AppSettings.load(core: core, store: store);
        expect(next.serverAddress, isNot('wss://evil.example:443'));
      },
    );

    testWidgets('a server tapped from a DNS chip is remembered', (
      tester,
    ) async {
      final core = FakeCoreApi()
        ..discovery = _found([('srv.example.org:8443', 'dns-srv')]);
      final store = MemorySettingsStore();
      final settings = AppSettings.fromDefaults(
        core.serverDefaults(),
        store: store,
      );
      await openSend(tester, core, settings);
      await tester.tap(
        find.byKey(const Key('serverSuggestion:srv.example.org:8443')),
      );
      await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
      await tester.tap(find.byKey(const Key('startButton')));
      await tester.pumpAndSettle();
      expect(store.server, 'srv.example.org:8443');
    });

    testWidgets('the server is trimmed before it is sent', (tester) async {
      final core = FakeCoreApi();
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openSend(tester, core, settings);
      await tester.enterText(
        find.byKey(const Key('serverField')),
        '  10.0.0.7:8443 ',
      );
      await tester.enterText(find.byKey(const Key('codeField')), 'K7M2QX');
      await tester.tap(find.byKey(const Key('startButton')));
      await tester.pumpAndSettle();
      expect(core.senderServer, '10.0.0.7:8443');
    });

    testWidgets('a lock that arrives late hides the nearby list', (
      tester,
    ) async {
      final core = FakeCoreApi()
        ..discoveryGate = Completer()
        ..discovery = _found([('wss://mdm.example:443', 'mdm')], locked: true);
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openSend(tester, core, settings);
      expect(find.byKey(const Key('nearbyReceivers')), findsOneWidget);
      core.discoveryGate!.complete();
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('nearbyReceivers')), findsNothing);
      expect(core.nearbyCancelled, isTrue);
      expect(core.stopWatchingCalls, 1);
    });

    testWidgets('the list says "and more" after a flood', (tester) async {
      final core = FakeCoreApi();
      final settings = AppSettings.fromDefaults(core.serverDefaults());
      await openSend(tester, core, settings);
      for (var i = 0; i < NearbyReceivers.maxShown + 1; i++) {
        core.nearbyEvents!.add(
          NearbyReceiverEvent.found(id: 'r$i', name: 'R$i'),
        );
      }
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('nearbyMore')), findsOneWidget);
    });

    test(
      'a listener that closed before the start never starts the core',
      () async {
        final core = FakeCoreApi();
        NearbyReceivers(core)
          ..start()
          ..dispose();
        final second = NearbyReceivers(core)..start();
        await Future<void>.delayed(Duration.zero);
        expect(core.watchCalls, 1);
        expect(core.stopWatchingCalls, 1);
        second.dispose();
      },
    );

    test('SerialQueue runs jobs in order even when one fails', () async {
      final q = SerialQueue();
      final log = <String>[];
      final gate = Completer<void>();
      final a = q.run(() async {
        await gate.future;
        log.add('a');
        throw StateError('x');
      });
      final b = q.run(() async => log.add('b'));
      await Future<void>.delayed(Duration.zero);
      expect(log, isEmpty);
      gate.complete();
      await expectLater(a, throwsStateError);
      await b;
      expect(log, ['a', 'b']);
    });
  });
}

class _GatedCore extends FakeCoreApi {
  Completer<AdvertiseOutcome>? firstGate;
  final order = <String>[];

  @override
  Future<AdvertiseOutcome> startAdvertising({
    required String name,
    String? preferredServer,
  }) async {
    advertiseCalls.add((name: name, server: preferredServer));
    order.add('start');
    if (advertiseCalls.length == 1 && firstGate != null) {
      return firstGate!.future;
    }
    return AdvertiseOutcome.started;
  }

  @override
  Future<void> stopAdvertising() async {
    order.add('stop');
    stopAdvertisingCalls++;
  }
}
