// SPDX-License-Identifier: GPL-3.0-or-later
//
// Focus ring rule of the display (docs/design/first-run-and-display.md, 7.2): on page load the initial focus shows
// the ring (the page cannot know yet whether a keyboard or a remote drives it). The first mouse or touch input hides
// it; the next key press shows it again. Pure state, so that it can be tested without a page.

export type FocusRingEvent = 'key' | 'pointer'

/** Is the ring hidden after this event? `key` shows it, `pointer` hides it. */
export function ringHidden(prevHidden: boolean, e: FocusRingEvent): boolean {
  return e === 'pointer' ? true : e === 'key' ? false : prevHidden
}
