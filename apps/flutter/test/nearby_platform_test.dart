// SPDX-License-Identifier: GPL-3.0-or-later
// The Dart end of the platform bridge for the list of screens (spec 14, native-apps.md 4.1): what
// the Kotlin and Swift code sends over the channel reaches the core unchanged (the core checks
// it), and the calls to the platform.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/core/nearby_platform.dart';

import 'fake_core.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('test/nearby');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  Future<void> fromPlatform(String method, Object? args) async {
    await messenger.handlePlatformMessage(
      channel.name,
      channel.codec.encodeMethodCall(MethodCall(method, args)),
      (_) {},
    );
  }

  test('records, goodbyes, resets and permission go to the core', () async {
    final core = FakeCoreApi();
    final p = ChannelNearbyPlatform(core, channel: channel);
    await fromPlatform('seen', {
      'instance': 'CometCast-01234567',
      'addresses': ['10.0.0.5', 7],
      'txt': {'v': '1', 'id': '0123456789abcdef', 'bad': 3},
    });
    expect(core.nativeSeen.single.instance, 'CometCast-01234567');
    expect(core.nativeSeen.single.addresses, [
      '10.0.0.5',
    ], reason: 'only strings');
    await fromPlatform('gone', 'CometCast-01234567');
    expect(core.nativeGone, ['CometCast-01234567']);
    await fromPlatform('reset', null);
    expect(core.nativeResets, 1);
    await fromPlatform('permission', 'denied');
    await fromPlatform('permission', 'notAsked');
    await fromPlatform('permission', 'granted');
    expect(core.nativePermissions, [
      NearbyPermission.denied,
      NearbyPermission.notAsked,
      NearbyPermission.granted,
    ]);
    // Rubbish is ignored, not thrown.
    await fromPlatform('seen', 'not a map');
    await fromPlatform('seen', {'instance': 5});
    await fromPlatform('gone', 3);
    expect(core.nativeSeen.length, 1);
    final changes = <bool>[];
    final sub = p.networkChanges.listen(changes.add);
    await fromPlatform('network', false);
    await fromPlatform('network', true);
    await Future<void>.delayed(Duration.zero);
    expect(changes, [false, true]);
    await sub.cancel();
  });

  test('calls to the platform, and what happens when it does not answer', () async {
    final core = FakeCoreApi();
    final p = ChannelNearbyPlatform(core, channel: channel);
    final calls = <String>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      calls.add(call.method);
      return switch (call.method) {
        'permission' => 'notAsked',
        'hasNetwork' => false,
        'openSettings' => true,
        _ => null,
      };
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    expect(await p.permission(), NearbyPermission.notAsked);
    expect(await p.hasLocalNetwork(), isFalse);
    expect(await p.openSettings(), isTrue);
    await p.startBrowse();
    await p.stopBrowse();
    expect(calls, [
      'permission',
      'hasNetwork',
      'openSettings',
      'start',
      'stop',
    ]);
    // No handler (a platform without the bridge): safe defaults, no exception.
    messenger.setMockMethodCallHandler(channel, null);
    expect(await p.permission(), NearbyPermission.granted);
    expect(await p.hasLocalNetwork(), isTrue);
    expect(await p.openSettings(), isFalse);
    await p.startBrowse();
    expect(p.nativeBrowse, isTrue);
    expect(p.limitedTime, isTrue);
  });

  test('computers browse in the core, search without a limit and ask the core for the network', () async {
    final core = FakeCoreApi();
    final p = DesktopNearbyPlatform(
      core,
      poll: const Duration(milliseconds: 10),
    );
    expect(p.nativeBrowse, isFalse);
    expect(p.limitedTime, isFalse);
    expect(await p.permission(), NearbyPermission.granted);
    expect(await p.hasLocalNetwork(), isTrue);
    final changes = <bool>[];
    final sub = p.networkChanges.listen(changes.add);
    core.hasLocalNetwork = false;
    await Future<void>.delayed(const Duration(milliseconds: 60));
    core.hasLocalNetwork = true;
    await Future<void>.delayed(const Duration(milliseconds: 60));
    await sub.cancel();
    expect(changes, [false, true]);
  });
}
