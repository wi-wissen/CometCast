// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { ReceiverSession, type ReceiverEvent } from '../src/core/receiverSession'
import { SenderSession } from '../src/core/senderSession'
import { FakeSocket } from './helpers/fakeSocket'
import type { Peer, PeerHandlers } from '../src/core/peer'

class FakePeer implements Peer {
  added: MediaStream[] = []
  removed: MediaStream[] = []
  offers = 0
  constructor(public h: PeerHandlers) {}
  addStream(s: MediaStream) {
    this.added.push(s)
  }
  removeStream(s: MediaStream) {
    this.removed.push(s)
  }
  openChannel(): never {
    throw new Error('unused')
  }
  async createOffer() {
    this.offers++
    return `OFFER${this.offers}`
  }
  async acceptOffer() {
    return 'ANSWER'
  }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() {}
}

function receiver(joinMode?: 'open' | 'confirm' | 'locked') {
  const sock = new FakeSocket()
  const events: ReceiverEvent[] = []
  const peers: FakePeer[] = []
  const s = new ReceiverSession({
    url: 'ws://x/ws',
    onChange: () => {},
    socketFactory: () => sock,
    peerFactory: (h) => {
      const p = new FakePeer(h)
      peers.push(p)
      return p
    },
    joinMode,
    onEvent: (e) => events.push(e),
  })
  s.start()
  sock.open()
  sock.recv({ type: 'hello', proto_version: 2 })
  return { s, sock, events, peers }
}

describe('display toasts: what the receiver reports (4.6)', () => {
  it('Yes toasts "allowed" and the connection that follows is not toasted a second time', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.sock.recv({ type: 'join_request', id: 1, name: 'Lena', check: 47 })
    r.s.decide(1, true)
    r.sock.recv({ type: 'peer_joined', name: 'Lena' })
    expect(r.events).toEqual([{ kind: 'allowed', name: 'Lena' }])
  })
  it('an open-mode join toasts "joined"; the sender leaving toasts "left"', () => {
    const r = receiver()
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    r.sock.recv({ type: 'peer_joined', name: 'Ben' })
    r.sock.recv({ type: 'peer_left' })
    expect(r.events).toEqual([
      { kind: 'joined', name: 'Ben' },
      { kind: 'left', name: 'Ben' },
    ])
  })
  it('No, No and block, a timeout and a withdrawn request', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    for (const [id, name] of [[1, 'A'], [2, 'B'], [3, 'C'], [4, 'D'], [5, 'E']] as const) r.sock.recv({ type: 'join_request', id, name, check: 10 + id })
    r.s.decide(1, false)
    r.s.decide(2, false, true)
    r.sock.recv({ type: 'join_request_cancelled', id: 3, reason: 'timeout' })
    r.sock.recv({ type: 'join_request_cancelled', id: 4, reason: 'cancelled' })
    r.sock.recv({ type: 'join_request_cancelled', id: 5, reason: 'left' })
    r.sock.recv({ type: 'join_request_cancelled', id: 99, reason: 'timeout' }) // unknown: silent
    expect(r.events.map((e) => e.kind)).toEqual(['denied', 'blocked', 'expired', 'withdrawn', 'withdrawn'])
  })
  it('Locked with waiting requests toasts how many were denied', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.sock.recv({ type: 'join_request', id: 1, name: 'A', check: 11 })
    r.sock.recv({ type: 'join_request', id: 2, name: 'B', check: 12 })
    r.s.setJoinMode('locked')
    expect(r.events).toEqual([{ kind: 'lockedRejected', n: 2 }])
    r.s.setJoinMode('locked') // already locked: no second toast
    expect(r.events.length).toBe(1)
  })
})

describe('ending the session (4.5)', () => {
  it('asks for a new room, toasts it, and Locked does not survive: the default mode applies again', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.s.setJoinMode('locked')
    r.sock.recv({ type: 'join_mode_changed', join_mode: 'locked' })
    r.s.endRoom()
    expect(r.sock.sent.at(-1)).toEqual({ type: 'end_room' })
    r.sock.recv({ type: 'room_ended' })
    expect(r.sock.sent.at(-1)).toEqual({ type: 'create_room', join_mode: 'confirm' })
    expect(r.s.snapshot).toMatchObject({ phase: 'connecting', code: null, joinMode: 'confirm', requests: [], blocks: [] })
    expect(r.events.at(-1)).toEqual({ kind: 'sessionEnded' })
    r.sock.recv({ type: 'room_created', code: 'XYZ789', join_mode: 'confirm' })
    expect(r.s.snapshot.code).toBe('XYZ789')
  })
  it('a mode that was chosen in Settings (not Locked) is the default of the next room', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.s.setJoinMode('open')
    r.sock.recv({ type: 'join_mode_changed', join_mode: 'open' })
    r.sock.recv({ type: 'room_ended' })
    expect(r.sock.sent.at(-1)).toEqual({ type: 'create_room', join_mode: 'open' })
  })
})

describe('a presenter that stops showing but stays connected', () => {
  it('the receiver drops the stream when the track is taken away, and stays connected', () => {
    const r = receiver()
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    r.sock.recv({ type: 'peer_joined', name: 'Ben' })
    const stream = {} as MediaStream
    r.peers[0].h.onTrack(stream)
    expect(r.s.snapshot.stream).toBe(stream)
    r.peers[0].h.onTrackEnded?.({} as MediaStream) // another stream: ignored
    expect(r.s.snapshot.stream).toBe(stream)
    r.peers[0].h.onTrackEnded?.(stream)
    expect(r.s.snapshot.stream).toBeNull()
    expect(r.s.snapshot.peerName).toBe('Ben')
  })
})

function sender() {
  const sock = new FakeSocket()
  const peers: FakePeer[] = []
  const stopped: string[] = []
  const stream = {
    getTracks: () => [{ stop: () => stopped.push('t') }],
    getVideoTracks: () => [{ addEventListener: () => {} }],
  } as unknown as MediaStream
  let denied = false
  const states: any[] = []
  const s = new SenderSession({
    url: 'ws://x/ws',
    onChange: (st) => states.push(st),
    socketFactory: () => sock,
    peerFactory: (h) => {
      const p = new FakePeer(h)
      // The data channel is not needed here: pretend one exists.
      p.openChannel = () => ({ binaryType: 'arraybuffer', send() {}, close() {}, readyState: 'open', bufferedAmount: 0 }) as never
      peers.push(p)
      return p
    },
    getStream: async () => {
      if (denied) throw new DOMException('no', 'NotAllowedError')
      return stream
    },
  })
  return { s, sock, peers, stream, stopped, states, deny: () => (denied = true) }
}

async function connected(x: ReturnType<typeof sender>) {
  await x.s.start('ABC234', { capture: false })
  x.sock.open()
  x.sock.recv({ type: 'hello', proto_version: 2 })
  x.sock.recv({ type: 'joined' })
  await new Promise((r) => setTimeout(r, 0))
  x.peers[0].h.onState('connected')
}

describe('presenter: screen on a connected session', () => {
  it('joins without asking for the screen, then adds it and negotiates again', async () => {
    const x = sender()
    await connected(x)
    expect(x.s.snapshot.phase).toBe('sharing')
    expect(x.peers[0].added).toEqual([])
    expect(await x.s.shareScreen()).toBe('ok')
    expect(x.peers[0].added).toEqual([x.stream])
    expect(x.sock.sent.at(-1)).toEqual({ type: 'offer', sdp: 'OFFER2' })
    expect(x.s.snapshot.screen).toBe(true)
  })
  it('stopping removes the tracks, negotiates again and keeps the session connected', async () => {
    const x = sender()
    await connected(x)
    await x.s.shareScreen()
    await x.s.stopScreen()
    expect(x.peers[0].removed).toEqual([x.stream])
    expect(x.stopped.length).toBe(1)
    expect(x.s.snapshot.phase).toBe('sharing')
    expect(x.s.snapshot.screen).toBeUndefined()
    expect(x.sock.sent.at(-1)).toEqual({ type: 'offer', sdp: 'OFFER3' })
    // and the screen can be shown again
    expect(await x.s.shareScreen()).toBe('ok')
  })
  it('cancelling the system dialog is not an error', async () => {
    const x = sender()
    await connected(x)
    x.deny()
    expect(await x.s.shareScreen()).toBe('cancelled')
    expect(x.s.snapshot.phase).toBe('sharing')
  })
  it('does nothing before the connection exists', async () => {
    const x = sender()
    expect(await x.s.shareScreen()).toBe('failed')
  })
})
