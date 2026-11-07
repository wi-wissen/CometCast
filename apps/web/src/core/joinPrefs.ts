// SPDX-License-Identifier: GPL-3.0-or-later
//
// Per-browser conveniences for join approval: the name that a presenter uses when joining. localStorage can be missing or throw (private windows,
// blocked site data), so every access is wrapped; the app works without it.

export const DISPLAY_NAME_KEY = 'cometcast.displayName'
export const MAX_NAME_CHARS = 40

/** Removes control, bidi and invisible characters and cuts the name (the server does this too). */
export function cleanName(raw: string): string {
  // eslint-disable-next-line no-control-regex
  const cleaned = raw.replace(/[\t\n\r]/g, ' ').replace(/[\u0000-\u001f\u007f-\u009f\u061c\u200b-\u200f\u2028-\u202e\u2060-\u206f\ufeff\ue000-\uf8ff]/g, '')
  return Array.from(cleaned.replace(/\s+/g, ' ').trim()).slice(0, MAX_NAME_CHARS).join('')
}

export function loadDisplayName(): string {
  try {
    return cleanName(localStorage.getItem(DISPLAY_NAME_KEY) ?? '')
  } catch {
    return ''
  }
}

export function saveDisplayName(name: string): void {
  try {
    localStorage.setItem(DISPLAY_NAME_KEY, cleanName(name))
  } catch {
    /* not remembered */
  }
}

/** Shortens a name for a narrow card ("Anna-Maria Schmid" -> "Anna-Maria Sch…"). Plain text only. */
export function shortName(name: string, max = 18): string {
  const chars = Array.from(name)
  return chars.length <= max ? name : `${chars.slice(0, max).join('')}…`
}
