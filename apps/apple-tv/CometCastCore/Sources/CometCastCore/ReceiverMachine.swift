// SPDX-License-Identifier: GPL-3.0-or-later
//
// Connection state machine of the receiver: a pure port of the signalling logic of
// apps/web/src/core/receiverSession.ts. It receives inputs (server messages, results of async work) and returns
// effects (what the app has to do). It does no I/O, so the whole error behaviour is unit tested.
//
// Join approval (protocol version 2): the TV owns its room. The machine keeps the join mode (default `open`), the
// queue of waiting requests (the server times them out after 60 s and says so with `join_request_cancelled`, the
// machine has no timer), the block list and the name of the connected sender, and turns the person's decisions into
// messages. Names from the server are cleaned again (defence in depth). Like the web client, a decision removes the
// request from the queue at once.
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
    /// Who may join, as confirmed by the server (the mode that was asked for before the room exists).
    public var joinMode: JoinMode = .open
    /// Waiting join requests, oldest first.
    public var requests: [JoinRequest] = []
    /// Cleaned name of the connected sender.
    public var peerName: String?
    /// The sender's name was empty after cleaning (`peerName` is then the fallback "Guest N"): the screens show
    /// `common.noName` (spec 4.4).
    public var peerUnnamed = false
    /// Blocked senders; the host can unblock them.
    public var blocks: [BlockedEntry] = []
    public var notice: JoinNotice?
    public init() {}
}

/// A sender that waits for the host's decision (join mode `confirm`). Untrusted text: show as plain text.
public struct JoinRequest: Equatable, Sendable {
    public var id: Int
    /// Cleaned: no control or bidi characters, at most 40 characters.
    public var name: String
    /// Two-digit number (10 to 99) that the waiting sender sees too.
    public var check: Int
    /// The name was empty after cleaning (`name` is then the fallback "Guest N"): the screens show `common.noName`.
    public var unnamed = false
}

/// Something the host should be told about with a toast (spec 4.6). The machine collects them; the app takes them
/// with `takeEvents()` after every input. They are not part of `ReceiverState`.
public enum HostEvent: Equatable, Sendable {
    /// The server ended a waiting request after `join_approval_secs` (`toast.expired`).
    case requestExpired(name: String, unnamed: Bool)
    /// The person who asked left or withdrew the request (`toast.withdrawn`).
    case requestWithdrawn(name: String, unnamed: Bool)
    /// A sender joined (`toast.joined`, unless the host just allowed it: `toast.allowed` then says it).
    case senderJoined(name: String, unnamed: Bool)
    /// The sender left (`toast.left`); the room and its code stay.
    case senderLeft(name: String, unnamed: Bool)
    /// The room session ended on the host's request (`toast.sessionEnded`).
    case sessionEnded
}

/// An entry of the room's block list (blocked for the rest of the room session).
public struct BlockedEntry: Equatable, Sendable {
    public var id: Int
    public var name: String
}

/// Something that happened to the room's join settings that a person may need to know.
public enum JoinNotice: Equatable, Sendable {
    /// The host locked the room while requests were waiting: this many were discarded.
    case lockedDiscarded(count: Int)
    /// The server does not allow open rooms; `confirm` is used instead.
    case openDisabled
    /// The block list is full (32 entries); the request stays undecided on the server until it times out.
    case blockListFull
}

/// How long the server lets a request wait by default. UNCERTAIN: an operator can change it; the server decides.
public let joinRequestSecs = 60

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
    // Decisions of the host (the person at the TV). All of them need a room (a code); otherwise they do nothing.
    /// Changes who may join. The state changes when the server confirms with `join_mode_changed`.
    case setJoinMode(JoinMode)
    /// Allows or denies a waiting request; `block` (with a denial) also blocks the sender for this room session.
    case decide(id: Int, allow: Bool, block: Bool)
    /// Denies everybody who waits.
    case denyAll
    /// Ends the connected sender's presentation (the room stays); `block` blocks the sender too.
    case removePeer(block: Bool)
    case unblock(id: Int)
    /// Ends the room session; the machine asks for a new room on the same connection when the server confirms.
    case endRoom
    case dismissNotice
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
    /// The mode asked for (survives `start`, `stop` and room changes), like the web client's `mode`.
    private var mode: JoinMode
    /// The default mode (Open or Approval on): what a new session after End starts with. Locked is never the
    /// default; it lasts until End (spec 4.3).
    private var defaultMode: JoinMode
    private var altchaPayload: String?
    private var peersSeen = 0
    /// The room name sent with `create_room` (nil = none). No UI sets it yet.
    private var roomName: String?
    private var events: [HostEvent] = []

    /// The events since the last call (see `HostEvent`).
    public mutating func takeEvents() -> [HostEvent] {
        let e = events
        events = []
        return e
    }

    public init(joinMode: JoinMode = .open, roomName: String? = nil) {
        self.roomName = roomName
        mode = joinMode
        defaultMode = joinMode == .locked ? .open : joinMode
        state.joinMode = joinMode
    }

    /// `iceFetch` = the app fetches /ice-servers; peers are made only after `iceLoaded`.
    public var fetchesIce = true

    public mutating func handle(_ input: ReceiverInput) -> [ReceiverEffect] {
        switch input {
        case .start:
            var fx = teardown()
            altchaTried = false
            altchaPayload = nil
            peersSeen = 0
            iceReady = !fetchesIce
            held = []
            state = ReceiverState()
            state.joinMode = mode
            state.phase = .connecting
            fx.append(.openSocket)
            if fetchesIce { fx.append(.fetchIceServers) }
            return fx
        case .stop:
            let fx = teardown()
            state = ReceiverState()
            state.joinMode = mode
            return fx
        case .serverReady:
            return state.phase == .connecting ? [createRoom()] : []
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
            altchaPayload = payload
            return [createRoom()]
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
        case .setJoinMode(let m):
            mode = m
            if m != .locked { defaultMode = m }
            return state.code != nil ? [.send(.setJoinMode(m))] : []
        case .decide(let id, let allow, let block):
            guard state.code != nil, state.requests.contains(where: { $0.id == id }) else { return [] }
            state.requests.removeAll { $0.id == id }
            return [.send(.joinDecision(id: id, allow: allow, block: block && !allow))]
        case .denyAll:
            guard state.code != nil, !state.requests.isEmpty else { return [] }
            state.requests = []
            return [.send(.joinDecision(id: nil, allow: false, block: false))]
        case .removePeer(let block):
            return state.code != nil ? [.send(.removePeer(block: block))] : []
        case .unblock(let id):
            return state.code != nil ? [.send(.unblock(id: id))] : []
        case .endRoom:
            return state.code != nil ? [.send(.endRoom)] : []
        case .dismissNotice:
            state.notice = nil
            return []
        }
    }

    private func createRoom() -> ReceiverEffect { .send(.createRoom(altcha: altchaPayload, joinMode: mode, roomName: roomName)) }

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
        state.requests = []
        state.peerName = nil
        state.peerUnnamed = false
        state.blocks = []
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
        case .roomCreated(let code, let joinMode):
            state.phase = .waitingSender
            state.code = code
            state.joinMode = joinMode
            // A mode that was chosen while the room was being created is applied now.
            return mode != joinMode ? [.send(.setJoinMode(mode))] : []
        case .joinRequest(let id, let name, let check):
            if state.requests.contains(where: { $0.id == id }) { return [] }
            state.requests.append(JoinRequest(id: id, name: sanitizeDisplayName(name, guestNumber: id), check: check,
                                              unnamed: sanitizedDisplayNameOrNil(name) == nil))
            return []
        case .joinRequestCancelled(let id, let reason):
            if let r = state.requests.first(where: { $0.id == id }) {
                if reason == "timeout" { events.append(.requestExpired(name: r.name, unnamed: r.unnamed)) }
                else if reason == "left" || reason == "cancelled" { events.append(.requestWithdrawn(name: r.name, unnamed: r.unnamed)) }
            }
            state.requests.removeAll { $0.id == id }
            if reason == "locked" {
                var count = 1
                if case .lockedDiscarded(let n)? = state.notice { count = n + 1 }
                state.notice = .lockedDiscarded(count: count)
            }
            return []
        case .joinModeChanged(let m):
            mode = m
            state.joinMode = m
            if m != .locked { state.notice = nil }
            return []
        case .blocked(let id, let name):
            if !state.blocks.contains(where: { $0.id == id }) {
                state.blocks.append(BlockedEntry(id: id, name: sanitizeDisplayName(name, guestNumber: id)))
            }
            return []
        case .unblocked(let id):
            state.blocks.removeAll { $0.id == id }
            return []
        case .roomEnded:
            events.append(.sessionEnded)
            // The room and its code are gone; the connection stays, so ask for a new room at once.
            var fx: [ReceiverEffect] = [.closePeer, .resetContent]
            hasPeer = false
            altchaPayload = nil
            altchaTried = false
            peersSeen = 0
            // End: the next session starts in the default mode, not in Locked (spec 4.3).
            mode = defaultMode
            state.phase = .connecting
            state.code = nil
            state.hasVideo = false
            state.sas = nil
            state.peerName = nil
            state.peerUnnamed = false
            state.requests = []
            state.blocks = []
            state.notice = nil
            state.joinMode = mode
            fx.append(createRoom())
            return fx
        case .peerJoined(let name):
            peersSeen += 1
            let fx = newPeer()
            state.phase = .negotiating
            state.hasVideo = false
            state.sas = nil
            state.peerName = sanitizeDisplayName(name, guestNumber: peersSeen)
            state.peerUnnamed = sanitizedDisplayNameOrNil(name) == nil
            events.append(.senderJoined(name: state.peerName ?? "", unnamed: state.peerUnnamed))
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
            if let n = state.peerName { events.append(.senderLeft(name: n, unnamed: state.peerUnnamed)) }
            hasPeer = false
            state.phase = .peerLeft
            state.hasVideo = false
            state.sas = nil
            state.peerName = nil
            state.peerUnnamed = false
            return [.closePeer, .resetContent]
        case .error(let code, _):
            // Open servers answer the first create_room with altcha_required; the connection stays open.
            if code == "altcha_required", !altchaTried, state.phase == .connecting {
                altchaTried = true
                state.phase = .checking
                return [.solveAltcha]
            }
            // The server forbids open rooms: ask for one that needs approval instead of failing.
            if code == "open_rooms_disabled" {
                if state.phase == .connecting, mode == .open {
                    mode = .confirm
                    state.joinMode = .confirm
                    state.notice = .openDisabled
                    return [createRoom()]
                }
                state.notice = .openDisabled
                return []
            }
            // A decision for a request that is already gone (timed out at the same moment), or an unblock for an
            // entry that is not there any more: nothing to do.
            if code == "no_such_request" || code == "no_such_block" { return [] }
            if code == "block_list_full" { state.notice = .blockListFull; return [] }
            // not_in_room can arrive when the sender has just left; the room itself survives.
            if code == "not_in_room", state.phase != .connecting { return [] }
            return fail(code)
        case .hello, .joined, .joinPending, .answer, .control:
            return [] // not used by the receiver
        }
    }
}
