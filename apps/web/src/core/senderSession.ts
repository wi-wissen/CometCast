// SPDX-License-Identifier: GPL-3.0-or-later
//
// Sender state machine: capture the screen, join the room, send the offer, stream until stopped.
import { createPeer, type Peer, type PeerFactory, type PeerState } from './peer'
import { CONTROL_CHANNEL_LABEL, ControlSender, type ControlCmd, type ImageSource } from './control'
import { SignalingClient, type ServerMessage, type SocketFactory } from './signaling'
import { BusyMemory } from './busyMemory'
import type { IceFetchState } from './altchaClient'

export type SenderPhase =
  | 'idle'
  | 'capturing' // waiting for the browser's screen picker
  | 'connecting'
  | 'joining'
  | 'waiting_approval' // the host has to allow the join; nothing is sent until then
  | 'negotiating'
  | 'sharing'
  | 'peer_left' // the receiver went away
  | 'stopped' // the user stopped sharing
  | 'error'

export interface SenderState {
  phase: SenderPhase
  /** Server error code or a local one: capture_denied, capture_failed, connection_lost, peer_failed. */
  errorCode: string | null
  /** True while the control data channel is open (content can be sent). */
  channelOpen: boolean
  /** Connection check code once connected (compare with the receiver), else absent. */
  sas?: string
  /** While `waiting_approval`: the two-digit number that the display shows on the request. */
  check?: number
  /** While `waiting_approval`: seconds until the server gives up. */
  timeoutSecs?: number
  /** The display's name for its room (`join_pending.room_name` / `joined.room_name`); absent when it set none or the server is older. Never replaced by the code. */
  roomName?: string
  /** Milliseconds since the epoch when an error happened (for "ask again in 15 s"). */
  errorAt?: number
  /** With `peer_failed`: how the last `GET /ice-servers` of this connection ended (spec 6, decision 13.45). Absent when unknown. */
  iceState?: IceFetchState
  /** The shown PDF page and the page count, as reported by the display (`page` message of the control channel, decision 13.31). */
  pdfPage?: { page: number; pages: number }
  /** True while the screen is being shown (see {@link SenderSession.shareScreen}). */
  screen?: boolean
}

export interface SenderDeps {
  url: string
  onChange: (s: SenderState) => void
  /** Omit for content-only sessions (images, links): no screen is captured, only the data channel is used. */
  getStream?: () => Promise<MediaStream>
  socketFactory?: SocketFactory
  peerFactory?: PeerFactory
  /** STUN/TURN list (GET /ice-servers). Must not reject; an empty list means none. */
  iceServers?: () => Promise<RTCIceServer[]>
  /** How the last fetch of `iceServers` ended (read when the connection fails; nothing is shown before that). */
  iceState?: () => IceFetchState | undefined
  /** Name shown to the host when it has to approve the join (the server cleans it). */
  name?: () => string | undefined
  /** Test seam: the clock (default Date.now). */
  now?: () => number
}

/** How long a renegotiation waits for its answer before the next one may go out. */
const RENEGOTIATE_ANSWER_MS = 10_000

export class SenderSession {
  private state: SenderState = { phase: 'idle', errorCode: null, channelOpen: false }
  private control: ControlSender | null = null
  private channelOpen = false
  private sas: string | undefined
  private check: { check: number; timeoutSecs: number } | undefined
  private roomName: string | undefined
  private client: SignalingClient | null = null
  private peer: Peer | null = null
  private stream: MediaStream | null = null
  private screenOn = false
  private run = 0 // invalidates async work of an earlier start()/stop()
  /** Renegotiations run one after another: the next offer waits for the answer to the previous one. */
  private offerChain: Promise<void> = Promise.resolve()
  /** Called when the answer to the renegotiation offer in flight has been applied (or given up). */
  private answered: (() => void) | null = null
  private readonly busy = new BusyMemory()
  private pdfPage: { page: number; pages: number } | undefined
  private icePromise: Promise<RTCIceServer[]> = Promise.resolve([])
  private readonly peerFactory: PeerFactory

  constructor(private readonly deps: SenderDeps) {
    this.peerFactory = deps.peerFactory ?? ((h, c) => createPeer(h, c))
  }

  get snapshot(): SenderState {
    return this.state
  }

  private set(phase: SenderPhase, errorCode: string | null = null): void {
    this.state = { phase, errorCode, channelOpen: this.channelOpen }
    if (phase === 'sharing' && this.sas) this.state.sas = this.sas
    if (phase === 'waiting_approval' && this.check) {
      this.state.check = this.check.check
      this.state.timeoutSecs = this.check.timeoutSecs
    }
    if (phase === 'error') {
      this.state.errorAt = (this.deps.now ?? Date.now)()
      const ice = errorCode === 'peer_failed' ? this.deps.iceState?.() : undefined
      if (ice) this.state.iceState = ice
    }
    if (this.pdfPage && phase === 'sharing') this.state.pdfPage = this.pdfPage
    if (this.roomName && (phase === 'waiting_approval' || phase === 'negotiating' || phase === 'sharing' || phase === 'peer_left')) {
      this.state.roomName = this.roomName
    }
    if (this.screenOn) this.state.screen = true
    this.deps.onChange(this.state)
  }

  private setPdfPage(p: { page: number; pages: number } | undefined): void {
    const cur = this.pdfPage
    if (cur?.page === p?.page && cur?.pages === p?.pages) return
    this.pdfPage = p
    const { pdfPage: _gone, ...rest } = this.state
    this.state = p ? { ...rest, pdfPage: p } : rest
    this.deps.onChange(this.state)
  }

  private setChannelOpen(open: boolean): void {
    if (this.channelOpen === open) return
    this.channelOpen = open
    this.state = { ...this.state, channelOpen: open }
    this.deps.onChange(this.state)
  }

  private requireControl(): ControlSender {
    if (!this.control) throw new Error('control channel not open')
    return this.control
  }

  sendImage(file: ImageSource, onProgress?: (sent: number, total: number) => void): Promise<void> {
    return this.requireControl().sendImage(file, onProgress)
  }

  /** Sends a PDF (same protocol as the native clients). `file` is a File or Blob. */
  sendDocument(file: ImageSource, onProgress?: (sent: number, total: number) => void): Promise<void> {
    // The previous document's page state does not apply to the next one.
    this.setPdfPage(undefined)
    return this.requireControl().sendDocument(file, onProgress)
  }

  /** Returns false if the URL is not an acceptable http(s) link. */
  sendLink(url: string): boolean {
    return this.requireControl().sendLink(url)
  }

  sendCmd(cmd: ControlCmd): void {
    this.requireControl().sendCmd(cmd)
  }

  /**
   * Shows the screen on an already connected session: asks for the screen (must be called from a
   * user gesture), adds it to the connection and negotiates again. Returns 'cancelled' when the
   * person closed the system dialog, 'failed' when the browser could not capture and 'ok' when the
   * screen is being sent. The session stays connected when the screen stops (see stopScreen).
   */
  async shareScreen(): Promise<'ok' | 'cancelled' | 'failed'> {
    const getStream = this.deps.getStream
    const run = this.run
    if (!getStream || !this.peer || !this.client || this.state.phase !== 'sharing' || this.screenOn) return 'failed'
    let stream: MediaStream
    try {
      stream = await getStream()
    } catch (e) {
      const denied = e instanceof DOMException && (e.name === 'NotAllowedError' || e.name === 'AbortError')
      return denied ? 'cancelled' : 'failed'
    }
    if (run !== this.run || !this.peer || !this.client) {
      stream.getTracks().forEach((t) => t.stop())
      return 'cancelled'
    }
    this.stream = stream
    this.screenOn = true
    // The browser's own "stop sharing" button ends the track.
    stream.getVideoTracks()[0]?.addEventListener('ended', () => {
      if (run === this.run && this.stream === stream) void this.stopScreen()
    })
    try {
      this.peer.addStream(stream)
      await this.renegotiate(run)
    } catch {
      if (run === this.run) this.fail('peer_failed')
      return 'failed'
    }
    this.state = { ...this.state, screen: true }
    this.deps.onChange(this.state)
    return 'ok'
  }

  /** Stops showing the screen; the session stays connected (the host's display goes back to idle). */
  async stopScreen(): Promise<void> {
    const stream = this.stream
    if (!stream || !this.screenOn) return
    const run = this.run
    this.screenOn = false
    this.stream = null
    this.peer?.removeStream?.(stream)
    stream.getTracks().forEach((t) => t.stop())
    const { screen: _screen, ...rest } = this.state
    this.state = rest
    this.deps.onChange(this.state)
    try {
      await this.renegotiate(run)
    } catch {
      /* the connection is going away anyway */
    }
  }

  /**
   * Sends a new offer on the current connection once the previous renegotiation has its answer.
   * Two offers in flight at once (share, then the browser's "stop sharing" right away) would be
   * answered out of order and fail the connection.
   */
  private renegotiate(run: number): Promise<void> {
    // `sent` settles once the offer is out (or creating it failed); the chain itself goes on until
    // the answer has been applied, so the next offer waits for it.
    let offerSent!: () => void
    let offerFailed!: (e: unknown) => void
    const sent = new Promise<void>((resolve, reject) => {
      offerSent = resolve
      offerFailed = reject
    })
    this.offerChain = this.offerChain.then(async () => {
      const peer = this.peer
      if (run !== this.run || !peer) return offerSent()
      let sdp: string
      try {
        sdp = await peer.createOffer()
      } catch (e) {
        return offerFailed(e)
      }
      if (run !== this.run || this.peer !== peer) return offerSent()
      await new Promise<void>((resolve) => {
        // UNCERTAIN: 10 s is a placeholder; an answer that never comes must not block later offers.
        const timer = setTimeout(done, RENEGOTIATE_ANSWER_MS)
        function done() {
          clearTimeout(timer)
          resolve()
        }
        this.answered = done
        this.client?.send({ type: 'offer', sdp })
        offerSent()
      })
    })
    return sent
  }

  /**
   * Must be called from a user gesture (getDisplayMedia requires one). With `capture: false` no
   * screen is asked for (even when `getStream` is set): the session only joins; the screen can be
   * added later with {@link shareScreen}.
   */
  async start(code: string, opts: { capture?: boolean } = {}): Promise<void> {
    this.teardown()
    const run = ++this.run
    // Started first, so it runs while the screen picker is open. Failure means "no list".
    this.icePromise = this.deps.iceServers ? this.deps.iceServers().catch(() => []) : Promise.resolve([])
    if (this.deps.getStream && opts.capture !== false) {
      this.set('capturing')
      let stream: MediaStream
      try {
        // Capture first, so the user gesture is still fresh; the network comes after.
        stream = await this.deps.getStream()
      } catch (e) {
        if (run !== this.run) return
        const denied = e instanceof DOMException && (e.name === 'NotAllowedError' || e.name === 'AbortError')
        this.set('error', denied ? 'capture_denied' : 'capture_failed')
        return
      }
      if (run !== this.run) {
        stream.getTracks().forEach((t) => t.stop())
        return
      }
      this.stream = stream
      // The browser's own "stop sharing" button ends the track.
      stream.getVideoTracks()[0]?.addEventListener('ended', () => {
        if (run === this.run) this.stop()
      })
    }
    this.set('connecting')
    this.client = new SignalingClient(
      this.deps.url,
      'sender',
      {
        onReady: () => {
          this.set('joining')
          const name = this.deps.name?.()
          this.client?.send({ type: 'join_room', code, ...(name ? { name } : {}) })
        },
        onMessage: (m) => this.onMessage(m),
        onClose: () => this.fail('connection_lost'),
      },
      this.deps.socketFactory,
    )
    this.client.open()
  }

  stop(): void {
    this.run++
    this.teardown()
    this.set('stopped')
  }

  private teardown(): void {
    this.answered?.()
    this.answered = null
    this.offerChain = Promise.resolve()
    this.sas = undefined
    this.check = undefined
    this.roomName = undefined
    this.control = null
    this.channelOpen = false
    this.pdfPage = undefined
    this.peer?.close()
    this.peer = null
    this.client?.close()
    this.client = null
    this.stream?.getTracks().forEach((t) => t.stop())
    this.stream = null
    this.screenOn = false
  }

  private fail(code: string): void {
    const p = this.state.phase
    if (p === 'error' || p === 'idle' || p === 'stopped' || p === 'peer_left') return
    // For 60 s after a refusal a closed connection is the same refusal (the server closes it after a ban).
    const reported = this.busy.apply(code, (this.deps.now ?? Date.now)())
    this.teardown()
    this.set('error', reported)
  }

  private onMessage(m: ServerMessage): void {
    switch (m.type) {
      case 'join_pending':
        this.check = { check: m.check, timeoutSecs: m.timeout_secs }
        this.roomName = m.room_name
        this.set('waiting_approval')
        break
      case 'joined':
        this.check = undefined
        // `joined` carries the same name as `join_pending`; take the newest, keep the earlier one if it is absent.
        if (m.room_name) this.roomName = m.room_name
        this.set('negotiating')
        void this.sendOffer()
        break
      case 'answer': {
        const peer = this.peer
        const done = this.answered
        this.answered = null
        if (!peer) {
          done?.()
          break
        }
        peer
          .acceptAnswer(m.sdp)
          .catch(() => {
            // A peer that was closed or replaced meanwhile is not this connection's failure.
            if (this.peer === peer) this.fail('peer_failed')
          })
          .finally(() => done?.())
        break
      }
      case 'ice_candidate':
        this.peer?.addIceCandidate(m.candidate).catch(() => {
          // A single bad candidate is not fatal.
        })
        break
      case 'peer_left':
        // Work still running for this connection (offer, answer) must not turn "ended" into an error.
        this.run++
        this.control = null
        this.channelOpen = false
        this.peer?.close()
        this.peer = null
        this.client?.close()
        this.client = null
        this.stream?.getTracks().forEach((t) => t.stop())
        this.stream = null
        this.screenOn = false
        this.set('peer_left')
        break
      case 'error':
        // Locked while this sender waited: the code was right, the room was locked in the meantime.
        if (m.code === 'room_locked' && this.state.phase === 'waiting_approval') this.fail('room_locked_waiting')
        else this.fail(m.code)
        break
      default:
        break
    }
  }

  private async sendOffer(): Promise<void> {
    const run = this.run
    try {
      const iceServers = await this.icePromise
      if (run !== this.run || !this.client) return // stopped, or the receiver left meanwhile
      const peer = this.peerFactory(
        {
          onIceCandidate: (candidate) => this.client?.send({ type: 'ice_candidate', candidate }),
          onTrack: () => {},
          onState: (s: PeerState) => {
            if (s === 'connected') {
              this.set('sharing')
              void peer.connectionCode?.().then((c) => {
                if (c && run === this.run && this.peer === peer) {
                  this.sas = c
                  this.state = { ...this.state, sas: c }
                  this.deps.onChange(this.state)
                }
              })
            }
            else if (s === 'failed') this.fail('peer_failed')
          },
        },
        { iceServers },
      )
      this.peer = peer
      if (this.stream) peer.addStream(this.stream)
      // Created before the offer so that the offer contains the data section.
      const ch = peer.openChannel(CONTROL_CHANNEL_LABEL)
      ch.binaryType = 'arraybuffer'
      this.control = new ControlSender(ch, (p) => {
        if (run === this.run) this.setPdfPage(p)
      })
      ch.onopen = () => {
        if (run === this.run) this.setChannelOpen(true)
      }
      ch.onclose = () => {
        if (run === this.run) this.setChannelOpen(false)
      }
      const sdp = await peer.createOffer()
      if (run !== this.run) return
      this.client?.send({ type: 'offer', sdp })
    } catch {
      if (run === this.run) this.fail('peer_failed')
    }
  }
}
