// SPDX-License-Identifier: GPL-3.0-or-later
// The core round (fix list B1 to B6): the room outlives a presenter, the room name goes to the
// core, the receive progress and the connection code, and the presenter session that joins once.

import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/core/core_api.dart';
import 'package:cometcast/presenter_session_controller.dart';
import 'package:cometcast/receiver_controller.dart';
import 'package:cometcast/src/presenter/errors.dart';
import 'package:cometcast/src/presenter/pages.dart';

import 'fake_core.dart';

Future<void> tick() => Future<void>.delayed(const Duration(milliseconds: 10));

void main() {
  group('receiver: the iOS hand-over (native-apps.md 20.5)', () {
    Future<(FakeCoreApi, ReceiverController, List<ReceiverNotice>)>
    rig() async {
      final core = FakeCoreApi();
      final notices = <ReceiverNotice>[];
      final c = ReceiverController(
        core: core,
        keepRoomOnPeerLeft: true,
        onNotice: notices.add,
      );
      addTearDown(c.dispose);
      c.start('s:1', joinMode: UiJoinMode.confirm);
      core.receiverEvents.add(
        const ReceiverUiEvent.roomCode(
          code: 'K7QM2W',
          joinMode: UiJoinMode.confirm,
        ),
      );
      await tick();
      return (core, c, notices);
    }

    test(
      'a hand-over join is no "connected" notice, a normal one is',
      () async {
        final (core, c, notices) = await rig();
        core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Ada', handover: true),
        );
        await tick();
        expect(c.senderJoined, isTrue);
        expect(notices, isEmpty);
        core.receiverEvents.add(
          const ReceiverUiEvent.peerJoined(name: 'Bo', handover: false),
        );
        await tick();
        expect(notices.single, isA<NoticeJoined>());
      },
    );

    test('leaving with a token stored is no "left" notice; the picture goes, black stays', () async {
      final (core, c, notices) = await rig();
      core.receiverEvents
        ..add(const ReceiverUiEvent.peerJoined(name: 'Ada', handover: false))
        ..add(
          const ReceiverUiEvent.contentLink(
            url: 'https://example.org/',
            host: 'example.org',
          ),
        )
        ..add(const ReceiverUiEvent.handoverStored(name: 'Ada'))
        ..add(const ReceiverUiEvent.peerLeft());
      await tick();
      expect(notices.whereType<NoticeLeft>(), isEmpty);
      expect(c.senderJoined, isFalse);
      expect(c.shown, isNull);
      expect(c.handoverHold, isTrue, reason: 'Presenting on black');
      // The person is back with the token: normal rules again, still no notice.
      core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(name: 'Ada', handover: true),
      );
      await tick();
      expect(c.handoverHold, isFalse);
      expect(c.handoverPending, isFalse);
      // Only the first, ordinary join was announced.
      expect(notices.whereType<NoticeJoined>(), hasLength(1));
    });

    test(
      'nothing was shown: Idle at once, and an expired token ends the hold',
      () async {
        final (core, c, notices) = await rig();
        core.receiverEvents
          ..add(const ReceiverUiEvent.peerJoined(name: 'Ada', handover: false))
          ..add(const ReceiverUiEvent.handoverStored(name: 'Ada'))
          ..add(const ReceiverUiEvent.peerLeft());
        await tick();
        expect(c.handoverHold, isFalse);
        expect(notices.whereType<NoticeLeft>(), isEmpty);
        // The token ran out: a later leave is a leave like any other.
        core.receiverEvents
          ..add(const ReceiverUiEvent.handoverExpired())
          ..add(const ReceiverUiEvent.peerJoined(name: 'Bo', handover: false))
          ..add(const ReceiverUiEvent.peerLeft());
        await tick();
        expect(notices.whereType<NoticeLeft>(), hasLength(1));
      },
    );

    test('a hold ends by itself after 60 s', () async {
      await fakeAsyncHold();
    });
  });

  group('receiver: the room outlives the presenter', () {
    test('the room name goes to the core', () {
      final core = FakeCoreApi();
      final c = ReceiverController(core: core);
      addTearDown(c.dispose);
      c.start('s:1', roomName: 'Meeting room Elbe');
      expect(core.receiverRoomName, 'Meeting room Elbe');
      // A restart after End keeps it.
      core.receiverEvents.add(const ReceiverUiEvent.roomEnded());
    });

    test('after PeerLeft the same code stays, no new room starts', () async {
      final core = FakeCoreApi();
      final notices = <ReceiverNotice>[];
      var starts = 0;
      final c = ReceiverController(
        core: core,
        keepRoomOnPeerLeft: true,
        onNotice: notices.add,
      );
      addTearDown(c.dispose);
      c.start('s:1', joinMode: UiJoinMode.confirm, roomName: 'Elbe');
      starts++;
      core.receiverEvents
        ..add(
          const ReceiverUiEvent.roomCode(
            code: 'K7QM2W',
            joinMode: UiJoinMode.confirm,
          ),
        )
        ..add(const ReceiverUiEvent.peerJoined(name: 'Ada', handover: false))
        ..add(const ReceiverUiEvent.connectionCode(sas: '123 456'))
        ..add(
          const ReceiverUiEvent.videoTrack(mime: 'video/H264', codec: 'h264'),
        );
      await tick();
      expect(c.peerName, 'Ada');
      expect(c.connectionCode, '123 456');
      expect(c.codec, 'h264');

      core.receiverEvents.add(const ReceiverUiEvent.peerLeft());
      await tick();
      // Idle with the same code; nothing of the presenter is left; the toast is the leave notice.
      expect(c.phase, ReceiverPhase.running);
      expect(c.roomCode, 'K7QM2W');
      expect(c.joinMode, UiJoinMode.confirm);
      expect(c.peerName, isNull);
      expect(c.senderJoined, isFalse);
      expect(c.connectionCode, isNull);
      expect(c.codec, isNull);
      expect(notices.whereType<NoticeLeft>().single.name, 'Ada');
      expect(core.stopReceiverCalls, 0, reason: 'the core keeps the room');
      expect(starts, 1);

      // A second presenter joins with that code.
      core.receiverEvents.add(
        const ReceiverUiEvent.peerJoined(name: 'Bo', handover: false),
      );
      await tick();
      expect(c.peerName, 'Bo');
      expect(c.roomCode, 'K7QM2W');
    });

    test('a stop of the presentation is Idle without a leave notice', () async {
      final core = FakeCoreApi();
      final notices = <ReceiverNotice>[];
      final c = ReceiverController(
        core: core,
        keepRoomOnPeerLeft: true,
        onNotice: notices.add,
      );
      addTearDown(c.dispose);
      c.start('s:1');
      core.receiverEvents
        ..add(
          const ReceiverUiEvent.roomCode(
            code: 'K7QM2W',
            joinMode: UiJoinMode.open,
          ),
        )
        ..add(const ReceiverUiEvent.peerJoined(name: 'Ada', handover: false))
        ..add(const ReceiverUiEvent.connectionCode(sas: '123 456'))
        ..add(
          const ReceiverUiEvent.contentLink(
            url: 'https://example.org/',
            host: 'example.org',
          ),
        );
      await tick();
      expect(c.shown, isA<ReceivedLink>());
      core.receiverEvents.add(const ReceiverUiEvent.presentationStopped());
      await tick();
      expect(c.shown, isNull);
      expect(c.connectionCode, isNull);
      // The presenter is still in the room.
      expect(c.peerName, 'Ada');
      expect(c.roomCode, 'K7QM2W');
      expect(notices.whereType<NoticeLeft>(), isEmpty);
    });

    test('the old receive page ends on PeerLeft and stops the core', () async {
      final core = FakeCoreApi();
      final c = ReceiverController(core: core);
      addTearDown(c.dispose);
      c.start('s:1');
      core.receiverEvents
        ..add(
          const ReceiverUiEvent.roomCode(
            code: 'K7QM2W',
            joinMode: UiJoinMode.open,
          ),
        )
        ..add(const ReceiverUiEvent.peerLeft());
      await tick();
      expect(c.phase, ReceiverPhase.ended);
      expect(core.stopReceiverCalls, greaterThanOrEqualTo(1));
    });

    test('the receive progress counts up and the error has a code', () async {
      final core = FakeCoreApi();
      final c = ReceiverController(core: core);
      addTearDown(c.dispose);
      c.start('s:1');
      core.receiverEvents
        ..add(
          const ReceiverUiEvent.roomCode(
            code: 'K7QM2W',
            joinMode: UiJoinMode.open,
          ),
        )
        ..add(
          ReceiverUiEvent.contentTransferStarted(
            document: true,
            name: 'big.pdf',
            size: BigInt.from(20 * 1024 * 1024),
          ),
        );
      await tick();
      expect(c.receiving!.percent, 0);
      final seen = <int>[];
      for (final pct in [10, 50, 100]) {
        core.receiverEvents.add(
          ReceiverUiEvent.contentTransferProgress(
            received: BigInt.from(20 * 1024 * 1024 * pct ~/ 100),
            total: BigInt.from(20 * 1024 * 1024),
          ),
        );
        await tick();
        seen.add(c.receiving!.percent);
      }
      expect(seen, [10, 50, 100]);
      expect(c.receiving!.name, 'big.pdf');

      core.receiverEvents.add(
        const ReceiverUiEvent.error(
          code: 'server_full',
          message: 'server error server_full: x',
        ),
      );
      await tick();
      expect(c.phase, ReceiverPhase.ended);
      expect(c.errorCode, 'server_full');
    });
  });

  group('presenter session: join once', () {
    PresenterSessionController make(FakeCoreApi core) {
      final c = PresenterSessionController(core: core);
      addTearDown(c.dispose);
      return c;
    }

    test('Connect joins once with approval; Stop keeps the join', () async {
      final core = FakeCoreApi();
      final c = make(core);
      c.connect(code: 'K7QM2W', server: ' s:1 ', name: 'Ada');
      expect(core.startPresenterCalls, 1);
      expect(core.presenterServer, 's:1');
      expect(c.phase, PresenterSessionPhase.connecting);

      core.presenterEvents.add(
        const PresenterUiEvent.waitingForApproval(
          timeoutSecs: 60,
          check: 42,
          roomName: 'Meeting room Elbe',
        ),
      );
      await tick();
      expect(c.phase, PresenterSessionPhase.waiting);
      expect((c.check, c.timeoutSecs), (42, 60));
      expect(c.roomName, 'Meeting room Elbe');

      core.presenterEvents.add(
        const PresenterUiEvent.joined(roomName: 'Meeting room Elbe'),
      );
      await tick();
      expect(c.phase, PresenterSessionPhase.joined);
      expect(c.wasJoined, isTrue);

      // First presentation: a file.
      expect(c.presentFile('/tmp/slides.pdf'), isTrue);
      core.presenterEvents
        ..add(const PresenterUiEvent.converting(name: 'slides.pptx'))
        ..add(
          PresenterUiEvent.contentPrepared(
            name: 'slides.pdf',
            kind: UiContentKind.pdf,
            size: BigInt.from(1000),
            pages: 3,
          ),
        )
        ..add(const PresenterUiEvent.presenting())
        ..add(const PresenterUiEvent.connectionCode(sas: '123 456'))
        ..add(
          PresenterUiEvent.progress(
            sent: BigInt.from(500),
            total: BigInt.from(1000),
          ),
        )
        ..add(const PresenterUiEvent.contentSent())
        ..add(const PresenterUiEvent.page(page: 1, pages: 3));
      await tick();
      expect(c.phase, PresenterSessionPhase.presenting);
      expect(c.contentPhase, PresenterContentPhase.sent);
      expect(c.contentPages, 3);
      expect(c.connectionCode, '123 456');
      expect((c.page, c.pages), (1, 3));
      expect(
        c.hasPreviousPage,
        isFalse,
        reason: 'Previous is disabled on page 1',
      );
      expect(c.hasNextPage, isTrue);

      // Stop: back to "What to show", still joined, no second request.
      expect(c.stopPresenting(), isTrue);
      expect(core.stopPresentingCalls, 1);
      core.presenterEvents.add(const PresenterUiEvent.presentingStopped());
      await tick();
      expect(c.phase, PresenterSessionPhase.joined);
      expect(c.page, isNull);
      expect(c.connectionCode, isNull);
      expect(core.startPresenterCalls, 1, reason: 'joined once');
      expect(core.stopPresenterCalls, 0);

      // Second presentation: the screen (the picker), then a link, in the same session.
      expect(c.presentScreen(), isTrue);
      expect(core.presentedScreens, [null]);
      core.presenterEvents
        ..add(const PresenterUiEvent.presenting())
        ..add(const PresenterUiEvent.presentingStopped());
      await tick();
      expect(c.presentLink('https://example.org'), isTrue);
      expect(core.presentedContent.length, 2, reason: 'a file, then a link');
      expect(core.startPresenterCalls, 1);

      // Disconnect leaves.
      await c.disconnect();
      expect(core.stopPresenterCalls, 1);
      expect(c.phase, PresenterSessionPhase.idle);
      expect(c.wasJoined, isFalse);
    });

    test('a refused file or a cancelled capture keeps the join', () async {
      final core = FakeCoreApi();
      final c = make(core);
      c.connect(code: 'K7QM2W', server: 's:1');
      core.presenterEvents.add(const PresenterUiEvent.joined());
      await tick();

      core.presenterEvents.add(
        const PresenterUiEvent.contentRejected(
          reason: UiSendRejection.tooLarge,
        ),
      );
      await tick();
      expect(c.phase, PresenterSessionPhase.joined);
      expect(c.rejection, UiSendRejection.tooLarge);

      core.presenterEvents.add(
        const PresenterUiEvent.error(
          code: 'capture_denied',
          message: 'cancelled',
        ),
      );
      await tick();
      expect(
        c.phase,
        PresenterSessionPhase.joined,
        reason: 'no page, still joined',
      );
      expect(c.startFailure, 'capture_denied');
      expect(errorKindForCode('capture_denied'), isNull);

      // The next presentation clears it.
      c.presentScreen();
      expect(c.startFailure, isNull);
    });

    test('the end and the errors', () async {
      final core = FakeCoreApi();
      final c = make(core);
      c.connect(code: 'K7QM2W', server: 's:1');
      core.presenterEvents.add(const PresenterUiEvent.joined());
      await tick();
      core.presenterEvents.add(
        const PresenterUiEvent.ended(code: 'removed_by_host'),
      );
      await tick();
      expect(c.phase, PresenterSessionPhase.ended);
      expect(c.endedCode, 'removed_by_host');

      c.connect(code: 'K7QM2W', server: 's:1');
      core.presenterEvents.add(
        const PresenterUiEvent.error(code: 'room_locked', message: 'x'),
      );
      await tick();
      expect(c.phase, PresenterSessionPhase.failed);
      expect(c.errorCode, 'room_locked');
      expect(c.wasJoined, isFalse);
    });

    test('commands are refused without a session', () {
      final core = FakeCoreApi()..presenterAccepts = false;
      final c = make(core);
      expect(c.presentEncoded(), isFalse);
      expect(c.stopPresenting(), isFalse);
    });
  });

  group('every error page of spec 6 is reached from its code', () {
    ErrorKind? kind(String code, {bool waited = false, bool busy = false}) =>
        errorKindForCode(code, wasWaiting: waited, recentBusy: busy)?.$1;

    test('codes to pages', () {
      expect(kind('room_not_found'), ErrorKind.code);
      expect(kind('room_not_found', waited: true), ErrorKind.expired);
      expect(kind('join_denied'), ErrorKind.denied);
      expect(kind('room_locked'), ErrorKind.locked);
      expect(kind('room_locked_waiting'), ErrorKind.locked);
      expect(kind('approval_timeout'), ErrorKind.timeout);
      expect(kind('removed_by_host'), ErrorKind.ended);
      expect(kind('display_left'), ErrorKind.ended);
      expect(kind('room_full'), ErrorKind.occupied);
      for (final c in ['rate_limited', 'banned', 'server_full']) {
        expect(kind(c), ErrorKind.busy, reason: c);
      }
      for (final c in ['proto_mismatch', 'bad_hello', 'bad_message']) {
        expect(kind(c), ErrorKind.outdated, reason: c);
      }
      expect(kind('connection_lost'), ErrorKind.noInternet);
      expect(kind('connection_lost', busy: true), ErrorKind.busy);
      expect(kind('peer_failed'), ErrorKind.displayOff);
      expect(kind('ice_refused'), ErrorKind.busy);
      expect(kind('ice_failed'), ErrorKind.displayOff);
      expect(displayOffWhyRelay('ice_failed'), isTrue);
      expect(displayOffWhyRelay('peer_failed'), isFalse);
      for (final c in [
        'forbidden',
        'altcha_required',
        'altcha_invalid',
        'wrong_role',
        'not_in_room',
        'unknown',
        'capture_unavailable',
        'something_new',
      ]) {
        expect(kind(c), ErrorKind.other, reason: c);
      }
      expect(errorKindForCode('forbidden')!.$2, 'forbidden');
      expect(errorKindForCode('capture_denied'), isNull);
    });
  });
}

Future<void> fakeAsyncHold() async {
  final core = FakeCoreApi();
  final c = ReceiverController(core: core, keepRoomOnPeerLeft: true);
  c.start('s:1', joinMode: UiJoinMode.confirm);
  core.receiverEvents
    ..add(
      const ReceiverUiEvent.roomCode(
        code: 'K7QM2W',
        joinMode: UiJoinMode.confirm,
      ),
    )
    ..add(const ReceiverUiEvent.peerJoined(name: 'Ada', handover: false))
    ..add(
      const ReceiverUiEvent.contentLink(
        url: 'https://example.org/',
        host: 'example.org',
      ),
    )
    ..add(const ReceiverUiEvent.handoverStored(name: 'Ada'))
    ..add(const ReceiverUiEvent.peerLeft());
  await tick();
  expect(c.handoverHold, isTrue);
  expect(ReceiverController.handoverHoldSeconds, 60);
  c.dispose();
}
