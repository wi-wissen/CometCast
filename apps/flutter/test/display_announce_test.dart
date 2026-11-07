// SPDX-License-Identifier: GPL-3.0-or-later
// When the display tells the core that it may announce itself in the local network (spec 14.8,
// points 1 and 5): only while it shows Idle or Presenting with a code, not in setup, not offline
// and not in the background. What is announced (the code, the mode) comes from the running room
// in the core and is tested in Rust.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/src/display/display_logic.dart';

import 'display_support.dart';

Future<void> settle(WidgetTester t) async {
  for (var i = 0; i < 4; i++) {
    await t.pump(const Duration(milliseconds: 50));
  }
}

void main() {
  testWidgets(
    'announces while Idle with a code, with the stable id of the installation',
    (t) async {
      final h = await pumpDisplay(t);
      await settle(t);
      final calls = h.core.announceCalls;
      expect(calls, isNotEmpty);
      expect(calls.last.active, isTrue);
      expect(isInstanceId(calls.last.id), isTrue);
      expect(
        calls.last.id,
        await h.store.instanceId(),
        reason: 'the id is kept between runs',
      );
    },
  );

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

  testWidgets('the id is created once and new after a reset of the data', (
    t,
  ) async {
    final s = MemoryDisplayStore();
    final a = await s.instanceId();
    expect(a, await s.instanceId());
    await s.reset();
    expect(await s.instanceId(), isNot(a));
    expect(newInstanceId(), hasLength(16));
    expect(newInstanceId(), isNot(newInstanceId()));
    expect(isInstanceId('not an id'), isFalse);
    expect(isInstanceId('0123456789abcdef'), isTrue);
  });
}
