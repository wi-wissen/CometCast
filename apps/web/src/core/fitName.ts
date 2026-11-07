// SPDX-License-Identifier: GPL-3.0-or-later
// A file name shortened in the middle with "…", keeping its start and its extension (spec 3.5, decision 13.63):
// `holiday-photos-from-2026.png` -> `holiday-pho…2026.png`. `keep` is the number of name characters that stay
// (the "…" comes on top); a name that already fits is returned unchanged.
export function shortenMiddle(name: string, keep: number): string {
  const chars = Array.from(name)
  if (keep >= chars.length) return name
  const n = Math.max(4, keep)
  const dot = name.lastIndexOf('.')
  const extLen = dot > 0 && name.length - dot <= 8 ? Array.from(name.slice(dot)).length : 0
  // The tail holds the extension and up to 4 characters before it; the head keeps at least 3.
  const tail = Math.min(extLen + 4, n - 3)
  const head = n - tail
  return chars.slice(0, head).join('') + '…' + chars.slice(chars.length - tail).join('')
}
