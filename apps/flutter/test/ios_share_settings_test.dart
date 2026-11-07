// SPDX-License-Identifier: GPL-3.0-or-later
// The share extension's settings through the platform channel (native-apps.md 21.3).

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/ios_share_settings.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('de.cometcast/ios_broadcast');

  void answer(Future<Object?> Function(MethodCall) f) =>
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, f);

  tearDown(
    () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null),
  );

  test('the app writes name, confirmed servers and the last server', () async {
    Object? sent;
    answer((c) async {
      if (c.method == 'writeShareSettings') sent = c.arguments;
      return true;
    });
    const data = ShareSettingsData(
      name: 'Anna',
      confirmedServers: ['cast.example.org'],
      lastServer: 'cast.example.org',
    );
    expect(await ChannelIosShareSettings().write(data), isTrue);
    expect(sent, {
      'name': 'Anna',
      'confirmedServers': ['cast.example.org'],
      'lastServer': 'cast.example.org',
    });
  });

  test(
    'reading gives what the extension left, rubbish gives defaults',
    () async {
      answer(
        (c) async => {
          'name': 'Ben',
          'confirmedServers': ['a.example', 7, 'b.example'],
          'lastServer': 3,
        },
      );
      final d = await ChannelIosShareSettings().read();
      expect(
        d,
        const ShareSettingsData(
          name: 'Ben',
          confirmedServers: ['a.example', 'b.example'],
        ),
      );
    },
  );

  test('without an App Group or the plugin there is nothing', () async {
    answer((c) async => null);
    expect(await ChannelIosShareSettings().read(), isNull);
    expect(
      await ChannelIosShareSettings().write(const ShareSettingsData()),
      isFalse,
    );
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
    expect(await ChannelIosShareSettings().read(), isNull);
  });
}
