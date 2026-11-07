// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { ReceiverSession } from '../src/core/receiverSession'
import { SenderSession } from '../src/core/senderSession'
import { FakeSocket } from './helpers/fakeSocket'
import type { Peer, PeerHandlers } from '../src/core/peer'

class FakePeer implements Peer {
  constructor(public h: PeerHandlers) {}
  addStream() {}
  openChannel(): never { throw new Error('unused') }
  async createOffer() { return 'OFFER' }
  async acceptOffer() { return 'ANSWER' }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() {}
}

function receiver(joinMode?: 'open' | 'confirm' | 'locked', roomName?: string) {
  const sock = new FakeSocket()
  let now = 1000
  const s = new ReceiverSession({
    url: 'ws://x/ws',
    onChange: () => {},
    socketFactory: () => sock,
    peerFactory: (h) => new FakePeer(h),
    joinMode,
    roomName,
    now: () => now,
  })
  s.start()
  sock.open()
  sock.recv({ type: 'hello', proto_version: 2 })
  return { s, sock, advance: (ms: number) => (now += ms) }
}

function sender(name?: string) {
  const sock = new FakeSocket()
  let now = 5000
  const s = new SenderSession({
    url: 'ws://x/ws',
    onChange: () => {},
    socketFactory: () => sock,
    name: () => name,
    now: () => now,
  })
  return { s, sock, advance: (ms: number) => (now += ms) }
}

describe('ReceiverSession join approval', () => {
  it('creates the room in the chosen mode and defaults to open', () => {
    expect(receiver().sock.sent[1]).toEqual({ type: 'create_room', join_mode: 'open' })
    const r = receiver('confirm')
    expect(r.sock.sent[1]).toEqual({ type: 'create_room', join_mode: 'confirm' })
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    expect(r.s.snapshot).toMatchObject({ joinMode: 'confirm', phase: 'waiting_sender' })
  })

  it('queues requests in arrival order and removes decided ones', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.sock.recv({ type: 'join_request', id: 1, name: 'Ana', check: 11 })
    r.advance(500)
    r.sock.recv({ type: 'join_request', id: 2, name: 'Ben', check: 22 })
    r.sock.recv({ type: 'join_request', id: 2, name: 'Ben', check: 22 }) // duplicate ignored
    expect(r.s.snapshot.requests.map((q) => [q.id, q.name, q.check, q.receivedAt])).toEqual([
      [1, 'Ana', 11, 1000],
      [2, 'Ben', 22, 1500],
    ])
    r.s.decide(1, false)
    expect(r.sock.sent.at(-1)).toEqual({ type: 'join_decision', id: 1, allow: false })
    expect(r.s.snapshot.requests.map((q) => q.id)).toEqual([2])
    r.s.decide(99, true) // unknown: nothing sent
    expect(r.sock.sent.at(-1)).toEqual({ type: 'join_decision', id: 1, allow: false })
    r.s.decide(2, true)
    expect(r.sock.sent.at(-1)).toEqual({ type: 'join_decision', id: 2, allow: true })
    r.sock.recv({ type: 'peer_joined', name: 'Ben' })
    expect(r.s.snapshot).toMatchObject({ peerName: 'Ben', phase: 'negotiating' })
    r.sock.recv({ type: 'peer_left' })
    expect(r.s.snapshot.peerName).toBeNull()
  })

  it('denies all in one message', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.sock.recv({ type: 'join_request', id: 1, name: 'A', check: 11 })
    r.sock.recv({ type: 'join_request', id: 2, name: 'B', check: 12 })
    r.s.denyAll()
    expect(r.sock.sent.at(-1)).toEqual({ type: 'join_decision', allow: false })
    expect(r.s.snapshot.requests).toEqual([])
  })

  it('changes the mode on the server and shows the confirmed one; locking tells what was discarded', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.sock.recv({ type: 'join_request', id: 1, name: 'A', check: 11 })
    r.sock.recv({ type: 'join_request', id: 2, name: 'B', check: 12 })
    r.s.setJoinMode('locked')
    expect(r.sock.sent.at(-1)).toEqual({ type: 'set_join_mode', join_mode: 'locked' })
    expect(r.s.snapshot.joinMode).toBe('confirm') // only the server's answer counts
    r.sock.recv({ type: 'join_request_cancelled', id: 1, reason: 'locked' })
    r.sock.recv({ type: 'join_request_cancelled', id: 2, reason: 'locked' })
    r.sock.recv({ type: 'join_mode_changed', join_mode: 'locked' })
    expect(r.s.snapshot).toMatchObject({ joinMode: 'locked', requests: [], notice: { kind: 'locked_discarded', count: 2 } })
    r.sock.recv({ type: 'join_mode_changed', join_mode: 'open' })
    expect(r.s.snapshot.notice).toBeNull()
  })

  it('removes the peer and tolerates a decision for a vanished request', () => {
    const r = receiver()
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    r.s.removePeer()
    expect(r.sock.sent.at(-1)).toEqual({ type: 'remove_peer' })
    r.sock.recv({ type: 'error', code: 'no_such_request', message: '' })
    expect(r.s.snapshot.phase).toBe('waiting_sender')
  })

  it('falls back to confirm when the server forbids open rooms', () => {
    const r = receiver()
    r.sock.recv({ type: 'error', code: 'open_rooms_disabled', message: '' })
    expect(r.sock.sent.at(-1)).toEqual({ type: 'create_room', join_mode: 'confirm' })
    expect(r.s.snapshot).toMatchObject({ phase: 'connecting', joinMode: 'confirm', notice: { kind: 'open_disabled' } })
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    expect(r.s.snapshot.phase).toBe('waiting_sender')
  })

  it('applies a mode chosen while the room was still being created', () => {
    const r = receiver()
    r.s.setJoinMode('locked')
    expect(r.sock.sent.at(-1)).toEqual({ type: 'create_room', join_mode: 'open' })
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    expect(r.sock.sent.at(-1)).toEqual({ type: 'set_join_mode', join_mode: 'locked' })
  })
})

describe('ReceiverSession block, end presentation, end session', () => {
  it('"No and block" sends the block flag only with a denial and lists the entry when the server confirms', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.sock.recv({ type: 'join_request', id: 1, name: 'Pest', check: 11 })
    r.sock.recv({ type: 'join_request', id: 2, name: 'Ana', check: 12 })
    r.s.decide(1, false, true)
    expect(r.sock.sent.at(-1)).toEqual({ type: 'join_decision', id: 1, allow: false, block: true })
    r.s.decide(2, true, true) // block is ignored for an allow
    expect(r.sock.sent.at(-1)).toEqual({ type: 'join_decision', id: 2, allow: true })
    r.sock.recv({ type: 'blocked', id: 5, name: 'Pest' })
    r.sock.recv({ type: 'blocked', id: 5, name: 'Pest' })
    expect(r.s.snapshot.blocks).toEqual([{ id: 5, name: 'Pest' }])
    r.s.unblock(5)
    expect(r.sock.sent.at(-1)).toEqual({ type: 'unblock', id: 5 })
    r.sock.recv({ type: 'unblocked', id: 5 })
    expect(r.s.snapshot.blocks).toEqual([])
  })

  it('ends the presentation (kick), with or without blocking', () => {
    const r = receiver()
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    r.s.removePeer()
    expect(r.sock.sent.at(-1)).toEqual({ type: 'remove_peer' })
    r.s.removePeer(true)
    expect(r.sock.sent.at(-1)).toEqual({ type: 'remove_peer', block: true })
  })

  it('ending the session creates a new room with a new code on the same connection', () => {
    const r = receiver('confirm')
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.sock.recv({ type: 'join_request', id: 1, name: 'A', check: 11 })
    r.sock.recv({ type: 'blocked', id: 1, name: 'B' })
    r.s.endRoom()
    expect(r.sock.sent.at(-1)).toEqual({ type: 'end_room' })
    r.sock.recv({ type: 'room_ended' })
    expect(r.s.snapshot).toMatchObject({ phase: 'connecting', code: null, requests: [], blocks: [], peerName: null })
    expect(r.sock.sent.at(-1)).toEqual({ type: 'create_room', join_mode: 'confirm' })
    r.sock.recv({ type: 'room_created', code: 'XYZ789', join_mode: 'confirm' })
    expect(r.s.snapshot).toMatchObject({ phase: 'waiting_sender', code: 'XYZ789' })
  })
})

describe('SenderSession join approval', () => {
  it('sends the name, waits with the check number, and goes on after joined', async () => {
    const { s, sock } = sender('Ana')
    await s.start('ABC234')
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    expect(sock.sent[1]).toEqual({ type: 'join_room', code: 'ABC234', name: 'Ana' })
    sock.recv({ type: 'join_pending', timeout_secs: 60, check: 42 })
    expect(s.snapshot).toMatchObject({ phase: 'waiting_approval', check: 42, timeoutSecs: 60 })
    sock.recv({ type: 'joined' })
    expect(s.snapshot.phase).toBe('negotiating')
    expect(s.snapshot.check).toBeUndefined()
  })

  it('maps denied, timeout, locked (also while waiting) and removed', async () => {
    const cases: [string, string, boolean][] = [
      ['join_denied', 'join_denied', true],
      ['approval_timeout', 'approval_timeout', true],
      ['room_locked', 'room_locked_waiting', true],
      ['room_locked', 'room_locked', false],
      ['removed_by_host', 'removed_by_host', false],
    ]
    for (const [code, want, waiting] of cases) {
      const { s, sock, advance } = sender()
      await s.start('ABC234')
      sock.open()
      sock.recv({ type: 'hello', proto_version: 2 })
      if (waiting) sock.recv({ type: 'join_pending', timeout_secs: 60, check: 11 })
      advance(10)
      sock.recv({ type: 'error', code, message: '' })
      expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: want, errorAt: 5010 })
    }
  })

  it('cancel while waiting closes the connection (the server tells the host)', async () => {
    const { s, sock } = sender()
    await s.start('ABC234')
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    sock.recv({ type: 'join_pending', timeout_secs: 60, check: 11 })
    s.stop()
    expect(s.snapshot.phase).toBe('stopped')
    expect(sock.closed).toBe(true)
  })
})

describe('room name through the protocol', () => {
  it('the display sends its room name with every create_room (also the next room after End); without one the field is absent', () => {
    const r = receiver('confirm', 'Meeting room Elbe')
    expect(r.sock.sent[1]).toEqual({ type: 'create_room', join_mode: 'confirm', room_name: 'Meeting room Elbe' })
    r.sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'confirm' })
    r.sock.recv({ type: 'room_ended' })
    expect(r.sock.sent.at(-1)).toEqual({ type: 'create_room', join_mode: 'confirm', room_name: 'Meeting room Elbe' })
    expect(receiver('open').sock.sent[1]).toEqual({ type: 'create_room', join_mode: 'open' })
    expect(receiver('open', '').sock.sent[1]).toEqual({ type: 'create_room', join_mode: 'open' })
  })

  it('the presenter keeps the name from join_pending and joined; an old server gives none; the code is never a name', async () => {
    const { s, sock } = sender('Ana')
    await s.start('ABC234')
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    sock.recv({ type: 'join_pending', timeout_secs: 60, check: 42, room_name: 'Wohnzimmer' })
    expect(s.snapshot).toMatchObject({ phase: 'waiting_approval', roomName: 'Wohnzimmer' })
    sock.recv({ type: 'joined' }) // joined without a name keeps the one from join_pending
    expect(s.snapshot.roomName).toBe('Wohnzimmer')
    const open = sender()
    await open.s.start('ABC234')
    open.sock.open()
    open.sock.recv({ type: 'hello', proto_version: 2 })
    open.sock.recv({ type: 'joined', room_name: 'Raum 204' })
    expect(open.s.snapshot.roomName).toBe('Raum 204')
    const old = sender()
    await old.s.start('ABC234')
    old.sock.open()
    old.sock.recv({ type: 'hello', proto_version: 2 })
    old.sock.recv({ type: 'join_pending', timeout_secs: 60, check: 11 })
    expect(old.s.snapshot.roomName).toBeUndefined()
    old.sock.recv({ type: 'joined' })
    expect(old.s.snapshot.roomName).toBeUndefined()
    expect(JSON.stringify(old.s.snapshot)).not.toContain('ABC234')
  })
})
