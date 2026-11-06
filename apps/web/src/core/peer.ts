// SPDX-License-Identifier: GPL-3.0-or-later
//
// Thin RTCPeerConnection wrapper. Signalling happens elsewhere; this class only deals with WebRTC.

export type PeerState = 'connecting' | 'connected' | 'failed'

export interface PeerHandlers {
  /** JSON text of an RTCIceCandidateInit (what the protocol carries in ice_candidate.candidate). */
  onIceCandidate(candidate: string): void
  onTrack(stream: MediaStream): void
  onState(state: PeerState): void
}

export interface Peer {
  addStream(stream: MediaStream): void
  createOffer(): Promise<string>
  /** Takes the remote offer SDP, returns the answer SDP. */
  acceptOffer(sdp: string): Promise<string>
  acceptAnswer(sdp: string): Promise<void>
  addIceCandidate(candidateJson: string): Promise<void>
  close(): void
}

export type PeerFactory = (handlers: PeerHandlers) => Peer

// UNCERTAIN: no STUN/TURN servers by default, so this only works for direct/LAN connections.
// STUN/TURN comes with milestone M8 and measurement point 1 (docs/measurement-points.md).
export const DEFAULT_RTC_CONFIG: RTCConfiguration = { iceServers: [] }

export function createPeer(handlers: PeerHandlers, config: RTCConfiguration = DEFAULT_RTC_CONFIG): Peer {
  const pc = new RTCPeerConnection(config)
  let remoteSet = false
  const pending: RTCIceCandidateInit[] = []

  pc.onicecandidate = (ev) => {
    if (ev.candidate) handlers.onIceCandidate(JSON.stringify(ev.candidate.toJSON()))
  }
  pc.ontrack = (ev) => {
    handlers.onTrack(ev.streams[0] ?? new MediaStream([ev.track]))
  }
  pc.onconnectionstatechange = () => {
    const s = pc.connectionState
    if (s === 'connected') handlers.onState('connected')
    else if (s === 'failed') handlers.onState('failed')
    // UNCERTAIN: 'disconnected' can be temporary; it is not reported as failure here. Not measured yet.
  }

  async function flush(): Promise<void> {
    remoteSet = true
    for (const c of pending.splice(0)) await pc.addIceCandidate(c)
  }

  return {
    addStream(stream) {
      for (const t of stream.getTracks()) pc.addTrack(t, stream)
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
    close() {
      pc.onicecandidate = pc.ontrack = pc.onconnectionstatechange = null
      pc.close()
    },
  }
}
