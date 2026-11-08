// SPDX-License-Identifier: GPL-3.0-or-later
// When the display tells the core that it may announce itself in the local network (spec 14.8,
// points 1 and 5): only while it shows Idle or Presenting with a code, not in setup, not offline
// and not in the background. What is announced (the code, the mode) comes from the running room
// in the core and is tested in Rust.

import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/display/display_logic.dart';

import 'display_support.dart';
import 'fake_core.dart';

Future<void> settle(WidgetTester t) async {
  for (var i = 0; i < 4; i++) {
    await t.pump(const Duration(milliseconds: 50));
  }
}

void main() {
  testWidgets(
    'announces while Idle with a code, with the key of the installation',
    (t) async {
      final h = await pumpDisplay(t);
      await settle(t);
      final calls = h.core.announceCalls;
      expect(calls, isNotEmpty);
      expect(calls.last.active, isTrue);
      // The core gets the 32 secret bytes of the stored key; the id derives from them there.
      final stored = await h.store.key(h.core.newDisplaySecret);
      expect(calls.last.key, stored.secret);
      expect(calls.last.key, hasLength(32));
      expect(
        calls.last.id,
        h.core.displayIdentity(stored.secret)!.id,
        reason: 'the key (and so the id) is kept between runs',
      );
    },
  );

  testWidgets('the idle screen shows the tag small in the footer', (t) async {
    final h = await pumpDisplay(t);
    await settle(t);
    final stored = await h.store.key(h.core.newDisplaySecret);
    final tag = h.core.displayIdentity(stored.secret)!.tag;
    expect(find.byKey(const Key('idle-tag')), findsOneWidget);
    expect(find.text('ID $tag'), findsOneWidget);
  });

  testWidgets('the persisted seq follows the core and is passed back', (
    t,
  ) async {
    final core = FakeCoreApi()..seqToPersist = 5000;
    final h = await pumpDisplay(t, core: core);
    await settle(t);
    // Written to the store once the announcement was set up.
    expect((await h.store.key(core.newDisplaySecret)).seqFloor, 5000);
    // A later start with the same store passes it on as the floor.
    core.seqToPersist = 9000;
    await t.pump(const Duration(seconds: 61));
    expect((await h.store.key(core.newDisplaySecret)).seqFloor, 9000);
  });

  testWidgets('not in setup', (t) async {
    final h = await pumpDisplay(t, setupDone: false);
    await settle(t);
    expect(h.core.announceCalls.where((c) => c.active), isEmpty);
  });

  testWidgets('not while there is no code (connecting, offline)', (t) async {
    final h = await pumpDisplay(t, start: false);
    await settle(t);
    expect(h.core.announceCalls.where((c) => c.active), isEmpty);
    // The code arrives: announcing starts. The connection fails: it stops again.
    h.core.receiverEvents.add(
      const ReceiverUiEvent.roomCode(code: 'K7QM2W', joinMode: UiJoinMode.open),
    );
    await settle(t);
    expect(h.core.announceCalls.last.active, isTrue);
    h.core.receiverEvents.add(
      const ReceiverUiEvent.error(code: 'connection_lost', message: 'x'),
    );
    await settle(t);
    expect(h.core.announceCalls.last.active, isFalse);
  });

  testWidgets('goodbye when the app goes to the background', (t) async {
    final h = await pumpDisplay(t);
    await settle(t);
    expect(h.core.announceCalls.last.active, isTrue);
    t.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    t.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    t.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    await settle(t);
    expect(h.core.announceCalls.last.active, isFalse);
  });

  testWidgets('the key is created once and new after a reset of the data', (
    t,
  ) async {
    final s = MemoryDisplayStore();
    var n = 0;
    Uint8List make() => Uint8List.fromList(List.filled(32, ++n));
    final a = await s.key(make);
    expect((await s.key(make)).secret, a.secret);
    expect(n, 1, reason: 'made once');
    await s.saveSeq(77);
    expect((await s.key(make)).seqFloor, 77);
    await s.reset();
    final b = await s.key(make);
    expect(b.secret, isNot(a.secret));
    expect(b.seqFloor, 0, reason: 'a new key starts again');
  });
}
