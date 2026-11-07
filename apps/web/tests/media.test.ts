// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { canPlayHlsNatively, classifyMediaUrl, initialPlayer, mediaErrorCode, playerReduce } from '../src/core/media'

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
