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

const urlIsSane = (u: string) => u.length <= MAX_ICE_FIELD && /^(stuns?|turns?):/i.test(u) && !/[\u0000-\u001f\u007f]/.test(u)

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
 * GET /ice-servers. `404` (no TURN configured) and every failure mean "no list": the page then
 * connects without STUN/TURN, as before.
 */
export async function fetchIceServers(baseUrl: string, deps: { fetchImpl?: typeof fetch; timeoutMs?: number } = {}): Promise<RTCIceServer[]> {
  const doFetch = deps.fetchImpl ?? fetch
  const ctl = new AbortController()
  const timer = setTimeout(() => ctl.abort(), deps.timeoutMs ?? ICE_FETCH_TIMEOUT_MS)
  try {
    const res = await doFetch(`${baseUrl}/ice-servers`, { cache: 'no-store', signal: ctl.signal })
    if (!res.ok) return []
    return parseIceServers(await res.json())
  } catch {
    return []
  } finally {
    clearTimeout(timer)
  }
}
