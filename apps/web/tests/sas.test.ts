// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import { connectionCode, sdpFingerprint } from '../src/core/sas'
import { ReceiverSession } from '../src/core/receiverSession'
import { SenderSession } from '../src/core/senderSession'
import type { Peer, PeerHandlers } from '../src/core/peer'
import type { ControlChannel } from '../src/core/control'
import { FakeChannel } from './helpers/fakeChannel'
import { FakeSocket } from './helpers/fakeSocket'

const fp = (byte: string) => Array.from({ length: 32 }, () => byte).join(':')
const sdp = (f: string) => `v=0\r\na=group:BUNDLE 0\r\na=fingerprint:sha-256 ${f}\r\na=setup:actpass\r\n`
const A = fp('AB')
const B = fp('12')
const tick = () => new Promise((r) => setTimeout(r, 0))

describe('connection check code', () => {
  it('reads the sha-256 fingerprint, lowercased', () => {
    expect(sdpFingerprint(sdp(A))).toBe(fp('ab'))
    expect(sdpFingerprint('v=0\r\n')).toBeNull()
    expect(sdpFingerprint(sdp('AB:CD'))).toBeNull()
    expect(sdpFingerprint(`a=fingerprint:sha-1 ${A}\r\n`)).toBeNull()
  })
  it('is 6 digits, the same on both sides (order and case independent), different for a swapped fingerprint', async () => {
    const sender = await connectionCode(sdp(A), sdp(B))
    const receiver = await connectionCode(sdp(B), sdp(A))
    expect(sender).toMatch(/^\d{3} \d{3}$/)
    expect(receiver).toBe(sender)
    expect(await connectionCode(sdp(A.toLowerCase()), sdp(B))).toBe(sender)
    // A relay that gives the receiver another certificate (M) changes the code on one side.
    const mitm = await connectionCode(sdp(B), sdp(fp('ee')))
    expect(mitm).not.toBe(sender)
  })
  it('is a fixed, domain-separated SHA-256 derivation (known answer)', async () => {
    const data = new TextEncoder().encode(`cometcast/connection-check/v1\n${fp('12')}\n${fp('ab')}`)
    const h = new Uint8Array(await crypto.subtle.digest('SHA-256', data))
    const n = ((h[0] << 24) | (h[1] << 16) | (h[2] << 8) | h[3]) >>> 0
    const d = String(n % 1_000_000).padStart(6, '0')
    expect(await connectionCode(sdp(A), sdp(B))).toBe(`${d.slice(0, 3)} ${d.slice(3)}`)
  })
  it('is null without a fingerprint or without WebCrypto', async () => {
    expect(await connectionCode('v=0', sdp(B))).toBeNull()
    expect(await connectionCode(sdp(A), sdp(B), null)).toBeNull()
  })
})

class FakePeer implements Peer {
  constructor(public h: PeerHandlers, private code: string | null) {}
  addStream() {}
  openChannel(): ControlChannel { return new FakeChannel() }
  async createOffer() { return 'OFFER' }
  async acceptOffer() { return 'ANSWER' }
  async acceptAnswer() {}
  async addIceCandidate() {}
  async connectionCode() { return this.code }
  close() {}
}

describe('sessions expose the code once connected', () => {
  it('receiver', async () => {
    const sock = new FakeSocket()
    const peers: FakePeer[] = []
    const s = new ReceiverSession({
      url: 'ws://x/ws',
      onChange: () => {},
      socketFactory: () => sock,
      peerFactory: (h) => { const p = new FakePeer(h, '123 456'); peers.push(p); return p },
    })
    s.start()
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    sock.recv({ type: 'peer_joined', name: 'Guest 1' })
    expect(s.snapshot.sas).toBeUndefined()
    peers[0].h.onState('connected')
    await tick()
    expect(s.snapshot).toMatchObject({ phase: 'receiving', sas: '123 456' })
    sock.recv({ type: 'peer_left' })
    expect(s.snapshot.sas).toBeUndefined()
  })
  it('sender', async () => {
    const sock = new FakeSocket()
    const peers: FakePeer[] = []
    const s = new SenderSession({
      url: 'ws://x/ws',
      onChange: () => {},
      socketFactory: () => sock,
      peerFactory: (h) => { const p = new FakePeer(h, '123 456'); peers.push(p); return p },
    })
    await s.start('ABC234')
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    sock.recv({ type: 'joined' })
    await tick()
    peers[0].h.onState('connected')
    await tick()
    expect(s.snapshot).toMatchObject({ phase: 'sharing', sas: '123 456' })
    s.stop()
    expect(s.snapshot.sas).toBeUndefined()
  })
})
