// SPDX-License-Identifier: GPL-3.0-or-later
//
// Error codes of the WASM renderer (crates/cc-docs-wasm `code()`), and how the worker maps what it catches.

/** The codes that the wasm side throws on purpose (a short message of an Error). Keep in step with lib.rs. */
export const KNOWN_CODES: ReadonlySet<string> = new Set([
  'invalid',
  'encrypted',
  'too_large',
  'too_many_pages',
  'page_out_of_range',
  'bad_size',
  'rejected',
  'memory',
  'timeout',
  'failed',
])

/**
 * Accepts only known codes. Anything else is a trap or a bug inside the module (`unreachable`, an
 * out-of-bounds access, an allocation that hit the memory cap, a JS error): the wasm instance may be
 * poisoned then, so it becomes `memory` (for an out-of-memory looking trap) or `failed`. The renderer ends the
 * worker and closes the document for both.
 */
export function codeOf(e: unknown): string {
  const m = e instanceof Error ? e.message : ''
  if (KNOWN_CODES.has(m)) return m
  return /memory|out of bounds|unreachable|alloc/i.test(m) ? 'memory' : 'failed'
}
