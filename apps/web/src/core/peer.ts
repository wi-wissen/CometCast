// SPDX-License-Identifier: GPL-3.0-or-later
//
// Thin RTCPeerConnection wrapper. Signalling happens elsewhere; this class only deals with WebRTC.
import type { ControlChannel } from './control'
import { connectionCode } from './sas'

export type PeerState = 'connecting' | 'connected' | 'failed'

export interface PeerHandlers {
  /** JSON text of an RTCIceCandidateInit (what the protocol carries in ice_candidate.candidate). */
  onIceCandidate(candidate: string): void
  onTrack(stream: MediaStream): void
  /** The remote side took its track(s) away again (a sender stopped showing its screen but stays connected). */
  onTrackEnded?(stream: MediaStream): void
  onState(state: PeerState): void
  /** The remote side opened a data channel (receiver side). */
  onDataChannel?(channel: ControlChannel): void
}

export interface Peer {
  addStream(stream: MediaStream): void
  /** Removes the tracks of a stream that was added with addStream (the caller renegotiates with createOffer). */
  removeStream?(stream: MediaStream): void
  /** Creates an ordered, reliable data channel (sender side; call before createOffer). */
  openChannel(label: string): ControlChannel
  createOffer(): Promise<string>
  /** Takes the remote offer SDP, returns the answer SDP. */
  acceptOffer(sdp: string): Promise<string>
  acceptAnswer(sdp: string): Promise<void>
  addIceCandidate(candidateJson: string): Promise<void>
  /** Connection check code from the final local and remote SDPs (null if unavailable). */
  connectionCode?(): Promise<string | null>
  close(): void
}

/** `config.iceServers` comes from the server's GET /ice-servers when it offers one (altchaClient.ts). */
export type PeerFactory = (handlers: PeerHandlers, config?: RTCConfiguration) => Peer

// Without a server-provided list there are no STUN/TURN servers, so this only works for
// direct/LAN connections. UNCERTAIN: how well TURN works is not measured (measurement point 1,
// docs/measurement-points.md).
export const DEFAULT_RTC_CONFIG: RTCConfiguration = { iceServers: [] }

export function createPeer(handlers: PeerHandlers, config: RTCConfiguration = DEFAULT_RTC_CONFIG): Peer {
  const pc = new RTCPeerConnection(config)
  let remoteSet = false
  const pending: RTCIceCandidateInit[] = []

  pc.onicecandidate = (ev) => {
    if (ev.candidate) handlers.onIceCandidate(JSON.stringify(ev.candidate.toJSON()))
  }
  const watched = new WeakSet<MediaStream>()
  pc.ontrack = (ev) => {
    const stream = ev.streams[0] ?? new MediaStream([ev.track])
    if (!watched.has(stream)) {
      watched.add(stream)
      // Fired when a renegotiation takes the tracks away (the sender stopped showing, but stays connected).
      stream.addEventListener('removetrack', () => {
        if (stream.getTracks().length === 0) handlers.onTrackEnded?.(stream)
      })
    }
    handlers.onTrack(stream)
  }
  pc.ondatachannel = (ev) => handlers.onDataChannel?.(ev.channel)
  pc.onconnectionstatechange = () => {
    const s = pc.connectionState
    if (s === 'connected') handlers.onState('connected')
    else if (s === 'failed') handlers.onState('failed')
    // UNCERTAIN: 'disconnected' can be temporary; it is not reported as failure here. Not measured yet.
  }

  async function flush(): Promise<void> {
    remoteSet = true
    for (const c of pending.splice(0)) {
      // One bad candidate must not fail the whole negotiation.
      await pc.addIceCandidate(c).catch(() => undefined)
    }
  }

  return {
    addStream(stream) {
      for (const t of stream.getTracks()) pc.addTrack(t, stream)
    },
    removeStream(stream) {
      const tracks = new Set(stream.getTracks())
      for (const s of pc.getSenders()) if (s.track && tracks.has(s.track)) pc.removeTrack(s)
    },
    openChannel(label) {
      return pc.createDataChannel(label, { ordered: true })
    },
    async createOffer() {
      const offer = await pc.createOffer()
      await pc.setLocalDescription(offer)
      return offer.sdp ?? ''
    },
    async acceptOffer(sdp) {
      await pc.setRemoteDescription({ type: 'offer', sdp })
      await flush()
      const answer = await pc.createAnswer()
      await pc.setLocalDescription(answer)
      return answer.sdp ?? ''
    },
    async acceptAnswer(sdp) {
      await pc.setRemoteDescription({ type: 'answer', sdp })
      await flush()
    },
    async addIceCandidate(candidateJson) {
      const init = JSON.parse(candidateJson) as RTCIceCandidateInit
      // Candidates may arrive before the remote description; they are queued until it is set.
      if (!remoteSet) pending.push(init)
      else await pc.addIceCandidate(init)
    },
    async connectionCode() {
      const l = pc.currentLocalDescription?.sdp
      const r = pc.currentRemoteDescription?.sdp
      return l && r ? connectionCode(l, r) : null
    },
    close() {
      pc.onicecandidate = pc.ontrack = pc.ondatachannel = pc.onconnectionstatechange = null
      pc.close()
    },
  }
}
