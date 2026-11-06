// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { ReceiverSession, type ReceiverState } from '../src/core/receiverSession'
import { SenderSession, type SenderState } from '../src/core/senderSession'
import type { Peer, PeerHandlers } from '../src/core/peer'
import type { ControlChannel } from '../src/core/control'
import { FakeChannel } from './helpers/fakeChannel'
import { FakeSocket } from './helpers/fakeSocket'

class FakePeer implements Peer {
  calls: string[] = []
  closed = false
  constructor(public h: PeerHandlers) {}
  addStream() { this.calls.push('addStream') }
  openChannel(): ControlChannel { return new FakeChannel() }
  async createOffer() { return 'OFFER' }
  async acceptOffer(sdp: string) { this.calls.push('acceptOffer:' + sdp); return 'ANSWER' }
  async acceptAnswer(sdp: string) { this.calls.push('acceptAnswer:' + sdp) }
  async addIceCandidate(c: string) { this.calls.push('ice:' + c) }
  close() { this.closed = true }
}

const tick = () => new Promise((r) => setTimeout(r, 0))

function setupReceiver() {
  const sock = new FakeSocket()
  const peers: FakePeer[] = []
  const states: ReceiverState[] = []
  const s = new ReceiverSession({
    url: 'ws://x/ws',
    onChange: (st) => states.push(st),
    socketFactory: () => sock,
    peerFactory: (h) => { const p = new FakePeer(h); peers.push(p); return p },
  })
  return { s, sock, peers, states }
}

describe('ReceiverSession', () => {
  it('goes through connecting, waiting, negotiating, receiving, peer_left', async () => {
    const { s, sock, peers } = setupReceiver()
    s.start()
    expect(s.snapshot.phase).toBe('connecting')
    sock.open()
    expect(sock.sent[0]).toMatchObject({ type: 'hello', role: 'receiver' })
    sock.recv({ type: 'hello', proto_version: 1 })
    expect(sock.sent[1]).toEqual({ type: 'create_room' })
    sock.recv({ type: 'room_created', code: 'ABC234' })
    expect(s.snapshot).toMatchObject({ phase: 'waiting_sender', code: 'ABC234' })
    sock.recv({ type: 'peer_joined' })
    expect(s.snapshot.phase).toBe('negotiating')
    sock.recv({ type: 'offer', sdp: 'O' })
    await tick()
    expect(sock.sent.at(-1)).toEqual({ type: 'answer', sdp: 'ANSWER' })
    sock.recv({ type: 'ice_candidate', candidate: 'C' })
    expect(peers[0].calls).toEqual(['acceptOffer:O', 'ice:C'])
    peers[0].h.onIceCandidate('LOCAL')
    expect(sock.sent.at(-1)).toEqual({ type: 'ice_candidate', candidate: 'LOCAL' })
    peers[0].h.onState('connected')
    expect(s.snapshot.phase).toBe('receiving')
    sock.recv({ type: 'peer_left' })
    expect(s.snapshot).toMatchObject({ phase: 'peer_left', code: 'ABC234', stream: null })
    expect(peers[0].closed).toBe(true)
    // a new sender may join the same room
    sock.recv({ type: 'peer_joined' })
    expect(s.snapshot.phase).toBe('negotiating')
    expect(peers).toHaveLength(2)
  })
  it('maps server errors and connection loss to error states', () => {
    const a = setupReceiver()
    a.s.start()
    a.sock.recv({ type: 'error', code: 'bad_hello', message: '' })
    expect(a.s.snapshot).toMatchObject({ phase: 'error', errorCode: 'bad_hello' })
    const b = setupReceiver()
    b.s.start()
    b.sock.recv({ type: 'room_created', code: 'ABC234' })
    b.sock.drop()
    expect(b.s.snapshot).toMatchObject({ phase: 'error', errorCode: 'connection_lost' })
  })
  it('stop returns to idle and closes the socket', () => {
    const { s, sock } = setupReceiver()
    s.start()
    s.stop()
    expect(s.snapshot.phase).toBe('idle')
    expect(sock.closed).toBe(true)
  })
  it('reports a failed peer', () => {
    const { s, sock, peers } = setupReceiver()
    s.start()
    sock.recv({ type: 'peer_joined' })
    peers[0].h.onState('failed')
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'peer_failed' })
  })
})

function fakeStream(): { stream: MediaStream; stopped: () => boolean } {
  let stopped = false
  const track = { stop: () => { stopped = true }, addEventListener: () => {} }
  const stream = { getTracks: () => [track], getVideoTracks: () => [track] } as unknown as MediaStream
  return { stream, stopped: () => stopped }
}

function setupSender(getStream?: () => Promise<MediaStream>) {
  const sock = new FakeSocket()
  const peers: FakePeer[] = []
  const fs = fakeStream()
  const s = new SenderSession({
    url: 'ws://x/ws',
    onChange: (_st: SenderState) => {},
    getStream: getStream ?? (async () => fs.stream),
    socketFactory: () => sock,
    peerFactory: (h) => { const p = new FakePeer(h); peers.push(p); return p },
  })
  return { s, sock, peers, fs }
}

describe('SenderSession', () => {
  it('captures, joins, offers, shares, ends when the receiver leaves', async () => {
    const { s, sock, peers, fs } = setupSender()
    const p = s.start('ABC234')
    expect(s.snapshot.phase).toBe('capturing')
    await p
    expect(s.snapshot.phase).toBe('connecting')
    sock.open()
    sock.recv({ type: 'hello', proto_version: 1 })
    expect(s.snapshot.phase).toBe('joining')
    expect(sock.sent[1]).toEqual({ type: 'join_room', code: 'ABC234' })
    sock.recv({ type: 'joined' })
    expect(s.snapshot.phase).toBe('negotiating')
    await tick()
    expect(sock.sent.at(-1)).toEqual({ type: 'offer', sdp: 'OFFER' })
    expect(peers[0].calls).toContain('addStream')
    sock.recv({ type: 'answer', sdp: 'A' })
    expect(peers[0].calls).toContain('acceptAnswer:A')
    peers[0].h.onState('connected')
    expect(s.snapshot.phase).toBe('sharing')
    sock.recv({ type: 'peer_left' })
    expect(s.snapshot.phase).toBe('peer_left')
    expect(fs.stopped()).toBe(true)
  })
  it('stop() releases the stream and closes the socket', async () => {
    const { s, sock, fs } = setupSender()
    await s.start('ABC234')
    s.stop()
    expect(s.snapshot.phase).toBe('stopped')
    expect(fs.stopped()).toBe(true)
    expect(sock.closed).toBe(true)
  })
  it('maps a cancelled picker and server errors', async () => {
    const a = setupSender(async () => { throw new DOMException('no', 'NotAllowedError') })
    await a.s.start('ABC234')
    expect(a.s.snapshot).toMatchObject({ phase: 'error', errorCode: 'capture_denied' })
    const b = setupSender()
    await b.s.start('ABC234')
    b.sock.recv({ type: 'error', code: 'room_not_found', message: '' })
    expect(b.s.snapshot).toMatchObject({ phase: 'error', errorCode: 'room_not_found' })
    expect(b.fs.stopped()).toBe(true)
  })
})
