// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from 'vitest'
import { ReceiverSession, type ReceiverState } from '../src/core/receiverSession'
import { SenderSession } from '../src/core/senderSession'
import type { Peer, PeerHandlers } from '../src/core/peer'
import type { ControlChannel } from '../src/core/control'
import { FakeChannel } from './helpers/fakeChannel'
import { FakeSocket } from './helpers/fakeSocket'

class FakePeer implements Peer {
  constructor(public h: PeerHandlers, public config?: RTCConfiguration) {}
  addStream() {}
  openChannel(): ControlChannel { return new FakeChannel() }
  async createOffer() { return 'OFFER' }
  async acceptOffer() { return 'ANSWER' }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() {}
}

const tick = () => new Promise((r) => setTimeout(r, 0))
const TURN: RTCIceServer[] = [{ urls: ['turn:t.example.org:3478'], username: 'u', credential: 'c' }]

function deferred<T>() {
  let resolve!: (v: T) => void
  let reject!: (e: unknown) => void
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

function setupReceiver(extra: Partial<ConstructorParameters<typeof ReceiverSession>[0]> = {}) {
  const sock = new FakeSocket()
  const peers: FakePeer[] = []
  const states: ReceiverState[] = []
  const s = new ReceiverSession({
    url: 'ws://x/ws',
    onChange: (st) => states.push(st),
    socketFactory: () => sock,
    peerFactory: (h, c) => { const p = new FakePeer(h, c); peers.push(p); return p },
    ...extra,
  })
  s.start()
  sock.open()
  sock.recv({ type: 'hello', proto_version: 1 })
  return { s, sock, peers, states }
}

describe('ReceiverSession with ALTCHA', () => {
  it('solves the puzzle after altcha_required and creates the room on the same connection', async () => {
    const d = deferred<string>()
    let signal: AbortSignal | undefined
    const { s, sock, states } = setupReceiver({ altcha: (sg) => { signal = sg; return d.promise } })
    expect(sock.sent[1]).toEqual({ type: 'create_room' })
    sock.recv({ type: 'error', code: 'altcha_required', message: '' })
    expect(s.snapshot.phase).toBe('checking')
    expect(signal?.aborted).toBe(false)
    d.resolve('PAYLOAD')
    await tick()
    expect(sock.sent[2]).toEqual({ type: 'create_room', altcha: 'PAYLOAD' })
    expect(s.snapshot.phase).toBe('connecting')
    expect(sock.closed).toBe(false)
    sock.recv({ type: 'room_created', code: 'ABC234' })
    expect(s.snapshot).toMatchObject({ phase: 'waiting_sender', code: 'ABC234' })
    expect(states.map((x) => x.phase)).toEqual(['connecting', 'checking', 'connecting', 'waiting_sender'])
  })

  it('shows HTTP-side failures with their own code (rate_limited, banned, ...)', async () => {
    const { s, sock } = setupReceiver({ altcha: () => Promise.reject({ code: 'rate_limited' }) })
    sock.recv({ type: 'error', code: 'altcha_required', message: '' })
    await tick()
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'rate_limited' })
    const b = setupReceiver({ altcha: () => Promise.reject(new Error('boom')) })
    b.sock.recv({ type: 'error', code: 'altcha_required', message: '' })
    await tick()
    expect(b.s.snapshot).toMatchObject({ phase: 'error', errorCode: 'altcha_failed' })
  })

  it('shows altcha_invalid from the server and does not try a second puzzle', async () => {
    let calls = 0
    const { s, sock } = setupReceiver({ altcha: async () => { calls++; return 'P' } })
    sock.recv({ type: 'error', code: 'altcha_required', message: '' })
    await tick()
    sock.recv({ type: 'error', code: 'altcha_invalid', message: '' })
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'altcha_invalid' })
    expect(calls).toBe(1)
  })

  it('shows altcha_required as an error when no solver is configured', () => {
    const { s, sock } = setupReceiver()
    sock.recv({ type: 'error', code: 'altcha_required', message: '' })
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'altcha_required' })
  })

  it('aborts solving and ignores the result after stop()', async () => {
    const d = deferred<string>()
    let signal: AbortSignal | undefined
    const { s, sock } = setupReceiver({ altcha: (sg) => { signal = sg; return d.promise } })
    sock.recv({ type: 'error', code: 'altcha_required', message: '' })
    s.stop()
    expect(signal?.aborted).toBe(true)
    d.resolve('LATE')
    await tick()
    expect(sock.sent).toHaveLength(2) // hello, create_room: nothing sent after stop
    expect(s.snapshot.phase).toBe('idle')
  })

  it('does not call the solver when the server accepts create_room right away (internal mode)', () => {
    let calls = 0
    const { s, sock } = setupReceiver({ altcha: async () => { calls++; return 'P' } })
    sock.recv({ type: 'room_created', code: 'ABC234' })
    expect(s.snapshot.phase).toBe('waiting_sender')
    expect(calls).toBe(0)
  })
})

describe('ICE servers', () => {
  it('receiver passes the list to the peer', async () => {
    const { sock, peers } = setupReceiver({ iceServers: async () => TURN })
    await tick()
    sock.recv({ type: 'room_created', code: 'ABC234' })
    sock.recv({ type: 'peer_joined' })
    expect(peers[0].config).toEqual({ iceServers: TURN })
  })

  it('receiver waits for the list if a sender joins first', async () => {
    const d = deferred<RTCIceServer[]>()
    const { s, sock, peers } = setupReceiver({ iceServers: () => d.promise })
    sock.recv({ type: 'room_created', code: 'ABC234' })
    sock.recv({ type: 'peer_joined' })
    expect(peers).toHaveLength(0)
    d.resolve(TURN)
    await tick()
    expect(peers).toHaveLength(1)
    expect(peers[0].config?.iceServers).toEqual(TURN)
    expect(s.snapshot.phase).toBe('negotiating')
  })

  it('the default peer factory hands the list to RTCPeerConnection', async () => {
    const seen: unknown[] = []
    class FakeRtc {
      constructor(cfg: unknown) { seen.push(cfg) }
      close() {}
    }
    vi.stubGlobal('RTCPeerConnection', FakeRtc)
    try {
      const sock = new FakeSocket()
      const s = new ReceiverSession({ url: 'ws://x/ws', onChange: () => {}, socketFactory: () => sock, iceServers: async () => TURN })
      s.start()
      await tick()
      sock.recv({ type: 'peer_joined' })
      expect(seen).toEqual([{ iceServers: TURN }])
      s.stop()
    } finally {
      vi.unstubAllGlobals()
    }
  })

  it('receiver works with an empty list', async () => {
    const { sock, peers } = setupReceiver({ iceServers: async () => [] })
    await tick()
    sock.recv({ type: 'peer_joined' })
    expect(peers[0].config).toEqual({ iceServers: [] })
  })

  it('sender passes the list to the peer and still works when the fetch rejects', async () => {
    const cases: [() => Promise<RTCIceServer[]>, RTCIceServer[]][] = [
      [async () => TURN, TURN],
      [() => Promise.reject(new Error('x')), []],
    ]
    for (const [ice, expected] of cases) {
      const sock = new FakeSocket()
      const peers: FakePeer[] = []
      const s = new SenderSession({
        url: 'ws://x/ws',
        onChange: () => {},
        socketFactory: () => sock,
        peerFactory: (h, c) => { const p = new FakePeer(h, c); peers.push(p); return p },
        iceServers: ice,
      })
      await s.start('ABC234')
      sock.open()
      sock.recv({ type: 'hello', proto_version: 1 })
      sock.recv({ type: 'joined' })
      await tick()
      expect(sock.sent.at(-1)).toEqual({ type: 'offer', sdp: 'OFFER' })
      expect(peers[0].config).toEqual({ iceServers: expected })
    }
  })
})
