// SPDX-License-Identifier: GPL-3.0-or-later
//
// The relay in use, shared by the whole app. `initRelay()` runs once before the app is mounted
// (it reads the optional config.json); until then the state is the same as without that file.
import { computed, shallowRef } from 'vue'
import { loadRelayConfig, parseRelay, resolveRelay, type RelayChoice, type RelayErrorCode } from '../core/relay'

const STORAGE_KEY = 'cometcast.relay'

function readStored(): string | null {
  try {
    return sessionStorage.getItem(STORAGE_KEY)
  } catch {
    return null
  }
}
function writeStored(v: string | null): void {
  try {
    if (v === null) sessionStorage.removeItem(STORAGE_KEY)
    else sessionStorage.setItem(STORAGE_KEY, v)
  } catch {
    /* storage can be blocked; the choice then lasts until the page is reloaded */
  }
}

let configRelays: Awaited<ReturnType<typeof loadRelayConfig>> = []
let userChoice: string | null = null
/** After "use the default" the `?server=` of the link no longer counts for this page view. */
let ignoreQuery = false

function compute(): RelayChoice {
  return resolveRelay({
    loc: ignoreQuery ? { ...location, search: '' } : location,
    configRelays,
    buildDefault: import.meta.env.VITE_DEFAULT_RELAY,
    userChoice,
  })
}

const state = shallowRef<RelayChoice>(compute())

/** Reads config.json (short timeout) and the choice kept for this tab. Never throws. */
export async function initRelay(deps: { fetchImpl?: typeof fetch; base?: string } = {}): Promise<void> {
  configRelays = await loadRelayConfig(deps)
  // A `?server=` in the link is explicit and newer than anything remembered.
  userChoice = new URLSearchParams(location.search).has('server') ? null : readStored()
  state.value = compute()
}

/** Test helper: forget everything. */
export function resetRelayState(): void {
  configRelays = []
  userChoice = null
  ignoreQuery = false
  writeStored(null)
  state.value = compute()
}

export function useRelay() {
  return {
    current: computed(() => state.value.current),
    default: computed(() => state.value.default),
    choices: computed(() => state.value.choices),
    nonDefault: computed(() => state.value.nonDefault),
    /** Use the relay in `text`. Returns an error code, or null when it was accepted. */
    set(text: string): RelayErrorCode | null {
      const p = parseRelay(text, { pageProtocol: location.protocol })
      if (!p.ok) return p.error
      userChoice = p.relay.wsUrl
      ignoreQuery = false
      writeStored(userChoice)
      state.value = compute()
      return null
    },
    /** Back to the default (also drops a `?server=` of the link for this page view). */
    reset(): void {
      userChoice = null
      ignoreQuery = true
      writeStored(null)
      state.value = compute()
    },
  }
}
