// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { canPlayHlsNatively, classifyMediaUrl, initialPlayer, isMixedContent, mediaErrorCode, playerReduce, shortHost, urlHostIsLocalOrIp } from '../src/core/media'

describe('classifyMediaUrl', () => {
  it('classifies by extension', () => {
    for (const e of ['mp4', 'webm', 'ogv']) expect(classifyMediaUrl(`https://h.example/a.${e}`)).toBe('video')
    for (const e of ['mp3', 'ogg', 'wav', 'm4a']) expect(classifyMediaUrl(`https://h.example/a.${e}`)).toBe('audio')
    expect(classifyMediaUrl('https://h.example/live/index.m3u8')).toBe('hls')
  })
  it('ignores query, fragment and case', () => {
    expect(classifyMediaUrl('http://h.example/V/Clip.MP4?token=1#t=5')).toBe('video')
  })
  it('does not look at the query or the host', () => {
    expect(classifyMediaUrl('https://h.example/watch?v=a.mp4')).toBeNull()
    expect(classifyMediaUrl('https://x.mp4/')).toBeNull()
  })
  it('refuses other types and schemes', () => {
    expect(classifyMediaUrl('https://h.example/a.pdf')).toBeNull()
    expect(classifyMediaUrl('https://h.example/mp4')).toBeNull()
    expect(classifyMediaUrl('https://h.example/a.mp4/')).toBeNull()
    expect(classifyMediaUrl('javascript:a.mp4')).toBeNull()
    expect(classifyMediaUrl('file:///a.mp4')).toBeNull()
    expect(classifyMediaUrl('not a url')).toBeNull()
  })
})

describe('hls support probe', () => {
  it('needs a positive answer from canPlayType', () => {
    expect(canPlayHlsNatively({ canPlayType: () => '' })).toBe(false)
    expect(canPlayHlsNatively({ canPlayType: (t) => (t === 'application/vnd.apple.mpegurl' ? 'maybe' : '') })).toBe(true)
    expect(canPlayHlsNatively(null)).toBe(false)
  })
})

describe('mediaErrorCode', () => {
  it('maps MediaError codes', () => {
    expect([1, 2, 3, 4, undefined, 9].map(mediaErrorCode)).toEqual(['aborted', 'network', 'decode', 'unsupported', 'network', 'network'])
  })
})

describe('player state', () => {
  it('starts as card and only a play event loads', () => {
    expect(initialPlayer.phase).toBe('card')
    expect(playerReduce(initialPlayer, { type: 'playing' })).toBe(initialPlayer)
    expect(playerReduce(initialPlayer, { type: 'error', code: 'network' })).toBe(initialPlayer)
    const l = playerReduce(initialPlayer, { type: 'play' })
    expect(l).toEqual({ phase: 'loading' })
    expect(playerReduce(l, { type: 'playing' })).toEqual({ phase: 'playing' })
  })
  it('error can be retried and stop always releases', () => {
    const e = playerReduce({ phase: 'loading' }, { type: 'error', code: 'decode' })
    expect(e).toEqual({ phase: 'error', code: 'decode' })
    expect(playerReduce(e, { type: 'play' })).toEqual({ phase: 'loading' })
    for (const s of [e, { phase: 'playing' } as const, { phase: 'loading' } as const]) expect(playerReduce(s, { type: 'stop' })).toEqual(initialPlayer)
  })
})

describe('mixed content', () => {
  it('flags http media on an https page only', () => {
    expect(isMixedContent('https:', 'http://h.example/a.mp4')).toBe(true)
    expect(isMixedContent('https:', 'https://h.example/a.mp4')).toBe(false)
    expect(isMixedContent('http:', 'http://h.example/a.mp4')).toBe(false)
    expect(isMixedContent('https:', 'not a url')).toBe(false)
  })
})

describe('local or IP hosts', () => {
  it('warns for IP literals and private-looking names', () => {
    for (const u of ['http://127.0.0.1:8080/a.mp4', 'http://192.168.1.5/a.mp4', 'http://8.8.8.8/a.mp4', 'http://[::1]/a.mp4', 'http://[fd00::1]:81/a.mp4', 'http://localhost/a.mp4', 'http://nas/a.mp4', 'http://printer.local/a.mp4', 'http://a.b.lan/x.mp3', 'http://x.home.arpa/x.mp3', 'http://LOCALHOST./a.mp4'])
      expect(urlHostIsLocalOrIp(u), u).toBe(true)
  })
  it('does not warn for ordinary public names', () => {
    for (const u of ['https://media.example.org/a.mp4', 'https://cdn.example.com:8443/a.mp3', 'bad'])
      expect(urlHostIsLocalOrIp(u), u).toBe(false)
  })
})

describe('shortHost', () => {
  it('keeps short hosts and shortens long ones', () => {
    expect(shortHost('media.example.org')).toBe('media.example.org')
    const long = `${'a'.repeat(60)}.example.org`
    const s = shortHost(long)
    expect(s.length).toBeLessThanOrEqual(40)
    expect(s.startsWith('aaaa')).toBe(true)
    expect(s.endsWith('example.org')).toBe(true)
  })
})
