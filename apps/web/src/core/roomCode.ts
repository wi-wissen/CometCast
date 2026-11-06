// SPDX-License-Identifier: GPL-3.0-or-later

// UNCERTAIN: final alphabet/length is defined in cc-proto
export const ROOM_CODE_ALPHABET = 'ABCDEFGHJKMNPQRSTUVWXYZ23456789'
export const ROOM_CODE_LENGTH = 6

/** Uppercase, drop characters outside the alphabet, cut to length. */
export function normalizeRoomCode(input: string): string {
  return [...input.toUpperCase()]
    .filter((c) => ROOM_CODE_ALPHABET.includes(c))
    .join('')
    .slice(0, ROOM_CODE_LENGTH)
}

export function isValidRoomCode(code: string): boolean {
  return code.length === ROOM_CODE_LENGTH && [...code].every((c) => ROOM_CODE_ALPHABET.includes(c))
}
