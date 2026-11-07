// SPDX-License-Identifier: GPL-3.0-or-later
//
// Which error page of the presenter (docs/design/first-run-and-display.md, section 6) belongs to a
// failure of the sender session. Pure, so that it can be tested without a page.
import type { IceFetchState } from './altchaClient'

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
  /** `room_full`: someone else is already connected (one presenter per room). */
  | 'occupied'
  /** `rate_limited`, `banned`, `server_full`. */
  | 'busy'
  /** `proto_mismatch`, `bad_hello`, `bad_message`: page and server do not match. */
  | 'outdated'
  /** Every other failure (`forbidden`, `altcha_*`, `wrong_role`, `not_in_room`, `unknown`, anything unlisted): spec 6, decision 13.32. */
  | 'other'

export interface ErrorContext {
  /** True when the presenter had already been accepted as a valid code before (waiting for approval). */
  wasWaiting?: boolean
  /** How the last `GET /ice-servers` of this connection ended (decision 13.45); only read for `peer_failed`. */
  ice?: IceFetchState | null
}

/** The ICE list was refused (429 / 403): the server is busy for this address. */
const iceRefused = (ice: IceFetchState | null | undefined): boolean => ice?.kind === 'rate_limited' || ice?.kind === 'refused'
/** The ICE list failed for another reason (HTTP error, broken answer, not reachable). */
const iceFailed = (ice: IceFetchState | null | undefined): boolean =>
  ice?.kind === 'http_error' || ice?.kind === 'bad_answer' || ice?.kind === 'unreachable'

/** Which "why" text the `displayOff` page shows: the relay could not help (decision 13.45) or the plain one. */
export function displayOffWhy(ice: IceFetchState | null | undefined): 'relay' | 'plain' {
  return iceFailed(ice) ? 'relay' : 'plain'
}

/**
 * `code` = SenderState.errorCode, or 'peer_left' for a sender whose room ended. `null` means "no error page":
 * `capture_denied` (the person closed the system dialog) goes back to "What to show".
 */
export function errorKindFor(code: string | null, ctx: ErrorContext = {}): ErrorKind | null {
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
      // The list of STUN/TURN servers was refused for this connection: the same page as any other refusal.
      return iceRefused(ctx.ice) ? 'busy' : 'displayOff'
    case 'capture_denied':
      return null
    case 'capture_failed':
      return 'noScreen'
    case 'room_full':
      return 'occupied'
    case 'rate_limited':
    case 'banned':
    case 'server_full':
      return 'busy'
    case 'proto_mismatch':
    case 'bad_hello':
    case 'bad_message':
      return 'outdated'
    default:
      return 'other'
  }
}

/** The "why" text variant for locked rooms (spec 6: at once / while waiting). */
export const lockedWhileWaiting = (code: string | null): boolean => code === 'room_locked_waiting'
