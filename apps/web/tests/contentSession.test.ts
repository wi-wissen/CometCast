// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { ReceiverSession } from '../src/core/receiverSession'
import { SenderSession } from '../src/core/senderSession'
import type { Peer, PeerHandlers } from '../src/core/peer'
import type { ControlChannel } from '../src/core/control'
import { FakeSocket } from './helpers/fakeSocket'
import { pair, type FakeChannel } from './helpers/fakeChannel'

const tick = () => new Promise((r) => setTimeout(r, 0))

class P implements Peer {
  constructor(public h: PeerHandlers, public ch?: ControlChannel) {}
  addStream() {}
  openChannel(): ControlChannel { return this.ch! }
  async createOffer() { return 'O' }
  async acceptOffer() { return 'A' }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() {}
}

describe('content over the control channel (sessions with fake peers)', () => {
  it('receiver shows a link only as state, revokes image URLs, and clears on stop', async () => {
    const sock = new FakeSocket()
    let rp!: P
    const revoked: string[] = []
    const r = new ReceiverSession({
      url: 'ws://x/ws',
      onChange: () => {},
      socketFactory: () => sock,
      peerFactory: (h) => (rp = new P(h)),
      objectUrl: { create: () => 'blob:fake', revoke: (u) => revoked.push(u) },
    })
    r.start()
    sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    sock.recv({ type: 'peer_joined', name: 'Guest 1' })
    const [a, b] = pair()
    rp.h.onDataChannel!(b)
    expect(b.binaryType).toBe('arraybuffer')
    a.send('{"kind":"link","url":"https://example.org/x"}')
    await tick()
    expect(r.snapshot.content).toEqual({ kind: 'link', url: 'https://example.org/x', host: 'example.org' })
    // A PNG signature and IHDR of a 2 x 2 image (the display reads the pixel size before showing it).
    const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 2, 0, 0, 0, 2])
    a.send(`{"kind":"image","name":"p.png","mime":"image/png","size":${png.length}}`)
    a.send(png.buffer)
    a.send('{"kind":"image_end"}')
    await tick()
    expect(r.snapshot.content).toMatchObject({ kind: 'image', url: 'blob:fake', name: 'p.png' })
    a.send('{"kind":"cmd","cmd":"stop"}')
    await tick()
    expect(r.snapshot.content).toBeNull()
    expect(revoked).toEqual(['blob:fake'])
  })

  it('ignores channels with another label', async () => {
    const sock = new FakeSocket()
    let rp!: P
    const r = new ReceiverSession({ url: 'ws://x/ws', onChange: () => {}, socketFactory: () => sock, peerFactory: (h) => (rp = new P(h)) })
    r.start()
    sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    sock.recv({ type: 'peer_joined', name: 'Guest 1' })
    const [a, b] = pair()
    b.label = 'other'
    rp.h.onDataChannel!(b)
    a.send('{"kind":"link","url":"https://example.org/x"}')
    await tick()
    expect(r.snapshot.content).toBeNull()
  })

  it('content-only sender creates the channel, reports open, and sends', async () => {
    const sock = new FakeSocket()
    const [a, b] = pair()
    ;(a as FakeChannel).readyState = 'connecting'
    const states: boolean[] = []
    const s = new SenderSession({
      url: 'ws://x/ws',
      onChange: (st) => states.push(st.channelOpen),
      socketFactory: () => sock,
      peerFactory: (h) => new P(h, a),
      // no getStream: content only
    })
    await s.start('ABC234')
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    sock.recv({ type: 'joined' })
    await tick()
    expect(s.snapshot.channelOpen).toBe(false)
    expect(() => s.sendLink('https://example.org')).toThrow() // not open yet
    ;(a as FakeChannel).readyState = 'open'
    a.onopen?.({})
    expect(s.snapshot.channelOpen).toBe(true)
    const got: unknown[] = []
    b.onmessage = (ev) => got.push(ev.data)
    expect(s.sendLink('javascript:alert(1)')).toBe(false)
    expect(s.sendLink('https://example.org')).toBe(true)
    await tick()
    expect(got).toEqual(['{"kind":"link","url":"https://example.org/"}'])
    s.stop()
    expect(s.snapshot.channelOpen).toBe(false)
  })
})
