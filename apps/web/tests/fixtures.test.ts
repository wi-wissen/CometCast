// SPDX-License-Identifier: GPL-3.0-or-later
//
// The shared fixtures in <repository>/tests/fixtures checked against the web implementation. The same files are
// consumed by the Swift tests of the Apple TV receiver (apps/apple-tv/CometCastCore/Tests), so both
// implementations are held to the same vectors. `expect_apple_tv` (a documented, deliberately stricter result of the
// Swift code) is ignored here: the web code must always produce `expect`.
import { describe, expect, it, vi } from 'vitest'
import { parseControlMessage, validateLinkUrl, ControlReceiver, type ControlReceiverHandlers } from '../src/core/control'
import { classifyMediaUrl, hostIsLocalOrIp, shortHost } from '../src/core/media'
import { connectionCode, sdpFingerprint } from '../src/core/sas'
import { solveChallenge, checkChallengeLimits, encodePayload, type AltchaChallenge } from '../src/core/altcha'
import { parseRelay } from '../src/core/relay'
import { decodeServerMessage } from '../src/core/signaling'
import { parseIceServers, httpErrorCode } from '../src/core/altchaClient'
import { normalizeRoomCode, isValidRoomCode } from '../src/core/roomCode'
import controlParse from '../../../tests/fixtures/control-parse.json'
import linkUrl from '../../../tests/fixtures/link-url.json'
import media from '../../../tests/fixtures/media.json'
import sas from '../../../tests/fixtures/sas.json'
import altcha from '../../../tests/fixtures/altcha.json'
import relay from '../../../tests/fixtures/relay.json'
import signaling from '../../../tests/fixtures/signaling.json'
import iceServers from '../../../tests/fixtures/ice-servers.json'
import roomCode from '../../../tests/fixtures/room-code.json'
import contentScenarios from '../../../tests/fixtures/content-scenarios.json'

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Case = Record<string, any>
const files: Record<string, unknown> = {
  'control-parse.json': controlParse, 'link-url.json': linkUrl, 'media.json': media, 'sas.json': sas, 'altcha.json': altcha,
  'relay.json': relay, 'signaling.json': signaling, 'ice-servers.json': iceServers, 'room-code.json': roomCode,
  'content-scenarios.json': contentScenarios,
}
const load = (name: string): Case => files[name] as Case
type Part = string | { repeat: string; count: number }
const text = (c: Case): string => (c.input !== undefined ? c.input : (c.input_parts as Part[]).map((p) => (typeof p === 'string' ? p : p.repeat.repeat(p.count))).join(''))

describe('shared fixtures, web side', () => {
  it('control-parse', () => {
    const f = load('control-parse.json')
    expect(f.cases.length).toBeGreaterThan(100)
    for (const c of f.cases) expect(parseControlMessage(text(c)), c.name).toEqual(c.expect)
  })
  it('link-url', () => {
    const f = load('link-url.json')
    expect(f.cases.length).toBeGreaterThan(100)
    for (const c of f.cases) expect(validateLinkUrl(text(c)), c.name).toEqual(c.expect)
  })
  it('media', () => {
    const f = load('media.json')
    for (const c of f.classify) expect(classifyMediaUrl(c.url), c.url).toEqual(c.expect)
    for (const c of f.host_local_or_ip) expect(hostIsLocalOrIp(c.host), c.host).toEqual(c.expect)
    for (const c of f.short_host) expect(shortHost(c.host), 'short host').toEqual(c.expect)
  })
  it('sas', async () => {
    const f = load('sas.json')
    for (const c of f.code) expect(await connectionCode(c.local_sdp, c.remote_sdp), c.name).toEqual(c.expect)
    for (const c of f.fingerprint) expect(sdpFingerprint(c.sdp), c.name).toEqual(c.expect)
  })
  it('altcha', async () => {
    const f = load('altcha.json')
    for (const c of f.puzzles) {
      const s = await solveChallenge(c.challenge as AltchaChallenge, { timeoutMs: 0 })
      expect({ counter: s.counter, derivedKey: s.derivedKey }, c.name).toEqual(c.expect_solution)
      const payload = JSON.parse(atob(encodePayload(c.challenge, { ...s, time: 0 })))
      expect(payload, c.name).toEqual(c.expect_payload)
    }
    for (const c of f.limits) {
      let got = 'ok'
      try {
        checkChallengeLimits(c.challenge, c.now_seconds * 1000)
      } catch (e) {
        got = (e as { reason: string }).reason
      }
      expect(got, c.name).toBe(c.expect)
    }
    for (const c of f.solve_errors) {
      let got = 'solved'
      try {
        await solveChallenge(c.challenge as AltchaChallenge, { timeoutMs: 0 })
      } catch (e) {
        got = (e as { reason: string }).reason
      }
      expect(got, c.name).toBe(c.expect)
    }
    for (const c of f.http) expect(httpErrorCode(c.status, c.retry_after), `http ${c.status}`).toBe(c.expect)
  })
  it('relay', () => {
    for (const c of load('relay.json').cases) {
      const r = parseRelay(c.input, { strictWs: !!c.strict_ws })
      expect(r.ok ? { ok: true, ...r.relay } : { ok: false, error: r.error }, c.input).toEqual(c.expect)
    }
  })
  it('signaling', () => {
    for (const c of load('signaling.json').server_messages) expect(decodeServerMessage(c.input), c.input).toEqual(c.expect)
  })
  it('ice-servers', () => {
    for (const c of load('ice-servers.json').cases) {
      let v: unknown = null
      try {
        v = JSON.parse(c.input)
      } catch {
        v = null
      }
      expect(parseIceServers(v), c.input.slice(0, 80)).toEqual(c.expect)
    }
  })
  it('room-code', () => {
    for (const c of load('room-code.json').cases) {
      const n = normalizeRoomCode(c.input)
      expect(n, c.input).toBe(c.normalized)
      expect(isValidRoomCode(c.input)).toBe(c.valid_input)
      expect(isValidRoomCode(n)).toBe(c.valid_normalized)
    }
  })
  it('content scenarios', () => {
    const f = load('content-scenarios.json')
    expect(f.scenarios.length).toBeGreaterThan(10)
    for (const s of f.scenarios) {
      vi.useFakeTimers()
      vi.setSystemTime(0)
      const events: string[] = []
      const h: ControlReceiverHandlers = {
        onCmd: (c) => events.push('cmd:' + JSON.stringify(c)),
        onProgress: (p) => events.push('progress:' + (p ? `${p.name}:${p.received}:${p.size}` : 'null')),
        onImage: (blob, name) => events.push(`image:${name}:${blob.size}`),
        onDocument: (bytes, name) => events.push(`document:${name}:${bytes.length}:${bytes.reduce((a, b) => (a + b) % 65521, 0)}`),
        onLink: (u) => events.push('link:' + u),
        onError: (c) => events.push('error:' + c),
      }
      let now = 0
      const rx = new ControlReceiver(h, () => now)
      let cur = 0
      for (const op of s.ops) {
        vi.advanceTimersByTime(op.t - cur)
        cur = op.t
        now = op.t
        if (op.text !== undefined) rx.handle(op.text)
        else if (op.bin !== undefined) rx.handle(Uint8Array.from({ length: op.bin }, (_, i) => (i * 31 + 7) % 251).buffer)
      }
      vi.useRealTimers()
      expect(events, s.name).toEqual(s.events)
    }
  })
})
