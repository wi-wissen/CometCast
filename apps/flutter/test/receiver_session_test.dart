// SPDX-License-Identifier: GPL-3.0-or-later
// The receiver controller as the display uses it (what the old receive page showed, as rules): the
// room code, a stream that ends silently, a start error, join requests and what leaves with a
// decision, a track that is not H.264, the stop that clears everything.

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/receiver_controller.dart';

import 'fake_core.dart';

Future<void> tick() => Future<void>.delayed(const Duration(milliseconds: 10));

ReceiverController make(FakeCoreApi core, {bool keep = true}) {
  final c = ReceiverController(core: core, keepRoomOnPeerLeft: keep);
  addTearDown(c.dispose);
  return c;
}

void main() {
  test('start passes the trimmed server, the mode and the name', () async {
    final core = FakeCoreApi();
    final c = make(core);
    c.start(
      ' cast.example.org ',
      joinMode: UiJoinMode.confirm,
      roomName: 'Elbe',
    );
    expect(core.receiverServer, 'cast.example.org');
    expect(core.receiverJoinMode, UiJoinMode.confirm);
    expect(core.receiverRoomName, 'Elbe');
    expect(c.phase, ReceiverPhase.starting);
  });

  test('the room code arrives, then the picture view can start', () async {
    final core = FakeCoreApi();
    final c = make(core);
    c.start('s:1');
    core.receiverEvents.add(
      const ReceiverUiEvent.roomCode(code: 'K7QM2W', joinMode: UiJoinMode.open),
    );
    await tick();
    expect(c.roomCode, 'K7QM2W');
    expect(c.phase, ReceiverPhase.running);
    expect(c.joinMode, UiJoinMode.open);
    expect(c.senderJoined, isFalse);
  });

  test('a stream that ends silently ends the session', () async {
    final core = FakeCoreApi();
    final c = make(core);
    c.start('s:1');
    core.receiverEvents.add(
      const ReceiverUiEvent.roomCode(code: 'K7QM2W', joinMode: UiJoinMode.open),
    );
    await tick();
    await core.receiverEvents.close();
    await tick();
    expect(c.phase, ReceiverPhase.ended);
  });

  test('an error event carries its code, not a text', () async {
    final core = FakeCoreApi();
    final c = make(core);
    c.start('s:1');
    core.receiverEvents.add(
      const ReceiverUiEvent.error(code: 'rate_limited', message: 'slow down'),
    );
    await tick();
    expect(c.errorCode, 'rate_limited');
    expect(c.phase, ReceiverPhase.ended);
  });

  test('stop clears the code and the name of the presenter', () async {
    final core = FakeCoreApi();
    final c = make(core);
    c.start('s:1');
    core.receiverEvents
      ..add(
        const ReceiverUiEvent.roomCode(
          code: 'K7QM2W',
          joinMode: UiJoinMode.open,
        ),
      )
      ..add(const ReceiverUiEvent.peerJoined(name: 'Lena'));
    await tick();
    await c.stop();
    expect(c.roomCode, isNull);
    expect(c.peerName, isNull);
    expect(core.stopReceiverCalls, 1);
  });

  test('a track that is not H.264 is noted instead of a picture', () async {
    final core = FakeCoreApi();
    final c = make(core);
    c.start('s:1');
    core.receiverEvents
      ..add(
        const ReceiverUiEvent.roomCode(
          code: 'K7QM2W',
          joinMode: UiJoinMode.open,
        ),
      )
      ..add(const ReceiverUiEvent.peerJoined(name: 'Lena'))
      ..add(const ReceiverUiEvent.videoTrack(mime: 'video/VP9', codec: 'vp9'));
    await tick();
    expect(c.codec, 'vp9');
    expect(c.codecNotShown, isTrue);
    expect(c.hasLiveVideo, isFalse);
  });

  group('join requests', () {
    test(
      'a request is listed, a decision removes it and tells the core',
      () async {
        final core = FakeCoreApi();
        final c = make(core);
        c.start('s:1');
        core.receiverEvents
          ..add(
            ReceiverUiEvent.joinRequest(
              id: BigInt.one,
              name: 'Lena',
              check: 42,
            ),
          )
          ..add(
            ReceiverUiEvent.joinRequest(id: BigInt.two, name: 'Tom', check: 17),
          );
        await tick();
        expect(c.requests.map((r) => r.name), ['Lena', 'Tom']);
        c.decideJoin(BigInt.one, allow: true);
        expect(c.requests.map((r) => r.name), ['Tom']);
        expect(core.joinDecisions.single.allow, isTrue);
        c.decideJoin(BigInt.two, allow: false, block: true);
        expect(core.blockedRequests, [BigInt.two]);
        expect(c.requests, isEmpty);
      },
    );

    test('a decision for an unknown request does nothing', () async {
      final core = FakeCoreApi();
      final c = make(core);
      c.start('s:1');
      c.decideJoin(BigInt.from(99), allow: true);
      expect(core.joinDecisions, isEmpty);
    });

    test('Deny all drops everybody who waits', () async {
      final core = FakeCoreApi();
      final c = make(core);
      c.start('s:1');
      core.receiverEvents
        ..add(
          ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 1),
        )
        ..add(
          ReceiverUiEvent.joinRequest(id: BigInt.two, name: 'Tom', check: 2),
        );
      await tick();
      c.denyAllJoins();
      expect(c.requests, isEmpty);
      expect(core.joinDecisions.single.id, isNull);
      expect(core.joinDecisions.single.allow, isFalse);
    });

    test('a cancelled request leaves with a notice', () async {
      final core = FakeCoreApi();
      final notices = <ReceiverNotice>[];
      final c = ReceiverController(core: core, onNotice: notices.add);
      addTearDown(c.dispose);
      c.start('s:1');
      core.receiverEvents
        ..add(
          ReceiverUiEvent.joinRequest(id: BigInt.one, name: 'Lena', check: 1),
        )
        ..add(
          ReceiverUiEvent.joinRequestCancelled(
            id: BigInt.one,
            reason: 'timeout',
          ),
        );
      await tick();
      expect(c.requests, isEmpty);
      expect(notices.whereType<NoticeExpired>().single.name, 'Lena');
    });

    test('a mode change is shown only when the server confirms', () async {
      final core = FakeCoreApi();
      final c = make(core);
      c.start('s:1');
      core.receiverEvents.add(
        const ReceiverUiEvent.roomCode(
          code: 'K7QM2W',
          joinMode: UiJoinMode.open,
        ),
      );
      await tick();
      c.setJoinMode(UiJoinMode.confirm);
      expect(core.joinModeRequests, [UiJoinMode.confirm]);
      expect(c.joinMode, UiJoinMode.open);
      core.receiverEvents.add(
        const ReceiverUiEvent.joinModeChanged(mode: UiJoinMode.confirm),
      );
      await tick();
      expect(c.joinMode, UiJoinMode.confirm);
    });

    test('End asks the core, the room itself ends on its event', () async {
      final core = FakeCoreApi();
      final c = make(core);
      c.start('s:1');
      expect(c.endRoom(), isTrue);
      expect(core.endRoomCalls, 1);
    });
  });
}
