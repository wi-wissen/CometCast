// SPDX-License-Identifier: GPL-3.0-or-later
import { beforeEach, describe, expect, it } from 'vitest'
import {
  isLocalOrPrivateHost,
  loadRelayConfig,
  MAX_CONFIG_RELAYS,
  parseRelay,
  parseRelayConfig,
  resolveRelay,
  sameOriginRelay,
} from '../src/core/relay'
import { normalizeBase } from '../vite.config'

const page = { protocol: 'https:', host: 'cometcast.de', search: '' }
const ok = (t: string, o = {}) => {
  const p = parseRelay(t, o)
  if (!p.ok) throw new Error(`${t}: ${p.error}`)
  return p.relay
}
const err = (t: string, o = {}) => {
  const p = parseRelay(t, o)
  return p.ok ? 'OK' : p.error
}

describe('parseRelay', () => {
  it('normalises wss URLs to host and /ws', () => {
    expect(ok('wss://relay.cometcast.de')).toEqual({
      wsUrl: 'wss://relay.cometcast.de/ws',
      httpBase: 'https://relay.cometcast.de',
      host: 'relay.cometcast.de',
      insecure: false,
    })
    expect(ok('wss://relay.cometcast.de/ws').wsUrl).toBe('wss://relay.cometcast.de/ws')
    expect(ok('  WSS://Relay.CometCast.de:443/  ').wsUrl).toBe('wss://relay.cometcast.de/ws')
    expect(ok('wss://relay.example.org:8443/ws/').host).toBe('relay.example.org:8443')
  })
  it('accepts https as a spelling of wss, but not in strict (config) mode', () => {
    expect(ok('https://relay.example.org').wsUrl).toBe('wss://relay.example.org/ws')
    expect(err('https://relay.example.org', { strictWs: true })).toBe('invalid')
    expect(err('relay.example.org', { strictWs: true })).toBe('invalid')
  })
  it('allows ws:// only for local and private hosts', () => {
    expect(err('ws://relay.example.org')).toBe('insecure')
    expect(err('http://relay.example.org')).toBe('insecure')
    expect(err('ws://8.8.8.8:80')).toBe('insecure')
    for (const h of ['localhost:8443', '127.0.0.1:1', '192.168.1.5', '10.0.0.2', '172.16.0.1', 'relay.local', 'nas']) {
      expect(ok(`ws://${h}`).insecure, h).toBe(true)
    }
    expect(ok('ws://[::1]:8443').wsUrl).toBe('ws://[::1]:8443/ws')
    expect(err('ws://172.32.0.1')).toBe('insecure')
    expect(ok('ws://localhost:1', { strictWs: true }).httpBase).toBe('http://localhost:1')
  })
  it('bare host[:port]: wss for public hosts, page protocol for local ones', () => {
    expect(ok('relay.example.org').wsUrl).toBe('wss://relay.example.org/ws')
    expect(ok('relay.example.org:9', { pageProtocol: 'http:' }).wsUrl).toBe('wss://relay.example.org:9/ws')
    expect(ok('127.0.0.1:8443', { pageProtocol: 'http:' }).wsUrl).toBe('ws://127.0.0.1:8443/ws')
    expect(ok('127.0.0.1:8443', { pageProtocol: 'https:' }).wsUrl).toBe('wss://127.0.0.1:8443/ws')
  })
  it('rejects everything else', () => {
    expect(err('')).toBe('empty')
    expect(err('   ')).toBe('empty')
    expect(err('wss://user:pw@relay.example.org')).toBe('credentials')
    expect(err('wss://relay.example.org/other')).toBe('path')
    expect(err('wss://relay.example.org/ws?x=1')).toBe('path')
    expect(err('wss://relay.example.org/ws#x')).toBe('path')
    expect(err('wss://relay.example.org?')).toBe('path')
    expect(err('javascript:alert(1)')).toBe('invalid')
    expect(err('javascript://x/%0aalert(1)')).toBe('invalid')
    expect(err('ftp://relay.example.org')).toBe('invalid')
    expect(err('data:text/html,x')).toBe('invalid')
    expect(err('evil.com/x')).toBe('invalid')
    expect(err('wss://relay example.org')).toBe('invalid')
    expect(err('wss://relay.example.org\\@evil.com')).toBe('invalid')
    expect(err('wss://')).toBe('invalid')
    expect(err('wss://relay.example.org:99999')).toBe('invalid')
    expect(err('x'.repeat(400))).toBe('invalid')
    expect(err('a:b:c')).toBe('invalid')
  })
  it('isLocalOrPrivateHost', () => {
    expect(isLocalOrPrivateHost('cometcast.de')).toBe(false)
    expect(isLocalOrPrivateHost('1.2.3.4')).toBe(false)
    expect(isLocalOrPrivateHost('169.254.1.1')).toBe(true)
    expect(isLocalOrPrivateHost('[fd00::1]')).toBe(true)
    expect(isLocalOrPrivateHost('[2001:db8::1]')).toBe(false)
  })
})

describe('parseRelayConfig', () => {
  it('reads a relays list (first = default), dropping invalid and duplicate entries', () => {
    const r = parseRelayConfig(
      JSON.stringify({
        relays: ['wss://relay.cometcast.de/ws', 'ws://evil.example.org', 42, 'javascript:1', 'wss://relay.cometcast.de', 'ws://192.168.1.2:8443', { u: 1 }],
      }),
    )
    expect(r.map((x) => x.wsUrl)).toEqual(['wss://relay.cometcast.de/ws', 'ws://192.168.1.2:8443/ws'])
    expect(r.every((x) => x.source === 'config')).toBe(true)
  })
  it('accepts the single form and ignores unknown fields', () => {
    expect(parseRelayConfig('{"relay":"wss://a.example","script":"alert(1)","other":[1]}').map((x) => x.host)).toEqual(['a.example'])
  })
  it('gives nothing for anything that is not the documented shape', () => {
    for (const t of ['', 'not json', '[]', 'null', '"wss://a.example"', '{}', '{"relays":"wss://a.example"}', '{"relays":[]}', '<html></html>', '{"relays":["https://a.example"]}', '{"relays":["a.example"]}']) {
      expect(parseRelayConfig(t), t).toEqual([])
    }
    expect(parseRelayConfig(`{"relays":["wss://a.example"],"pad":"${'x'.repeat(9000)}"}`)).toEqual([])
  })
  it('keeps at most MAX_CONFIG_RELAYS entries', () => {
    const list = Array.from({ length: 30 }, (_, i) => `wss://r${i}.example.org`)
    expect(parseRelayConfig(JSON.stringify({ relays: list }))).toHaveLength(MAX_CONFIG_RELAYS)
  })
})

describe('loadRelayConfig', () => {
  const resp = (body: string, status = 200) => async () => new Response(body, { status })
  it('fetches <base>config.json without credentials', async () => {
    let seen: { url: string; init?: RequestInit } | null = null
    const fetchImpl = (async (url: string, init?: RequestInit) => {
      seen = { url, init }
      return new Response('{"relays":["wss://a.example"]}')
    }) as unknown as typeof fetch
    const r = await loadRelayConfig({ fetchImpl, base: '/app/' })
    expect(r.map((x) => x.host)).toEqual(['a.example'])
    expect(seen!.url).toBe('/app/config.json')
    expect(seen!.init?.credentials).toBe('omit')
  })
  it('is empty for 404, HTML fallbacks, network errors and timeouts', async () => {
    expect(await loadRelayConfig({ fetchImpl: resp('', 404) as unknown as typeof fetch })).toEqual([])
    expect(await loadRelayConfig({ fetchImpl: resp('<!doctype html>') as unknown as typeof fetch })).toEqual([])
    expect(await loadRelayConfig({ fetchImpl: (async () => { throw new TypeError('x') }) as unknown as typeof fetch })).toEqual([])
    const hang = ((_u: string, init: RequestInit) => new Promise((_res, rej) => init.signal!.addEventListener('abort', () => rej(new Error('aborted'))))) as unknown as typeof fetch
    const t0 = Date.now()
    expect(await loadRelayConfig({ fetchImpl: hang, timeoutMs: 30 })).toEqual([])
    expect(Date.now() - t0).toBeLessThan(1000)
  })
})

describe('resolveRelay precedence: ?server= > config.json > build default > same origin', () => {
  const cfg = parseRelayConfig('{"relays":["wss://config.example.org","wss://second.example.org"]}')
  const build = 'wss://build.example.org/ws'
  it('same origin without anything else', () => {
    const r = resolveRelay({ loc: page })
    expect(r.current).toEqual(sameOriginRelay(page))
    expect(r.current.wsUrl).toBe('wss://cometcast.de/ws')
    expect(r.current.source).toBe('origin')
    expect(r.nonDefault).toBe(false)
    expect(resolveRelay({ loc: { protocol: 'http:', host: '127.0.0.1:8443', search: '' } }).current.wsUrl).toBe('ws://127.0.0.1:8443/ws')
  })
  it('build default beats same origin', () => {
    const r = resolveRelay({ loc: page, buildDefault: build })
    expect(r.current.wsUrl).toBe(build)
    expect(r.current.source).toBe('build')
  })
  it('an invalid build default is ignored', () => {
    expect(resolveRelay({ loc: page, buildDefault: 'ws://public.example.org' }).current.source).toBe('origin')
    expect(resolveRelay({ loc: page, buildDefault: 'nonsense' }).current.source).toBe('origin')
  })
  it('config.json beats the build default; the other config entries are choices', () => {
    const r = resolveRelay({ loc: page, buildDefault: build, configRelays: cfg })
    expect(r.current.wsUrl).toBe('wss://config.example.org/ws')
    expect(r.current.source).toBe('config')
    expect(r.choices.map((c) => c.host)).toEqual(['config.example.org', 'second.example.org'])
  })
  it('?server= beats config.json, build default and origin; the default stays known', () => {
    const r = resolveRelay({ loc: { ...page, search: '?server=wss%3A%2F%2Fq.example.org' }, buildDefault: build, configRelays: cfg })
    expect(r.current.wsUrl).toBe('wss://q.example.org/ws')
    expect(r.current.source).toBe('query')
    expect(r.default.wsUrl).toBe('wss://config.example.org/ws')
    expect(r.nonDefault).toBe(true)
  })
  it('?server= with a bare host[:port]', () => {
    const r = resolveRelay({ loc: { protocol: 'http:', host: 'localhost:5173', search: '?server=127.0.0.1:8443' } })
    expect(r.current.wsUrl).toBe('ws://127.0.0.1:8443/ws')
    expect(r.current.httpBase).toBe('http://127.0.0.1:8443')
  })
  it('an invalid ?server= falls through to the next source', () => {
    for (const q of ['?server=evil.com/x', '?server=ws%3A%2F%2Fpublic.example.org', '?server=javascript%3Aalert(1)', '?server=']) {
      const r = resolveRelay({ loc: { ...page, search: q }, configRelays: cfg })
      expect(r.current.source, q).toBe('config')
    }
  })
  it('a choice made in the UI wins, and choosing the default is not "non-default"', () => {
    const base = { loc: { ...page, search: '?server=wss%3A%2F%2Fq.example.org' }, configRelays: cfg }
    expect(resolveRelay({ ...base, userChoice: 'wss://u.example.org' }).current.source).toBe('user')
    const same = resolveRelay({ loc: page, configRelays: cfg, userChoice: 'wss://config.example.org' })
    expect(same.nonDefault).toBe(false)
  })
})

describe('Vite base', () => {
  it('normalises VITE_BASE', () => {
    expect(normalizeBase(undefined)).toBe('/')
    expect(normalizeBase('')).toBe('/')
    expect(normalizeBase('/app')).toBe('/app/')
    expect(normalizeBase('/app/')).toBe('/app/')
    expect(() => normalizeBase('./')).toThrow()
    expect(() => normalizeBase('app/')).toThrow()
    expect(() => normalizeBase('/a/../b/')).toThrow()
    expect(() => normalizeBase('https://x.example/')).toThrow()
  })
})

describe('useRelay (UI choice, stored per tab)', () => {
  beforeEach(async () => {
    const m = await import('../src/composables/useRelay')
    m.resetRelayState()
  })
  it('set() validates, stores and resets', async () => {
    const { useRelay, initRelay } = await import('../src/composables/useRelay')
    await initRelay({ fetchImpl: (async () => new Response('{"relays":["wss://config.example.org"]}')) as unknown as typeof fetch })
    const r = useRelay()
    expect(r.current.value.host).toBe('config.example.org')
    expect(r.set('ws://public.example.org')).toBe('insecure')
    expect(r.current.value.host).toBe('config.example.org')
    expect(r.set('wss://mine.example.org')).toBeNull()
    expect(r.current.value.host).toBe('mine.example.org')
    expect(r.nonDefault.value).toBe(true)
    expect(sessionStorage.getItem('cometcast.relay')).toBe('wss://mine.example.org/ws')
    r.reset()
    expect(r.current.value.host).toBe('config.example.org')
    expect(sessionStorage.getItem('cometcast.relay')).toBeNull()
  })
})
