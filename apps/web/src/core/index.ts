// SPDX-License-Identifier: GPL-3.0-or-later
//
// Placeholder for the Rust-WASM core (wasm-bindgen). Nothing is loaded here yet.
// UNCERTAIN: the shape of this API (init, room handling, transport) depends on
// the Rust core and cc-proto, which do not exist in this repo yet.

export interface CoreInfo {
  /** True once the WASM core is actually loaded. Always false in v0.1. */
  ready: boolean
  version: string
}

export async function initCore(): Promise<CoreInfo> {
  return { ready: false, version: '0.0.0-placeholder' }
}
