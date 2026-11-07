// SPDX-License-Identifier: GPL-3.0-or-later
// iPadOS display role: the core's announcement events reach the platform channel (decision N50).

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/native_announce.dart';

import 'fake_core.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('publish and withdraw are handed to the platform', () async {
    final core = FakeCoreApi();
    const ch = MethodChannel('de.cometcast/announce');
    final calls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(ch, (c) async {
          calls.add(c);
          return null;
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(ch, null),
    );
    final a = NativeAnnouncer(core)..start();
    core.nativeAnnounce.single.add(
      const NativeAnnounceEvent.publish(
        instance: 'CometCast-01234567',
        txt: [('v', '1'), ('code', 'K7QM2W')],
      ),
    );
    core.nativeAnnounce.single.add(const NativeAnnounceEvent.withdraw());
    await Future<void>.delayed(Duration.zero);
    await Future<void>.delayed(Duration.zero);
    expect(calls.map((c) => c.method), ['publish', 'withdraw']);
    expect(calls.first.arguments, {
      'instance': 'CometCast-01234567',
      'txt': {'v': '1', 'code': 'K7QM2W'},
    });
    a.dispose();
    expect(core.nativeAnnounceStops, 1);
  });
}
