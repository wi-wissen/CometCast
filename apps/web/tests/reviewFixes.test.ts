// SPDX-License-Identifier: GPL-3.0-or-later
// Regression tests for the review fixes of the web core (receiver, sender, control, ICE list, signalling).
import { describe, expect, it } from 'vitest'
import { ReceiverSession } from '../src/core/receiverSession'
import { ControlSender, SendError } from '../src/core/control'
import { parseIceServers } from '../src/core/altchaClient'
import { decodeServerMessage } from '../src/core/signaling'
import type { Peer, PeerHandlers } from '../src/core/peer'
import type { ControlChannel } from '../src/core/control'
import { FakeChannel } from './helpers/fakeChannel'
import { FakeSocket } from './helpers/fakeSocket'

const tick = () => new Promise((r) => setTimeout(r, 0))

class SlowPeer implements Peer {
  closed = false
  release: (() => void) | null = null
  fail = false
  constructor(public h: PeerHandlers) {}
  addStream() {}
  openChannel(): ControlChannel { return new FakeChannel() }
  async createOffer() { return 'OFFER' }
  acceptOffer(): Promise<string> {
    return new Promise((resolve, reject) => {
      this.release = () => (this.fail ? reject(new Error('closed')) : resolve('ANSWER'))
    })
  }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() { this.closed = true }
}

function receiver(factory?: (h: PeerHandlers, c?: RTCConfiguration) => Peer) {
  const sock = new FakeSocket()
  const peers: SlowPeer[] = []
  const s = new ReceiverSession({
    url: 'ws://x/ws',
    onChange: () => {},
    socketFactory: () => sock,
    peerFactory: factory ?? ((h) => { const p = new SlowPeer(h); peers.push(p); return p }),
  })
  s.start()
  sock.open()
  sock.recv({ type: 'hello', proto_version: 2 })
  sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
  return { s, sock, peers }
}

describe('receiver: results of a peer that is gone', () => {
  it('a late failure of the old peer does not end the room', async () => {
    const { s, sock, peers } = receiver()
    sock.recv({ type: 'peer_joined', name: 'A' })
    sock.recv({ type: 'offer', sdp: 'O' })
    await tick()
    sock.recv({ type: 'peer_left' })
    peers[0].fail = true
    peers[0].release?.()
    await tick()
    expect(s.snapshot.phase).toBe('peer_left')
    expect(s.snapshot.code).toBe('ABC234')
  })
  it('a late answer of the old peer is not sent', async () => {
    const { sock, peers } = receiver()
    sock.recv({ type: 'peer_joined', name: 'A' })
    sock.recv({ type: 'offer', sdp: 'O' })
    await tick()
    sock.recv({ type: 'peer_left' })
    peers[0].release?.()
    await tick()
    expect(sock.sent.some((m) => (m as { type?: string }).type === 'answer')).toBe(false)
  })
})

describe('receiver: an ICE list the browser refuses', () => {
  it('falls back to no ICE servers instead of getting stuck', () => {
    const configs: RTCConfiguration[] = []
    const { s, sock } = receiver((h, c = {}) => {
      configs.push(c)
      if ((c.iceServers ?? []).length > 0) throw new SyntaxError('bad url')
      return new SlowPeer(h)
    })
    ;(s as any).iceServers = [{ urls: ['stun:x'] }]
    sock.recv({ type: 'peer_joined', name: 'A' })
    expect(s.snapshot.phase).toBe('negotiating')
    expect(configs.map((c) => c.iceServers?.length)).toEqual([1, 0])
  })
})

describe('ICE URLs', () => {
  it('drops shapes RTCPeerConnection would throw for', () => {
    expect(
      parseIceServers([
        { urls: ['stun:h:3478?transport=udp'] },
        { urls: ['stun:h:abc'] },
        { urls: ['turn:h:abc'], username: 'u', credential: 'c' },
        { urls: ['stun:[2001:db8::1]:3478'] },
        { urls: ['turn:h:3478?transport=tcp'], username: 'u', credential: 'c' },
      ]),
    ).toEqual([
      { urls: ['stun:[2001:db8::1]:3478'] },
      { urls: ['turn:h:3478?transport=tcp'], username: 'u', credential: 'c' },
    ])
  })
})

describe('signalling: room code from the server', () => {
  it('takes only a well-formed code', () => {
    expect(decodeServerMessage(JSON.stringify({ type: 'room_created', code: 'ABC234', join_mode: 'open' }))).not.toBeNull()
    expect(decodeServerMessage(JSON.stringify({ type: 'room_created', code: 'ABC234?server=evil', join_mode: 'open' }))).toBeNull()
    expect(decodeServerMessage(JSON.stringify({ type: 'room_created', code: 'abc234', join_mode: 'open' }))).toBeNull()
  })
})

describe('control: one transfer at a time', () => {
  it('refuses a second file while the first is being sent', async () => {
    const ch = new FakeChannel()
    ch.simulateBuffer = true
    ch.bufferedAmount = 10 * 1024 * 1024 // full: the first transfer waits for the drain
    const c = new ControlSender(ch, () => {})
    const img = new Blob([new Uint8Array(100)], { type: 'image/png' })
    const file = Object.assign(img, { name: 'a.png' })
    const first = c.sendImage(file)
    await expect(c.sendImage(file)).rejects.toEqual(new SendError('busy'))
    ch.drain()
    await first
    await expect(c.sendImage(file)).resolves.toBeUndefined()
  })
})
