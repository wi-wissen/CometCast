// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here). Written against the headers of the tvos-arm64 slice of
// LiveKitWebRTC 150.7871.05 (Objective-C classes carry the prefix LK: LKRTCPeerConnection, ...). Swift names are
// derived from the headers by the usual import rules, not checked by a compiler.
//
// One WebRTC peer connection of the receiver: answers the sender's offer, receives one video track and the
// "control" data channel. Rendering is done by LKRTCMTLVideoView (VideoSurface.swift). Decoding: H.264 through
// VideoToolbox by the library's default decoder factory (also VP8/VP9/AV1 in software, whatever the sender offers).

import CometCastCore
import Foundation
import LiveKitWebRTC

@MainActor
final class PeerSession: NSObject {
    struct Handlers {
        var onIceCandidate: (String) -> Void
        var onVideoTrack: (LKRTCVideoTrack) -> Void
        var onControlText: (String) -> Void
        var onControlBinary: ([UInt8]) -> Void
        var onConnected: () -> Void
        var onFailed: () -> Void
    }

    // One factory for the process: creating it is expensive.
    private static let factory: LKRTCPeerConnectionFactory = {
        LKRTCPeerConnectionFactory(encoderFactory: nil, decoderFactory: LKRTCDefaultVideoDecoderFactory())
    }()

    private var pc: LKRTCPeerConnection?
    private let h: Handlers
    private(set) var localSdp: String?
    private(set) var remoteSdp: String?
    private var channel: LKRTCDataChannel?
    private var closed = false

    init?(iceServers: [IceServerConfig], handlers: Handlers) {
        h = handlers
        super.init()
        let cfg = LKRTCConfiguration()
        cfg.sdpSemantics = .unifiedPlan
        cfg.iceServers = iceServers.map { LKRTCIceServer(urlStrings: $0.urls, username: $0.username, credential: $0.credential) }
        let constraints = LKRTCMediaConstraints(mandatoryConstraints: nil, optionalConstraints: nil)
        guard let pc = Self.factory.peerConnection(with: cfg, constraints: constraints, delegate: self) else { return nil }
        self.pc = pc
    }

    /// Sets the offer, prefers H.264, creates and sets the answer. Returns the answer's SDP text.
    func accept(offer: String) async throws -> String {
        guard let pc else { throw ServerError(code: "peer_failed") }
        remoteSdp = offer
        try await pc.setRemoteDescription(LKRTCSessionDescription(type: .offer, sdp: offer))
        preferH264(pc)
        let constraints = LKRTCMediaConstraints(mandatoryConstraints: nil, optionalConstraints: nil)
        let answer = try await pc.answer(for: constraints)
        try await pc.setLocalDescription(answer)
        localSdp = answer.sdp
        return answer.sdp
    }

    /// UNCERTAIN: the capability API names. Without this the library picks the first codec both sides know.
    private func preferH264(_ pc: LKRTCPeerConnection) {
        let caps = Self.factory.rtpReceiverCapabilities(forKind: kLKRTCMediaStreamTrackKindVideo)
        let h264 = caps.codecs.filter { $0.mimeType.lowercased() == "video/h264" }
        let rest = caps.codecs.filter { $0.mimeType.lowercased() != "video/h264" }
        for t in pc.transceivers where t.mediaType == .video {
            try? t.setCodecPreferences(h264 + rest)
        }
    }

    /// The library's statistics report as `RtcStat` values (W3C names). UNCERTAIN: `statistics()` as an async method
    /// and the `LKRTCStatistics` property names are taken from the headers by the usual import rules.
    func statistics() async -> [RtcStat] {
        guard let pc else { return [] }
        let report = await pc.statistics()
        return report.statistics.values.map { st in
            var v: [String: JSONValue] = [:]
            for (k, any) in st.values {
                if let n = any as? NSNumber { v[k] = CFGetTypeID(n) == CFBooleanGetTypeID() ? .bool(n.boolValue) : .number(n.doubleValue) }
                else if let str = any as? String { v[k] = .string(str) }
            }
            return RtcStat(id: st.id, type: st.type, values: v)
        }
    }

    func add(candidateJson: String) {
        guard let c = IceCandidateInit.parse(candidateJson), let pc else { return }
        let cand = LKRTCIceCandidate(sdp: c.candidate, sdpMLineIndex: Int32(c.sdpMLineIndex), sdpMid: c.sdpMid)
        pc.add(cand) { _ in } // a single bad candidate is not fatal
    }

    func close() {
        closed = true
        channel?.delegate = nil
        channel?.close()
        channel = nil
        pc?.delegate = nil
        pc?.close()
        pc = nil
    }
}

extension PeerSession: LKRTCPeerConnectionDelegate {
    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didChange stateChanged: LKRTCSignalingState) {}
    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didAdd stream: LKRTCMediaStream) {}
    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didRemove stream: LKRTCMediaStream) {}
    nonisolated func peerConnectionShouldNegotiate(_ peerConnection: LKRTCPeerConnection) {}
    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didChange newState: LKRTCIceConnectionState) {}
    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didChange newState: LKRTCIceGatheringState) {}
    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didRemove candidates: [LKRTCIceCandidate]) {}

    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didGenerate candidate: LKRTCIceCandidate) {
        // Same JSON as RTCIceCandidateInit of browsers (docs/protocol.md).
        let text = IceCandidateInit(candidate: candidate.sdp, sdpMid: candidate.sdpMid, sdpMLineIndex: Int(candidate.sdpMLineIndex)).encode()
        DispatchQueue.main.async { [weak self] in MainActor.assumeIsolated { if self?.closed == false { self?.h.onIceCandidate(text) } } }
    }

    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didChange newState: LKRTCPeerConnectionState) {
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated {
                guard let self, !self.closed else { return }
                switch newState {
                case .connected: self.h.onConnected()
                case .failed: self.h.onFailed()
                default: break
                }
            }
        }
    }

    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didAdd rtpReceiver: LKRTCRtpReceiver, streams: [LKRTCMediaStream]) {
        guard let track = rtpReceiver.track as? LKRTCVideoTrack else { return }
        DispatchQueue.main.async { [weak self] in MainActor.assumeIsolated { if self?.closed == false { self?.h.onVideoTrack(track) } } }
    }

    nonisolated func peerConnection(_ peerConnection: LKRTCPeerConnection, didOpen dataChannel: LKRTCDataChannel) {
        guard dataChannel.label == controlChannelLabel else { return } // other labels are not defined
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated {
                guard let self, !self.closed else { return }
                self.channel = dataChannel
                dataChannel.delegate = self
            }
        }
    }
}

extension PeerSession: LKRTCDataChannelDelegate {
    nonisolated func dataChannelDidChangeState(_ dataChannel: LKRTCDataChannel) {}

    nonisolated func dataChannel(_ dataChannel: LKRTCDataChannel, didReceiveMessageWith buffer: LKRTCDataBuffer) {
        // DispatchQueue.main keeps the order of messages (separate Tasks would not).
        let isBinary = buffer.isBinary
        let data = buffer.data
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated {
                guard let self, !self.closed else { return }
                if isBinary { self.h.onControlBinary([UInt8](data)) }
                else if let s = String(data: data, encoding: .utf8) { self.h.onControlText(s) }
                else { self.h.onControlText("\u{0}") } // not text: counts as a bad message
            }
        }
    }
}
