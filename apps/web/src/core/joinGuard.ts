// SPDX-License-Identifier: GPL-3.0-or-later
//
// A join request that has just appeared must not be answered by an accidental key press or click
// (for example an OK press that was meant for something else): its buttons ignore input for one
// second after the dialog shows it (docs/design/first-run-and-display.md, 4.4). The guard starts
// again when the dialog switches to the next request.

/** Milliseconds after which the buttons of a request react (spec: 1 s). */
export const REQUEST_GUARD_MS = 1000

/** A request that nobody answers leaves the queue after this time (spec: 60 s; the server's own limit is the same). */
export const REQUEST_EXPIRY_MS = 60_000

/** Is the dialog old enough to be answered? `shownAt` = when this request became the one on screen. */
export function isArmed(shownAt: number, now: number): boolean {
  return now - shownAt >= REQUEST_GUARD_MS
}
