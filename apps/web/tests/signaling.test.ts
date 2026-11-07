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
    expect(JSON.parse(encodeClientMessage({ type: 'hello', proto_version: 2, role: 'receiver' }))).toEqual({
      type: 'hello',
      proto_version: 2,
      role: 'receiver',
    })
    expect(JSON.parse(encodeClientMessage({ type: 'join_room', code: 'ABC234' }))).toEqual({
      type: 'join_room',
      code: 'ABC234',
    })
    expect(JSON.parse(encodeClientMessage({ type: 'create_room', join_mode: 'confirm' }))).toEqual({
      type: 'create_room',
      join_mode: 'confirm',
    })
    expect(JSON.parse(encodeClientMessage({ type: 'join_decision', id: 2, allow: true }))).toEqual({
      type: 'join_decision',
      id: 2,
      allow: true,
    })
    expect(JSON.parse(encodeClientMessage({ type: 'ice_candidate', candidate: '{}' })).type).toBe('ice_candidate')
  })
  it('decodes every server message type', () => {
    const cases: ServerMessage[] = [
      { type: 'hello', proto_version: 2 },
      { type: 'room_created', code: 'ABC234', join_mode: 'confirm' },
      { type: 'joined' },
      { type: 'joined', room_name: 'Meeting room Elbe' },
      { type: 'peer_joined', name: 'Ana' },
      { type: 'join_pending', timeout_secs: 60, check: 42 },
      { type: 'join_pending', timeout_secs: 60, check: 42, room_name: 'Wohnzimmer' },
      { type: 'join_request', id: 3, name: 'Ana', check: 42 },
      { type: 'join_request_cancelled', id: 3, reason: 'timeout' },
      { type: 'join_mode_changed', join_mode: 'locked' },
      { type: 'offer', sdp: 'x' },
      { type: 'answer', sdp: 'y' },
      { type: 'ice_candidate', candidate: '{}' },
      { type: 'control', cmd: 'stop' },
      { type: 'peer_left' },
      { type: 'error', code: 'room_full', message: 'm' },
    ]
    for (const c of cases) expect(decodeServerMessage(JSON.stringify(c))).toEqual(c)
  })
  it('room_name is optional: absent, null, not a string or only white space mean "no room name" and the rest stays valid', () => {
    for (const v of ['null', '""', '"  \\t "', '5', '["x"]', '{"a":1}', 'false']) {
      expect(decodeServerMessage(`{"type":"joined","room_name":${v}}`), v).toEqual({ type: 'joined' })
      expect(decodeServerMessage(`{"type":"join_pending","timeout_secs":60,"check":42,"room_name":${v}}`), v).toEqual({
        type: 'join_pending',
        timeout_secs: 60,
        check: 42,
      })
    }
    expect(Object.keys(decodeServerMessage('{"type":"joined"}')!)).toEqual(['type'])
    // the message itself is still checked: a missing check is invalid with or without a name
    expect(decodeServerMessage('{"type":"join_pending","timeout_secs":60,"room_name":"x"}')).toBeNull()
    expect(JSON.parse(encodeClientMessage({ type: 'create_room', join_mode: 'open', room_name: 'Room 204' }))).toEqual({
      type: 'create_room',
      join_mode: 'open',
      room_name: 'Room 204',
    })
  })
  it('rejects invalid input', () => {
    for (const bad of ['', 'nope', '[]', 'null', '{"type":"nope"}', '{"type":"offer"}', '{"type":"room_created","code":1}', '{"type":"room_created","code":"ABC234"}', '{"type":"join_request","id":"1","name":"x","check":1}', '{"type":"join_mode_changed","join_mode":"x"}', '{"type":"peer_joined"}']) {
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
    expect(sock.sent[0]).toEqual({ type: 'hello', proto_version: 2, role: 'sender' })
    sock.recv({ type: 'hello', proto_version: 2 })
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
    sock.recv({ type: 'hello', proto_version: 1 })
    expect(got).toEqual(['proto_mismatch'])
    expect(sock.closed).toBe(true)
  })
})
