// SPDX-License-Identifier: GPL-3.0-or-later
/// <reference types="vite/client" />
declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<object, object, unknown>
  export default component
}

interface ImportMetaEnv {
  /** Build-time default relay, e.g. wss://relay.cometcast.de/ws (see src/core/relay.ts). */
  readonly VITE_DEFAULT_RELAY?: string
  /** TEST-ONLY, build time: shortens the idle-dim delay for end-to-end builds (src/core/idleDim.ts). */
  readonly VITE_E2E_DIM_MS?: string
}
