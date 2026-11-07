// SPDX-License-Identifier: GPL-3.0-or-later
//
// What the idle screen shows about the connection (docs/design/first-run-and-display.md, 4 and 4.2):
//
//   online      a code is there
//   connecting  no code yet: the code line is empty (it keeps its height), the QR box is empty at 25 %, no caption
//   offline     the box "No connection to {dom}"; the QR box stays empty
//
// The box is shown only after the connection has been gone for 3 s (no flicker on short drops). Start (or a
// restart after the connection came back) without a code for 8 s also ends in the box. Pure, so that it can be
// tested without a clock or a page.

export type ConnectionView = 'online' | 'connecting' | 'offline'
/** Which body text the box shows (spec 4.2). */
export type OfflineCause = 'network' | 'busy' | 'outdated'

/** Box after a lost connection or a failed attempt. */
export const OFFLINE_AFTER_FAILURE_MS = 3000
/** Box when there was no failure but also no code yet. */
export const OFFLINE_AFTER_SILENCE_MS = 8000

export interface ConnectionInput {
  /** The room code, or null. */
  code: string | null
  /** The receiver's phase is `error` right now. */
  phase: string
  /** An error happened since the display last had a code (a retry that is running does not clear it). */
  failed: boolean
  /** Milliseconds since the display had no code. */
  withoutCodeMs: number
}

export function connectionView(i: ConnectionInput): ConnectionView {
  if (i.code !== null && i.phase !== 'error') return 'online'
  if (i.phase === 'error' || i.failed) return i.withoutCodeMs >= OFFLINE_AFTER_FAILURE_MS ? 'offline' : 'connecting'
  return i.withoutCodeMs >= OFFLINE_AFTER_SILENCE_MS ? 'offline' : 'connecting'
}

const BUSY = new Set(['rate_limited', 'banned', 'server_full'])
const OUTDATED = new Set(['proto_mismatch', 'bad_hello'])

/** The body variant for the last error code (null or anything else: the network). */
export function offlineCause(code: string | null): OfflineCause {
  if (code && BUSY.has(code)) return 'busy'
  if (code && OUTDATED.has(code)) return 'outdated'
  return 'network'
}

/** Milliseconds until the next attempt after an error, or null when the page has to be reloaded (spec 4.2). */
export function retryDelayMs(code: string | null): number | null {
  if (code && OUTDATED.has(code)) return null
  // The longer wait also applies to `forbidden` (as before): the network is refused, hammering does not help.
  if (code && (BUSY.has(code) || code === 'forbidden')) return 30_000
  return 4000
}
