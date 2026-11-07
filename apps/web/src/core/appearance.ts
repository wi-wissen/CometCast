// SPDX-License-Identifier: GPL-3.0-or-later
//
// Display appearance (docs/design/first-run-and-display.md, 3.2). Dark is the default (D3).
// "auto" follows the system setting and never picks OLED black. The choice is kept on the display
// device only (localStorage; every access is wrapped because storage can be missing or throw).

export type Appearance = 'dark' | 'light' | 'oled' | 'auto'
export type Theme = 'dark' | 'light' | 'oled'
export const APPEARANCES: readonly Appearance[] = ['dark', 'light', 'oled', 'auto']
export const DEFAULT_APPEARANCE: Appearance = 'dark'
export const APPEARANCE_KEY = 'cometcast.display.appearance'

export const isAppearance = (v: unknown): v is Appearance => typeof v === 'string' && (APPEARANCES as readonly string[]).includes(v)

export function loadAppearance(): Appearance {
  try {
    const v = localStorage.getItem(APPEARANCE_KEY)
    return isAppearance(v) ? v : DEFAULT_APPEARANCE
  } catch {
    return DEFAULT_APPEARANCE
  }
}

export function saveAppearance(a: Appearance): void {
  try {
    localStorage.setItem(APPEARANCE_KEY, a)
  } catch {
    /* not remembered */
  }
}

/** Automatic: dark system -> Dark, light system -> Light. Never OLED black. */
export function resolveAppearance(a: Appearance, systemDark: boolean): Theme {
  return a === 'auto' ? (systemDark ? 'dark' : 'light') : a
}
