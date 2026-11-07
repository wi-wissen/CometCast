// SPDX-License-Identifier: GPL-3.0-or-later
// Which error page of the presenter belongs to a failure (spec 6). The presenter session of the
// core reports a structured `code` (native-apps.md 20.1, decision N9): `errorKindForCode` maps
// it. Message text is never parsed.

import 'dart:ui' show VoidCallback;

import '../../join_state.dart';
import 'pages.dart';

/// The page for [code], the structured code of the core (native-apps.md 20.1, spec 6).
/// [wasWaiting]: the code had been accepted before. [recentBusy]: a `rate_limited`, `banned` or
/// `server_full` refusal within the last 60 s (decision 13.44). Returns null when no page is
/// shown (`capture_denied`). The second value is the code for the "why" text of
/// [ErrorKind.other] (`errors.<code>`).
(ErrorKind, String?)? errorKindForCode(
  String code, {
  bool wasWaiting = false,
  bool recentBusy = false,
}) {
  switch (code) {
    case 'room_not_found':
      return (wasWaiting ? ErrorKind.expired : ErrorKind.code, null);
    case 'join_denied':
      return (ErrorKind.denied, null);
    case 'room_locked' || 'room_locked_waiting':
      return (ErrorKind.locked, null);
    case 'approval_timeout':
      return (ErrorKind.timeout, null);
    case 'removed_by_host' || 'display_left' || 'peer_left':
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
    // The list of STUN/TURN servers was refused: the presenter is told to try again later.
    case 'ice_refused':
      return (ErrorKind.busy, null);
    // The list failed otherwise: the display does not answer, with the relay as the "why".
    case 'ice_failed':
      return (ErrorKind.displayOff, null);
    // `error.noScreen` is web-only: an app that cannot capture shows `error.other`.
    case 'capture_unavailable':
      return (ErrorKind.other, 'capture_unavailable');
    case 'unknown':
      return (ErrorKind.other, 'unknown');
    default:
      return (ErrorKind.other, code);
  }
}

/// Whether the "why" of [ErrorKind.displayOff] names the relay (`error.displayOff.whyRelay`):
/// the list of STUN/TURN servers failed for this connection (`ice_failed`).
bool displayOffWhyRelay(String code) => code == 'ice_failed';

/// The page for a refused join.
(ErrorKind, bool) refusalPage(JoinRefusal r) => switch (r) {
  JoinRefusal.denied => (ErrorKind.denied, false),
  JoinRefusal.locked => (ErrorKind.locked, false),
  JoinRefusal.lockedWhileWaiting => (ErrorKind.locked, true),
  JoinRefusal.timedOut => (ErrorKind.timeout, false),
  JoinRefusal.removed => (ErrorKind.ended, false),
};

typedef Action0 = VoidCallback;
