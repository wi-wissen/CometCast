// SPDX-License-Identifier: GPL-3.0-or-later
//
// A join request that has just appeared must not be answered by an accidental key press or click
// (for example an OK press that was meant for something else): its buttons ignore input for a
// short time after the request arrived.

/** Milliseconds after which the buttons of a new request react. UNCERTAIN: 700 ms is a placeholder. */
export const REQUEST_GUARD_MS = 700

/** Is the request old enough to be answered? */
export function isArmed(receivedAt: number, now: number): boolean {
  return now - receivedAt >= REQUEST_GUARD_MS
}
