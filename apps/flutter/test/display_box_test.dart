// SPDX-License-Identifier: GPL-3.0-or-later
// The in-house box line of the idle screen (spec 4.2, decision N40): the app passes the IPv4
// address of the box announcement when the server came from that announcement and its address
// is a host name; the QR code then encodes that IP.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/settings.dart';

import 'display_support.dart';
import 'fake_core.dart';

Future<DisplayHarness> pumpBox(
  WidgetTester t, {
  required List<ServerCandidate> found,
  required String server,
}) async {
  final core = FakeCoreApi()
    ..discovery = ServerDiscovery(
      servers: found,
      locked: false,
      announcementsAllowed: true,
      notices: const [],
      warnings: const [],
    );
  final settings = AppSettings.fromDefaults(core.serverDefaults());
  await settings.discover(core);
  final h = await pumpDisplay(
    t,
    core: core,
    settingsOverride: settings,
    server: server,
  );
  await t.pump(const Duration(milliseconds: 100));
  return h;
}

/// The QR box that encodes [url] (its semantics label is the URL; the painter gets the same).
Finder _qr(String url) => find.byWidgetPredicate(
  (w) => w is Semantics && w.properties.label == 'QR: $url',
);

void main() {
  testWidgets(
    'a box announcement under a host name: "or {ip}" and a QR with the IP',
    (t) async {
      await pumpBox(
        t,
        found: const [
          ServerCandidate(
            address: 'wss://cometcast.box:443',
            source: 'display-announcement',
            ip: '192.168.1.20',
          ),
        ],
        server: 'wss://cometcast.box:443',
      );
      expect(find.text('cometcast.box'), findsOneWidget);
      expect(find.text('or 192.168.1.20'), findsOneWidget);
      // The QR code encodes https://{ip}/{CODE} (the box's own certificate or none: the code
      // is what decides).
      expect(_qr('https://192.168.1.20/K7QM2W'), findsOneWidget);
    },
  );

  testWidgets(
    'no second line for an IP address, another source or no address',
    (t) async {
      // The server already is an IP address.
      await pumpBox(
        t,
        found: const [
          ServerCandidate(
            address: 'wss://192.168.1.20:443',
            source: 'display-announcement',
            ip: '192.168.1.20',
          ),
        ],
        server: 'wss://192.168.1.20:443',
      );
      expect(find.textContaining('or 192'), findsNothing);
    },
  );

  testWidgets('a relay from DNS or the default list has no IP line', (t) async {
    await pumpBox(
      t,
      found: const [
        ServerCandidate(
          address: 'wss://relay.example:443',
          source: 'dns-srv',
          ip: '10.0.0.9',
        ),
      ],
      server: 'wss://relay.example:443',
    );
    expect(find.textContaining('or 10.0.0.9'), findsNothing);
    expect(_qr('https://relay.example/K7QM2W'), findsOneWidget);
  });

  testWidgets('an announced name without an address has no IP line', (t) async {
    await pumpBox(
      t,
      found: const [
        ServerCandidate(
          address: 'wss://cometcast.box:443',
          source: 'display-announcement',
        ),
      ],
      server: 'wss://cometcast.box:443',
    );
    expect(find.textContaining('or '), findsNothing);
  });
}
