// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import {
  CHUNK_SIZE,
  ControlReceiver,
  ControlSender,
  ImageAssembler,
  MAX_IMAGE_BYTES,
  SendError,
  chunkBytes,
  parseControlMessage,
  stripBidi,
  validateLinkUrl,
  type ControlReceiverHandlers,
  type ImageSource,
} from '../src/core/control'
import { FakeChannel, pair } from './helpers/fakeChannel'

function bytes(n: number): Uint8Array {
  const b = new Uint8Array(n)
  for (let i = 0; i < n; i++) b[i] = i % 251
  return b
}

function source(data: Uint8Array, name = 'a.png', type = 'image/png'): ImageSource {
  return {
    name,
    type,
    size: data.byteLength,
    slice: (a, b) => ({
      arrayBuffer: async () => data.slice(a, b).buffer as ArrayBuffer,
    }),
  }
}

async function blobBytes(b: Blob): Promise<Uint8Array> {
  // jsdom's Blob has no arrayBuffer()/stream(); FileReader works.
  return new Promise((resolve, reject) => {
    const r = new FileReader()
    r.onload = () => resolve(new Uint8Array(r.result as ArrayBuffer))
    r.onerror = () => reject(r.error)
    r.readAsArrayBuffer(b)
  })
}

describe('chunkBytes', () => {
  it('splits into chunks of at most 16 KiB covering all bytes', () => {
    const data = bytes(CHUNK_SIZE * 2 + 5)
    const chunks = chunkBytes(data)
    expect(chunks.map((c) => c.byteLength)).toEqual([CHUNK_SIZE, CHUNK_SIZE, 5])
    const joined = new Uint8Array(data.byteLength)
    let o = 0
    for (const c of chunks) {
      joined.set(c, o)
      o += c.byteLength
    }
    expect(joined).toEqual(data)
  })
  it('handles empty input and exact multiples', () => {
    expect(chunkBytes(new Uint8Array(0))).toEqual([])
    expect(chunkBytes(bytes(CHUNK_SIZE)).length).toBe(1)
  })
})

describe('validateLinkUrl', () => {
  it('accepts http and https and normalises', () => {
    expect(validateLinkUrl(' https://Example.org/a?b=1 ')).toBe('https://example.org/a?b=1')
    expect(validateLinkUrl('http://localhost:8080')).toBe('http://localhost:8080/')
  })
  it.each([
    'javascript:alert(1)',
    'data:text/html,hi',
    'file:///etc/passwd',
    'ftp://example.org/',
    'example.org',
    '//example.org',
    'https://user:pw@example.org/',
    'https://exa mple.org/',
    'https://example.org/\nfoo',
    'ht\ttps://example.org/',
    '',
    '   ',
    'https://',
    'https://example.org/' + 'a'.repeat(3000),
  ])('rejects %j', (s) => {
    expect(validateLinkUrl(s)).toBeNull()
  })
})

describe('parseControlMessage', () => {
  it('accepts the cc-proto command forms', () => {
    expect(parseControlMessage('{"kind":"cmd","cmd":"next_page"}')).toEqual({ kind: 'cmd', cmd: 'next_page' })
    expect(parseControlMessage('{"kind":"cmd","cmd":"stop"}')).toEqual({ kind: 'cmd', cmd: 'stop' })
    expect(parseControlMessage('{"kind":"cmd","cmd":{"go_to_page":{"page":3}}}')).toEqual({
      kind: 'cmd',
      cmd: { go_to_page: { page: 3 } },
    })
  })
  it('rejects bad commands, unknown kinds and garbage', () => {
    for (const s of [
      '{"kind":"cmd","cmd":"explode"}',
      '{"kind":"cmd","cmd":{"go_to_page":{"page":0}}}',
      '{"kind":"cmd","cmd":{"go_to_page":{"page":"3"}}}',
      '{"kind":"nope"}',
      '[]',
      'null',
      'not json',
      '"str"',
    ]) {
      expect(parseControlMessage(s), s).toBeNull()
    }
  })
  it('validates image metadata', () => {
    const ok = { kind: 'image', name: 'x.png', mime: 'image/png', size: 10 }
    expect(parseControlMessage(JSON.stringify(ok))).toEqual(ok)
    for (const bad of [
      { ...ok, mime: 'image/svg+xml' },
      { ...ok, mime: 'text/html' },
      { ...ok, size: 0 },
      { ...ok, size: -1 },
      { ...ok, size: 1.5 },
      { ...ok, size: MAX_IMAGE_BYTES + 1 },
      { ...ok, name: '' },
      { ...ok, name: 'x'.repeat(256) },
    ]) {
      expect(parseControlMessage(JSON.stringify(bad)), JSON.stringify(bad)).toBeNull()
    }
  })
  it('re-validates links on the receiving side', () => {
    expect(parseControlMessage('{"kind":"link","url":"https://example.org"}')).toEqual({
      kind: 'link',
      url: 'https://example.org/',
    })
    expect(parseControlMessage('{"kind":"link","url":"javascript:alert(1)"}')).toBeNull()
  })
})

describe('ImageAssembler', () => {
  it('reassembles chunks into the original bytes', async () => {
    const data = bytes(CHUNK_SIZE * 3 + 77)
    const asm = new ImageAssembler()
    asm.start({ name: 'a.png', mime: 'image/png', size: data.byteLength })
    for (const c of chunkBytes(data)) expect(asm.push(c)).toBeNull()
    const r = asm.finish()
    if (typeof r === 'string') throw new Error(r)
    expect(r.blob.type).toBe('image/png')
    expect(r.blob.size).toBe(data.byteLength)
    expect(await blobBytes(r.blob)).toEqual(data)
  })
  it('rejects data beyond the announced size and drops the transfer', () => {
    const asm = new ImageAssembler()
    asm.start({ name: 'a.png', mime: 'image/png', size: CHUNK_SIZE + 10 })
    expect(asm.push(bytes(CHUNK_SIZE))).toBeNull()
    expect(asm.push(bytes(11))).toBe('too_large')
    expect(asm.active).toBe(false)
  })
  it('rejects short chunks that do not complete the image (message flood)', () => {
    const asm = new ImageAssembler()
    asm.start({ name: 'a.png', mime: 'image/png', size: MAX_IMAGE_BYTES })
    expect(asm.push(bytes(1))).toBe('bad_chunk')
    expect(asm.active).toBe(false)
  })
  it('accepts a short final chunk that completes the image', () => {
    const asm = new ImageAssembler()
    asm.start({ name: 'a.png', mime: 'image/png', size: 5 })
    expect(asm.push(bytes(5))).toBeNull()
    expect(asm.finish()).toMatchObject({ name: 'a.png' })
  })
  it('reports missing data and chunks without a transfer', () => {
    const asm = new ImageAssembler()
    expect(asm.push(bytes(1))).toBe('unexpected_chunk')
    expect(asm.finish()).toBe('no_transfer')
    asm.start({ name: 'a.png', mime: 'image/png', size: CHUNK_SIZE + 10 })
    asm.push(bytes(CHUNK_SIZE))
    expect(asm.finish()).toBe('size_mismatch')
  })
})

function collect() {
  const log: string[] = []
  const handlers: ControlReceiverHandlers & { blob?: Blob; name?: string } = {
    onCmd: (c) => log.push('cmd:' + JSON.stringify(c)),
    onProgress: (p) => log.push(p ? `progress:${p.received}/${p.size}` : 'progress:null'),
    onImage(blob, name) {
      handlers.blob = blob
      handlers.name = name
      log.push('image:' + name)
    },
    onLink: (u) => log.push('link:' + u),
    onError: (c) => log.push('error:' + c),
  }
  return { log, handlers }
}

describe('ControlReceiver', () => {
  it('handles a full image transfer and a stop command', async () => {
    const { log, handlers } = collect()
    let clock = 0
    const r = new ControlReceiver(handlers, () => (clock += 200)) // every call is past the throttle interval
    const data = bytes(CHUNK_SIZE + 1)
    r.handle(JSON.stringify({ kind: 'image', name: 'p.png', mime: 'image/png', size: data.byteLength }))
    for (const c of chunkBytes(data)) r.handle(c.slice().buffer)
    r.handle('{"kind":"image_end"}')
    expect(log).toEqual([
      `progress:0/${data.byteLength}`,
      `progress:${CHUNK_SIZE}/${data.byteLength}`,
      `progress:${data.byteLength}/${data.byteLength}`,
      'progress:null',
      'image:p.png',
    ])
    expect(await blobBytes(handlers.blob!)).toEqual(data)
    r.handle('{"kind":"cmd","cmd":"stop"}')
    expect(log.at(-1)).toBe('cmd:"stop"')
  })
  it('reports an incomplete image and malformed messages', () => {
    const { log, handlers } = collect()
    const r = new ControlReceiver(handlers)
    r.handle('{"kind":"image","name":"p.png","mime":"image/png","size":10}')
    r.handle(bytes(3).buffer as ArrayBuffer)
    r.handle('garbage')
    r.handle(42)
    expect(log).toContain('error:bad_chunk')
    expect(log.filter((l) => l === 'error:bad_message').length).toBe(2)
    expect(log.some((l) => l.startsWith('image:'))).toBe(false)
  })
  it('delivers a valid link and ignores an invalid one', () => {
    const { log, handlers } = collect()
    const r = new ControlReceiver(handlers)
    r.handle('{"kind":"link","url":"https://example.org/x"}')
    r.handle('{"kind":"link","url":"javascript:alert(1)"}')
    expect(log).toEqual(['link:https://example.org/x', 'error:bad_message'])
  })
})

describe('ControlReceiver resource limits (regression: 1-byte message flood)', () => {
  it('drops a flood of tiny binary messages at the first one and keeps no data', () => {
    const { log, handlers } = collect()
    const r = new ControlReceiver(handlers)
    r.handle(JSON.stringify({ kind: 'image', name: 'a', mime: 'image/png', size: MAX_IMAGE_BYTES }))
    for (let i = 0; i < 100_000; i++) r.handle(new ArrayBuffer(1))
    expect(log.filter((l) => l.startsWith('error:')).length).toBe(100_000) // each rejected, none stored
    expect(log).toContain('error:bad_chunk')
    expect(log).not.toContain('error:too_large')
  })
  it('throttles progress callbacks', () => {
    const { log, handlers } = collect()
    let clock = 0
    const r = new ControlReceiver(handlers, () => clock)
    r.handle(JSON.stringify({ kind: 'image', name: 'a', mime: 'image/png', size: CHUNK_SIZE * 10 }))
    for (let i = 0; i < 10; i++) {
      clock += 10 // 10 ms apart: well under the 100 ms interval
      r.handle(new ArrayBuffer(CHUNK_SIZE))
    }
    expect(log.filter((l) => l.startsWith('progress:') && l !== 'progress:null').length).toBeLessThanOrEqual(2)
  })
})

describe('stripBidi', () => {
  it('removes bidi controls', () => {
    expect(stripBidi('ab\u202Ecd\u2066e\u200F')).toBe('abcde')
    expect(parseControlMessage(JSON.stringify({ kind: 'image', name: 'a\u202Epng', mime: 'image/png', size: 5 }))).toMatchObject({ name: 'apng' })
  })
})

describe('ControlSender -> ControlReceiver end to end (fake channel pair)', () => {
  it('sends an image in 16 KiB chunks and the receiver reassembles it', async () => {
    const [a, b] = pair()
    const { log, handlers } = collect()
    const recv = new ControlReceiver(handlers)
    b.onmessage = (ev) => recv.handle(ev.data)
    const data = bytes(CHUNK_SIZE * 4 + 123)
    const progress: number[] = []
    await new ControlSender(a).sendImage(source(data), (sent) => progress.push(sent))
    await new Promise((r) => setTimeout(r, 0))
    // meta + 5 chunks + end
    expect(a.sent.length).toBe(1 + 5 + 1)
    for (const m of a.sent.slice(1, -1)) expect((m as ArrayBuffer).byteLength).toBeLessThanOrEqual(CHUNK_SIZE)
    expect(progress.at(-1)).toBe(data.byteLength)
    expect(log).toContain('image:a.png')
    expect(await blobBytes(handlers.blob!)).toEqual(data)
  })
  it('refuses unsuitable files before sending anything', async () => {
    const a = new FakeChannel()
    const s = new ControlSender(a)
    await expect(s.sendImage(source(bytes(5), 'x.svg', 'image/svg+xml'))).rejects.toMatchObject({ code: 'bad_type' })
    await expect(s.sendImage(source(new Uint8Array(0)))).rejects.toMatchObject({ code: 'empty' })
    const big = { ...source(bytes(1)), size: MAX_IMAGE_BYTES + 1 }
    await expect(s.sendImage(big)).rejects.toBeInstanceOf(SendError)
    expect(a.sent).toEqual([])
  })
  it('waits for the buffer to drain (back-pressure)', async () => {
    const a = new FakeChannel()
    a.simulateBuffer = true
    const data = bytes(CHUNK_SIZE * 80) // 1.25 MiB > high-water mark
    const p = new ControlSender(a).sendImage(source(data))
    let done = false
    void p.then(() => (done = true))
    for (let i = 0; i < 20 && !done; i++) {
      await new Promise((r) => setTimeout(r, 0))
      if (!done) a.drain()
    }
    await p
    expect(a.sent.length).toBe(1 + 80 + 1)
  })
  it('does not hang when the channel closes while waiting for the buffer to drain', async () => {
    const a = new FakeChannel()
    a.simulateBuffer = true
    a.bufferedAmount = 5_000_000 // stuck above the high-water mark
    const sessionClose = () => {}
    a.onclose = sessionClose
    const p = new ControlSender(a).sendImage(source(bytes(CHUNK_SIZE * 4)))
    const settled = p.then(() => 'ok', (e) => (e as SendError).code)
    await new Promise((r) => setTimeout(r, 30))
    a.close()
    expect(await settled).toBe('channel_closed')
    expect(a.onclose).toBe(sessionClose) // the previous handler is restored
  })
  it('fails when the channel closes mid transfer', async () => {
    const a = new FakeChannel()
    const s = new ControlSender(a)
    const src = source(bytes(CHUNK_SIZE * 3))
    const orig = src.slice
    let n = 0
    src.slice = (x, y) => {
      if (++n === 2) a.close()
      return orig(x, y)
    }
    await expect(s.sendImage(src)).rejects.toMatchObject({ code: 'channel_closed' })
  })
  it('sends commands and only valid links', () => {
    const a = new FakeChannel()
    const s = new ControlSender(a)
    s.sendCmd({ go_to_page: { page: 2 } })
    expect(s.sendLink('javascript:alert(1)')).toBe(false)
    expect(s.sendLink('https://example.org')).toBe(true)
    expect(a.sent).toEqual([
      '{"kind":"cmd","cmd":{"go_to_page":{"page":2}}}',
      '{"kind":"link","url":"https://example.org/"}',
    ])
    a.readyState = 'closed'
    expect(() => s.sendCmd('stop')).toThrow(SendError)
  })
})
