// SPDX-License-Identifier: GPL-3.0-or-later
// The iOS broadcast hand-over as the presenter app uses it (SenderController.startBroadcast): the
// room is handed to the extension, the system sheet opens, the extension reports, a silent or
// missing extension ends the wait. The pages that showed the status are gone; the controller rules
// are not.

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/ios_broadcast.dart';
import 'package:cometcast/sender_controller.dart';

import 'fake_core.dart';
import 'ios_broadcast_test.dart' show FakeIosBroadcast;

Future<void> tick() => Future<void>.delayed(const Duration(milliseconds: 10));

SenderController make(
  FakeIosBroadcast ios, {
  Duration? timeout,
  DateTime Function()? clock,
}) {
  final c = SenderController(
    core: FakeCoreApi(),
    iosBroadcast: ios,
    broadcastStartTimeout: timeout ?? const Duration(minutes: 2),
    clock: clock ?? DateTime.now,
  );
  addTearDown(c.dispose);
  return c;
}

void main() {
  test('starting hands the room over, opens the sheet and waits', () async {
    final ios = FakeIosBroadcast();
    final c = make(ios);
    await c.startBroadcast(code: 'K7QM2W', server: ' relay.example.org ');
    expect(ios.prepared!.code, 'K7QM2W');
    expect(ios.prepared!.relay, 'relay.example.org');
    expect(ios.showPickerCalls, 1);
    expect(c.phase, SenderPhase.starting);
    expect(c.awaitingBroadcast, isTrue);
  });

  test('the status of the extension is followed and ends cleanly', () async {
    final ios = FakeIosBroadcast();
    final c = make(ios);
    await c.startBroadcast(code: 'K7QM2W', server: 's:1');
    ios.controller.add(
      const IosBroadcastStatus(state: IosBroadcastState.connecting),
    );
    await tick();
    expect(c.phase, SenderPhase.starting);
    expect(c.awaitingBroadcast, isFalse);
    ios.controller.add(
      const IosBroadcastStatus(
        state: IosBroadcastState.streaming,
        sent: 12,
        bytes: 900,
      ),
    );
    await tick();
    expect(c.phase, SenderPhase.running);
    expect(c.encodedFrames, 12);
    ios.controller.add(
      const IosBroadcastStatus(state: IosBroadcastState.ended),
    );
    await tick();
    expect(c.phase, SenderPhase.ended);
    expect(c.captureStopped, isTrue);
    expect(ios.stopCalls, 1);
  });

  test('the receiver leaving is its own message', () async {
    final ios = FakeIosBroadcast();
    final c = make(ios);
    await c.startBroadcast(code: 'K7QM2W', server: 's:1');
    ios.controller.add(
      const IosBroadcastStatus(
        state: IosBroadcastState.ended,
        error: IosBroadcastError.peerLeft,
      ),
    );
    await tick();
    expect(c.receiverLeft, isTrue);
    expect(c.captureStopped, isFalse);
  });

  test('a failure carries its reason', () async {
    final ios = FakeIosBroadcast();
    final c = make(ios);
    await c.startBroadcast(code: 'K7QM2W', server: 's:1');
    ios.controller.add(
      const IosBroadcastStatus(
        state: IosBroadcastState.failed,
        error: IosBroadcastError.roomNotFound,
      ),
    );
    await tick();
    expect(c.phase, SenderPhase.ended);
    expect(c.broadcastError, IosBroadcastError.roomNotFound);
  });

  test('a silent extension is reported as lost', () async {
    final ios = FakeIosBroadcast();
    final c = make(ios);
    await c.startBroadcast(code: 'K7QM2W', server: 's:1');
    ios.controller.add(const IosBroadcastStatus(state: IosBroadcastState.lost));
    await tick();
    expect(c.broadcastError, IosBroadcastError.internalError);
  });

  test(
    'a failed picker is remembered for the Control Center fallback',
    () async {
      final ios = FakeIosBroadcast()..pickerOpens = false;
      final c = make(ios);
      await c.startBroadcast(code: 'K7QM2W', server: 's:1');
      expect(c.broadcastPickerFailed, isTrue);
      expect(c.awaitingBroadcast, isTrue);
    },
  );

  test('a missing App Group is an error, not a hang', () async {
    final ios = FakeIosBroadcast()..prepareError = StateError('no app group');
    final c = make(ios);
    await c.startBroadcast(code: 'K7QM2W', server: 's:1');
    expect(c.phase, SenderPhase.ended);
    expect(c.error, contains('no app group'));
  });

  test('nobody starting the broadcast ends the wait', () async {
    final ios = FakeIosBroadcast();
    final c = make(ios, timeout: const Duration(milliseconds: 30));
    await c.startBroadcast(code: 'K7QM2W', server: 's:1');
    await Future<void>.delayed(const Duration(milliseconds: 80));
    expect(c.broadcastNotStarted, isTrue);
    expect(c.phase, SenderPhase.idle);
    expect(ios.stopCalls, 1);
  });

  test('stop withdraws the hand-over', () async {
    final ios = FakeIosBroadcast();
    final c = make(ios);
    await c.startBroadcast(code: 'K7QM2W', server: 's:1');
    await c.stop();
    expect(ios.stopCalls, 1);
    expect(c.phase, SenderPhase.idle);
  });

  test(
    'a report written before this request belongs to an earlier broadcast',
    () async {
      final ios = FakeIosBroadcast();
      final now = DateTime(2026, 11, 7, 12);
      final c = make(ios, clock: () => now);
      await c.startBroadcast(code: 'K7QM2W', server: 's:1');
      ios.controller.add(
        IosBroadcastStatus(
          state: IosBroadcastState.streaming,
          updatedAtMs: now.millisecondsSinceEpoch - 60000,
        ),
      );
      await tick();
      expect(c.phase, SenderPhase.starting);
    },
  );
}
