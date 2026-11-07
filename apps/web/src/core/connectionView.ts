// SPDX-License-Identifier: GPL-3.0-or-later
//
// What the idle screen shows about the connection (docs/design/first-run-and-display.md, 4 and 4.2):
//
//   online      a code is there
//   connecting  no code yet: the code line is empty (it keeps its height), the QR box is empty at 25 %, no caption
//   offline     the box "No connection to {dom}"; the QR box stays empty
//
// Reconnect schedule (spec 4.2, decision 13.30): after a loss the display tries again at 1 s, 2 s and 3 s, then every
// 4 s. The box appears when the attempt at 3 s has failed (no separate timer), so a drop shorter than 3 s never shows
// it and it never flickers away while a later attempt runs. Busy refusals show the box at once and retry after 30 s.
// Start (or a restart after the connection came back) without a code and without any failure for 8 s also ends in the
// box. Pure, so that it can be tested without a clock or a page.

export type ConnectionView = 'online' | 'connecting' | 'offline'
/** Which body text the box shows (spec 4.2). */
export type OfflineCause = 'network' | 'busy' | 'outdated'

/** Retries after a loss, in ms since the loss: 1 s, 2 s, 3 s; then every {@link RETRY_EVERY_MS}. */
export const RETRY_AT_MS: readonly number[] = [1000, 2000, 3000]
export const RETRY_EVERY_MS = 4000
/** The box shows once this many attempts have failed in a row since the last code: the failure itself plus the retries at 1, 2 and 3 s. */
export const FAILED_ATTEMPTS_FOR_BOX = RETRY_AT_MS.length + 1
/** Retry after a busy refusal. */
export const BUSY_RETRY_MS = 30_000
/** Box when there was no failure but also no code yet. */
export const OFFLINE_AFTER_SILENCE_MS = 8000

export interface ConnectionInput {
  /** The room code, or null. */
  code: string | null
  /** The receiver's phase is `error` right now. */
  phase: string
  /** An error happened since the display last had a code (a retry that is running does not clear it). */
  failed: boolean
  /** Attempts that ended in an error since the display last had a code (0 while none failed). */
  failedAttempts: number
  /** The cause of the last error (busy and outdated show the box at once). */
  cause: OfflineCause
  /** Milliseconds since the display had no code. */
  withoutCodeMs: number
}

export function connectionView(i: ConnectionInput): ConnectionView {
  if (i.code !== null && i.phase !== 'error') return 'online'
  if (i.phase === 'error' || i.failed) {
    if (i.cause !== 'network') return 'offline'
    return i.failedAttempts >= FAILED_ATTEMPTS_FOR_BOX ? 'offline' : 'connecting'
  }
  return i.withoutCodeMs >= OFFLINE_AFTER_SILENCE_MS ? 'offline' : 'connecting'
}

const BUSY = new Set(['rate_limited', 'banned', 'server_full', 'forbidden'])
const OUTDATED = new Set(['proto_mismatch', 'bad_hello'])

/** The body variant for the last error code (null or anything else: the network). `forbidden` reads as busy (spec 4.2). */
export function offlineCause(code: string | null): OfflineCause {
  if (code && BUSY.has(code)) return 'busy'
  if (code && OUTDATED.has(code)) return 'outdated'
  return 'network'
}

/**
 * Milliseconds until the next attempt after an error, or null when the page has to be reloaded (spec 4.2).
 * `failedAttempts` counts the failures in a row (the first one is the loss itself), `sinceLossMs` is the time since the
 * loss: the retries come at 1, 2 and 3 s after it, then every 4 s (7 s, 11 s ...), measured from the loss, so a retry
 * that took a while to fail does not push the next one back.
 */
export function retryDelayMs(code: string | null, failedAttempts = 1, sinceLossMs = 0): number | null {
  const cause = offlineCause(code)
  if (cause === 'outdated') return null
  if (cause === 'busy') return BUSY_RETRY_MS
  const n = Math.max(0, failedAttempts - 1) // index of the retry that is due
  const at = n < RETRY_AT_MS.length ? RETRY_AT_MS[n]! : RETRY_AT_MS[RETRY_AT_MS.length - 1]! + (n - RETRY_AT_MS.length + 1) * RETRY_EVERY_MS
  return Math.max(0, at - sinceLossMs)
}
