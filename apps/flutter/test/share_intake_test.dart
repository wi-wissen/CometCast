// SPDX-License-Identifier: GPL-3.0-or-later
// The Android share item from the platform channel (native-apps.md 21.2).

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/share_intake.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('cometcast/share');

  void answer(Future<Object?> Function(MethodCall) f) =>
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, f);

  tearDown(
    () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null),
  );

  test('a file, a link, no link and too large', () async {
    final answers = <Object?>[
      {
        'kind': 'file',
        'path': '/cache/share/ab/a.png',
        'name': 'a.png',
        'size': 12,
        'mime': 'image/png',
      },
      {'kind': 'link', 'url': 'https://example.org/x'},
      {'kind': 'noLink'},
      {'kind': 'tooLarge'},
    ];
    answer((c) async => answers.removeAt(0));
    final intake = ChannelShareIntake();
    final f = await intake.take();
    expect(f!.kind, SharedKind.file);
    expect(f.path, '/cache/share/ab/a.png');
    expect(f.size, 12);
    expect((await intake.take())!.url, 'https://example.org/x');
    expect((await intake.take())!.kind, SharedKind.noLink);
    expect((await intake.take())!.kind, SharedKind.tooLarge);
  });

  test('nothing, rubbish and a file without a path are no item', () async {
    final answers = <Object?>[
      null,
      'text',
      {'kind': 'zip'},
      {'kind': 'file'},
    ];
    answer((c) async => answers.removeAt(0));
    final intake = ChannelShareIntake();
    for (var i = 0; i < 4; i++) {
      expect(await intake.take(), isNull);
    }
  });

  test('without the plugin there is no item and nothing breaks', () async {
    final intake = ChannelShareIntake();
    expect(await intake.take(), isNull);
    await intake.discard('/x');
    expect(await const NoShareIntake().take(), isNull);
  });

  test(
    'a new item while the app runs is announced, discard reaches the platform',
    () async {
      String? discarded;
      answer((c) async {
        if (c.method == 'discardSharedItem') discarded = c.arguments as String;
        return null;
      });
      final intake = ChannelShareIntake();
      final got = intake.arrivals.first;
      await TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .handlePlatformMessage(
            'cometcast/share',
            const StandardMethodCodec().encodeMethodCall(
              const MethodCall('shared'),
            ),
            (_) {},
          );
      await got;
      await intake.discard('/cache/share/ab/a.png');
      expect(discarded, '/cache/share/ab/a.png');
    },
  );
}
