// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fix list 2 (docs/design/fix-list-web-2.md): busy memory, the STUN/TURN refusal, the PDF page state, the content states
// while presenting, the presenter's pages. Page-level flows of the presenter are in presenter.test.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { createAppI18n } from '../src/i18n'
import { BUSY_MEMORY_MS, BusyMemory } from '../src/core/busyMemory'
import { ControlSender, parseControlMessage } from '../src/core/control'
import { displayOffWhy, errorKindFor } from '../src/core/presenterErrors'
import { ReceiverSession, type ReceiverState } from '../src/core/receiverSession'
import { SenderSession, type SenderState } from '../src/core/senderSession'
import type { Peer, PeerHandlers } from '../src/core/peer'
import type { ControlChannel } from '../src/core/control'
import type { PdfRenderer } from '../src/core/pdfRenderer'
import PresentingPage from '../src/components/presenter/PresentingPage.vue'
import PresentingStage from '../src/components/display/PresentingStage.vue'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { FakeSocket } from './helpers/fakeSocket'
import { FakeChannel, pair } from './helpers/fakeChannel'

const tick = () => new Promise((r) => setTimeout(r, 0))
const i18n = () => ({ plugins: [createAppI18n('en')] })

class FakePeer implements Peer {
  closed = false
  ch = new FakeChannel()
  constructor(public h: PeerHandlers) {}
  addStream() {}
  openChannel(): ControlChannel {
    return this.ch
  }
  async createOffer() {
    return 'OFFER'
  }
  async acceptOffer() {
    return 'ANSWER'
  }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() {
    this.closed = true
  }
}

describe('busy stays busy (decision 13.44)', () => {
  it('the memory: a closed connection within 60 s after a refusal is the same refusal', () => {
    const m = new BusyMemory()
    expect(m.apply('connection_lost', 0)).toBe('connection_lost')
    expect(m.apply('banned', 1000)).toBe('banned')
    expect(m.apply('connection_lost', 1000 + BUSY_MEMORY_MS - 1)).toBe('banned')
    expect(m.apply('connection_lost', 1000 + BUSY_MEMORY_MS)).toBe('connection_lost')
    expect(m.apply('server_full', 5000)).toBe('server_full')
    expect(m.apply('connection_lost', 6000)).toBe('server_full')
    expect(m.apply('room_not_found', 6000)).toBe('room_not_found') // other codes pass unchanged
    expect(m.apply('forbidden', 6000)).toBe('forbidden') // forbidden is not one of the three
    m.reset()
    expect(m.apply('connection_lost', 6000)).toBe('connection_lost')
  })

  it('presenter: after banned, the second attempt (connection closed) is busy, not "No internet"; after 60 s it is "No internet" again', async () => {
    let now = 1_000_000
    const socks: FakeSocket[] = []
    const s = new SenderSession({
      url: 'ws://x/ws',
      onChange: () => {},
      socketFactory: () => {
        const k = new FakeSocket()
        socks.push(k)
        return k
      },
      now: () => now,
    })
    const attempt = async () => {
      await s.start('ABC234', { capture: false })
      socks.at(-1)!.open()
      return socks.at(-1)!
    }
    let k = await attempt()
    k.recv({ type: 'hello', proto_version: 2 })
    k.recv({ type: 'error', code: 'banned', message: '' })
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'banned' })
    expect(errorKindFor(s.snapshot.errorCode)).toBe('busy')
    // the server closed the connection: the next attempt ends as a closed connection
    now += 5000
    k = await attempt()
    k.drop()
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'banned' })
    expect(errorKindFor(s.snapshot.errorCode)).toBe('busy')
    now += 60_000
    k = await attempt()
    k.drop()
    expect(s.snapshot.errorCode).toBe('connection_lost')
    expect(errorKindFor(s.snapshot.errorCode)).toBe('noInternet')
  })

  it('display: rate_limited, then a closed connection within 60 s is the same refusal (box with the busy body, retry after 30 s)', () => {
    let now = 5_000
    const socks: FakeSocket[] = []
    const s = new ReceiverSession({
      url: 'ws://x/ws',
      onChange: () => {},
      socketFactory: () => {
        const k = new FakeSocket()
        socks.push(k)
        return k
      },
      now: () => now,
    })
    s.start()
    socks[0]!.recv({ type: 'error', code: 'rate_limited', message: '' })
    expect(s.snapshot.errorCode).toBe('rate_limited')
    now += 31_000
    s.start()
    socks[1]!.drop()
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'rate_limited' })
    now += 31_000 // 62 s after the refusal
    s.start()
    socks[2]!.drop()
    expect(s.snapshot.errorCode).toBe('connection_lost')
  })
})

describe('STUN/TURN list refused (D1, decision 13.45)', () => {
  it('the mapping of the presenter pages', () => {
    const k = (kind: string, extra: object = {}) => errorKindFor('peer_failed', { ice: { kind, ...extra } as never })
    expect(k('rate_limited')).toBe('busy')
    expect(k('refused')).toBe('busy')
    for (const kind of ['http_error', 'bad_answer', 'unreachable']) {
      expect(k(kind), kind).toBe('displayOff')
      expect(displayOffWhy({ kind } as never), kind).toBe('relay')
    }
    for (const kind of ['ok', 'none']) {
      expect(k(kind), kind).toBe('displayOff')
      expect(displayOffWhy({ kind } as never), kind).toBe('plain')
    }
    expect(errorKindFor('peer_failed')).toBe('displayOff')
    expect(displayOffWhy(undefined)).toBe('plain')
    // the state matters for peer_failed only
    expect(errorKindFor('connection_lost', { ice: { kind: 'refused' } })).toBe('noInternet')
  })

  it('the sender session exposes the state with peer_failed and nothing before it', async () => {
    const sock = new FakeSocket()
    const peers: FakePeer[] = []
    const states: SenderState[] = []
    const s = new SenderSession({
      url: 'ws://x/ws',
      onChange: (st) => states.push(st),
      socketFactory: () => sock,
      peerFactory: (h) => {
        const p = new FakePeer(h)
        peers.push(p)
        return p
      },
      iceServers: async () => [],
      iceState: () => ({ kind: 'rate_limited' }),
    })
    await s.start('ABC234', { capture: false })
    sock.open()
    sock.recv({ type: 'joined' })
    await tick()
    expect(states.every((st) => st.iceState === undefined)).toBe(true)
    peers[0]!.h.onState('failed')
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'peer_failed', iceState: { kind: 'rate_limited' } })
    expect(errorKindFor(s.snapshot.errorCode, { ice: s.snapshot.iceState })).toBe('busy')
  })

  it('the receiver session exposes it too (the display shows nothing of it)', () => {
    const sock = new FakeSocket()
    let peer!: FakePeer
    const s = new ReceiverSession({
      url: 'ws://x/ws',
      onChange: () => {},
      socketFactory: () => sock,
      peerFactory: (h) => (peer = new FakePeer(h)),
      iceState: () => ({ kind: 'unreachable', timedOut: true }),
    })
    s.start()
    sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    sock.recv({ type: 'peer_joined', name: 'Guest 1' })
    peer.h.onState('failed')
    expect(s.snapshot).toMatchObject({ phase: 'error', errorCode: 'peer_failed', iceState: { kind: 'unreachable' } })
  })
})

describe('PDF page state from the display (decision 13.31)', () => {
  it('the page message is validated', () => {
    expect(parseControlMessage('{"kind":"page","page":2,"pages":5}')).toEqual({ kind: 'page', page: 2, pages: 5 })
    for (const bad of ['{"kind":"page","page":0,"pages":5}', '{"kind":"page","page":6,"pages":5}', '{"kind":"page","page":1}', '{"kind":"page","page":"1","pages":5}', '{"kind":"page","page":1.5,"pages":5}'])
      expect(parseControlMessage(bad), bad).toBeNull()
  })

  it('the receiver reports the shown page and the page count whenever either changes; the sender session keeps them', async () => {
    const sock = new FakeSocket()
    let rp!: FakePeer
    const renderer: PdfRenderer = { open: async () => 3, render: async () => ({ width: 1, height: 1, rgba: new Uint8ClampedArray(4) }), dispose() {} }
    const r = new ReceiverSession({ url: 'ws://x/ws', onChange: () => {}, socketFactory: () => sock, peerFactory: (h) => (rp = new FakePeer(h)), pdfRenderer: () => renderer })
    r.start()
    sock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
    sock.recv({ type: 'peer_joined', name: 'Guest 1' })
    const [a, b] = pair()
    rp.h.onDataChannel!(b)
    const got: Array<{ page: number; pages: number }> = []
    new ControlSender(a, (p) => got.push(p))
    a.send('{"kind":"document","name":"plan.pdf","mime":"application/pdf","size":20}')
    a.send(new Uint8Array(20).buffer)
    a.send('{"kind":"document_end"}')
    await tick()
    await tick()
    expect(got).toEqual([{ page: 1, pages: 3 }]) // nothing while the page count is unknown (pages 0)
    a.send('{"kind":"cmd","cmd":"next_page"}')
    await tick()
    a.send('{"kind":"cmd","cmd":"next_page"}')
    a.send('{"kind":"cmd","cmd":"next_page"}') // clamped: no change, no message
    await tick()
    expect(got).toEqual([{ page: 1, pages: 3 }, { page: 2, pages: 3 }, { page: 3, pages: 3 }])
    a.send('{"kind":"cmd","cmd":"stop"}')
    await tick()
    expect(got).toHaveLength(3) // nothing for a closed document
  })

  it('the sender session puts the state into SenderState and forgets it with the next document', async () => {
    const sock = new FakeSocket()
    const peers: FakePeer[] = []
    const s = new SenderSession({ url: 'ws://x/ws', onChange: () => {}, socketFactory: () => sock, peerFactory: (h) => { const p = new FakePeer(h); peers.push(p); return p } })
    await s.start('ABC234', { capture: false })
    sock.open()
    sock.recv({ type: 'joined' })
    await tick()
    peers[0]!.h.onState('connected')
    const ch = peers[0]!.ch
    ch.onmessage?.({ data: '{"kind":"page","page":2,"pages":9}' })
    expect(s.snapshot.pdfPage).toEqual({ page: 2, pages: 9 })
    ch.onmessage?.({ data: '{"kind":"page","page":3,"pages":9}' })
    expect(s.snapshot.pdfPage).toEqual({ page: 3, pages: 9 })
    ch.onmessage?.({ data: 'garbage' })
    expect(s.snapshot.pdfPage).toEqual({ page: 3, pages: 9 })
    await s.sendDocument({ name: 'a.pdf', type: 'application/pdf', size: 8, slice: () => ({ arrayBuffer: async () => new ArrayBuffer(8) }) }).catch(() => undefined)
    expect(s.snapshot.pdfPage).toBeUndefined()
  })

  it('the presenter page: "Page n of m" and disabled ends (p-disabled); unknown = both enabled and no line', async () => {
    const mk = (extra: object) => mount(PresentingPage, { global: i18n(), props: { kind: 'file', keepBar: false, pdf: true, ...extra } })
    const unknown = mk({})
    expect(unknown.find('[data-testid=presenting-pageof]').exists()).toBe(false)
    expect(unknown.get('[data-testid=send-prev-page]').attributes('disabled')).toBeUndefined()
    expect(unknown.get('[data-testid=send-next-page]').attributes('disabled')).toBeUndefined()
    const first = mk({ page: 1, total: 4 })
    expect(first.get('[data-testid=presenting-pageof]').text()).toBe('Page 1 of 4')
    expect(first.get('[data-testid=send-prev-page]').attributes('disabled')).toBeDefined()
    expect(first.get('[data-testid=send-next-page]').attributes('disabled')).toBeUndefined()
    const last = mk({ page: 4, total: 4 })
    expect(last.get('[data-testid=send-prev-page]').attributes('disabled')).toBeUndefined()
    expect(last.get('[data-testid=send-next-page]').attributes('disabled')).toBeDefined()
    const single = mk({ page: 1, total: 1 })
    expect(single.findAll('button[disabled]')).toHaveLength(2)
  })

  it('the page row is part of the bottom action area, right above Stop', () => {
    const w = mount(PresentingPage, { global: i18n(), props: { kind: 'file', keepBar: false, pdf: true, page: 2, total: 5 } })
    const bottom = w.get('.cp-bottom')
    expect([...bottom.element.children].map((c) => c.className.split(' ')[0])).toEqual(['cp-pdf', 'cp-pbtn'])
    expect(bottom.get('.cp-pdf').element.children[0]!.getAttribute('data-testid')).toBe('presenting-pageof')
  })
})

describe('content states while presenting (decision 13.41)', () => {
  const base: ReceiverState = {
    phase: 'receiving', code: 'K7QM2W', joinMode: 'open', requests: [], peerName: 'Lena', blocks: [], notice: null,
    errorCode: null, stream: null, content: null, transfer: null, contentError: null,
  }
  beforeEach(() => {
    HTMLCanvasElement.prototype.getContext = (() => null) as never
    vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  })
  afterEach(() => {
    vi.unstubAllGlobals()
    document.body.innerHTML = ''
  })
  const mk = (state: ReceiverState, controlsShown = true) =>
    mount(PresentingStage, { attachTo: document.body, global: i18n(), props: { state, renderPage: async () => ({ width: 1, height: 1, rgba: new Uint8ClampedArray(4) }), controlsShown } })

  it('the transfer progress is a name-tag-look pill in the canvas layer, a content error is no pill (it is a toast)', async () => {
    const w = mk({ ...base, content: { kind: 'image', url: 'blob:x', name: 'a.png' }, transfer: { name: 'b.png', received: 50, size: 100 }, contentError: 'stalled' })
    const pill = w.get('[data-testid=transfer]')
    expect(pill.text()).toBe('Receiving b.png… 50 %')
    expect(pill.classes()).toEqual(expect.arrayContaining(['cd-tag', 'cd-transfer']))
    expect(pill.element.closest('.cd-hud')).not.toBeNull()
    expect(w.find('[data-testid=content-error]').exists()).toBe(false)
    expect(w.html()).not.toMatch(/bg-err|bg-black\/75|text-white/)
  })

  it('black screen, frozen picture and strokes end with the presentation; the next one is live and visible', async () => {
    const w = mk({ ...base, content: { kind: 'image', url: 'blob:x', name: 'a.png' } })
    await w.get('[data-testid=tb-black]').trigger('click')
    expect(w.find('[data-testid=black-screen]').exists()).toBe(true)
    // the presenter stops: nothing is shown any more
    await w.setProps({ state: { ...base } })
    await nextTick()
    expect(w.find('[data-testid=black-screen]').exists()).toBe(false)
    // the presenter shares again: the picture is visible
    await w.setProps({ state: { ...base, content: { kind: 'image', url: 'blob:y', name: 'b.png' } } })
    expect(w.find('[data-testid=black-screen]').exists()).toBe(false)
    expect(w.find('[data-testid=received-image]').exists()).toBe(true)
  })
})

describe('the style rules of fix list 2 (read from the CSS)', () => {
  const read = (rel: string) => readFileSync(fileURLToPath(new URL(rel, import.meta.url)), 'utf8')
  const display = read('../src/styles/display.css')
  const presenter = read('../src/styles/presenter.css')
  const rule = (css: string, selector: string) => {
    const i = css.indexOf(`${selector} {`)
    expect(i, selector).toBeGreaterThanOrEqual(0)
    return css.slice(i, css.indexOf('}', i))
  }
  it('every control layer (toasts, dialogs, toolbar, name tag, End, pills) is on the centred 960 x 540 canvas, not the viewport', () => {
    const hud = rule(display, '.cd-hud')
    expect(hud).toContain('width: 960px')
    expect(hud).toContain('height: 540px')
    expect(hud).not.toMatch(/100vw|100vh/)
    expect(hud).toContain('translate(-50%, -50%) scale(var(--s, 1))')
  })
  it('QR box: Light has a 1 px --line edge; the empty box is --panel2 with a --line border at full opacity', () => {
    expect(rule(display, ".cd[data-theme='light'] .cd-qrbox")).toContain('border: 1px solid var(--line)')
    const empty = rule(display, '.cd-qrbox.empty')
    expect(empty).toContain('background: var(--panel2)')
    expect(empty).toContain('border: 1px solid var(--line)')
    expect(empty).toContain('opacity: 1')
  })
  it('text fields get the display focus ring without scale; the thin focus border goes', () => {
    expect(rule(display, ".cd input[type='text']:focus")).toContain('outline: none')
    const ring = rule(display, ".cd:not(.cd-pointer) input[type='text']:focus")
    expect(ring).toContain('outline: 4px solid var(--focus)')
    expect(ring).toContain('outline-offset: 3px')
    expect(ring).toContain('box-shadow: 0 0 0 9px')
    expect(ring).not.toContain('scale')
  })
  it('the wizard card top is fixed at 64 from step 1 on', () => {
    expect(rule(display, '.cd-setup.top')).toMatch(/padding-top: 64px/)
  })
  it('laptop presenter pages: top padding max(40px, 10vh), bottom 40; the relay page has no card or scrim', () => {
    expect(presenter).toMatch(/padding: max\(40px, 10vh\) 40px 40px/)
    expect(presenter).not.toContain('cp-over')
  })
  it('the warning lines are 16/700 --fg with the icon in --acc', () => {
    const line = rule(display, '.cd-wz .cd-warn')
    expect(line).toContain('font-size: 16px')
    expect(line).toContain('font-weight: 700')
    expect(line).toContain('color: var(--fg)')
    expect(rule(display, '.cd-wz .cd-warn svg')).toContain('color: var(--acc)')
  })
})
