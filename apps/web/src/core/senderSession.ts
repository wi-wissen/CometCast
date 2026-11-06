// SPDX-License-Identifier: GPL-3.0-or-later
//
// Sender state machine: capture the screen, join the room, send the offer, stream until stopped.
import { createPeer, type Peer, type PeerFactory, type PeerState } from './peer'
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
}

export interface SenderDeps {
  url: string
  onChange: (s: SenderState) => void
  getStream: () => Promise<MediaStream>
  socketFactory?: SocketFactory
  peerFactory?: PeerFactory
}

export class SenderSession {
  private state: SenderState = { phase: 'idle', errorCode: null }
  private client: SignalingClient | null = null
  private peer: Peer | null = null
  private stream: MediaStream | null = null
  private run = 0 // invalidates async work of an earlier start()/stop()
  private readonly peerFactory: PeerFactory

  constructor(private readonly deps: SenderDeps) {
    this.peerFactory = deps.peerFactory ?? ((h) => createPeer(h))
  }

  get snapshot(): SenderState {
    return this.state
  }

  private set(phase: SenderPhase, errorCode: string | null = null): void {
    this.state = { phase, errorCode }
    this.deps.onChange(this.state)
  }

  /** Must be called from a user gesture (getDisplayMedia requires one). */
  async start(code: string): Promise<void> {
    this.teardown()
    const run = ++this.run
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
      const peer = this.peerFactory({
        onIceCandidate: (candidate) => this.client?.send({ type: 'ice_candidate', candidate }),
        onTrack: () => {},
        onState: (s: PeerState) => {
          if (s === 'connected') this.set('sharing')
          else if (s === 'failed') this.fail('peer_failed')
        },
      })
      this.peer = peer
      if (this.stream) peer.addStream(this.stream)
      const sdp = await peer.createOffer()
      if (run !== this.run) return
      this.client?.send({ type: 'offer', sdp })
    } catch {
      if (run === this.run) this.fail('peer_failed')
    }
  }
}
