// SPDX-License-Identifier: GPL-3.0-or-later
// The notes of the nearby list (docs/design/signed-announcements.md): "two screens use this
// name", "new device with a known name", the tag only on rows with a note, and the remembered
// screens (trust on first use). The marks themselves are computed in Rust (cc-discovery) and
// tested there; here the list shows them and the app keeps the pairs.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/settings.dart';

import 'fake_core.dart';
import 'nearby_support.dart';

const _idA = 'a3f7aaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const _idB = '3f7a00000000000000000000000000bb';

void main() {
  testWidgets('rows without a note show no tag', (t) async {
    final n = await pumpNearby(t);
    await found(t, n, [screen(_idA, name: 'Elbe')]);
    expect(find.byKey(const Key('nearby-tag-$_idA')), findsNothing);
    expect(find.textContaining('ID '), findsNothing);
    expect(find.byKey(const Key('nearby-same-name-$_idA')), findsNothing);
    expect(find.byKey(const Key('nearby-new-device-$_idA')), findsNothing);
    await done(t, n);
  });

  testWidgets(
    'two screens with one name: both rows show the note and the tag',
    (t) async {
      final n = await pumpNearby(t);
      await found(t, n, [
        screen(_idA, name: 'Room A', sameName: true),
        screen(_idB, name: 'Room A', sameName: true, code: 'P4X9TR'),
      ]);
      expect(
        find.text('Two screens use this name. Compare the ID with the screen.'),
        findsNWidgets(2),
      );
      expect(find.text('ID A3F7'), findsOneWidget);
      expect(find.text('ID 3F7A'), findsOneWidget);
      // The tag is small text under the row, not part of the room name.
      expect(find.text('“Room A”'), findsNWidgets(2));
      await done(t, n);
    },
  );

  testWidgets('the mark goes away when the core reports the row again', (
    t,
  ) async {
    final n = await pumpNearby(t);
    await found(t, n, [screen(_idA, name: 'Room A', sameName: true)]);
    expect(find.byKey(const Key('nearby-tag-$_idA')), findsOneWidget);
    n.core.search.add(
      NearbyUiEvent.updated(screen: screen(_idA, name: 'Room A')),
    );
    await settle(t);
    expect(find.byKey(const Key('nearby-tag-$_idA')), findsNothing);
    await done(t, n);
  });

  testWidgets('new device with a known name: note and tag, still tappable', (
    t,
  ) async {
    final n = await pumpNearby(t);
    await found(t, n, [screen(_idB, name: 'Elbe', newDevice: true)]);
    expect(
      find.text(
        'New device with a known name. Compare the ID with the screen.',
      ),
      findsOneWidget,
    );
    expect(find.text('ID 3F7A'), findsOneWidget);
    await t.tap(find.byKey(const Key('nearby-row-$_idB')));
    await settle(t);
    expect(n.core.startPresenterCalls, 1, reason: 'the row is a normal row');
    await done(t, n);
  });

  testWidgets('German: du texts', (t) async {
    final n = await pumpNearby(t, locale: const Locale('de'));
    await found(t, n, [
      screen(_idA, name: 'Raum', sameName: true, newDevice: true),
    ]);
    expect(
      find.text(
        'Zwei Bildschirme haben diesen Namen. Vergleiche die ID mit dem Bildschirm.',
      ),
      findsOneWidget,
    );
    expect(
      find.text(
        'Neues Gerät mit bekanntem Namen. Vergleiche die ID mit dem Bildschirm.',
      ),
      findsOneWidget,
    );
    expect(find.text('ID A3F7'), findsOneWidget);
    await done(t, n);
  });

  testWidgets('a joined screen is remembered and passed to the core', (
    t,
  ) async {
    final n = await pumpNearby(t);
    await found(t, n, [screen(_idA, name: 'Elbe')]);
    expect(n.core.remembered, isEmpty);
    await t.tap(find.byKey(const Key('nearby-row-$_idA')));
    await settle(t);
    // Not yet: the join did not succeed.
    expect(n.settings.rememberedScreens, isEmpty);
    n.core.presenterEvents.add(const PresenterUiEvent.joined(roomName: 'Elbe'));
    await settle(t);
    expect(n.settings.rememberedScreens.single.name, 'Elbe');
    expect(n.settings.rememberedScreens.single.id, _idA);
    expect(n.store.rememberedScreens.single.id, _idA, reason: 'saved');
    expect(n.core.remembered.single.id, _idA, reason: 'passed to the core');
    await done(t, n);
  });

  test(
    'remembered screens: at most 50, oldest dropped, no name no entry',
    () async {
      final store = MemorySettingsStore();
      final s = AppSettings.fromDefaults(
        FakeCoreApi().defaults,
        serverAddress: 'x',
        store: store,
      );
      await s.rememberScreen('', 'a' * 32);
      expect(s.rememberedScreens, isEmpty);
      for (var i = 0; i < 60; i++) {
        await s.rememberScreen('Room $i', i.toRadixString(16).padLeft(32, '0'));
      }
      expect(s.rememberedScreens, hasLength(AppSettings.maxRemembered));
      expect(s.rememberedScreens.first.name, 'Room 10');
      expect(s.rememberedScreens.last.name, 'Room 59');
      // The same pair again moves to the end instead of doubling.
      await s.rememberScreen(
        'Room 10',
        (10).toRadixString(16).padLeft(32, '0'),
      );
      expect(s.rememberedScreens, hasLength(AppSettings.maxRemembered));
      expect(s.rememberedScreens.last.name, 'Room 10');
      expect(store.rememberedScreens, hasLength(AppSettings.maxRemembered));
      // A new start reads them back.
      final again = AppSettings.fromDefaults(
        FakeCoreApi().defaults,
        serverAddress: 'x',
        store: store,
      );
      await again.loadRememberedScreens();
      expect(again.rememberedScreens.last.name, 'Room 10');
    },
  );
}
