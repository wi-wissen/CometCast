// SPDX-License-Identifier: GPL-3.0-or-later
// The server choice of the apps without any screen (what the old home, send and receive pages tested
// through their server field): sources and order, a policy that locks the address, late answers that
// do not overwrite a choice, a search that is cancelled or replaced, the list of nearby displays and
// the serial queue behind them. The pages that showed these are gone; the rules are not.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/discovery.dart';
import 'package:cometcast/settings.dart';

import 'fake_core.dart';

const _policy = ServerDefaults(
  servers: [ServerCandidate(address: 'wss://mdm.example:443', source: 'mdm')],
  locked: true,
  announcementsAllowed: false,
  notices: [],
  devFallback: '127.0.0.1:8443',
);

ServerDiscovery found(
  List<(String, String)> servers, {
  bool locked = false,
  bool announcementsAllowed = true,
  List<DiscoveryWarning> notices = const [],
}) => ServerDiscovery(
  servers: [
    for (final (address, source) in servers)
      ServerCandidate(address: address, source: source),
  ],
  locked: locked,
  announcementsAllowed: announcementsAllowed,
  notices: notices,
  warnings: const [],
);

Future<void> tick() => Future<void>.delayed(const Duration(milliseconds: 10));

void main() {
  group('sources and order', () {
    test(
      'policy first, then DNS, announcement, default; each labelled',
      () async {
        final core = FakeCoreApi()
          ..discovery = found([
            ('relay.cometcast.de:443', 'default-list'),
            ('box.local:8443', 'display-announcement'),
            ('cast.example.org:443', 'dns-srv'),
            ('wss://mdm.example:443', 'mdm'),
          ]);
        final s = AppSettings.fromDefaults(core.serverDefaults());
        await s.discover(core);
        final kinds = s.serverSuggestions.map((x) => x.kind).toList();
        expect(kinds.first, ServerSourceKind.defaultList);
        // The core hands the list out best first; the suggestion keeps its source.
        expect(
          s.serverSuggestions.map((x) => x.address),
          containsAll(['wss://mdm.example:443', 'box.local:8443']),
        );
        expect(
          s.serverSuggestions
              .firstWhere((x) => x.address == 'box.local:8443')
              .kind,
          ServerSourceKind.announced,
        );
      },
    );

    test(
      'an empty network result still leaves the development server',
      () async {
        final core = FakeCoreApi()..discovery = found([]);
        final s = AppSettings.fromDefaults(core.serverDefaults());
        await s.discover(core);
        expect(
          s.serverSuggestions.map((x) => x.address),
          contains('127.0.0.1:8443'),
        );
      },
    );

    test('a source name from a newer core gets a neutral kind', () {
      expect(
        ServerSourceKind.fromRust('from-the-future'),
        ServerSourceKind.unknown,
      );
      expect(ServerSourceKind.fromRust('mdm'), ServerSourceKind.policy);
      expect(ServerSourceKind.fromRust('dns-srv'), ServerSourceKind.dns);
    });

    test('the last used server comes right after the policy and DNS', () {
      final core = FakeCoreApi();
      final s = AppSettings.fromDefaults(
        core.serverDefaults(),
        savedServer: 'last.example.org:443',
      );
      expect(
        s.serverSuggestions.first.kind,
        anyOf(ServerSourceKind.lastUsed, ServerSourceKind.defaultList),
      );
      expect(
        s.serverSuggestions.map((x) => x.address),
        contains('last.example.org:443'),
      );
      expect(s.serverAddress, 'last.example.org:443');
    });

    test('only announced servers: nothing is pre-filled from them', () async {
      final core = FakeCoreApi()
        ..defaults = const ServerDefaults(
          servers: [],
          locked: false,
          announcementsAllowed: true,
          notices: [],
          devFallback: '',
        )
        ..discovery = found([('box.local:8443', 'display-announcement')]);
      final s = AppSettings.fromDefaults(core.serverDefaults());
      final before = s.serverAddress;
      await s.discover(core);
      expect(
        s.serverAddress,
        before,
        reason: 'an announcement is never taken unasked',
      );
      expect(
        s.serverSuggestions.map((x) => x.kind),
        contains(ServerSourceKind.announced),
      );
    });
  });

  group('locked by a policy', () {
    test('only the policy server is offered and the field is locked', () {
      final core = FakeCoreApi()..defaults = _policy;
      final s = AppSettings.fromDefaults(
        core.serverDefaults(),
        savedServer: 'other.example.org:443',
      );
      expect(s.locked, isTrue);
      expect(s.serverSuggestions.map((x) => x.address), [
        'wss://mdm.example:443',
      ]);
      expect(s.serverAddress, 'wss://mdm.example:443');
    });

    test('a locked address cannot be typed over', () {
      final core = FakeCoreApi()..defaults = _policy;
      final s = AppSettings.fromDefaults(core.serverDefaults());
      s.serverAddress = 'evil.example.org:443';
      expect(s.serverAddress, 'wss://mdm.example:443');
      s.chooseSuggestion(
        const ServerSuggestion('evil.example.org:443', ServerSourceKind.dns),
      );
      expect(s.serverAddress, 'wss://mdm.example:443');
    });

    test('a locked server is not remembered as the last used', () async {
      final core = FakeCoreApi()..defaults = _policy;
      final store = MemorySettingsStore();
      final s = AppSettings.fromDefaults(core.serverDefaults(), store: store);
      await s.rememberServer();
      expect(store.server, isNull);
    });

    test(
      'a policy server beats a saved or development server at start',
      () async {
        final core = FakeCoreApi()..defaults = _policy;
        final s = await AppSettings.load(
          core: core,
          store: MemorySettingsStore('saved.example.org:443'),
          envServer: null,
        );
        expect(s.serverAddress, 'wss://mdm.example:443');
      },
    );

    test('announcements are not allowed under that policy', () {
      final core = FakeCoreApi()..defaults = _policy;
      expect(
        AppSettings.fromDefaults(core.serverDefaults()).announcementsAllowed,
        isFalse,
      );
    });
  });

  group('notes of a failed source', () {
    test('every warning code of the core is kept as a code', () async {
      final core = FakeCoreApi()
        ..discovery = found([
          ('relay.cometcast.de:443', 'default-list'),
        ], notices: DiscoveryWarning.values);
      final s = AppSettings.fromDefaults(core.serverDefaults());
      await s.discover(core);
      expect(s.notices, containsAll(DiscoveryWarning.values));
    });

    test('a failing search is a calm state, not a crash', () async {
      final core = FakeCoreApi()..discoveryThrows = true;
      final s = AppSettings.fromDefaults(core.serverDefaults());
      await s.discover(core);
      expect(s.discovering, isFalse);
      expect(s.serverSuggestions, isNotEmpty);
    });
  });

  group('progress and cancel', () {
    test('discovering is true while the search runs', () async {
      final core = FakeCoreApi()..discoveryGate = Completer<void>();
      final s = AppSettings.fromDefaults(core.serverDefaults());
      final run = s.discover(core);
      await tick();
      expect(s.discovering, isTrue);
      core.discoveryGate!.complete();
      await run;
      expect(s.discovering, isFalse);
    });

    test('what the person typed is not overwritten by a late answer', () async {
      final core = FakeCoreApi()
        ..discoveryGate = Completer<void>()
        ..discovery = found([('dns.example.org:443', 'dns-srv')]);
      final s = AppSettings.fromDefaults(core.serverDefaults());
      final run = s.discover(core);
      s.serverAddress = 'typed.example.org:443';
      core.discoveryGate!.complete();
      await run;
      expect(s.serverAddress, 'typed.example.org:443');
    });

    test('closing the screen cancels: a late answer changes nothing', () async {
      final core = FakeCoreApi()
        ..discoveryGate = Completer<void>()
        ..discovery = found([('dns.example.org:443', 'dns-srv')]);
      final s = AppSettings.fromDefaults(core.serverDefaults());
      final before = s.serverSuggestions.length;
      final run = s.discover(core);
      s.cancelDiscovery();
      core.discoveryGate!.complete();
      await run;
      expect(s.serverSuggestions.length, before);
    });

    test('a newer search replaces a running one', () async {
      final core = FakeCoreApi()..discoveryGate = Completer<void>();
      final s = AppSettings.fromDefaults(core.serverDefaults());
      final first = s.discover(core);
      final second = s.discover(core);
      core.discoveryGate!.complete();
      await Future.wait([first, second]);
      expect(core.discoverCalls, 2);
      expect(s.discovering, isFalse);
    });
  });

  group('the server is remembered only after a join', () {
    test(
      'rememberServer stores the trimmed address; announced is not stored',
      () async {
        final core = FakeCoreApi();
        final store = MemorySettingsStore();
        final s = AppSettings.fromDefaults(
          core.serverDefaults(),
          serverAddress: ' cast.example.org:443 ',
          store: store,
        );
        await s.rememberServer();
        expect(store.server, 'cast.example.org:443');
      },
    );

    test('a tapped announced server is never stored', () async {
      final core = FakeCoreApi();
      final store = MemorySettingsStore();
      final s = AppSettings.fromDefaults(core.serverDefaults(), store: store);
      s.chooseSuggestion(
        const ServerSuggestion('box.local:8443', ServerSourceKind.announced),
      );
      await s.rememberServer();
      expect(store.server, isNull);
    });

    test('confirmed servers are kept once', () async {
      final core = FakeCoreApi();
      final store = MemorySettingsStore();
      final s = AppSettings.fromDefaults(core.serverDefaults(), store: store);
      await s.confirmServer('cast.example.org');
      await s.confirmServer('cast.example.org');
      expect(s.confirmedServers, {'cast.example.org'});
      expect(store.confirmedServers, ['cast.example.org']);
    });
  });

  group('the display name', () {
    test(
      'cleanDisplayName drops control and bidi characters and truncates',
      () {
        expect(
          cleanDisplayName('  Ada \u202E Lovelace\u0007 '),
          'Ada Lovelace',
        );
        expect(cleanDisplayName('a\tb\nc'), 'a b c');
        expect(cleanDisplayName('x' * 200).length, maxDisplayNameChars);
        expect(cleanDisplayName('\u200B\u200C'), '');
      },
    );

    test(
      'the clean name is remembered, an unchanged one is not saved again',
      () {
        final core = FakeCoreApi();
        final store = MemorySettingsStore();
        final s = AppSettings.fromDefaults(core.serverDefaults(), store: store);
        var notified = 0;
        s.addListener(() => notified++);
        s.setDisplayName(' Ada\u202E ');
        expect(s.displayName, 'Ada');
        expect(store.displayName, 'Ada');
        s.setDisplayName('Ada');
        expect(notified, 1);
      },
    );
  });

  group('the codec consent', () {
    const info = UiCodecInfo(
      fileName: 'libopenh264.so',
      version: '2.6.0',
      host: 'ciscobinary.openh264.org',
      downloadBytes: 1,
      autoDownload: false,
      adminDir: null,
    );

    test('the consent belongs to a host and a version', () async {
      final core = FakeCoreApi();
      final store = MemorySettingsStore();
      final s = AppSettings.fromDefaults(core.serverDefaults(), store: store);
      expect(s.codecConsentFor(info), isFalse);
      await s.rememberCodecConsent(info);
      expect(s.codecConsentFor(info), isTrue);
      expect(
        s.codecConsentFor(
          const UiCodecInfo(
            fileName: 'libopenh264.so',
            version: '2.7.0',
            host: 'ciscobinary.openh264.org',
            downloadBytes: 1,
            autoDownload: false,
            adminDir: null,
          ),
        ),
        isFalse,
      );
    });
  });

  group('nearby displays', () {
    NearbyReceiverEvent found1(String id, String name, [String? server]) =>
        NearbyReceiverEvent.found(id: id, name: name, server: server);

    test('found and lost displays come and go in order', () async {
      final core = FakeCoreApi();
      final n = NearbyReceivers(core)..start();
      await tick();
      core.nearbyEvents!
        ..add(found1('a', 'Room 12', 'box.local:8443'))
        ..add(found1('b', 'Room 13'));
      await tick();
      expect(n.receivers.map((r) => r.name), ['Room 12', 'Room 13']);
      core.nearbyEvents!.add(const NearbyReceiverEvent.lost(id: 'a'));
      await tick();
      expect(n.receivers.map((r) => r.name), ['Room 13']);
      n.dispose();
    });

    test('closing the screen stops listening', () async {
      final core = FakeCoreApi();
      final n = NearbyReceivers(core)..start();
      await tick();
      n.dispose();
      await tick();
      expect(core.nearbyCancelled, isTrue);
      expect(core.stopWatchingCalls, 1);
    });

    test('when listening is not possible a flag says so', () async {
      final core = FakeCoreApi();
      final n = NearbyReceivers(core)..start();
      await tick();
      core.nearbyEvents!.add(const NearbyReceiverEvent.unavailable());
      await tick();
      expect(n.unavailable, isTrue);
      n.dispose();
    });

    test('a flood keeps the newest and says "and more"', () async {
      final core = FakeCoreApi();
      final n = NearbyReceivers(core)..start();
      await tick();
      for (var i = 0; i < NearbyReceivers.maxShown + 5; i++) {
        core.nearbyEvents!.add(found1('id$i', 'Room $i'));
      }
      await tick();
      expect(n.receivers.length, NearbyReceivers.maxShown);
      expect(n.truncated, isTrue);
      expect(n.receivers.last.name, 'Room ${NearbyReceivers.maxShown + 4}');
      n.dispose();
    });

    test(
      'a receiver is never given a room code, only a name and a server',
      () async {
        final core = FakeCoreApi();
        final n = NearbyReceivers(core)..start();
        await tick();
        core.nearbyEvents!.add(found1('a', 'Room 12', 'box.local:8443'));
        await tick();
        expect(n.receivers.single.server, 'box.local:8443');
        n.dispose();
      },
    );
  });

  group('SerialQueue', () {
    test('runs jobs in order even when one fails', () async {
      final q = SerialQueue();
      final order = <int>[];
      final a = q.run(() async {
        await Future<void>.delayed(const Duration(milliseconds: 20));
        order.add(1);
      });
      final b = q.run<void>(() async {
        order.add(2);
        throw StateError('boom');
      });
      final c = q.run(() async => order.add(3));
      await a;
      await expectLater(b, throwsStateError);
      await c;
      expect(order, [1, 2, 3]);
    });

    test('a stop is queued behind the pending start', () async {
      final core = FakeCoreApi();
      final q1 = serialQueueFor(core, 'advertise');
      expect(identical(q1, serialQueueFor(core, 'advertise')), isTrue);
      expect(identical(q1, serialQueueFor(core, 'watch')), isFalse);
    });
  });
}
