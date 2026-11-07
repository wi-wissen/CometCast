// SPDX-License-Identifier: GPL-3.0-or-later
// iOS: the app hands name, confirmed servers and last server to the share extension and takes
// back the name the extension changed (spec 15.3, native-apps.md 21.3).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/ios_share_settings.dart';

import 'nearby_support.dart' show settle;
import 'round5_support.dart';

void main() {
  testWidgets(
    'the name the extension changed is taken at the start, then written',
    (t) async {
      final group = FakeIosShareSettings()
        ..stored = const ShareSettingsData(name: 'Anna (share)');
      final r = await pumpR5(t, iosShare: group);
      await settle(t);
      expect(r.settings.displayName, 'Anna (share)');
      expect(group.writes.last.name, 'Anna (share)');
      await finish(t, r);
    },
  );

  testWidgets(
    'Connect writes the name; a successful join writes the last server',
    (t) async {
      final group = FakeIosShareSettings();
      final r = await pumpR5(
        t,
        iosShare: group,
        server: 'cast.example.org:443',
      );
      await settle(t);
      await t.enterText(find.byKey(const Key('name-field')), 'Lena');
      await t.enterText(find.byKey(const Key('code-field')), 'K7QM2W');
      await tapText(t, 'Connect');
      expect(group.writes.last.name, 'Lena');
      expect(group.writes.last.lastServer, isNull);
      r.core.presenterEvents.add(const PresenterUiEvent.joined(roomName: 'A'));
      await settle(t);
      await t.pump(const Duration(milliseconds: 100));
      expect(group.writes.last.lastServer, 'cast.example.org:443');
      await finish(t, r);
    },
  );

  testWidgets('no App Group (null): nothing happens', (t) async {
    final r = await pumpR5(t);
    await settle(t);
    expect(find.text('Welcome to Comet Cast'), findsOneWidget);
    await finish(t, r);
  });
}
