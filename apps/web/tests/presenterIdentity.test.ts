// SPDX-License-Identifier: GPL-3.0-or-later
//
// Presenter identity in the web sessions and on the web display (docs/design/presenter-identity.md): the proof in
// `join_room`, the memory of names and devices, the note in the request dialog and the "connected" toast.
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createAppI18n } from '../src/i18n'
import en from '../src/locales/en.json'
import de from '../src/locales/de.json'
import deSie from '../src/locales/de-sie.json'
import RequestDialog from '../src/components/display/RequestDialog.vue'
import { ReceiverSession, type ReceiverEvent } from '../src/core/receiverSession'
import { SenderSession } from '../src/core/senderSession'
import { decodeServerMessage } from '../src/core/signaling'
import { DeviceMemory, DEVICE_MEMORY_KEY, MAX_PAIRS, localStorageDeviceMemory } from '../src/core/deviceMemory'
import { makeJoinProof, type JoinDevice, type KeyStore } from '../src/core/deviceIdentity'
import { FakeSocket } from './helpers/fakeSocket'
import type { Peer, PeerHandlers } from '../src/core/peer'

const A = '0123456789abcdef0123456789abcdef'
const B = 'fedcba9876543210fedcba9876543210'
const NOTE_EN = 'This name was used here before by a different device.'
const NOTE_DE = 'Dieser Name wurde hier schon von einem anderen Gerät benutzt.'

beforeEach(() => vi.stubGlobal('crypto', webcrypto))
afterEach(() => {
  vi.unstubAllGlobals()
  localStorage.clear()
})

class MemoryStore implements KeyStore {
  pair: CryptoKeyPair | undefined
  async get() {
    return this.pair
  }
  async put(p: CryptoKeyPair) {
    this.pair = p
  }
}

class FakePeer implements Peer {
  constructor(public h: PeerHandlers) {}
  addStream() {}
  openChannel(): never {
    throw new Error('unused')
  }
  async createOffer() {
    return 'OFFER'
  }
  async acceptOffer() {
    return 'ANSWER'
  }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() {}
}

const flush = async () => {
  for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 0))
}

describe('the proof in join_room', () => {
  function sender(deviceProof?: (code: string, now: number) => Promise<JoinDevice | null>) {
    const sock = new FakeSocket()
    const s = new SenderSession({
      url: 'ws://x/ws',
      onChange: () => {},
      socketFactory: () => sock,
      name: () => 'Ana',
      now: () => 1_700_000_000_000,
      deviceProof,
    })
    return { s, sock }
  }

  it('carries the device proof when the key is there', async () => {
    const store = new MemoryStore()
    const { s, sock } = sender((code, now) => makeJoinProof(code, now, store))
    await s.start('K7QM2W', { capture: false })
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    await flush()
    const join = sock.sent.find((m) => m.type === 'join_room')
    expect(join).toMatchObject({ code: 'K7QM2W', name: 'Ana', device: { ts: 1_700_000_000_000 } })
    expect(Object.keys(join.device).sort()).toEqual(['pk', 'sig', 'ts'])
  })

  it('joins as before without a proof (no key, a failing key, no dependency)', async () => {
    for (const proof of [undefined, async () => null, async () => Promise.reject(new Error('no key'))]) {
      const { s, sock } = sender(proof)
      await s.start('K7QM2W', { capture: false })
      sock.open()
      sock.recv({ type: 'hello', proto_version: 2 })
      await flush()
      expect(sock.sent.find((m) => m.type === 'join_room')).toEqual({ type: 'join_room', code: 'K7QM2W', name: 'Ana' })
    }
  })

  it('does not send a late proof after the session was stopped', async () => {
    let release: (d: JoinDevice | null) => void = () => {}
    const { s, sock } = sender(() => new Promise((r) => (release = r)))
    await s.start('K7QM2W', { capture: false })
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    s.stop()
    release(null)
    await flush()
    expect(sock.sent.find((m) => m.type === 'join_room')).toBeUndefined()
  })
})

describe('decoding the relayed proof', () => {
  const d = { pk: 'A6EHv_POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg', ts: 5, sig: 'x'.repeat(86) }
  it('takes a well-formed device and ignores a malformed one without losing the message', () => {
    const good = { ...d, sig: '_IgPFHBw41LTmmUHxmNcsczKk8n5zpyC5DYBwdBgICRrVMIcjq-V3nZJuBiBqNLzCQ6MV0fXgYZ-ndx3PYTvDg' }
    expect(decodeServerMessage(JSON.stringify({ type: 'join_request', id: 1, name: 'A', check: 20, device: good }))).toEqual({
      type: 'join_request',
      id: 1,
      name: 'A',
      check: 20,
      device: good,
    })
    expect(decodeServerMessage(JSON.stringify({ type: 'peer_joined', name: 'A', device: { pk: 1 } }))).toEqual({ type: 'peer_joined', name: 'A' })
    expect(decodeServerMessage(JSON.stringify({ type: 'peer_joined', name: 'A', device: 'x' }))).toEqual({ type: 'peer_joined', name: 'A' })
  })
})

describe('the memory of names and devices', () => {
  it('gives the note for a known name with another device only', () => {
    const m = new DeviceMemory()
    expect(m.usedByOtherDevice("Julian's iPad", A)).toBe(false)
    expect(m.remember("Julian's iPad", A)).toBe(true)
    expect(m.usedByOtherDevice("Julian's iPad", A)).toBe(false)
    expect(m.usedByOtherDevice("Julian's iPad", B)).toBe(true)
    expect(m.usedByOtherDevice("JULIAN'S IPAD", B)).toBe(true)
    expect(m.usedByOtherDevice("Anna's iPad", B)).toBe(false)
    m.remember("Julian's iPad", B)
    expect(m.usedByOtherDevice("Julian's iPad", B)).toBe(false)
    expect(m.usedByOtherDevice("Julian's iPad", A)).toBe(false)
  })

  it('never stores or compares generated names and bad ids', () => {
    const m = new DeviceMemory()
    expect(m.remember('Guest 3', A)).toBe(false)
    expect(m.remember('', A)).toBe(false)
    expect(m.remember('Ana', 'nope')).toBe(false)
    expect(m.size).toBe(0)
    m.remember('Ana', A)
    expect(m.usedByOtherDevice('Guest 3', B)).toBe(false)
    expect(m.remember('Guest', A)).toBe(true)
  })

  it('keeps at most 200 pairs, the least recently used goes first', () => {
    const m = new DeviceMemory()
    for (let i = 0; i < MAX_PAIRS; i++) m.remember(`name ${i}`, A)
    expect(m.size).toBe(MAX_PAIRS)
    m.remember('name 0', A) // used again: now the newest
    m.remember('one more', A)
    expect(m.size).toBe(MAX_PAIRS)
    expect(m.usedByOtherDevice('name 0', B)).toBe(true)
    expect(m.usedByOtherDevice('name 1', B)).toBe(false)
  })

  it('reads its own text and skips garbage', () => {
    const m = new DeviceMemory()
    m.remember('Ana', A)
    expect(DeviceMemory.fromJSON(m.toJSON()).usedByOtherDevice('Ana', B)).toBe(true)
    for (const bad of ['', 'null', '{}', '[1]', '[["Ana"]]', 'not json']) expect(DeviceMemory.fromJSON(bad).size).toBe(0)
  })

  it('lives in localStorage and survives a localStorage that throws', () => {
    const store = localStorageDeviceMemory()
    store.remember('Ana', A)
    expect(localStorage.getItem(DEVICE_MEMORY_KEY)).toContain('Ana')
    expect(store.usedByOtherDevice('ana', B)).toBe(true)
    const get = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    const set = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('blocked')
    })
    expect(store.usedByOtherDevice('Ana', B)).toBe(false)
    expect(() => store.remember('Bea', A)).not.toThrow()
    get.mockRestore()
    set.mockRestore()
  })
})

describe('the display session', () => {
  async function proofFor(code: string, now: number, store = new MemoryStore()) {
    return (await makeJoinProof(code, now, store))!
  }

  function display(mode: 'open' | 'confirm', memory = localStorageDeviceMemory()) {
    const sock = new FakeSocket()
    const events: ReceiverEvent[] = []
    const now = 1_700_000_000_000
    const s = new ReceiverSession({
      url: 'ws://x/ws',
      onChange: () => {},
      socketFactory: () => sock,
      peerFactory: (h) => new FakePeer(h),
      joinMode: mode,
      now: () => now,
      deviceMemory: memory,
      onEvent: (e) => events.push(e),
    })
    s.start()
    sock.open()
    sock.recv({ type: 'hello', proto_version: 2 })
    sock.recv({ type: 'room_created', code: 'ABC234', join_mode: mode })
    return { s, sock, events, now }
  }

  it('confirm mode: the note for a name another device used, none for the same device or a new name', async () => {
    const d = display('confirm')
    const k1 = new MemoryStore()
    const k2 = new MemoryStore()
    // Julian's iPad is allowed; its pair is remembered when it is in.
    d.sock.recv({ type: 'join_request', id: 1, name: "Julian's iPad", check: 11, device: await proofFor('ABC234', d.now, k1) })
    await flush()
    expect(d.s.snapshot.requests[0]).toMatchObject({ otherDevice: false })
    expect(d.s.snapshot.requests[0].deviceId).toMatch(/^[0-9a-f]{32}$/)
    d.s.decide(1, true)
    expect(localStorage.getItem(DEVICE_MEMORY_KEY)).toBeNull() // not before the presenter is in
    d.sock.recv({ type: 'peer_joined', name: "Julian's iPad" })
    expect(localStorage.getItem(DEVICE_MEMORY_KEY)).toContain("Julian's iPad")
    // Not a "connected" toast for an allowed request.
    expect(d.events.filter((e) => e.kind === 'joined')).toEqual([])
    d.sock.recv({ type: 'peer_left' })

    // The same name from another key: the note.
    d.sock.recv({ type: 'join_request', id: 2, name: "julian's ipad", check: 12, device: await proofFor('ABC234', d.now, k2) })
    await flush()
    expect(d.s.snapshot.requests[0]).toMatchObject({ id: 2, otherDevice: true })
    d.s.decide(2, false)
    // Denied: not remembered, so the first key still has no note.
    d.sock.recv({ type: 'join_request', id: 3, name: "Julian's iPad", check: 13, device: await proofFor('ABC234', d.now, k1) })
    await flush()
    expect(d.s.snapshot.requests[0]).toMatchObject({ id: 3, otherDevice: false })
    // A new name: no note.
    d.sock.recv({ type: 'join_request', id: 4, name: 'Someone else', check: 14, device: await proofFor('ABC234', d.now, k2) })
    await flush()
    expect(d.s.snapshot.requests.find((r) => r.id === 4)).toMatchObject({ otherDevice: false })
  })

  it('shows no device at all without a valid proof, and never for Guest N', async () => {
    const memory = localStorageDeviceMemory()
    memory.remember("Julian's iPad", A)
    memory.remember('Guest 2', A)
    const d = display('confirm', memory)
    const k = new MemoryStore()
    const good = await proofFor('ABC234', d.now, k)
    const cases: Array<[string, string, JoinDevice | undefined]> = [
      ['no proof', "Julian's iPad", undefined],
      ['another room', "Julian's iPad", await proofFor('XYZ234', d.now, k)],
      ['too old', "Julian's iPad", await proofFor('ABC234', d.now - 121_000, k)],
      ['bad signature', "Julian's iPad", { ...good, sig: good.sig.slice(0, 85) + (good.sig.endsWith('A') ? 'Q' : 'A') }],
    ]
    let id = 10
    for (const [what, name, device] of cases) {
      d.sock.recv({ type: 'join_request', id, name, check: 20, ...(device ? { device } : {}) })
      await flush()
      const r = d.s.snapshot.requests.find((q) => q.id === id)!
      expect(r.deviceId, what).toBeUndefined()
      expect(r.otherDevice, what).toBeUndefined()
      id++
    }
    d.sock.recv({ type: 'join_request', id, name: 'Guest 2', check: 20, device: good })
    await flush()
    expect(d.s.snapshot.requests.find((q) => q.id === id)).toMatchObject({ otherDevice: false })
  })

  it('open mode: the connected toast carries the note and verified joins are remembered', async () => {
    const d = display('open')
    const k1 = new MemoryStore()
    const k2 = new MemoryStore()
    d.sock.recv({ type: 'peer_joined', name: 'Ana', device: await proofFor('ABC234', d.now, k1) })
    await flush()
    expect(d.events).toEqual([{ kind: 'joined', name: 'Ana' }])
    d.sock.recv({ type: 'peer_left' })
    d.sock.recv({ type: 'peer_joined', name: 'ANA', device: await proofFor('ABC234', d.now, k2) })
    await flush()
    expect(d.events.at(-1)).toEqual({ kind: 'joined', name: 'ANA', otherDevice: true })
    // Without a proof it is the plain toast, as before.
    d.sock.recv({ type: 'peer_left' })
    d.sock.recv({ type: 'peer_joined', name: 'Ana' })
    expect(d.events.at(-1)).toEqual({ kind: 'joined', name: 'Ana' })
  })

  it('works without WebCrypto: requests are shown as always', async () => {
    const d = display('confirm')
    const device = await proofFor('ABC234', d.now)
    vi.stubGlobal('crypto', undefined)
    d.sock.recv({ type: 'join_request', id: 1, name: 'Ana', check: 11, device })
    await flush()
    expect(d.s.snapshot.requests[0]).toMatchObject({ id: 1, name: 'Ana' })
    expect(d.s.snapshot.requests[0].deviceId).toBeUndefined()
  })
})

describe('the sentence', () => {
  it('is the same word for word in en, de (du) and de-sie (Sie)', () => {
    expect(en.request.otherDevice).toBe(NOTE_EN)
    expect(de.request.otherDevice).toBe(NOTE_DE)
    expect(deSie.request.otherDevice).toBe(NOTE_DE)
  })
})

describe('the request dialog', () => {
  const mk = (locale: 'en' | 'de', otherDevice?: boolean) =>
    mount(RequestDialog, {
      attachTo: document.body,
      global: { plugins: [createAppI18n(locale)] },
      props: { request: { id: 1, name: 'Lena', check: 41, receivedAt: Date.now(), ...(otherDevice === undefined ? {} : { deviceId: A, otherDevice }) }, more: 0 },
    })

  it('shows the neutral note with an info icon, in the secondary style, only when the name was used by another device', async () => {
    for (const [locale, text] of [['en', NOTE_EN], ['de', NOTE_DE]] as const) {
      const w = mk(locale, true)
      await nextTick()
      const note = w.get('[data-testid=request-device-note]')
      expect(note.text()).toBe(text)
      expect(note.find('svg').exists()).toBe(true)
      expect(note.classes()).toContain('cd-note')
      expect(note.classes().join(' ')).not.toMatch(/warn|danger|error/)
      w.unmount()
    }
    for (const other of [false, undefined]) {
      const w = mk('en', other)
      await nextTick()
      expect(w.find('[data-testid=request-device-note]').exists()).toBe(false)
      w.unmount()
    }
  })
})
