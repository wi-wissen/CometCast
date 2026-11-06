// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the picture fetch loop of the receiver controller (session changes, failures).

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/receiver_controller.dart';

import 'fake_core.dart';
import 'receive_page_test.dart' show solidFrame;

/// Lets real time pass (image decoding and timers run on the real event loop).
Future<void> realWait(WidgetTester tester, int ms) =>
    tester.runAsync(() => Future<void>.delayed(Duration(milliseconds: ms)));

Future<void> waitFor(WidgetTester tester, bool Function() done) async {
  // Real time for the image decoder, fake time for the controller's timers.
  for (var i = 0; i < 100 && !done(); i++) {
    await realWait(tester, 50);
    await tester.pump(const Duration(milliseconds: 100));
  }
}

void main() {
  testWidgets('a picture fetched before stop and start is not shown', (
    tester,
  ) async {
    final core = FakeCoreApi();
    final c = ReceiverController(core: core);
    addTearDown(c.dispose);
    c.start('s:1');
    core.frames.add(solidFrame(8, 6, [255, 0, 0]));
    final oldGate = Completer<void>();
    core.takeFrameGate = oldGate;
    core.receiverEvents.add(const ReceiverUiEvent.frameReady());
    await realWait(tester, 50);
    expect(core.takeFrameCalls, 1);

    await c.stop();
    c.start('s:2');
    // The old fetch finishes now: its picture belongs to the old session.
    oldGate.complete();
    await realWait(tester, 300);
    expect(c.image, isNull);

    // The new session can fetch (the slot was freed) and shows its own picture.
    core.frames.add(solidFrame(4, 2, [0, 0, 255]));
    core.receiverEvents.add(const ReceiverUiEvent.frameReady());
    await waitFor(tester, () => c.image != null);
    expect((c.image!.width, c.image!.height), (4, 2));
  });

  testWidgets(
    'a failing takeFrame is retried and does not block later pictures',
    (tester) async {
      final core = FakeCoreApi();
      final c = ReceiverController(core: core);
      addTearDown(c.dispose);
      c.start('s:1');
      core.takeFrameFailures = 1;
      core.frames.add(solidFrame(6, 6, [0, 255, 0]));
      core.receiverEvents.add(const ReceiverUiEvent.frameReady());
      await waitFor(tester, () => c.image != null);
      expect(core.takeFrameCalls, 2, reason: 'one failure, one retry');
      expect((c.image!.width, c.image!.height), (6, 6));
    },
  );

  testWidgets('a takeFrame that never answers times out', (tester) async {
    final core = FakeCoreApi();
    final c = ReceiverController(
      core: core,
      fetchTimeout: const Duration(milliseconds: 100),
    );
    addTearDown(c.dispose);
    c.start('s:1');
    core.takeFrameGate = Completer<void>(); // never completed
    core.frames.add(solidFrame(2, 2, [1, 2, 3]));
    core.receiverEvents.add(const ReceiverUiEvent.frameReady());
    await realWait(tester, 50);
    core.frames.add(solidFrame(5, 3, [9, 9, 9]));
    await waitFor(tester, () => c.image != null);
    expect((c.image!.width, c.image!.height), (5, 3));
  });
}
