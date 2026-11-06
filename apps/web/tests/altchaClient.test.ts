// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import {
  fetchIceServers,
  httpBaseUrl,
  httpErrorCode,
  obtainAltchaPayload,
  parseIceServers,
  ServerHttpError,
} from '../src/core/altchaClient'
import { AltchaError, type AltchaChallenge } from '../src/core/altcha'

const challenge: AltchaChallenge = {
  parameters: { algorithm: 'PBKDF2/SHA-256', cost: 1, keyLength: 32, keyPrefix: '00', nonce: '00', salt: '00' },
  signature: 'sig',
}

const json = (body: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' }, ...init })

describe('httpBaseUrl', () => {
  it('maps the signalling URL to the http(s) origin', () => {
    expect(httpBaseUrl('ws://127.0.0.1:8443/ws')).toBe('http://127.0.0.1:8443')
    expect(httpBaseUrl('wss://cast.example.org/ws')).toBe('https://cast.example.org')
  })
})

describe('obtainAltchaPayload', () => {
  it('fetches the challenge, solves it and returns the encoded payload', async () => {
    let url = ''
    const text = await obtainAltchaPayload('http://h', {
      fetchImpl: async (u) => {
        url = String(u)
        return json(challenge)
      },
      solve: async (c) => {
        expect(c).toEqual(challenge)
        return { counter: 5, derivedKey: 'aa', time: 1 }
      },
    })
    expect(url).toBe('http://h/altcha/challenge')
    expect(JSON.parse(atob(text))).toEqual({ challenge, solution: { counter: 5, derivedKey: 'aa', time: 1 } })
  })

  it('maps HTTP refusals to translated error codes', async () => {
    const run = (res: Response) => obtainAltchaPayload('http://h', { fetchImpl: async () => res, solve: async () => ({ counter: 0, derivedKey: '', time: 0 }) })
    await expect(run(new Response('', { status: 429, headers: { 'retry-after': '3' } }))).rejects.toMatchObject({ code: 'rate_limited' })
    await expect(run(new Response('', { status: 403, headers: { 'retry-after': '60' } }))).rejects.toMatchObject({ code: 'banned' })
    await expect(run(new Response('', { status: 403 }))).rejects.toMatchObject({ code: 'forbidden' })
    await expect(run(new Response('', { status: 503 }))).rejects.toMatchObject({ code: 'server_full' })
    await expect(run(new Response('x', { status: 500 }))).rejects.toBeInstanceOf(ServerHttpError)
    expect(httpErrorCode(500, null)).toBe('altcha_failed')
  })

  it('reports network failures, bad answers and solver failures as altcha_failed, aborts as aborted', async () => {
    const fail = { fetchImpl: async () => { throw new TypeError('network') } }
    await expect(obtainAltchaPayload('http://h', fail)).rejects.toMatchObject({ code: 'altcha_failed' })
    await expect(obtainAltchaPayload('http://h', { fetchImpl: async () => new Response('<html>', { status: 200 }) })).rejects.toMatchObject({ code: 'altcha_failed' })
    await expect(
      obtainAltchaPayload('http://h', { fetchImpl: async () => json(challenge), solve: async () => { throw new AltchaError('timeout') } }),
    ).rejects.toMatchObject({ code: 'altcha_failed' })
    await expect(
      obtainAltchaPayload('http://h', { fetchImpl: async () => json(challenge), solve: async () => { throw new AltchaError('aborted') } }),
    ).rejects.toMatchObject({ reason: 'aborted' })
  })
})

describe('ICE servers', () => {
  it('keeps the RTCIceServer fields and drops the extras and unusable entries', () => {
    const list = parseIceServers([
      { urls: ['stun:s.example.org:3478'] },
      { urls: ['turn:t.example.org:3478?transport=udp'], username: '1:abc', credential: 'secret', ttl: 3600 },
      { urls: 'stun:single.example.org' },
      { urls: ['turn:no-auth.example.org'] }, // would make RTCPeerConnection throw
      { urls: [] },
      { nope: true },
      'junk',
    ])
    expect(list).toEqual([
      { urls: ['stun:s.example.org:3478'] },
      { urls: ['turn:t.example.org:3478?transport=udp'], username: '1:abc', credential: 'secret' },
      { urls: ['stun:single.example.org'] },
    ])
    expect(parseIceServers({})).toEqual([])
  })

  it('treats 404, other failures and broken JSON as "no list"', async () => {
    expect(await fetchIceServers('http://h', { fetchImpl: async () => new Response('', { status: 404 }) })).toEqual([])
    expect(await fetchIceServers('http://h', { fetchImpl: async () => new Response('', { status: 429 }) })).toEqual([])
    expect(await fetchIceServers('http://h', { fetchImpl: async () => new Response('<html>', { status: 200 }) })).toEqual([])
    expect(await fetchIceServers('http://h', { fetchImpl: async () => { throw new TypeError('x') } })).toEqual([])
  })

  it('returns the list from a 200 answer', async () => {
    let url = ''
    const list = await fetchIceServers('http://h', {
      fetchImpl: async (u) => {
        url = String(u)
        return json([{ urls: ['turn:t:1'], username: 'u', credential: 'c' }])
      },
    })
    expect(url).toBe('http://h/ice-servers')
    expect(list).toEqual([{ urls: ['turn:t:1'], username: 'u', credential: 'c' }])
  })
})
