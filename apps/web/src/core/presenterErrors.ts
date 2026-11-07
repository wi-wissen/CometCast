// SPDX-License-Identifier: GPL-3.0-or-later
//
// Which error page of the presenter (docs/design/first-run-and-display.md, section 6) belongs to a
// failure of the sender session. Pure, so that it can be tested without a page.

export type ErrorKind =
  | 'code'
  | 'expired'
  | 'otherServer'
  | 'noInternet'
  | 'noScreen'
  | 'displayOff'
  | 'denied'
  | 'locked'
  | 'timeout'
  | 'ended'
  /** A failure the spec has no page for (rate limits, protocol errors ...): same layout, the existing wording. */
  | 'generic'

export interface ErrorContext {
  /** True when the presenter had already been accepted as a valid code before (waiting for approval). */
  wasWaiting?: boolean
}

/** `code` = SenderState.errorCode, or 'peer_left' for a sender whose room ended. */
export function errorKindFor(code: string | null, ctx: ErrorContext = {}): ErrorKind {
  switch (code) {
    case 'room_not_found':
      // The code was valid a moment ago (the display ended its session or restarted): "no longer valid".
      return ctx.wasWaiting ? 'expired' : 'code'
    case 'join_denied':
      return 'denied'
    case 'room_locked':
    case 'room_locked_waiting':
      return 'locked'
    case 'approval_timeout':
      return 'timeout'
    case 'peer_left':
    case 'removed_by_host':
      return 'ended'
    case 'connection_lost':
      return 'noInternet'
    case 'peer_failed':
      return 'displayOff'
    case 'capture_failed':
      return 'noScreen'
    default:
      return 'generic'
  }
}

/** The "why" text variant for locked rooms (spec 6: at once / while waiting). */
export const lockedWhileWaiting = (code: string | null): boolean => code === 'room_locked_waiting'
