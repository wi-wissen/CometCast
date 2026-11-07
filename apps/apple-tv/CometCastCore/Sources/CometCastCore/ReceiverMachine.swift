// SPDX-License-Identifier: GPL-3.0-or-later
//
// Connection state machine of the receiver: a pure port of the signalling logic of
// apps/web/src/core/receiverSession.ts. It receives inputs (server messages, results of async work) and returns
// effects (what the app has to do). It does no I/O, so the whole error behaviour is unit tested.
//
// Phases are the web app's: idle, connecting, checking (ALTCHA), waiting_sender (code is shown), negotiating,
// receiving, peer_left, error. The app guards async work with a generation counter: results of an earlier
// `start`/`stop` are never fed in.

import Foundation

public enum ReceiverPhase: String, Equatable, Sendable, CaseIterable {
    case idle, connecting, checking
    case waitingSender = "waiting_sender"
    case negotiating, receiving
    case peerLeft = "peer_left"
    case error
}

public struct ReceiverState: Equatable, Sendable {
    public var phase: ReceiverPhase = .idle
    public var code: String?
    /// A server error code or a local one (connection_lost, peer_failed, altcha_failed, ...). Set in phase `error`.
    public var errorCode: String?
    public var hasVideo = false
    /// Connection check code once connected.
    public var sas: String?
    public init() {}
}

public enum ReceiverInput: Equatable, Sendable {
    case start
    case stop
    /// The server accepted our hello (proto version equal).
    case serverReady
    case message(ServerMessage)
    /// The socket ended without us closing it (or never opened).
    case socketClosed
    /// ALTCHA finished: the text for create_room.altcha, or the error code to show.
    case altchaSolved(String)
    case altchaFailed(code: String)
    /// The ICE server list is known (empty = none). Held messages are processed now.
    case iceLoaded
    case answerReady(sdp: String)
    case answerFailed
    case peerConnected
    case peerFailed
    case sasReady(String)
    case videoArrived
}

public enum ReceiverEffect: Equatable, Sendable {
    case openSocket
    case closeSocket
    case send(ClientMessage)
    case solveAltcha
    case cancelAltcha
    case fetchIceServers
    case createPeer
    case closePeer
    case acceptOffer(sdp: String)
    case addIceCandidate(String)
    /// Drop shown content and any transfer in progress.
    case resetContent
}

public struct ReceiverMachine: Sendable {
    public private(set) var state = ReceiverState()
    private var altchaTried = false
    private var iceReady = true
    private var held: [ServerMessage] = []
    private var hasPeer = false

    public init() {}

    /// `iceFetch` = the app fetches /ice-servers; peers are made only after `iceLoaded`.
    public var fetchesIce = true

    public mutating func handle(_ input: ReceiverInput) -> [ReceiverEffect] {
        switch input {
        case .start:
            var fx = teardown()
            altchaTried = false
            iceReady = !fetchesIce
            held = []
            state = ReceiverState()
            state.phase = .connecting
            fx.append(.openSocket)
            if fetchesIce { fx.append(.fetchIceServers) }
            return fx
        case .stop:
            let fx = teardown()
            state = ReceiverState()
            return fx
        case .serverReady:
            return state.phase == .connecting ? [.send(.createRoom(altcha: nil))] : []
        case .socketClosed:
            return fail("connection_lost")
        case .iceLoaded:
            iceReady = true
            var fx: [ReceiverEffect] = []
            let h = held
            held = []
            for m in h { fx += onMessage(m) }
            return fx
        case .message(let m):
            return onMessage(m)
        case .altchaSolved(let payload):
            guard state.phase == .checking else { return [] }
            state.phase = .connecting
            return [.send(.createRoom(altcha: payload))]
        case .altchaFailed(let code):
            guard state.phase == .checking else { return [] }
            return fail(code)
        case .answerReady(let sdp):
            return [.send(.answer(sdp: sdp))]
        case .answerFailed:
            return fail("peer_failed")
        case .peerConnected:
            state.phase = .receiving
            return []
        case .peerFailed:
            return fail("peer_failed")
        case .sasReady(let c):
            if hasPeer { state.sas = c }
            return []
        case .videoArrived:
            state.hasVideo = true
            return []
        }
    }

    private mutating func teardown() -> [ReceiverEffect] {
        held = []
        let fx: [ReceiverEffect] = [.cancelAltcha, .resetContent, .closePeer, .closeSocket]
        hasPeer = false
        return fx
    }

    private mutating func fail(_ code: String) -> [ReceiverEffect] {
        if state.phase == .error || state.phase == .idle { return [] }
        let fx = teardown()
        state.phase = .error
        state.errorCode = code
        state.hasVideo = false
        state.sas = nil
        return fx
    }

    private mutating func newPeer() -> [ReceiverEffect] {
        var fx: [ReceiverEffect] = []
        if hasPeer { fx.append(.closePeer) }
        hasPeer = true
        fx.append(.createPeer)
        return fx
    }

    private mutating func onMessage(_ m: ServerMessage) -> [ReceiverEffect] {
        // A peer must not be made before the ICE server list is known (rarely waits at all).
        if !iceReady {
            switch m {
            case .peerJoined, .offer, .iceCandidate:
                held.append(m)
                return []
            default: break
            }
        }
        switch m {
        case .roomCreated(let code):
            state.phase = .waitingSender
            state.code = code
            return []
        case .peerJoined:
            var fx = newPeer()
            state.phase = .negotiating
            state.hasVideo = false
            state.sas = nil
            return fx
        case .offer(let sdp):
            var fx: [ReceiverEffect] = []
            if !hasPeer { fx += newPeer() }
            if state.phase != .receiving { state.phase = .negotiating }
            fx.append(.acceptOffer(sdp: sdp))
            return fx
        case .iceCandidate(let c):
            return hasPeer ? [.addIceCandidate(c)] : []
        case .peerLeft:
            hasPeer = false
            state.phase = .peerLeft
            state.hasVideo = false
            state.sas = nil
            return [.closePeer, .resetContent]
        case .error(let code, _):
            // Open servers answer the first create_room with altcha_required; the connection stays open.
            if code == "altcha_required", !altchaTried, state.phase == .connecting {
                altchaTried = true
                state.phase = .checking
                return [.solveAltcha]
            }
            // not_in_room can arrive when the sender has just left; the room itself survives.
            if code == "not_in_room", state.phase != .connecting { return [] }
            return fail(code)
        case .hello, .joined, .answer, .control:
            return [] // not used by the receiver
        }
    }
}
