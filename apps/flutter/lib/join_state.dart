// SPDX-License-Identifier: GPL-3.0-or-later
// What a sender learns about joining a room that needs the host's approval. Shared by the
// controllers of the screen sender and of the content sender.

/// Why a sender is not in the room.
enum JoinRefusal {
  /// The host denied the request.
  denied,

  /// The room code is right but the room is locked.
  locked,

  /// The room was locked while this sender waited.
  lockedWhileWaiting,

  /// Nobody decided in time.
  timedOut,

  /// The host removed this sender from the room.
  removed,
}

/// The clock of the join screens (replaced in tests, which cannot move `DateTime.now`).
DateTime Function() joinClock = DateTime.now;

/// Seconds a sender waits before it asks again after a denial.
const deniedCooldownSeconds = 15;

/// Maps a server code of a refused join. Null for any other code.
JoinRefusal? refusalFromCode(String code, {required bool wasWaiting}) =>
    switch (code) {
      'join_denied' => JoinRefusal.denied,
      'room_locked' =>
        wasWaiting ? JoinRefusal.lockedWhileWaiting : JoinRefusal.locked,
      'approval_timeout' => JoinRefusal.timedOut,
      _ => null,
    };

/// The wait for the host's decision (what the waiting screen shows).
class ApprovalWait {
  const ApprovalWait({required this.check, required this.timeoutSecs});

  /// Two-digit number that the display shows on the request too.
  final int check;
  final int timeoutSecs;
}
