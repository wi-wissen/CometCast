// SPDX-License-Identifier: GPL-3.0-or-later
// The screen's address in the presenter app (spec 15.7, fix list F5).

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show LogicalKeyboardKey;
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';

import 'fake_core.dart';
import 'nearby_support.dart' show settle;
import 'round5_support.dart';

class SlowProbeCore extends FakeCoreApi {
  final gate = Completer<ProbeResult>();

  @override
  Future<ProbeResult> probeServer(String server) {
    probed.add(server);
    return gate.future;
  }
}

Future<void> openField(WidgetTester t) async {
  await tapKey(t, 'address-change');
}

Future<void> typeAddress(WidgetTester t, String text) async {
  await t.enterText(find.byKey(const Key('address-field')), text);
  await t.pump();
}

void main() {
  testWidgets(
    'the line shows the join address of the built-in relay and Change',
    (t) async {
      final r = await pumpR5(t);
      expect(find.textContaining('via cometcast.de/app'), findsOneWidget);
      expect(find.byKey(const Key('address-change')), findsOneWidget);
      expect(find.text('Change'), findsOneWidget);
      await finish(t, r);
    },
  );

  testWidgets('a self-hosted server shows its own host', (t) async {
    final r = await pumpR5(t, server: 'cast.example.org:443');
    expect(find.textContaining('via cast.example.org'), findsOneWidget);
    await finish(t, r);
  });

  testWidgets('not with a code chip', (t) async {
    final r = await pumpR5(t, code: 'K7QM2W');
    expect(find.byKey(const Key('code-chip')), findsOneWidget);
    expect(find.byKey(const Key('address-line')), findsNothing);
    await finish(t, r);
  });

  testWidgets('policy: the text, no link', (t) async {
    final r = await pumpR5(t, locked: true);
    expect(find.byKey(const Key('address-policy')), findsOneWidget);
    expect(find.text('set by your IT'), findsOneWidget);
    expect(find.byKey(const Key('address-change')), findsNothing);
    await finish(t, r);
  });

  testWidgets(
    'Change opens the field with Use and Cancel; Cancel and Escape return',
    (t) async {
      final r = await pumpR5(t);
      await openField(t);
      expect(find.text('Address from the screen'), findsOneWidget);
      expect(find.byKey(const Key('address-use')), findsOneWidget);
      expect(find.byKey(const Key('address-cancel')), findsOneWidget);
      // The placeholder is the current {dom}.
      expect(find.text('cometcast.de/app'), findsOneWidget);
      await tapKey(t, 'address-cancel');
      expect(find.byKey(const Key('address-line')), findsOneWidget);
      await openField(t);
      await t.sendKeyEvent(LogicalKeyboardKey.escape);
      await settle(t);
      expect(find.byKey(const Key('address-line')), findsOneWidget);
      await finish(t, r);
    },
  );

  testWidgets('a good address is used for the next join and stored after it', (
    t,
  ) async {
    final r = await pumpR5(t);
    await openField(t);
    await typeAddress(t, 'https://CAST.Example.org/');
    await tapKey(t, 'address-use');
    expect(r.core.probed, ['cast.example.org:443']);
    // The line returns with the new {dom}; nothing is stored yet.
    expect(find.textContaining('via cast.example.org'), findsOneWidget);
    expect(r.store.server, isNull);
    await t.enterText(find.byKey(const Key('code-field')), 'K7QM2W');
    await tapText(t, 'Connect');
    expect(r.core.presenterServer, 'cast.example.org:443');
    r.core.presenterEvents.add(const PresenterUiEvent.joined(roomName: 'A'));
    await settle(t);
    expect(r.store.server, 'cast.example.org:443');
    await finish(t, r);
  });

  testWidgets('cometcast.de means the built-in relay', (t) async {
    final r = await pumpR5(t, server: 'cast.example.org:443');
    await openField(t);
    await typeAddress(t, 'cometcast.de');
    await tapKey(t, 'address-use');
    expect(r.settings.serverAddress, 'relay.cometcast.de:443');
    expect(find.textContaining('via cometcast.de/app'), findsOneWidget);
    await finish(t, r);
  });

  testWidgets(
    'not a Comet Cast server: the error stays under the field with the text',
    (t) async {
      final core = FakeCoreApi()..probeResult = ProbeResult.notCometCast;
      final r = await pumpR5(t, fake: core);
      await openField(t);
      await typeAddress(t, 'example.org');
      await tapKey(t, 'address-use');
      expect(
        find.text(
          'No Comet Cast answers at this address. Compare it with the screen.',
        ),
        findsOneWidget,
      );
      expect(
        (t.widget(
          find.byKey(const Key('address-field')),
        ) as EditableText).controller.text,
        'example.org',
      );
      expect(r.settings.serverAddress, 'relay.cometcast.de');
      await finish(t, r);
    },
  );

  testWidgets('an unusable input is the same error without a probe', (t) async {
    final r = await pumpR5(t);
    await openField(t);
    await typeAddress(t, 'a b');
    await tapKey(t, 'address-use');
    expect(r.core.probed, isEmpty);
    expect(find.textContaining('No Comet Cast answers'), findsOneWidget);
    await finish(t, r);
  });

  testWidgets('while checking the button reads Checking and is disabled', (
    t,
  ) async {
    final core = SlowProbeCore();
    final r = await pumpR5(t, fake: core);
    await openField(t);
    await typeAddress(t, 'cast.example.org');
    await t.tap(find.byKey(const Key('address-use')));
    await t.pump();
    expect(find.text('Checking …'), findsOneWidget);
    // A second tap does nothing.
    await t.tap(find.byKey(const Key('address-use')), warnIfMissed: false);
    await t.pump();
    expect(core.probed.length, 1);
    core.gate.complete(ProbeResult.ok);
    await settle(t);
    expect(find.byKey(const Key('address-line')), findsOneWidget);
    await finish(t, r);
  });
}
