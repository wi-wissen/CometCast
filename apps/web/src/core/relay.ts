// SPDX-License-Identifier: GPL-3.0-or-later
//
// Which relay (signalling server) the app talks to. The web app is static files that can be
// served from anywhere, so the relay is not always the page's own origin.
//
// Precedence (first that gives a valid relay wins):
//   1. a relay chosen in the UI in this tab (sessionStorage), or `?server=` in the page URL
//   2. `config.json` next to index.html (optional, runtime, see parseRelayConfig)
//   3. the build-time default `VITE_DEFAULT_RELAY`
//   4. the page's own origin (the relay serves the page itself: Docker image, `--web-dir`)
// Everything is validated here; nothing from a URL, a file or the user is ever executed.

export type RelaySource = 'user' | 'query' | 'config' | 'build' | 'origin'

export interface Relay {
  /** ws:// or wss:// URL of the signalling endpoint, always ending in `/ws`. */
  wsUrl: string
  /** http(s) origin of the same server (ALTCHA and /ice-servers requests go here). */
  httpBase: string
  /** host[:port] for display. Never the path or anything else. */
  host: string
  /** ws:// to a local/private address (no TLS). */
  insecure: boolean
  source: RelaySource
}

export type RelayErrorCode = 'empty' | 'invalid' | 'insecure' | 'credentials' | 'path' | 'mixed'

export type ParsedRelay = { ok: true; relay: Omit<Relay, 'source'> } | { ok: false; error: RelayErrorCode }

const MAX_INPUT = 300

/** localhost, single-label names, .local/.lan/.home.arpa and private/loopback/link-local addresses. */
export function isLocalOrPrivateHost(hostname: string): boolean {
  const h = hostname.toLowerCase().replace(/^\[|\]$/g, '')
  if (h === 'localhost' || h.endsWith('.localhost') || h.endsWith('.local') || h.endsWith('.lan') || h.endsWith('.home.arpa')) return true
  const v4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(h)
  if (v4) {
    const [a, b] = [Number(v4[1]), Number(v4[2])]
    return a === 127 || a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168) || (a === 169 && b === 254)
  }
  if (h.includes(':')) return h === '::1' || /^f[cd][0-9a-f]{2}:/.test(h) || /^fe[89ab][0-9a-f]:/.test(h)
  // A name without a dot is an intranet name (never a public DNS name).
  return !h.includes('.')
}

/**
 * Validates and normalises a relay given by a person, a link or a file.
 * - `wss://host[:port][/ws]` always; `ws://` only for local/private hosts (LAN, development).
 * - A bare `host[:port]` means wss, except for local/private hosts where it follows the page
 *   (`pageProtocol`, default http: -> ws).
 * - `https://` and `http://` are accepted as spellings of wss/ws unless `strictWs`.
 * - No user info, query or fragment; the path may only be empty or `/ws`.
 */
export function parseRelay(input: string, opts: { strictWs?: boolean; pageProtocol?: string } = {}): ParsedRelay {
  const text = input.trim()
  if (!text) return { ok: false, error: 'empty' }
  if (text.length > MAX_INPUT || /[\s\u0000-\u001f\u007f\\]/.test(text)) return { ok: false, error: 'invalid' }
  let withScheme = text
  if (!/^[A-Za-z][A-Za-z0-9+.-]*:\/\//.test(text)) {
    if (opts.strictWs) return { ok: false, error: 'invalid' }
    // host[:port] or [v6][:port]; anything with a path/query is not a bare host.
    if (!/^(\[[0-9A-Fa-f:.]+\]|[A-Za-z0-9.-]+)(:\d{1,5})?$/.test(text)) return { ok: false, error: 'invalid' }
    const hostPart = text.startsWith('[') ? text.slice(0, text.indexOf(']') + 1) : text.replace(/:\d+$/, '')
    const plain = isLocalOrPrivateHost(hostPart) && opts.pageProtocol !== 'https:'
    withScheme = `${plain ? 'ws' : 'wss'}://${text}`
  }
  let u: URL
  try {
    u = new URL(withScheme)
  } catch {
    return { ok: false, error: 'invalid' }
  }
  let secure: boolean
  switch (u.protocol) {
    case 'wss:':
      secure = true
      break
    case 'ws:':
      secure = false
      break
    case 'https:':
    case 'http:':
      if (opts.strictWs) return { ok: false, error: 'invalid' }
      secure = u.protocol === 'https:'
      break
    default:
      return { ok: false, error: 'invalid' }
  }
  if (u.username || u.password) return { ok: false, error: 'credentials' }
  if (u.search || u.hash || withScheme.includes('?') || withScheme.includes('#')) return { ok: false, error: 'path' }
  if (!['', '/', '/ws', '/ws/'].includes(u.pathname)) return { ok: false, error: 'path' }
  if (!u.hostname || u.hostname === '[]') return { ok: false, error: 'invalid' }
  if (!secure && !isLocalOrPrivateHost(u.hostname)) return { ok: false, error: 'insecure' }
  // An https page may not open ws:// (mixed content): the browser would block it anyway.
  if (!secure && opts.pageProtocol === 'https:') return { ok: false, error: 'mixed' }
  // URL drops default ports (wss:443), which is what we want for display and for comparing.
  const wsScheme = secure ? 'wss:' : 'ws:'
  const httpScheme = secure ? 'https:' : 'http:'
  return {
    ok: true,
    relay: {
      wsUrl: `${wsScheme}//${u.host}/ws`,
      httpBase: `${httpScheme}//${u.host}`,
      host: u.host,
      insecure: !secure,
    },
  }
}

/** The relay of the page's own origin (the server that serves the page). */
export function sameOriginRelay(loc: { protocol: string; host: string }): Relay {
  const secure = loc.protocol === 'https:'
  return {
    wsUrl: `${secure ? 'wss:' : 'ws:'}//${loc.host}/ws`,
    httpBase: `${secure ? 'https:' : 'http:'}//${loc.host}`,
    host: loc.host,
    insecure: !secure,
    source: 'origin',
  }
}

const MAX_CONFIG_BYTES = 8192
export const MAX_CONFIG_RELAYS = 8

/**
 * Reads the text of `config.json`. Only this shape is understood:
 *   { "relays": ["wss://relay.example.org/ws", ...] }   (or { "relay": "wss://..." })
 * Entries must be ws/wss URLs (ws only for local/private hosts); invalid ones are dropped, at
 * most MAX_CONFIG_RELAYS are kept, duplicates removed. The first is the default. Anything else
 * in the file is ignored. The text is only parsed as JSON, never executed.
 */
export function parseRelayConfig(text: string, pageProtocol?: string): Relay[] {
  if (typeof text !== 'string' || text.length > MAX_CONFIG_BYTES) return []
  let v: unknown
  try {
    v = JSON.parse(text)
  } catch {
    return []
  }
  if (v === null || typeof v !== 'object' || Array.isArray(v)) return []
  const o = v as Record<string, unknown>
  const raw = Array.isArray(o.relays) ? o.relays : typeof o.relay === 'string' ? [o.relay] : []
  const out: Relay[] = []
  for (const e of raw.slice(0, MAX_CONFIG_RELAYS * 4)) {
    if (typeof e !== 'string') continue
    const p = parseRelay(e, { strictWs: true, pageProtocol })
    if (!p.ok || out.some((r) => r.wsUrl === p.relay.wsUrl)) continue
    out.push({ ...p.relay, source: 'config' })
    if (out.length >= MAX_CONFIG_RELAYS) break
  }
  return out
}

export interface RelayConfigResult {
  relays: Relay[]
  /** The file exists (or the request went wrong) but could not be used: the page tells the person. */
  failed: boolean
}

/** Reads at most `max` bytes of a response body as text; `null` when it is longer. */
async function readCapped(res: Response, max: number): Promise<string | null> {
  if (!res.body) {
    const t = await res.text()
    return t.length > max ? null : t
  }
  const reader = res.body.getReader()
  const chunks: Uint8Array[] = []
  let total = 0
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    total += value.byteLength
    if (total > max) {
      void reader.cancel().catch(() => undefined)
      return null
    }
    chunks.push(value)
  }
  const all = new Uint8Array(total)
  let at = 0
  for (const c of chunks) {
    all.set(c, at)
    at += c.byteLength
  }
  return new TextDecoder().decode(all)
}

/**
 * Fetches `config.json` from the app's base path. No file (404, or an HTML page from a
 * catch-all) is normal and not a failure. A timeout, a network error, another HTTP error, a body
 * over 8 KiB (reading stops there), invalid JSON or a file without any usable relay is `failed`.
 */
export async function loadRelayConfig(
  deps: { fetchImpl?: typeof fetch; base?: string; timeoutMs?: number; pageProtocol?: string } = {},
): Promise<RelayConfigResult> {
  const doFetch = deps.fetchImpl ?? fetch
  const base = deps.base ?? import.meta.env.BASE_URL
  const ctl = new AbortController()
  const timer = setTimeout(() => ctl.abort(), deps.timeoutMs ?? 1500)
  try {
    const res = await doFetch(`${base}config.json`, { cache: 'no-cache', credentials: 'omit', signal: ctl.signal })
    if (res.status === 404 || res.status === 410) return { relays: [], failed: false }
    if (!res.ok) return { relays: [], failed: true }
    if ((res.headers.get('content-type') ?? '').toLowerCase().includes('text/html')) return { relays: [], failed: false }
    const text = await readCapped(res, MAX_CONFIG_BYTES)
    if (text === null) return { relays: [], failed: true }
    const relays = parseRelayConfig(text, deps.pageProtocol)
    return { relays, failed: relays.length === 0 }
  } catch {
    return { relays: [], failed: true }
  } finally {
    clearTimeout(timer)
  }
}

/**
 * A relay that only a link (`?server=`) asked for must be confirmed by the person before any
 * connection or capture. Relays from config.json and the build default (all in `choices`), the
 * page's own origin, and one typed or picked in the relay field are trusted.
 */
export function needsRelayConfirmation(current: Relay, choices: Relay[]): boolean {
  return current.source === 'query' && !choices.some((c) => c.wsUrl === current.wsUrl)
}

export interface RelayInputs {
  loc: { protocol: string; host: string; search: string }
  /** Relays from config.json (first = default). */
  configRelays?: Relay[]
  /** VITE_DEFAULT_RELAY (raw text; invalid text is ignored). */
  buildDefault?: string
  /** Relay chosen in the UI of this tab (raw text). */
  userChoice?: string | null
}

export interface RelayChoice {
  /** The relay in use. */
  current: Relay
  /** What would be used without `?server=` and without a choice in the UI. */
  default: Relay
  /** Every relay offered in the UI (default first, then the other config.json entries). */
  choices: Relay[]
  /** current is not the default: the page shows a warning. */
  nonDefault: boolean
}

/** Applies the precedence described at the top of this file. Pure. */
export function resolveRelay(inp: RelayInputs): RelayChoice {
  const pageProtocol = inp.loc.protocol
  const configRelays = inp.configRelays ?? []
  let def: Relay
  if (configRelays.length > 0) {
    def = configRelays[0]
  } else {
    const b = inp.buildDefault ? parseRelay(inp.buildDefault, { strictWs: true }) : null
    def = b?.ok ? { ...b.relay, source: 'build' } : sameOriginRelay(inp.loc)
  }
  const choices = [def, ...configRelays.slice(1)]
  let current = def
  const q = new URLSearchParams(inp.loc.search).get('server')
  for (const [text, source] of [
    [inp.userChoice, 'user'],
    [q, 'query'],
  ] as const) {
    if (!text) continue
    const p = parseRelay(text, { pageProtocol })
    if (p.ok) {
      current = { ...p.relay, source }
      break
    }
  }
  return { current, default: def, choices, nonDefault: current.wsUrl !== def.wsUrl }
}
