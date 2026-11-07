// SPDX-License-Identifier: GPL-3.0-or-later
// The controller of the section "screens on this network" (spec 14.2, 14.3, 14.4, 14.5): the
// states, the timers, the order and the tap guard, with a fake core and a fake platform.

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/nearby_controller.dart';

import 'fake_core.dart';
import 'nearby_support.dart';

const _s = Duration(seconds: 1);

final _made = <NearbyController>[];

NearbyController make(
  WidgetTester t,
  FakeCoreApi core,
  FakeNearbyPlatform platform,
) {
  final c = NearbyController(
    core: core,
    platform: platform,
    now: () => t.binding.clock.now(),
  );
  _made.add(c);
  return c;
}

/// A test whose controllers are disposed inside the test body (the binding checks for pending
/// timers before tear-down functions run).
void nearbyTest(String name, Future<void> Function(WidgetTester t) body) {
  testWidgets(name, (t) async {
    try {
      await body(t);
    } finally {
      for (final c in _made) {
        c.dispose();
      }
      _made.clear();
    }
  });
}

Future<void> started(WidgetTester t, NearbyController c) async {
  c.setEnabled(true);
  await c.start();
  await t.pump();
}

void main() {
  nearbyTest('hidden until enabled, then searching with a placeholder', (
    t,
  ) async {
    final core = FakeCoreApi();
    final p = FakeNearbyPlatform();
    final c = make(t, core, p);
    expect(c.status, NearbyStatus.hidden);
    await started(t, c);
    expect(c.status, NearbyStatus.searching);
    expect(core.nearbyNativeFlags, [false]);
    // The platform browse starts when the core says the search runs.
    expect(p.startCalls, 0);
    core.search.add(const NearbyUiEvent.searchStarted());
    await t.pump();
    expect(p.startCalls, 1);
  });

  nearbyTest('rows keep the order of discovery and update in place', (t) async {
    final core = FakeCoreApi();
    final c = make(t, core, FakeNearbyPlatform());
    await started(t, c);
    core.search
      ..add(NearbyUiEvent.added(screen: screen('a', name: 'A')))
      ..add(NearbyUiEvent.added(screen: screen('b', name: 'B')))
      ..add(NearbyUiEvent.added(screen: screen('c', name: 'C')));
    await t.pump();
    expect(c.status, NearbyStatus.found);
    expect(c.rows.map((r) => r.id), ['a', 'b', 'c']);
    // A new code of the first row does not move it.
    core.search.add(
      NearbyUiEvent.updated(
        screen: screen('a', name: 'A', code: 'AAAAAA'),
      ),
    );
    await t.pump();
    expect(c.rows.map((r) => r.id), ['a', 'b', 'c']);
    expect(c.rows.first.screen.code, 'AAAAAA');
    // A row leaves at once.
    core.search.add(const NearbyUiEvent.removed(id: 'b'));
    await t.pump();
    expect(c.rows.map((r) => r.id), ['a', 'c']);
    // Two screens with the same name are two rows.
    core.search.add(NearbyUiEvent.added(screen: screen('d', name: 'A')));
    await t.pump();
    expect(c.rows.length, 3);
  });

  nearbyTest('nothing found after 8 s, a later screen replaces the text', (
    t,
  ) async {
    final core = FakeCoreApi();
    final c = make(t, core, FakeNearbyPlatform());
    await started(t, c);
    await t.pump(const Duration(seconds: 7));
    expect(c.status, NearbyStatus.searching);
    await t.pump(_s);
    expect(c.status, NearbyStatus.none);
    expect(c.searching, isTrue, reason: 'the search goes on');
    core.search.add(NearbyUiEvent.added(screen: screen('a')));
    await t.pump();
    expect(c.status, NearbyStatus.found);
    core.search.add(const NearbyUiEvent.removed(id: 'a'));
    await t.pump();
    expect(c.status, NearbyStatus.none);
  });

  nearbyTest(
    'phones and tablets stop after 60 s, the list is cleared, a button starts again',
    (t) async {
      final core = FakeCoreApi();
      final c = make(t, core, FakeNearbyPlatform(limited: true));
      await started(t, c);
      core.search.add(NearbyUiEvent.added(screen: screen('a')));
      await t.pump(const Duration(seconds: 59));
      expect(c.status, NearbyStatus.found);
      await t.pump(_s);
      expect(c.status, NearbyStatus.paused);
      expect(c.rows, isEmpty);
      expect(core.nearbyStopCalls, 1);
      c.searchNow();
      await t.pump();
      expect(core.nearbySearches.length, 2);
      expect(c.status, NearbyStatus.searching);
      // Another 60 s.
      await t.pump(const Duration(seconds: 60));
      expect(c.status, NearbyStatus.paused);
    },
  );

  nearbyTest('computers search as long as the page is visible', (t) async {
    final core = FakeCoreApi();
    final c = make(t, core, FakeNearbyPlatform(limited: false));
    await started(t, c);
    await t.pump(const Duration(minutes: 10));
    expect(c.status, NearbyStatus.none);
    expect(c.searching, isTrue);
  });

  nearbyTest(
    'not asked yet: no search until the button, then the platform asks',
    (t) async {
      final core = FakeCoreApi();
      final p = FakeNearbyPlatform(
        native: true,
        perm: NearbyPermission.notAsked,
      );
      final c = make(t, core, p);
      await started(t, c);
      expect(c.status, NearbyStatus.notAsked);
      expect(core.nearbySearches, isEmpty);
      expect(p.startCalls, 0);
      c.searchNow();
      await t.pump();
      expect(core.nearbyNativeFlags, [true]);
      expect(c.status, NearbyStatus.searching);
      core.search.add(const NearbyUiEvent.searchStarted());
      await t.pump();
      expect(p.startCalls, 1, reason: 'the system asks as a result of the tap');
    },
  );

  nearbyTest(
    'a refused permission is the state not allowed and clears the list',
    (t) async {
      final core = FakeCoreApi();
      final c = make(t, core, FakeNearbyPlatform(native: true));
      await started(t, c);
      core.search.add(NearbyUiEvent.added(screen: screen('a')));
      await t.pump();
      core.search.add(
        const NearbyUiEvent.permission(state: NearbyPermission.denied),
      );
      await t.pump();
      expect(c.status, NearbyStatus.denied);
      expect(c.rows, isEmpty);
      expect(await c.openSettings(), isTrue);
    },
  );

  nearbyTest(
    'no local network: no search; it starts by itself when Wi-Fi comes, stops when it goes',
    (t) async {
      final core = FakeCoreApi();
      final p = FakeNearbyPlatform(network: false);
      final c = make(t, core, p);
      await started(t, c);
      expect(c.status, NearbyStatus.noNetwork);
      expect(core.nearbySearches, isEmpty);
      p.networkChanged(true);
      await t.pump();
      await t.pump();
      expect(core.nearbySearches.length, 1);
      expect(c.status, NearbyStatus.searching);
      core.search.add(NearbyUiEvent.added(screen: screen('a')));
      await t.pump();
      p.networkChanged(false);
      await t.pump();
      expect(c.status, NearbyStatus.noNetwork);
      expect(c.rows, isEmpty);
    },
  );

  nearbyTest('a policy that forbids it hides the section', (t) async {
    final core = FakeCoreApi();
    final c = make(t, core, FakeNearbyPlatform());
    await started(t, c);
    core.search.add(const NearbyUiEvent.forbidden());
    await t.pump();
    expect(c.status, NearbyStatus.hidden);
  });

  nearbyTest(
    'tap guard: 500 ms after a row appeared, moved or changed; never locked',
    (t) async {
      final core = FakeCoreApi();
      final c = make(t, core, FakeNearbyPlatform());
      await started(t, c);
      core.search
        ..add(NearbyUiEvent.added(screen: screen('a')))
        ..add(NearbyUiEvent.added(screen: screen('b')))
        ..add(
          NearbyUiEvent.added(screen: screen('l', mode: NearbyMode.locked)),
        );
      await t.pump();
      final a = c.rows[0];
      expect(c.canTap(a), isFalse);
      await t.pump(const Duration(milliseconds: 499));
      expect(c.canTap(a), isFalse);
      await t.pump(const Duration(milliseconds: 1));
      expect(c.canTap(a), isTrue);
      expect(
        c.canTap(c.rows[2]),
        isFalse,
        reason: 'locked rows are never tappable',
      );
      // The row above leaves: the row below moved and ignores a tap for a moment.
      core.search.add(const NearbyUiEvent.removed(id: 'a'));
      await t.pump();
      expect(c.canTap(c.rows[0]), isFalse);
      await t.pump(const Duration(milliseconds: 500));
      expect(c.canTap(c.rows[0]), isTrue);
      // A change of the code starts the guard again.
      core.search.add(
        NearbyUiEvent.updated(screen: screen('b', code: 'BBBBBB')),
      );
      await t.pump();
      expect(c.canTap(c.rows[0]), isFalse);
    },
  );

  nearbyTest(
    'a tap stops the search and keeps the rows; the others are disabled',
    (t) async {
      final core = FakeCoreApi();
      final p = FakeNearbyPlatform();
      final c = make(t, core, p);
      await started(t, c);
      core.search
        ..add(NearbyUiEvent.added(screen: screen('a')))
        ..add(NearbyUiEvent.added(screen: screen('b')));
      await t.pump(_s);
      c.beginConnect('a');
      await t.pump();
      expect(c.searching, isFalse);
      expect(core.nearbyStopCalls, 1);
      expect(p.stopCalls, 1);
      expect(c.rows.length, 2);
      expect(c.connectingId, 'a');
      expect(c.status, NearbyStatus.found);
      expect(c.canTap(c.rows[1]), isFalse);
    },
  );

  nearbyTest(
    'a code that is gone: row removed, a line, a new search that starts empty',
    (t) async {
      final core = FakeCoreApi();
      final c = make(t, core, FakeNearbyPlatform());
      await started(t, c);
      core.search
        ..add(NearbyUiEvent.added(screen: screen('a')))
        ..add(NearbyUiEvent.added(screen: screen('b')));
      await t.pump(_s);
      c.beginConnect('a');
      c.markGone('a');
      expect(c.goneLine, isTrue);
      expect(c.rows.map((r) => r.id), ['b']);
      await c.restartKeepingLine();
      await t.pump();
      expect(core.nearbySearches.length, 2, reason: 'the search starts again');
      expect(
        c.rows,
        isEmpty,
        reason: 'rows of an earlier search are never kept',
      );
      expect(c.goneLine, isTrue);
      // The line goes away when the list changes.
      core.search.add(NearbyUiEvent.added(screen: screen('b')));
      await t.pump();
      expect(c.goneLine, isFalse);
    },
  );

  nearbyTest('stop clears everything and tells the core and the platform', (
    t,
  ) async {
    final core = FakeCoreApi();
    final p = FakeNearbyPlatform();
    final c = make(t, core, p);
    await started(t, c);
    core.search.add(NearbyUiEvent.added(screen: screen('a')));
    await t.pump();
    c.stop();
    await t.pump();
    expect(c.rows, isEmpty);
    expect(core.nearbyStopCalls, 1);
    expect(p.stopCalls, 1);
    // Back on the page: a fresh search.
    await c.start();
    await t.pump();
    expect(core.nearbySearches.length, 2);
  });

  nearbyTest('at most 32 rows', (t) async {
    final core = FakeCoreApi();
    final c = make(t, core, FakeNearbyPlatform());
    await started(t, c);
    for (var i = 0; i < 40; i++) {
      core.search.add(NearbyUiEvent.added(screen: screen('id$i')));
    }
    await t.pump();
    expect(c.rows.length, NearbyController.maxRows);
  });

  nearbyTest('unavailable (no multicast) reads as nothing found', (t) async {
    final core = FakeCoreApi();
    final c = make(t, core, FakeNearbyPlatform());
    await started(t, c);
    core.search.add(const NearbyUiEvent.unavailable());
    await t.pump();
    expect(c.status, NearbyStatus.none);
  });
}
