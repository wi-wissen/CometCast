// SPDX-License-Identifier: GPL-3.0-or-later
//
// The relay in use, shared by the whole app. `initRelay()` runs once before the app is mounted
// (it reads the optional config.json); until then the state is the same as without that file.
import { computed, ref, shallowRef } from 'vue'
import {
  loadRelayConfig,
  needsRelayConfirmation,
  parseRelay,
  resolveRelay,
  type Relay,
  type RelayChoice,
  type RelayErrorCode,
} from '../core/relay'

const STORAGE_KEY = 'cometcast.relay'
/** The one `?server=` relay the person confirmed in this tab (exact wss:// URL). */
const CONFIRMED_KEY = 'cometcast.relayConfirmed'

function readStored(key = STORAGE_KEY): string | null {
  try {
    return sessionStorage.getItem(key)
  } catch {
    return null
  }
}
function writeStored(v: string | null, key = STORAGE_KEY): void {
  try {
    if (v === null) sessionStorage.removeItem(key)
    else sessionStorage.setItem(key, v)
  } catch {
    /* storage can be blocked; the choice then lasts until the page is reloaded */
  }
}

let configRelays: Relay[] = []
const configFailed = ref(false)
let confirmedWs: string | null = null
interface PendingConfirm {
  relay: Relay
  done: Promise<boolean>
  settle: (ok: boolean) => void
}
const pending = shallowRef<PendingConfirm | null>(null)
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
  const cfg = await loadRelayConfig({ pageProtocol: location.protocol, ...deps })
  configRelays = cfg.relays
  configFailed.value = cfg.failed
  confirmedWs = readStored(CONFIRMED_KEY)
  // A `?server=` in the link is explicit and newer than anything remembered.
  userChoice = new URLSearchParams(location.search).has('server') ? null : readStored()
  state.value = compute()
}

/** Test helper: forget everything. */
export function resetRelayState(): void {
  pending.value?.settle(false)
  pending.value = null
  configRelays = []
  userChoice = null
  ignoreQuery = false
  configFailed.value = false
  confirmedWs = null
  writeStored(null)
  writeStored(null, CONFIRMED_KEY)
  state.value = compute()
}

/** True when the relay in use came from a link only and the person has not confirmed it yet. */
function untrusted(): boolean {
  const { current, choices } = state.value
  return needsRelayConfirmation(current, choices) && current.wsUrl !== confirmedWs
}

/**
 * Call before any connection or screen capture. Resolves true when the relay may be used: it is
 * trusted, or the person confirmed it in the dialog (remembered for this tab and this exact relay).
 * Resolves false when they cancelled; the relay is then the default again.
 */
export function ensureRelayTrusted(): Promise<boolean> {
  if (!untrusted()) return Promise.resolve(true)
  if (pending.value) return pending.value.done
  const relay = state.value.current
  let resolve!: (ok: boolean) => void
  const done = new Promise<boolean>((r) => (resolve = r))
  const settle = (ok: boolean) => {
    if (pending.value?.relay !== relay) return
    pending.value = null
    if (ok) {
      confirmedWs = relay.wsUrl
      writeStored(confirmedWs, CONFIRMED_KEY)
    } else {
      // Back to the default for this page view (also drops the ?server= of the link).
      userChoice = null
      ignoreQuery = true
      writeStored(null)
      state.value = compute()
    }
    resolve(ok)
  }
  pending.value = { relay, done, settle }
  return done
}

/** For the confirmation dialog: the relay waiting for a decision (or null) and the two answers. */
export function useRelayConfirmation() {
  return {
    relay: computed(() => pending.value?.relay ?? null),
    accept: () => pending.value?.settle(true),
    cancel: () => pending.value?.settle(false),
  }
}

export function useRelay() {
  return {
    current: computed(() => state.value.current),
    default: computed(() => state.value.default),
    choices: computed(() => state.value.choices),
    nonDefault: computed(() => state.value.nonDefault),
    /** config.json exists but could not be read or used (the start page says so). */
    configFailed: computed(() => configFailed.value),
    ensureTrusted: ensureRelayTrusted,
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
