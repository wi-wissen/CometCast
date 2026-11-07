// SPDX-License-Identifier: GPL-3.0-or-later
//
// "Busy stays busy" (docs/design/first-run-and-display.md, 4.2 and 6, decision 13.44): the server closes the
// connection after a ban, so the next attempt from the same address ends as `connection_lost`. For 60 s after a
// `rate_limited`, `banned` or `server_full` refusal such a closed connection counts as the same refusal.

/** How long a refusal is remembered. */
export const BUSY_MEMORY_MS = 60_000
export const BUSY_CODES: readonly string[] = ['rate_limited', 'banned', 'server_full']

export class BusyMemory {
  private code: string | null = null
  private until = 0

  /** Takes the code of a failure and returns the code to report: the remembered refusal for a closed connection. */
  apply(code: string, now: number): string {
    if (BUSY_CODES.includes(code)) {
      this.code = code
      this.until = now + BUSY_MEMORY_MS
      return code
    }
    if (code === 'connection_lost' && this.code !== null && now < this.until) return this.code
    return code
  }

  reset(): void {
    this.code = null
    this.until = 0
  }
}
