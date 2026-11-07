// SPDX-License-Identifier: GPL-3.0-or-later
// The section "screens on this network" on the app start page and what a tap does (spec 14.1 to
// 14.5, native-apps.md 4.1), with a fake core and a fake platform.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/presenter/pages.dart' show PresenterDevice;

import 'nearby_support.dart';

void nearbyPageTest(String name, Future<void> Function(WidgetTester t) body) {
  testWidgets(name, body);
}

void main() {
  nearbyPageTest(
    'phone: the section sits between the name field and the code',
    (t) async {
      final n = await pumpNearby(t);
      final name = t.getTopLeft(find.byKey(const Key('name-field'))).dy;
      final section = t.getTopLeft(find.byKey(const Key('nearby-section'))).dy;
      final or = t.getTopLeft(find.byKey(const Key('nearby-or'))).dy;
      final scan = t.getTopLeft(find.byKey(const Key('scan-button'))).dy;
      final code = t.getTopLeft(find.byKey(const Key('code-field'))).dy;
      expect(
        name < section && section < or && or < scan && scan < code,
        isTrue,
      );
      expect(find.text('Screens on this network'), findsOneWidget);
      expect(find.text('or'), findsOneWidget);
      expect(find.text('Searching …'), findsOneWidget);
      // The page without the section keeps its other texts; `app.scanAgainBody` is not shown with it.
      expect(find.byKey(const Key('scan-again-body')), findsNothing);
      await done(t, n);
    },
  );

  nearbyPageTest(
    'laptop: the section is a card on the right of the left column',
    (t) async {
      final n = await pumpNearby(
        t,
        size: const Size(1100, 800),
        desktop: true,
        device: PresenterDevice.computer,
        platform: FakeNearbyPlatform(limited: false),
      );
      final code = t.getTopLeft(find.byKey(const Key('code-field')));
      final card = t.getTopLeft(find.byKey(const Key('nearby-section')));
      expect(card.dx, greaterThan(code.dx + 250), reason: 'right column');
      expect(
        find.byKey(const Key('nearby-or')),
        findsNothing,
        reason: 'no divider on the laptop',
      );
      // The left column holds Connect.
      expect(t.getTopLeft(find.text('Connect')).dx, lessThan(card.dx));
      await done(t, n);
    },
  );

  nearbyPageTest(
    'not shown with a code chip, under a policy, without a platform',
    (t) async {
      var n = await pumpNearby(t, code: 'K7QM2W');
      expect(find.byKey(const Key('nearby-section')), findsNothing);
      expect(n.core.nearbySearches, isEmpty, reason: 'no search runs either');
      await done(t, n);

      n = await pumpNearby(t, announcementsAllowed: false);
      expect(find.byKey(const Key('nearby-section')), findsNothing);
      expect(n.core.nearbySearches, isEmpty);
      await done(t, n);

      n = await pumpNearby(t, locked: true);
      expect(find.byKey(const Key('nearby-section')), findsNothing);
      expect(n.core.nearbySearches, isEmpty);
      await done(t, n);

      n = await pumpNearby(t, withPlatform: false);
      expect(find.byKey(const Key('nearby-section')), findsNothing);
      await t.pumpWidget(const SizedBox());
    },
  );

  nearbyPageTest(
    'rows: quoted name, mode, via for another relay, code as XXX XXX',
    (t) async {
      final n = await pumpNearby(t);
      await found(t, n, [
        screen('a', name: 'Meeting room Elbe', code: 'K7QM2W'),
        screen(
          'b',
          name: 'Room 204',
          mode: NearbyMode.approval,
          code: 'P4X9TR',
          server: 'wss://cast.example.org:443',
        ),
        screen('c', name: '', code: 'R8M3VA', server: 'ws://10.0.0.5:8443'),
      ]);
      expect(find.text('“Meeting room Elbe”'), findsOneWidget);
      expect(find.text('Connects at once'), findsOneWidget);
      expect(find.text('K7Q M2W'), findsOneWidget);
      expect(find.text('“Room 204”'), findsOneWidget);
      expect(find.text('Approval on · via cast.example.org'), findsOneWidget);
      expect(find.text('P4X 9TR'), findsOneWidget);
      // No name: the "no name" text without quotes; a port other than 443 is shown.
      expect(find.text('No name'), findsOneWidget);
      expect(find.text('Connects at once · via 10.0.0.5:8443'), findsOneWidget);
      // The list does not say "searching".
      expect(find.text('Searching …'), findsNothing);
      // Order of discovery.
      final y = [
        for (final id in ['a', 'b', 'c'])
          t.getTopLeft(find.byKey(Key('nearby-row-$id'))).dy,
      ];
      expect(y[0] < y[1] && y[1] < y[2], isTrue);
      await done(t, n);
    },
  );

  nearbyPageTest(
    'the accessible name of a row has the name, the code and the mode',
    (t) async {
      final handle = t.ensureSemantics();
      final n = await pumpNearby(t);
      await found(t, n, [
        screen(
          'a',
          name: 'Room 204',
          mode: NearbyMode.approval,
          code: 'P4X9TR',
        ),
      ]);
      expect(
        find.bySemanticsLabel('“Room 204”, code P4X 9TR, Approval on'),
        findsOneWidget,
      );
      await done(t, n);
      handle.dispose();
    },
  );

  nearbyPageTest('a locked row stays, without a code, and cannot be tapped', (
    t,
  ) async {
    final n = await pumpNearby(t);
    await found(t, n, [
      screen('l', name: 'Clubhouse', mode: NearbyMode.locked),
    ]);
    expect(find.text('“Clubhouse”'), findsOneWidget);
    expect(find.text('Locked right now'), findsOneWidget);
    expect(find.byKey(const Key('nearby-code-l')), findsNothing);
    await t.tap(find.byKey(const Key('nearby-row-l')));
    await settle(t);
    expect(n.core.startPresenterCalls, 0);
    await done(t, n);
  });

  nearbyPageTest('never picks a screen and never fills the code field', (
    t,
  ) async {
    final n = await pumpNearby(t);
    await found(t, n, [screen('a')]);
    await t.pump(const Duration(seconds: 30));
    expect(n.core.startPresenterCalls, 0);
    final field = t.widget<EditableText>(find.byKey(const Key('code-field')));
    expect(field.controller.text, isEmpty);
    await done(t, n);
  });

  nearbyPageTest(
    'tap on a known server: spinner, other rows dimmed, joins with the announced code and name',
    (t) async {
      final n = await pumpNearby(t);
      await t.enterText(find.byKey(const Key('name-field')), 'Lena');
      await found(t, n, [
        screen(
          'a',
          name: 'Elbe',
          code: 'K7QM2W',
          server: 'wss://relay.cometcast.de:443',
        ),
        screen(
          'b',
          name: 'Oder',
          code: 'P4X9TR',
          server: 'wss://relay.cometcast.de:443',
        ),
      ]);
      await t.tap(find.byKey(const Key('nearby-row-a')));
      await settle(t);
      expect(n.core.startPresenterCalls, 1);
      expect(n.core.presenterCode, 'K7QM2W');
      expect(n.core.presenterServer, 'wss://relay.cometcast.de:443');
      expect(n.core.presenterName, 'Lena');
      expect(find.text('Connecting …'), findsOneWidget);
      expect(
        find.text('K7Q M2W'),
        findsNothing,
        reason: 'the spinner replaces the code',
      );
      expect(find.text('P4X 9TR'), findsOneWidget);
      expect(
        t
            .widget<Opacity>(
              find
                  .descendant(
                    of: find.byKey(const Key('nearby-row-b')),
                    matching: find.byType(Opacity),
                  )
                  .first,
            )
            .opacity,
        .55,
      );
      // The search stopped, and the other row cannot be tapped now.
      expect(n.core.nearbyStopCalls, 1);
      await t.tap(find.byKey(const Key('nearby-row-b')));
      await settle(t);
      expect(n.core.startPresenterCalls, 1);
      // Open: What to show.
      n.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Elbe'),
      );
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
      // What is saved as "last used" is the server of the settings, never the announced one.
      expect(n.store.server, 'relay.cometcast.de');
      await done(t, n);
    },
  );

  nearbyPageTest('approval on: the waiting page', (t) async {
    final n = await pumpNearby(t);
    await found(t, n, [screen('a', mode: NearbyMode.approval)]);
    await t.tap(find.byKey(const Key('nearby-row-a')));
    await settle(t);
    n.core.presenterEvents.add(
      const PresenterUiEvent.waitingForApproval(
        timeoutSecs: 60,
        check: 42,
        roomName: 'Room 204',
      ),
    );
    await settle(t);
    expect(find.text('Waiting for approval'), findsOneWidget);
    await done(t, n);
  });

  nearbyPageTest('the screen went locked meanwhile: the locked error page', (
    t,
  ) async {
    final n = await pumpNearby(t);
    await found(t, n, [screen('a')]);
    await t.tap(find.byKey(const Key('nearby-row-a')));
    await settle(t);
    n.core.presenterEvents.add(
      const PresenterUiEvent.error(code: 'room_locked', message: 'x'),
    );
    await settle(t);
    expect(find.text('Locked right now'), findsWidgets);
    expect(find.byKey(const Key('nearby-section')), findsNothing);
    await done(t, n);
  });

  nearbyPageTest(
    'a stale code: no error page, the row is removed, a line, a new search',
    (t) async {
      final n = await pumpNearby(t);
      await found(t, n, [
        screen('a', name: 'Elbe'),
        screen('b', name: 'Oder', code: 'P4X9TR'),
      ]);
      await t.tap(find.byKey(const Key('nearby-row-a')));
      await settle(t);
      n.core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'room_not_found', message: 'x'),
      );
      await settle(t);
      // Back on the start page (no error page), the row is gone, the line shows.
      expect(find.text('This code does not exist right now'), findsNothing);
      expect(find.byKey(const Key('nearby-row-a')), findsNothing);
      expect(find.text('This screen is no longer available.'), findsOneWidget);
      expect(
        n.core.nearbySearches.length,
        2,
        reason: 'the search starts again',
      );
      // The line goes away when the list changes.
      n.core.search.add(
        NearbyUiEvent.added(
          screen: screen('b', name: 'Oder', code: 'P4X9TR'),
        ),
      );
      await settle(t);
      expect(find.text('This screen is no longer available.'), findsNothing);
      await done(t, n);
    },
  );

  nearbyPageTest(
    'announced-only server: the relay page first; Cancel sends nothing',
    (t) async {
      final n = await pumpNearby(t);
      await found(t, n, [
        screen('a', name: 'Room 204', server: 'wss://cast.musterfirma.de:443'),
      ]);
      await t.tap(find.byKey(const Key('nearby-row-a')));
      await settle(t);
      expect(find.text('Use this relay?'), findsOneWidget);
      expect(
        find.text(
          'The screen “Room 204” uses the relay cast.musterfirma.de. Only continue if you know and trust its operator.',
        ),
        findsOneWidget,
      );
      expect(
        find.text('Cancel'),
        findsOneWidget,
        reason: '`common.cancel`, not the relay variant',
      );
      expect(find.text('Cancel (use the default relay)'), findsNothing);
      expect(n.core.startPresenterCalls, 0);
      await t.tap(find.byKey(const Key('relay-cancel')));
      await settle(t);
      expect(n.core.startPresenterCalls, 0, reason: 'nothing was sent');
      expect(find.text('Screens on this network'), findsOneWidget);
      expect(n.settings.confirmedServers, isEmpty);
      // The search started again on the start page.
      expect(n.core.nearbySearches.length, 2);
      await done(t, n);
    },
  );

  nearbyPageTest(
    'announced-only server: Use this relay remembers the host and joins',
    (t) async {
      final n = await pumpNearby(t);
      await found(t, n, [
        screen('a', name: 'Room 204', server: 'wss://cast.musterfirma.de:443'),
      ]);
      await t.tap(find.byKey(const Key('nearby-row-a')));
      await settle(t);
      await t.tap(find.byKey(const Key('relay-use')));
      await settle(t);
      expect(n.settings.confirmedServers, {'cast.musterfirma.de'});
      expect(n.core.startPresenterCalls, 1);
      expect(n.core.presenterServer, 'wss://cast.musterfirma.de:443');
      expect(n.core.presenterCode, 'K7QM2W');
      expect(find.text('Connecting …'), findsOneWidget);
      n.core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Room 204'),
      );
      await settle(t);
      expect(find.text('What would you like to show?'), findsOneWidget);
      // Confirmed is not "last used": the saved server is still the one of the settings.
      expect(n.store.server, 'relay.cometcast.de');
      await done(t, n);
    },
  );

  nearbyPageTest('a server confirmed before joins at once', (t) async {
    final n = await pumpNearby(t);
    await n.settings.confirmServer('cast.musterfirma.de');
    await found(t, n, [screen('a', server: 'wss://cast.musterfirma.de:443')]);
    // The row still names the server (it is not the default relay).
    expect(find.textContaining('via cast.musterfirma.de'), findsOneWidget);
    await t.tap(find.byKey(const Key('nearby-row-a')));
    await settle(t);
    expect(find.text('Use this relay?'), findsNothing);
    expect(n.core.startPresenterCalls, 1);
    await done(t, n);
  });

  nearbyPageTest('a row that just appeared ignores a tap', (t) async {
    final n = await pumpNearby(t);
    n.core.search.add(NearbyUiEvent.added(screen: screen('a')));
    await settle(t, 1);
    await t.tap(find.byKey(const Key('nearby-row-a')));
    await t.pump(const Duration(milliseconds: 50));
    expect(n.core.startPresenterCalls, 0);
    await t.pump(const Duration(milliseconds: 600));
    await t.tap(find.byKey(const Key('nearby-row-a')));
    await settle(t);
    expect(n.core.startPresenterCalls, 1);
    await done(t, n);
  });

  nearbyPageTest('states: nothing found, paused, not asked, denied, no Wi-Fi', (
    t,
  ) async {
    // Nothing found after 8 s.
    var n = await pumpNearby(t);
    await t.pump(const Duration(seconds: 8));
    expect(find.text('No screen found here.'), findsOneWidget);
    expect(
      find.textContaining('Many Wi-Fi networks hide devices'),
      findsOneWidget,
    );
    // Paused after 60 s: the text and the button.
    await t.pump(const Duration(seconds: 52));
    expect(find.text('Search stopped to save battery.'), findsOneWidget);
    expect(find.text('Search this network'), findsOneWidget);
    await t.tap(find.byKey(const Key('nearby-search')));
    await settle(t);
    expect(n.core.nearbySearches.length, 2);
    expect(find.text('Searching …'), findsOneWidget);
    await done(t, n);

    // Not asked yet: only the button.
    n = await pumpNearby(
      t,
      platform: FakeNearbyPlatform(
        native: true,
        perm: NearbyPermission.notAsked,
      ),
    );
    expect(find.byKey(const Key('nearby-search')), findsOneWidget);
    expect(n.core.nearbySearches, isEmpty);
    await t.tap(find.byKey(const Key('nearby-search')));
    await settle(t);
    expect(n.core.nearbySearches.length, 1);
    // Refused: the warning and the link to the settings.
    n.core.search.add(
      const NearbyUiEvent.permission(state: NearbyPermission.denied),
    );
    await settle(t);
    expect(
      find.text('Comet Cast may not search this network.'),
      findsOneWidget,
    );
    await t.tap(find.byKey(const Key('nearby-allow')));
    await settle(t);
    expect(n.platform.settingsCalls, 1);
    await done(t, n);

    // No Wi-Fi.
    n = await pumpNearby(t, platform: FakeNearbyPlatform(network: false));
    expect(
      find.text('Join the room’s Wi-Fi to find screens here.'),
      findsOneWidget,
    );
    expect(n.core.nearbySearches, isEmpty);
    n.platform.networkChanged(true);
    await settle(t);
    expect(n.core.nearbySearches.length, 1);
    await done(t, n);
  });

  nearbyPageTest('German: the same section in du', (t) async {
    final n = await pumpNearby(t, locale: const Locale('de'));
    expect(find.text('Bildschirme in diesem Netz'), findsOneWidget);
    expect(find.text('oder'), findsOneWidget);
    await found(t, n, [
      screen(
        'a',
        name: 'Raum 204',
        mode: NearbyMode.approval,
        server: 'wss://cast.musterfirma.de:443',
      ),
    ]);
    expect(find.text('„Raum 204“'), findsOneWidget);
    expect(
      find.text('Mit Freigabe · über cast.musterfirma.de'),
      findsOneWidget,
    );
    await done(t, n);
  });

  nearbyPageTest(
    'leaving the page stops the search and clears the rows; coming back starts a new one',
    (t) async {
      final n = await pumpNearby(t);
      await found(t, n, [screen('a')]);
      // Connect with a typed code leaves the page.
      await t.enterText(find.byKey(const Key('code-field')), 'K7QM2W');
      await t.tap(find.text('Connect'));
      await settle(t);
      expect(n.core.nearbyStopCalls, greaterThanOrEqualTo(1));
      expect(n.controller.rows, isEmpty);
      await done(t, n);
    },
  );
}
