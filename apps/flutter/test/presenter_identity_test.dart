// SPDX-License-Identifier: GPL-3.0-or-later
// Presenter device identity on the Flutter display and presenter (docs/design/presenter-identity.md):
// one key per installation, the neutral note in the request dialog and the open-mode toast,
// the memory that the app stores, and the key handed to the iOS broadcast extension.

import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/sender_controller.dart';
import 'package:cometcast/src/display/display_logic.dart';

import 'display_support.dart';
import 'fake_core.dart';
import 'ios_broadcast_test.dart' show FakeIosBroadcast;

const noteEn = 'This name was used here before by a different device.';
const noteDe = 'Dieser Name wurde hier schon von einem anderen Gerät benutzt.';

Future<void> two(WidgetTester t) async {
  await t.pump();
  await t.pump(const Duration(milliseconds: 50));
}

void main() {
  group('one key per installation', () {
    testWidgets('the presenter key is the display key, made once', (t) async {
      final h = await pumpDisplay(t);
      expect(h.core.madeSecrets, hasLength(1));
      expect(h.core.presenterKeys, [h.core.madeSecrets.single]);
    });

    testWidgets('a stored key is reused, none is made', (t) async {
      final h = await pumpDisplay(t);
      final stored = h.core.presenterKeys.single;
      // Asking again never makes another key: the generator must not run.
      final again = await h.store.key(() => throw StateError('no new key'));
      expect(again.secret, stored);
      expect(h.core.madeSecrets, hasLength(1));
    });
  });

  group('the request dialog', () {
    testWidgets('shows the neutral note only for another device', (t) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(
          id: BigInt.one,
          name: 'Lena',
          check: 42,
          device: 'a' * 32,
          otherDevice: true,
        ),
      );
      await two(t);
      expect(find.byKey(const Key('request-device-note')), findsOneWidget);
      expect(find.text(noteEn), findsOneWidget);
      await t.pump(const Duration(seconds: 1));
      await t.tap(find.byKey(const Key('request-no')));
      await two(t);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(
          id: BigInt.two,
          name: 'Lena',
          check: 43,
          device: 'a' * 32,
        ),
      );
      await two(t);
      expect(find.byKey(const Key('request-device-note')), findsNothing);
      expect(find.text(noteEn), findsNothing);
    });

    testWidgets('the sentence in German is the same for du and Sie', (t) async {
      final h = await pumpDisplay(
        t,
        mode: DefaultMode.confirm,
        locale: const Locale('de'),
      );
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(
          id: BigInt.one,
          name: 'Lena',
          check: 42,
          otherDevice: true,
        ),
      );
      await two(t);
      expect(find.text(noteDe), findsOneWidget);
    });

    testWidgets('a request without a proof has no note and no device', (
      t,
    ) async {
      final h = await pumpDisplay(t, mode: DefaultMode.confirm);
      h.core.receiverEvents.add(
        ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 42),
      );
      await two(t);
      expect(find.byKey(const Key('request-device-note')), findsNothing);
    });
  });

  group('the open-mode toast', () {
    testWidgets('appends the sentence for another device only', (t) async {
      final h = await pumpDisplay(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(
          name: 'Lena',
          handover: false,
          device: 'a',
          otherDevice: true,
        ),
      );
      await two(t);
      expect(find.text('“Lena” connected $noteEn'), findsOneWidget);
      h.core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
      await two(t);
      h.core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(name: 'Lena', handover: false),
      );
      await two(t);
      expect(find.text('“Lena” connected'), findsOneWidget);
    });
  });

  group('the memory of names and devices', () {
    testWidgets('is given to the core, and every change is stored', (t) async {
      final h = await pumpDisplay(t);
      expect(h.core.deviceMemories.first, '');
      h.core.receiverEvents.add(
        const ReceiverUiEvent.deviceMemoryChanged(json: '[["Lena","x"]]'),
      );
      await two(t);
      expect(h.store.deviceMemory, '[["Lena","x"]]');
      // The next room of this run starts from it too.
      expect(h.core.deviceMemories.last, '[["Lena","x"]]');
    });

    test('the store keeps it until the data is reset', () async {
      final s = MemoryDisplayStore();
      await s.saveDeviceMemory('[]');
      expect(await s.loadDeviceMemory(), '[]');
      await s.reset();
      expect(await s.loadDeviceMemory(), '');
    });

    test('a store that fails gives an empty memory', () async {
      // PrefsDisplayStore without the plugin in a unit test: the errors are swallowed.
      const s = PrefsDisplayStore();
      expect(await s.loadDeviceMemory(), '');
      await s.saveDeviceMemory('[]');
    });
  });

  group('the iOS broadcast extension', () {
    test('gets the key of the app as base64url, without padding', () async {
      final ios = FakeIosBroadcast();
      final c = SenderController(core: FakeCoreApi(), iosBroadcast: ios);
      addTearDown(c.dispose);
      final secret = Uint8List.fromList(List.generate(32, (i) => i));
      await c.startBroadcast(code: 'K7QM2W', server: 's:1', deviceKey: secret);
      expect(
        ios.prepared!.deviceKey,
        'AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8',
      );
      await c.startBroadcast(code: 'K7QM2W', server: 's:1');
      expect(ios.prepared!.deviceKey, isNull);
    });
  });
}
