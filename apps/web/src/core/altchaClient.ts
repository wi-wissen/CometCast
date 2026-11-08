// SPDX-License-Identifier: GPL-3.0-or-later
//
// HTTP side of the server's extras: GET /altcha/challenge (solved here, see altcha.ts) and
// GET /ice-servers (STUN/TURN list). Both are described in docs/server.md.
import {
  AltchaError,
  checkChallengeLimits,
  DEFAULT_SOLVE_TIMEOUT_MS,
  encodePayload,
  parseChallenge,
  solveChallenge,
  type AltchaChallenge,
  type AltchaSolution,
} from './altcha'
import type { WorkerReply, WorkerRequest } from './altchaWorker'

/** Error with a code that has a translation under `errors.<code>`. */
export class ServerHttpError extends Error {
  constructor(
    /** rate_limited | banned | forbidden | server_full | altcha_failed | altcha_too_hard | altcha_expired */
    readonly code: string,
  ) {
    super(code)
    this.name = 'ServerHttpError'
  }
}

/** http(s) origin that belongs to a signalling URL (`ws://host/ws` -> `http://host`). */
export function httpBaseUrl(wsUrl: string): string {
  const u = new URL(wsUrl)
  u.protocol = u.protocol === 'wss:' ? 'https:' : 'http:'
  return u.origin
}

/** Maps a non-200 answer to the same vocabulary as the signalling errors. */
export function httpErrorCode(status: number, retryAfter: string | null): string {
  if (status === 429) return 'rate_limited'
  // The server sends Retry-After with a ban and not with a plain "not allowed" (docs/server.md).
  if (status === 403) return retryAfter ? 'banned' : 'forbidden'
  if (status === 503) return 'server_full'
  return 'altcha_failed'
}

export type SolveFn = (challenge: AltchaChallenge, opts: { signal?: AbortSignal; timeoutMs?: number }) => Promise<AltchaSolution>

/**
 * Solves in a module Web Worker. If workers are not available (or cannot be created) the same
 * code runs on the main thread, which still works but may make the page less responsive.
 */
export const solveInWorker: SolveFn = (challenge, opts) => {
  if (typeof Worker === 'undefined') return solveChallenge(challenge, opts)
  let worker: Worker
  try {
    worker = new Worker(new URL('./altchaWorker.ts', import.meta.url), { type: 'module' })
  } catch {
    return solveChallenge(challenge, opts)
  }
  return new Promise<AltchaSolution>((resolve, reject) => {
    const done = () => {
      opts.signal?.removeEventListener('abort', onAbort)
      worker.terminate()
    }
    const onAbort = () => {
      done()
      reject(new AltchaError('aborted'))
    }
    if (opts.signal?.aborted) return onAbort()
    opts.signal?.addEventListener('abort', onAbort)
    worker.onmessage = (ev: MessageEvent<WorkerReply>) => {
      done()
      if (ev.data.ok) resolve(ev.data.solution)
      else reject(new AltchaError(ev.data.reason))
    }
    worker.onerror = () => {
      done()
      reject(new AltchaError('worker_failed'))
    }
    const req: WorkerRequest = { challenge, timeoutMs: opts.timeoutMs ?? DEFAULT_SOLVE_TIMEOUT_MS }
    worker.postMessage(req)
  })
}

export interface HttpDeps {
  fetchImpl?: typeof fetch
  signal?: AbortSignal
  solve?: SolveFn
  /** Timeout of the GET /altcha/challenge request (not of solving). */
  fetchTimeoutMs?: number
}

/** UNCERTAIN: 8 s is a placeholder; a slow answer must not hold the page in "checking" for long. */
const CHALLENGE_FETCH_TIMEOUT_MS = 8000

/** Maps an AltchaError reason from the limit check or the solver to a translated error code. */
function altchaErrorCode(e: unknown): string {
  if (e instanceof AltchaError) {
    if (e.reason === 'too_hard') return 'altcha_too_hard'
    if (e.reason === 'expired') return 'altcha_expired'
  }
  return 'altcha_failed'
}

/**
 * Fetches a puzzle, solves it and returns the text for create_room.altcha.
 * Throws ServerHttpError (rate_limited, banned, forbidden, server_full, altcha_failed) or AltchaError (aborted).
 */
export async function obtainAltchaPayload(baseUrl: string, deps: HttpDeps = {}): Promise<string> {
  const doFetch = deps.fetchImpl ?? fetch
  // The timeout covers the whole request including reading the body.
  const ctl = new AbortController()
  const onAbort = () => ctl.abort()
  deps.signal?.addEventListener('abort', onAbort)
  if (deps.signal?.aborted) ctl.abort()
  const timer = setTimeout(() => ctl.abort(), deps.fetchTimeoutMs ?? CHALLENGE_FETCH_TIMEOUT_MS)
  let challenge: AltchaChallenge
  try {
    let res: Response
    try {
      res = await doFetch(`${baseUrl}/altcha/challenge`, { cache: 'no-store', signal: ctl.signal })
    } catch {
      if (deps.signal?.aborted) throw new AltchaError('aborted')
      throw new ServerHttpError('altcha_failed')
    }
    if (!res.ok) throw new ServerHttpError(httpErrorCode(res.status, res.headers.get('retry-after')))
    try {
      challenge = parseChallenge(await res.json())
    } catch {
      if (deps.signal?.aborted) throw new AltchaError('aborted')
      throw new ServerHttpError('altcha_failed')
    }
  } finally {
    clearTimeout(timer)
    deps.signal?.removeEventListener('abort', onAbort)
  }
  // Refuse absurd puzzles before any solver (worker or main thread) starts.
  try {
    checkChallengeLimits(challenge)
  } catch (e) {
    throw new ServerHttpError(altchaErrorCode(e))
  }
  try {
    const solution = await (deps.solve ?? solveInWorker)(challenge, { signal: deps.signal })
    return encodePayload(challenge, solution)
  } catch (e) {
    if (e instanceof AltchaError && e.reason === 'aborted') throw e
    throw new ServerHttpError(altchaErrorCode(e))
  }
}

function isStringArray(v: unknown): v is string[] {
  return Array.isArray(v) && v.length > 0 && v.every((x) => typeof x === 'string')
}

/** UNCERTAIN: placeholders, the same as the native client (crates/cc-transport/src/ice.rs). */
export const MAX_ICE_SERVERS = 16
export const MAX_ICE_URLS = 8
export const MAX_ICE_FIELD = 512

/**
 * The URI shapes RTCPeerConnection accepts (RFC 7064, RFC 7065): `stun[s]:host[:port]` and
 * `turn[s]:host[:port][?transport=udp|tcp]`. Anything else (a query on a STUN URI, a port that is
 * not a number) makes the constructor throw for the whole list, so it is dropped here.
 */
const ICE_HOST = String.raw`(?:\[[0-9a-fA-F:.]+\]|[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?)`
const ICE_PORT = String.raw`(?::[0-9]{1,5})?`
const STUN_URL = new RegExp(`^stuns?:${ICE_HOST}${ICE_PORT}$`, 'i')
const TURN_URL = new RegExp(`^turns?:${ICE_HOST}${ICE_PORT}(?:\\?transport=(?:udp|tcp))?$`, 'i')
const urlIsSane = (u: string) => u.length <= MAX_ICE_FIELD && (STUN_URL.test(u) || TURN_URL.test(u))

/**
 * Keeps only the RTCIceServer fields we use; drops entries that cannot be valid. The list is
 * bounded (MAX_ICE_SERVERS entries, MAX_ICE_URLS URLs each, short fields, stun/turn schemes).
 */
export function parseIceServers(v: unknown): RTCIceServer[] {
  if (!Array.isArray(v)) return []
  const out: RTCIceServer[] = []
  for (const e of v.slice(0, MAX_ICE_SERVERS * 4)) {
    if (out.length >= MAX_ICE_SERVERS) break
    if (e === null || typeof e !== 'object') continue
    const o = e as Record<string, unknown>
    const rawUrls = typeof o.urls === 'string' ? [o.urls] : o.urls
    if (!isStringArray(rawUrls)) continue
    const urls = rawUrls.filter(urlIsSane).slice(0, MAX_ICE_URLS)
    if (urls.length === 0) continue
    const s: RTCIceServer = { urls }
    if (typeof o.username === 'string') s.username = o.username
    if (typeof o.credential === 'string') s.credential = o.credential
    if ((s.username?.length ?? 0) > MAX_ICE_FIELD || ((s.credential as string | undefined)?.length ?? 0) > MAX_ICE_FIELD) continue
    // A TURN entry without credentials would make RTCPeerConnection throw for the whole list.
    const needsAuth = urls.some((u) => /^turns?:/i.test(u))
    if (needsAuth && (!s.username || !s.credential)) continue
    out.push(s)
  }
  return out
}

/** UNCERTAIN: 5 s is a placeholder; a slow answer must not hold up the connection for long. */
const ICE_FETCH_TIMEOUT_MS = 5000

/**
 * How a `GET /ice-servers` call ended. Typed so that callers and tests can tell "this relay offers
 * no list" (`none`, normal) from "the relay refused us" (`rate_limited`, `refused`) and from a
 * broken answer or network. Every state except `ok` yields an empty list, so a connection that could
 * work directly is never stopped, but only `none` is silent: the others are logged by
 * {@link fetchIceServers}. The sessions keep the last state; only when the connection then fails the presenter's
 * page uses it (spec 6, decision 13.45): nothing is shown before that.
 */
export type IceFetchState =
  | { kind: 'ok' }
  | { kind: 'none' } // 404: no [stun]/[turn] configured
  | { kind: 'rate_limited'; retryAfterSecs?: number } // 429: the per-address /ice-servers budget is used up
  | { kind: 'refused'; retryAfterSecs?: number } // 403: address banned or not allowed
  | { kind: 'http_error'; status: number }
  | { kind: 'bad_answer' } // 200 but not a JSON list
  | { kind: 'unreachable'; timedOut: boolean }

export interface IceFetchResult {
  servers: RTCIceServer[]
  state: IceFetchState
}

function retryAfterOf(res: Response): number | undefined {
  const n = Number(res.headers.get('retry-after'))
  return Number.isFinite(n) && n > 0 ? n : undefined
}

/** GET /ice-servers with the outcome as a typed state (never rejects). */
export async function fetchIceServersDetailed(baseUrl: string, deps: { fetchImpl?: typeof fetch; timeoutMs?: number } = {}): Promise<IceFetchResult> {
  const doFetch = deps.fetchImpl ?? fetch
  const ctl = new AbortController()
  const timer = setTimeout(() => ctl.abort(), deps.timeoutMs ?? ICE_FETCH_TIMEOUT_MS)
  try {
    const res = await doFetch(`${baseUrl}/ice-servers`, { cache: 'no-store', signal: ctl.signal })
    if (res.status === 404) return { servers: [], state: { kind: 'none' } }
    if (res.status === 429) return { servers: [], state: { kind: 'rate_limited', retryAfterSecs: retryAfterOf(res) } }
    if (res.status === 403) return { servers: [], state: { kind: 'refused', retryAfterSecs: retryAfterOf(res) } }
    if (!res.ok) return { servers: [], state: { kind: 'http_error', status: res.status } }
    let body: unknown
    try {
      body = await res.json()
    } catch {
      return { servers: [], state: { kind: 'bad_answer' } }
    }
    return { servers: parseIceServers(body), state: { kind: 'ok' } }
  } catch {
    return { servers: [], state: { kind: 'unreachable', timedOut: ctl.signal.aborted } }
  } finally {
    clearTimeout(timer)
  }
}

/** One log line for a state that costs the client its ICE servers (nothing for `ok` and `none`). */
export function describeIceFetchState(state: IceFetchState): string | null {
  switch (state.kind) {
    case 'ok':
    case 'none':
      return null
    case 'rate_limited':
      return `rate limited (429${state.retryAfterSecs ? `, retry after ${state.retryAfterSecs} s` : ''}): connecting without STUN/TURN servers`
    case 'refused':
      return `refused (403${state.retryAfterSecs ? `, banned, retry after ${state.retryAfterSecs} s` : ''}): connecting without STUN/TURN servers`
    case 'http_error':
      return `unexpected HTTP status ${state.status}: connecting without STUN/TURN servers`
    case 'bad_answer':
      return 'the answer is not a JSON list: connecting without STUN/TURN servers'
    case 'unreachable':
      return `${state.timedOut ? 'timed out' : 'request failed'}: connecting without STUN/TURN servers`
  }
}

/**
 * GET /ice-servers. `404` (no list configured) and every failure mean "no list": the page then
 * connects without STUN/TURN, as before. A failure other than 404 is logged to the console
 * (`console.warn`) so that a 429 is not mistaken for a relay without STUN; see
 * {@link fetchIceServersDetailed} for the typed state; `onState` hands it to the caller.
 */
export async function fetchIceServers(
  baseUrl: string,
  deps: { fetchImpl?: typeof fetch; timeoutMs?: number; /** Gets how the fetch ended (the sessions keep it, see `iceState`). */ onState?: (s: IceFetchState) => void } = {},
): Promise<RTCIceServer[]> {
  const { servers, state } = await fetchIceServersDetailed(baseUrl, deps)
  deps.onState?.(state)
  const note = describeIceFetchState(state)
  if (note) console.warn(`[cometcast] GET /ice-servers ${note}`)
  return servers
}
