// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from 'vitest'
import {
  decodeServerMessage,
  encodeClientMessage,
  SignalingClient,
  type ServerMessage,
} from '../src/core/signaling'
import { FakeSocket } from './helpers/fakeSocket'

describe('message encoding', () => {
  it('encodes client messages with snake_case type tags', () => {
    expect(JSON.parse(encodeClientMessage({ type: 'hello', proto_version: 1, role: 'receiver' }))).toEqual({
      type: 'hello',
      proto_version: 1,
      role: 'receiver',
    })
    expect(JSON.parse(encodeClientMessage({ type: 'join_room', code: 'ABC234' }))).toEqual({
      type: 'join_room',
      code: 'ABC234',
    })
    expect(JSON.parse(encodeClientMessage({ type: 'ice_candidate', candidate: '{}' })).type).toBe('ice_candidate')
  })
  it('decodes every server message type', () => {
    const cases: ServerMessage[] = [
      { type: 'hello', proto_version: 1 },
      { type: 'room_created', code: 'ABC234' },
      { type: 'joined' },
      { type: 'peer_joined' },
      { type: 'offer', sdp: 'x' },
      { type: 'answer', sdp: 'y' },
      { type: 'ice_candidate', candidate: '{}' },
      { type: 'control', cmd: 'stop' },
      { type: 'peer_left' },
      { type: 'error', code: 'room_full', message: 'm' },
    ]
    for (const c of cases) expect(decodeServerMessage(JSON.stringify(c))).toEqual(c)
  })
  it('rejects invalid input', () => {
    for (const bad of ['', 'nope', '[]', 'null', '{"type":"nope"}', '{"type":"offer"}', '{"type":"room_created","code":1}']) {
      expect(decodeServerMessage(bad)).toBeNull()
    }
    expect(decodeServerMessage(new ArrayBuffer(2))).toBeNull()
  })
})

describe('SignalingClient', () => {
  it('sends hello on open, reports ready, passes messages', () => {
    const sock = new FakeSocket()
    const got: string[] = []
    let ready = 0
    const c = new SignalingClient('ws://x/ws', 'sender', {
      onReady: () => ready++,
      onMessage: (m) => got.push(m.type),
      onClose: () => got.push('close'),
    }, () => sock)
    c.open()
    sock.open()
    expect(sock.sent[0]).toEqual({ type: 'hello', proto_version: 1, role: 'sender' })
    sock.recv({ type: 'hello', proto_version: 1 })
    sock.recv({ type: 'joined' })
    sock.onmessage({ data: 'garbage' })
    expect(ready).toBe(1)
    expect(got).toEqual(['joined'])
    sock.drop()
    expect(got).toEqual(['joined', 'close'])
  })
  it('reports proto_mismatch and does not report close when closed by us', () => {
    const sock = new FakeSocket()
    const got: string[] = []
    const c = new SignalingClient('ws://x/ws', 'receiver', {
      onReady: () => got.push('ready'),
      onMessage: (m) => got.push(m.type === 'error' ? m.code : m.type),
      onClose: () => got.push('close'),
    }, () => sock)
    c.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    expect(got).toEqual(['proto_mismatch'])
    expect(sock.closed).toBe(true)
  })
})
