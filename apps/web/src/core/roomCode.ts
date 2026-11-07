// SPDX-License-Identifier: GPL-3.0-or-later

// Mirrors RoomCode::ALPHABET in crates/cc-proto (32 characters; no 0, O, 1, I).
export const ROOM_CODE_ALPHABET = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789'
export const ROOM_CODE_LENGTH = 6

/** Uppercase, drop characters outside the alphabet, cut to length. */
export function normalizeRoomCode(input: string): string {
  return [...input.toUpperCase()]
    .filter((c) => ROOM_CODE_ALPHABET.includes(c))
    .join('')
    .slice(0, ROOM_CODE_LENGTH)
}

/** Longest text the code field takes: a code (6) with a space or dash in the middle, and a little slack. */
export const CODE_FIELD_MAX = 9

/**
 * What the code field shows (spec 5.2): every typed character, in uppercase, at most 9 of them. Nothing is
 * dropped silently: an O, 0, I or 1 stays visible until the person submits.
 */
export function codeFieldText(input: string): string {
  return Array.from(input.toUpperCase()).slice(0, CODE_FIELD_MAX).join('')
}

/** On submit: spaces and dashes are removed (and the text is uppercased). The result is checked with {@link isValidRoomCode}. */
export function codeFromField(input: string): string {
  return input.toUpperCase().replace(/[\s-]+/g, '')
}

export function isValidRoomCode(code: string): boolean {
  return code.length === ROOM_CODE_LENGTH && [...code].every((c) => ROOM_CODE_ALPHABET.includes(c))
}
