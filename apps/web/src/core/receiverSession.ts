// SPDX-License-Identifier: GPL-3.0-or-later
//
// Receiver state machine: creates a room, waits for a sender, answers its offer, exposes the stream.
import { createPeer, type Peer, type PeerFactory, type PeerState } from './peer'
import { SignalingClient, type ServerMessage, type SocketFactory } from './signaling'

export type ReceiverPhase =
  | 'idle'
  | 'connecting' // opening the WebSocket / handshake / room creation
  | 'waiting_sender' // room code is shown
  | 'negotiating' // a sender joined, WebRTC is being set up
  | 'receiving' // media is flowing
  | 'peer_left' // sender left; the room stays, a new sender may join
  | 'error'

export interface ReceiverState {
  phase: ReceiverPhase
  code: string | null
  /** Server error code or a local one (connection_lost, peer_failed). Set when phase is 'error'. */
  errorCode: string | null
  stream: MediaStream | null
}

export interface ReceiverDeps {
  url: string
  onChange: (s: ReceiverState) => void
  socketFactory?: SocketFactory
  peerFactory?: PeerFactory
}

export class ReceiverSession {
  private state: ReceiverState = { phase: 'idle', code: null, errorCode: null, stream: null }
  private client: SignalingClient | null = null
  private peer: Peer | null = null
  private readonly peerFactory: PeerFactory

  constructor(private readonly deps: ReceiverDeps) {
    this.peerFactory = deps.peerFactory ?? ((h) => createPeer(h))
  }

  get snapshot(): ReceiverState {
    return this.state
  }

  private set(patch: Partial<ReceiverState>): void {
    this.state = { ...this.state, ...patch }
    this.deps.onChange(this.state)
  }

  start(): void {
    this.teardown()
    this.set({ phase: 'connecting', code: null, errorCode: null, stream: null })
    this.client = new SignalingClient(
      this.deps.url,
      'receiver',
      {
        onReady: () => this.client?.send({ type: 'create_room' }),
        onMessage: (m) => this.onMessage(m),
        onClose: () => this.fail('connection_lost'),
      },
      this.deps.socketFactory,
    )
    this.client.open()
  }

  stop(): void {
    this.teardown()
    this.set({ phase: 'idle', code: null, errorCode: null, stream: null })
  }

  private teardown(): void {
    this.peer?.close()
    this.peer = null
    this.client?.close()
    this.client = null
  }

  private fail(code: string): void {
    if (this.state.phase === 'error' || this.state.phase === 'idle') return
    this.teardown()
    this.set({ phase: 'error', errorCode: code, stream: null })
  }

  private newPeer(): Peer {
    this.peer?.close()
    const peer = this.peerFactory({
      onIceCandidate: (candidate) => this.client?.send({ type: 'ice_candidate', candidate }),
      onTrack: (stream) => this.set({ stream }),
      onState: (s: PeerState) => {
        if (s === 'connected') this.set({ phase: 'receiving' })
        else if (s === 'failed') this.fail('peer_failed')
      },
    })
    this.peer = peer
    return peer
  }

  private onMessage(m: ServerMessage): void {
    switch (m.type) {
      case 'room_created':
        this.set({ phase: 'waiting_sender', code: m.code })
        break
      case 'peer_joined':
        this.newPeer()
        this.set({ phase: 'negotiating', stream: null })
        break
      case 'offer': {
        const peer = this.peer ?? this.newPeer()
        if (this.state.phase !== 'receiving') this.set({ phase: 'negotiating' })
        peer
          .acceptOffer(m.sdp)
          .then((sdp) => this.client?.send({ type: 'answer', sdp }))
          .catch(() => this.fail('peer_failed'))
        break
      }
      case 'ice_candidate':
        this.peer?.addIceCandidate(m.candidate).catch(() => {
          // A single bad candidate is not fatal; the connection may still succeed with others.
        })
        break
      case 'peer_left':
        this.peer?.close()
        this.peer = null
        this.set({ phase: 'peer_left', stream: null })
        break
      case 'error':
        // not_in_room can arrive when the sender has just left; the room itself survives.
        if (m.code === 'not_in_room' && this.state.phase !== 'connecting') break
        this.fail(m.code)
        break
      default:
        break // control, answer, joined: not used by the receiver in M1
    }
  }
}
