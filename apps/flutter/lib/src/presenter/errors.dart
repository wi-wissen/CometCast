// SPDX-License-Identifier: GPL-3.0-or-later
// Which error page of the presenter belongs to a failure (spec 6). The Rust core reports
// failures as English technical messages (`server error <code>: ...`, `signalling connection
// failed: ...`, see cc-transport/src/error.rs); the server code is read from the message.

import 'dart:ui' show VoidCallback;

import '../../join_state.dart';
import 'pages.dart';

/// The server code inside a core message, or null.
String? serverCodeOf(String message) {
  final m = RegExp(r'server error ([a-z_]+)').firstMatch(message);
  if (m != null) return m.group(1);
  // A bare code (tests, other callers).
  return RegExp(r'^[a-z_]+$').hasMatch(message) ? message : null;
}

/// The page for [message]. [wasWaiting]: the code had been accepted before. [recentBusy]: a
/// `rate_limited`, `banned` or `server_full` refusal within the last 60 s (decision 13.44).
/// Returns null when no page is shown (`capture_denied`, spec 6).
(ErrorKind, String?)? errorKindFor(
  String message, {
  bool wasWaiting = false,
  bool recentBusy = false,
}) {
  final code = serverCodeOf(message);
  switch (code) {
    case 'room_not_found':
      return (wasWaiting ? ErrorKind.expired : ErrorKind.code, null);
    case 'join_denied':
      return (ErrorKind.denied, null);
    case 'room_locked' || 'room_locked_waiting':
      return (ErrorKind.locked, null);
    case 'approval_timeout':
      return (ErrorKind.timeout, null);
    case 'removed_by_host' || 'peer_left':
      return (ErrorKind.ended, null);
    case 'room_full':
      return (ErrorKind.occupied, null);
    case 'rate_limited' || 'banned' || 'server_full':
      return (ErrorKind.busy, null);
    case 'proto_mismatch' || 'bad_hello' || 'bad_message':
      return (ErrorKind.outdated, null);
    case 'capture_denied':
      return null;
    case 'connection_lost':
      return (recentBusy ? ErrorKind.busy : ErrorKind.noInternet, null);
    case 'peer_failed':
      return (ErrorKind.displayOff, null);
    case final c?:
      return (ErrorKind.other, c);
  }
  if (message.startsWith('signalling connection') ||
      message.startsWith('timeout')) {
    return (recentBusy ? ErrorKind.busy : ErrorKind.noInternet, null);
  }
  if (message.startsWith('webrtc error')) return (ErrorKind.displayOff, null);
  return (ErrorKind.other, 'unknown');
}

/// The page for a refused join.
(ErrorKind, bool) refusalPage(JoinRefusal r) => switch (r) {
  JoinRefusal.denied => (ErrorKind.denied, false),
  JoinRefusal.locked => (ErrorKind.locked, false),
  JoinRefusal.lockedWhileWaiting => (ErrorKind.locked, true),
  JoinRefusal.timedOut => (ErrorKind.timeout, false),
  JoinRefusal.removed => (ErrorKind.ended, false),
};

typedef Action0 = VoidCallback;
