// SPDX-License-Identifier: GPL-3.0-or-later
//
// Sender state machine: capture the screen, join the room, send the offer, stream until stopped.
import { createPeer, type Peer, type PeerFactory, type PeerState } from './peer'
import { CONTROL_CHANNEL_LABEL, ControlSender, type ControlCmd, type ImageSource } from './control'
import { SignalingClient, type ServerMessage, type SocketFactory } from './signaling'

export type SenderPhase =
  | 'idle'
  | 'capturing' // waiting for the browser's screen picker
  | 'connecting'
  | 'joining'
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
}

export class SenderSession {
  private state: SenderState = { phase: 'idle', errorCode: null, channelOpen: false }
  private control: ControlSender | null = null
  private channelOpen = false
  private client: SignalingClient | null = null
  private peer: Peer | null = null
  private stream: MediaStream | null = null
  private run = 0 // invalidates async work of an earlier start()/stop()
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

  /** Returns false if the URL is not an acceptable http(s) link. */
  sendLink(url: string): boolean {
    return this.requireControl().sendLink(url)
  }

  sendCmd(cmd: ControlCmd): void {
    this.requireControl().sendCmd(cmd)
  }

  /** Must be called from a user gesture (getDisplayMedia requires one). */
  async start(code: string): Promise<void> {
    this.teardown()
    const run = ++this.run
    // Started first, so it runs while the screen picker is open. Failure means "no list".
    this.icePromise = this.deps.iceServers ? this.deps.iceServers().catch(() => []) : Promise.resolve([])
    if (this.deps.getStream) {
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
          this.client?.send({ type: 'join_room', code })
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
    this.control = null
    this.channelOpen = false
    this.peer?.close()
    this.peer = null
    this.client?.close()
    this.client = null
    this.stream?.getTracks().forEach((t) => t.stop())
    this.stream = null
  }

  private fail(code: string): void {
    const p = this.state.phase
    if (p === 'error' || p === 'idle' || p === 'stopped') return
    this.teardown()
    this.set('error', code)
  }

  private onMessage(m: ServerMessage): void {
    switch (m.type) {
      case 'joined':
        this.set('negotiating')
        void this.sendOffer()
        break
      case 'answer':
        this.peer?.acceptAnswer(m.sdp).catch(() => this.fail('peer_failed'))
        break
      case 'ice_candidate':
        this.peer?.addIceCandidate(m.candidate).catch(() => {
          // A single bad candidate is not fatal.
        })
        break
      case 'peer_left':
        this.control = null
        this.channelOpen = false
        this.peer?.close()
        this.peer = null
        this.client?.close()
        this.client = null
        this.stream?.getTracks().forEach((t) => t.stop())
        this.stream = null
        this.set('peer_left')
        break
      case 'error':
        this.fail(m.code)
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
            if (s === 'connected') this.set('sharing')
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
      this.control = new ControlSender(ch)
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
