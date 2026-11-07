// SPDX-License-Identifier: GPL-3.0-or-later

/** Shows a join code in two groups separated by one space (4 + 4 on the default server in the spec; this protocol's code has 6 characters, so 3 + 3). */
export function formatCode(code: string): string {
  const c = code.replace(/\s+/g, '')
  if (c.length < 4) return c
  const half = Math.ceil(c.length / 2)
  return `${c.slice(0, half)} ${c.slice(half)}`
}
