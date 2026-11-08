// SPDX-License-Identifier: GPL-3.0-or-later
//
// Pause from either side (docs/design/pause.md): the two flags, the wire messages, the display's frozen badge and the
// presenter's banner. Sessions run against each other over a fake data channel; the components are mounted with states.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createAppI18n } from '../src/i18n'
import { ReceiverSession, type ReceiverState } from '../src/core/receiverSession'
import { SenderSession } from '../src/core/senderSession'
import { parseControlMessage } from '../src/core/control'
import { createPeer } from '../src/core/peer'
import type { Peer, PeerHandlers } from '../src/core/peer'
import type { ControlChannel } from '../src/core/control'
import PresentingStage from '../src/components/display/PresentingStage.vue'
import PresentingPage from '../src/components/presenter/PresentingPage.vue'
import { FakeSocket } from './helpers/fakeSocket'
import { pair, type FakeChannel } from './helpers/fakeChannel'

const tick = () => new Promise((r) => setTimeout(r, 0))
const i18n = (l: 'en' | 'de' = 'en') => ({ plugins: [createAppI18n(l)] })

class P implements Peer {
  paused: boolean[] = []
  constructor(public h: PeerHandlers, public ch?: ControlChannel) {}
  addStream() {}
  removeStream() {}
  async setStreamPaused(_s: MediaStream, paused: boolean) {
    this.paused.push(paused)
  }
  openChannel(): ControlChannel {
    return this.ch!
  }
  async createOffer() {
    return 'O'
  }
  async acceptOffer() {
    return 'A'
  }
  async acceptAnswer() {}
  async addIceCandidate() {}
  close() {}
}

const track = { addEventListener() {}, stop() {} }
const fakeStream = { getTracks: () => [track], getVideoTracks: () => [track] } as unknown as MediaStream

/** A presenter and a display connected by a fake channel, a screen shown on both. */
async function rig() {
  const [a, b] = pair()
  // Display
  const dSock = new FakeSocket()
  let dp!: P
  const display = new ReceiverSession({ url: 'ws://x/ws', onChange: () => {}, socketFactory: () => dSock, peerFactory: (h) => (dp = new P(h)) })
  display.start()
  dSock.recv({ type: 'room_created', code: 'ABC234', join_mode: 'open' })
  dSock.recv({ type: 'peer_joined', name: 'Ada' })
  dp.h.onDataChannel!(b)
  dp.h.onTrack(fakeStream)
  // Presenter: joins without capture and shows the screen once connected (the way the page does it)
  const pSock = new FakeSocket()
  let pp!: P
  const presenter = new SenderSession({
    url: 'ws://x/ws',
    onChange: () => {},
    socketFactory: () => pSock,
    peerFactory: (h) => (pp = new P(h, a)),
    getStream: async () => fakeStream,
  })
  await presenter.start('ABC234', { capture: false })
  pSock.open()
  pSock.recv({ type: 'hello', proto_version: 2 })
  pSock.recv({ type: 'joined' })
  await tick()
  pp.h.onState('connected')
  ;(a as FakeChannel).onopen?.({})
  await tick()
  expect(await presenter.shareScreen()).toBe('ok')
  pSock.recv({ type: 'answer', sdp: 'A' }) // the renegotiation of the screen is answered
  await tick()
  return { display, presenter, dp, pp, a, b, dSock, pSock }
}

describe('the wire messages', () => {
  it('pause and resume are commands; pause_state needs two booleans', () => {
    expect(parseControlMessage('{"kind":"cmd","cmd":"pause"}')).toEqual({ kind: 'cmd', cmd: 'pause' })
    expect(parseControlMessage('{"kind":"cmd","cmd":"resume"}')).toEqual({ kind: 'cmd', cmd: 'resume' })
    expect(parseControlMessage('{"kind":"cmd","cmd":"Pause"}')).toBeNull()
    expect(parseControlMessage('{"kind":"pause_state","presenter":true,"display":false}')).toEqual({ kind: 'pause_state', presenter: true, display: false })
    expect(parseControlMessage('{"kind":"pause_state","presenter":1,"display":false}')).toBeNull()
    expect(parseControlMessage('{"kind":"pause_state","presenter":true}')).toBeNull()
  })
})

describe('the flags (sessions over a fake channel)', () => {
  it('the display tells the presenter both flags once when the channel opens, and the presenter keeps no state for it', async () => {
    const { b, presenter } = await rig()
    expect((b as FakeChannel).sent).toEqual(['{"kind":"pause_state","presenter":false,"display":false}'])
    expect(presenter.snapshot.pause).toBeUndefined()
  })

  it('presenter pause: the video stops, the display is frozen, the presenter hears the echo', async () => {
    const { display, presenter, pp } = await rig()
    await presenter.pausePresenter()
    await tick()
    expect(pp.paused).toEqual([true])
    expect(display.snapshot.pause).toEqual({ presenter: true, display: false })
    expect(presenter.snapshot.pause).toEqual({ requested: true, presenter: true, display: false })
    await presenter.resumePresenter()
    await tick()
    expect(pp.paused).toEqual([true, false])
    expect(display.snapshot.pause).toEqual({ presenter: false, display: false })
    expect(presenter.snapshot.pause).toBeUndefined()
  })

  it('the presenter stops sending video first and tells the display after; on resume it tells first', async () => {
    const { presenter, pp, a } = await rig()
    const order: string[] = []
    pp.setStreamPaused = async (_s, p) => {
      order.push(`video:${p ? 'off' : 'on'}`)
    }
    const send = a.send.bind(a)
    a.send = (d) => {
      order.push(`msg:${typeof d === 'string' ? (JSON.parse(d).cmd ?? JSON.parse(d).kind) : 'bin'}`)
      send(d)
    }
    await presenter.pausePresenter()
    await presenter.resumePresenter()
    expect(order).toEqual(['video:off', 'msg:pause', 'msg:resume', 'video:on'])
  })

  it('display freeze: the presenter keeps sending and is told', async () => {
    const { display, presenter, pp } = await rig()
    display.setDisplayPaused(true)
    await tick()
    expect(display.snapshot.pause).toEqual({ presenter: false, display: true })
    expect(presenter.snapshot.pause).toEqual({ requested: false, presenter: false, display: true })
    expect(pp.paused).toEqual([]) // the presenter's video goes on
    display.setDisplayPaused(false)
    await tick()
    expect(presenter.snapshot.pause).toBeUndefined()
  })

  it('both flags: resuming one side keeps the other frozen; each side clears only its own', async () => {
    const { display, presenter } = await rig()
    display.setDisplayPaused(true)
    await presenter.pausePresenter()
    await tick()
    expect(display.snapshot.pause).toEqual({ presenter: true, display: true })
    await presenter.resumePresenter()
    await tick()
    expect(display.snapshot.pause).toEqual({ presenter: false, display: true })
    expect(presenter.snapshot.pause).toEqual({ requested: false, presenter: false, display: true })
    await presenter.pausePresenter()
    await tick()
    display.setDisplayPaused(false)
    await tick()
    expect(display.snapshot.pause).toEqual({ presenter: true, display: false })
    expect(presenter.snapshot.pause).toEqual({ requested: true, presenter: true, display: false })
  })

  it('pausing twice is idempotent: one command, one echo', async () => {
    const { presenter, pp, b } = await rig()
    ;(b as FakeChannel).sent.length = 0
    await presenter.pausePresenter()
    await presenter.pausePresenter()
    await tick()
    expect(pp.paused).toEqual([true])
    expect((b as FakeChannel).sent).toEqual(['{"kind":"pause_state","presenter":true,"display":false}'])
  })

  it('the display ignores a pause without a screen, and a display ignores pause_state while a presenter ignores commands', async () => {
    const { display, dp, a } = await rig()
    dp.h.onTrackEnded!(fakeStream)
    expect(display.snapshot.stream).toBeNull()
    display.setDisplayPaused(true)
    a.send('{"kind":"cmd","cmd":"pause"}')
    await tick()
    expect(display.snapshot.pause ?? { presenter: false, display: false }).toEqual({ presenter: false, display: false })
    // A display ignores a pause_state it receives (a bad message is told, a good one is silent).
    dp.h.onTrack(fakeStream)
    a.send('{"kind":"pause_state","presenter":true,"display":true}')
    await tick()
    expect(display.snapshot.pause).toBeUndefined()
    expect(display.snapshot.contentError).toBeNull()
  })

  it('the screen ending clears both flags on both sides (display: stream gone; presenter: stopScreen)', async () => {
    const { display, presenter, dp, b, pSock } = await rig()
    display.setDisplayPaused(true)
    await presenter.pausePresenter()
    await tick()
    ;(b as FakeChannel).sent.length = 0
    dp.h.onTrackEnded!(fakeStream)
    expect(display.snapshot.pause).toEqual({ presenter: false, display: false })
    await tick()
    // The presenter hears both flags false and holds no flag of the display; its own request is still its own.
    expect(presenter.snapshot.pause).toEqual({ requested: true, presenter: false, display: false })
    const stopped = presenter.stopScreen()
    await tick()
    pSock.recv({ type: 'answer', sdp: 'A' })
    await stopped
    expect(presenter.snapshot.pause).toBeUndefined()
  })

  it('the peer leaving clears the flags of the display', async () => {
    const { display, presenter, dSock } = await rig()
    await presenter.pausePresenter()
    await tick()
    expect(display.snapshot.pause?.presenter).toBe(true)
    dSock.recv({ type: 'peer_left' })
    expect(display.snapshot.pause).toEqual({ presenter: false, display: false })
    expect(display.snapshot.stream).toBeNull()
  })

  it('a display that does not echo leaves the presenter in a pending state (asked, not confirmed)', async () => {
    const { presenter, b } = await rig()
    b.onmessage = null // an old display: nobody answers
    await presenter.pausePresenter()
    expect(presenter.snapshot.pause).toEqual({ requested: true, presenter: false, display: false })
  })
})

describe('the display: frozen badge and canvas', () => {
  const base: ReceiverState = {
    phase: 'receiving', code: 'K7QM2W', joinMode: 'open', requests: [], peerName: 'Lena', blocks: [], notice: null,
    errorCode: null, stream: fakeStream, content: null, transfer: null, contentError: null,
  }
  beforeEach(() => {
    HTMLCanvasElement.prototype.getContext = (() => null) as never
    vi.stubGlobal('ResizeObserver', class { observe() {} unobserve() {} disconnect() {} })
  })
  afterEach(() => {
    vi.unstubAllGlobals()
    document.body.innerHTML = ''
  })
  const mk = (state: ReceiverState, locale: 'en' | 'de' = 'en') =>
    mount(PresentingStage, { attachTo: document.body, global: i18n(locale), props: { state, renderPage: async () => ({ width: 1, height: 1, rgba: new Uint8ClampedArray(4) }), controlsShown: false } })
  const give = (w: ReturnType<typeof mk>) => {
    const v = w.get('video').element as HTMLVideoElement
    Object.defineProperty(v, 'videoWidth', { value: 640, configurable: true })
    Object.defineProperty(v, 'videoHeight', { value: 360, configurable: true })
  }

  it('the presenter paused: frozen, with the presenter text; resumed: live again', async () => {
    const w = mk(base)
    give(w)
    expect(w.find('[data-testid=frozen-badge]').exists()).toBe(false)
    await w.setProps({ state: { ...base, pause: { presenter: true, display: false } } })
    expect(w.get('[data-testid=stage]').attributes('data-frozen')).toBe('true')
    expect(w.get('[data-testid=frozen-badge]').text()).toBe('Paused by the presenter')
    expect(w.get('[data-testid=freeze-canvas]').isVisible()).toBe(true)
    await w.setProps({ state: { ...base, pause: { presenter: false, display: false } } })
    expect(w.get('[data-testid=stage]').attributes('data-frozen')).toBe('false')
    expect(w.find('[data-testid=frozen-badge]').exists()).toBe(false)
  })

  it('only the display froze: the existing text; both: the presenter text; the presenter resumes: the display text again', async () => {
    const w = mk(base)
    give(w)
    await w.setProps({ state: { ...base, pause: { presenter: false, display: true } } })
    // (the local flag follows the button; the session confirms it)
    const setup = (w.vm as unknown as { $: { setupState: { toolbar: { frozen: boolean } } } }).$.setupState
    setup.toolbar = { ...setup.toolbar, frozen: true }
    await w.vm.$nextTick()
    expect(w.get('[data-testid=frozen-badge]').text()).toBe('Frozen. The live stream keeps running in the background.')
    await w.setProps({ state: { ...base, pause: { presenter: true, display: true } } })
    expect(w.get('[data-testid=frozen-badge]').text()).toBe('Paused by the presenter')
    await w.setProps({ state: { ...base, pause: { presenter: false, display: true } } })
    expect(w.get('[data-testid=frozen-badge]').text()).toBe('Frozen. The live stream keeps running in the background.')
    expect(w.get('[data-testid=stage]').attributes('data-frozen')).toBe('true')
  })

  it('the Freeze button sets the display flag and tells the session; pressing it again clears it', async () => {
    const w = mk(base)
    give(w)
    const setup = (w.vm as unknown as { $: { setupState: { onFreeze: () => void } } }).$.setupState
    setup.onFreeze()
    await w.vm.$nextTick()
    expect(w.emitted('displayPause')).toEqual([[true]])
    expect(w.get('[data-testid=stage]').attributes('data-frozen')).toBe('true')
    setup.onFreeze()
    await w.vm.$nextTick()
    expect(w.emitted('displayPause')).toEqual([[true], [false]])
  })

  it('the German text', async () => {
    const w = mk({ ...base, pause: { presenter: true, display: false } }, 'de')
    expect(w.get('[data-testid=frozen-badge]').text()).toBe('Vom Vortragenden pausiert')
  })
})

describe('the presenter page: button and banner', () => {
  const mk = (pause: { requested: boolean; presenter: boolean; display: boolean } | undefined, kind: 'screen' | 'file' | 'link' = 'screen', locale: 'en' | 'de' = 'en') =>
    mount(PresentingPage, { global: i18n(locale), props: { kind, keepBar: false, pause } })

  it('screen: a Pause button; pressing it asks to pause; no banner while nothing is paused', async () => {
    const w = mk({ requested: false, presenter: false, display: false })
    expect(w.get('[data-testid=presenting-pause]').text()).toBe('Pause')
    expect(w.find('[data-testid=pause-banner]').exists()).toBe(false)
    await w.get('[data-testid=presenting-pause]').trigger('click')
    expect(w.emitted('pause')).toHaveLength(1)
  })

  it('pending: the button already reads Resume, the banner waits for the display', async () => {
    const w = mk({ requested: true, presenter: false, display: false })
    expect(w.get('[data-testid=presenting-pause]').text()).toBe('Resume')
    expect(w.find('[data-testid=pause-banner]').exists()).toBe(false)
    await w.get('[data-testid=presenting-pause]').trigger('click')
    expect(w.emitted('resume')).toHaveLength(1)
  })

  it('confirmed: the banner and a Resume button', () => {
    const w = mk({ requested: true, presenter: true, display: false })
    expect(w.get('[data-testid=pause-banner]').text()).toBe('Paused: the audience sees the last picture.')
    expect(w.get('[data-testid=presenting-pause]').text()).toBe('Resume')
  })

  it('only the display froze: its banner, and the button stays Pause', () => {
    const w = mk({ requested: false, presenter: false, display: true })
    expect(w.get('[data-testid=pause-banner]').text()).toBe('The display has frozen the picture.')
    expect(w.get('[data-testid=presenting-pause]').text()).toBe('Pause')
  })

  it('both: the presenter text', () => {
    const w = mk({ requested: true, presenter: true, display: true })
    expect(w.get('[data-testid=pause-banner]').text()).toBe('Paused: the audience sees the last picture.')
  })

  it('not offered for a file or a link, or without a channel', () => {
    expect(mk({ requested: false, presenter: false, display: false }, 'file').find('[data-testid=presenting-pause]').exists()).toBe(false)
    expect(mk({ requested: false, presenter: false, display: false }, 'link').find('[data-testid=presenting-pause]').exists()).toBe(false)
    expect(mk(undefined).find('[data-testid=presenting-pause]').exists()).toBe(false)
  })

  it('German', () => {
    const w = mk({ requested: true, presenter: true, display: false }, 'screen', 'de')
    expect(w.get('[data-testid=pause-banner]').text()).toBe('Pausiert: Das Publikum sieht das letzte Bild.')
    expect(w.get('[data-testid=presenting-pause]').text()).toBe('Fortsetzen')
  })
})

describe('the real peer: pause is replaceTrack, no renegotiation', () => {
  afterEach(() => vi.unstubAllGlobals())
  it('replaceTrack(null) stops the video, replaceTrack(track) starts it, and a paused sender can still be removed', async () => {
    const calls: string[] = []
    class Sender {
      constructor(public track: unknown) {}
      async replaceTrack(t: unknown) {
        calls.push(t === null ? 'null' : 'track')
        this.track = t
      }
    }
    const senders: Sender[] = []
    class FakePC {
      addTrack(t: unknown) {
        const s = new Sender(t)
        senders.push(s)
        return s
      }
      removeTrack(s: Sender) {
        calls.push('remove')
        s.track = null
      }
      createOffer() {
        calls.push('offer')
        return { sdp: '' }
      }
      close() {}
    }
    vi.stubGlobal('RTCPeerConnection', FakePC)
    const peer = createPeer({ onIceCandidate() {}, onTrack() {}, onState() {} })
    peer.addStream(fakeStream)
    await peer.setStreamPaused!(fakeStream, true)
    await peer.setStreamPaused!(fakeStream, false)
    await peer.setStreamPaused!(fakeStream, true)
    // Stopping the screen while paused: the sender has no track any more, it is found by the track it was added with.
    peer.removeStream!(fakeStream)
    expect(calls).toEqual(['null', 'track', 'null', 'remove'])
  })
})
