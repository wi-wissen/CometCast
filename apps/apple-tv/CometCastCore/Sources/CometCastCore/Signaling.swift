// SPDX-License-Identifier: GPL-3.0-or-later
//
// Signalling messages, version 2 (docs/protocol.md; crates/cc-proto; apps/web/src/core/signaling.ts).
// The Apple TV is a receiver and the owner of its room: it sends hello (role receiver), create_room (with join_mode),
// set_join_mode, join_decision, remove_peer, unblock, end_room, answer and ice_candidate, and understands every
// server message. Decoding is strict, like the web client: a message with a missing or mistyped field is nil.

import Foundation

public let protoVersion = 2

public enum Role: String, Sendable { case sender, receiver }

/// Who may join a room (docs/protocol.md, "Join approval"). The default everywhere is `open`.
public enum JoinMode: String, Sendable, CaseIterable {
    case open, confirm, locked
}

public enum ClientMessage: Equatable, Sendable {
    case hello(role: Role)
    /// `roomName` (optional, protocol field `room_name`): the name the display device gives its room. The server
    /// cleans it and cuts it at 32 characters; senders get it back in `join_pending` and `joined`.
    case createRoom(altcha: String?, joinMode: JoinMode?, roomName: String? = nil)
    /// Sender side; not used by the TV, kept for the shared message vectors.
    case joinRoom(code: String, name: String?)
    case setJoinMode(JoinMode)
    /// `id` nil with `allow` false denies every waiting request. `block` is only sent with a denial of one request.
    case joinDecision(id: Int?, allow: Bool, block: Bool)
    case removePeer(block: Bool)
    case unblock(id: Int)
    case endRoom
    /// Sender side; not used by the TV.
    case cancelJoin
    case offer(sdp: String)
    case answer(sdp: String)
    case iceCandidate(candidate: String)

    public var json: JSONValue {
        switch self {
        case .hello(let r): return .object(["type": .string("hello"), "proto_version": .number(Double(protoVersion)), "role": .string(r.rawValue)])
        case .createRoom(let a, let mode, let roomName):
            var o: [String: JSONValue] = ["type": .string("create_room")]
            if let a { o["altcha"] = .string(a) }
            if let mode { o["join_mode"] = .string(mode.rawValue) }
            if let roomName { o["room_name"] = .string(roomName) }
            return .object(o)
        case .joinRoom(let c, let name):
            var o: [String: JSONValue] = ["type": .string("join_room"), "code": .string(c)]
            if let name { o["name"] = .string(name) }
            return .object(o)
        case .setJoinMode(let m): return .object(["type": .string("set_join_mode"), "join_mode": .string(m.rawValue)])
        case .joinDecision(let id, let allow, let block):
            var o: [String: JSONValue] = ["type": .string("join_decision"), "allow": .bool(allow)]
            if let id { o["id"] = .number(Double(id)) }
            if block && !allow && id != nil { o["block"] = .bool(true) }
            return .object(o)
        case .removePeer(let block):
            var o: [String: JSONValue] = ["type": .string("remove_peer")]
            if block { o["block"] = .bool(true) }
            return .object(o)
        case .unblock(let id): return .object(["type": .string("unblock"), "id": .number(Double(id))])
        case .endRoom: return .object(["type": .string("end_room")])
        case .cancelJoin: return .object(["type": .string("cancel_join")])
        case .offer(let s): return .object(["type": .string("offer"), "sdp": .string(s)])
        case .answer(let s): return .object(["type": .string("answer"), "sdp": .string(s)])
        case .iceCandidate(let c): return .object(["type": .string("ice_candidate"), "candidate": .string(c)])
        }
    }

    public func encode() -> String { JSON.serialize(json) }
}

public enum ServerMessage: Equatable, Sendable {
    case hello(protoVersion: Double)
    case roomCreated(code: String, joinMode: JoinMode)
    /// `roomName`: the room's name as the display device set it (`room_name`); nil when absent, not a string, or empty.
    case joined(roomName: String? = nil)
    /// `name` is untrusted text as the server sent it (it cleans it, the machine cleans it again). `device`: the
    /// presenter's proof (docs/design/presenter-identity.md), relayed in open mode; nil when absent or malformed.
    case peerJoined(name: String, device: JoinDevice? = nil)
    case joinPending(timeoutSecs: Double, check: Int, roomName: String? = nil)
    case joinRequest(id: Int, name: String, check: Int, device: JoinDevice? = nil)
    case joinRequestCancelled(id: Int, reason: String)
    case joinModeChanged(JoinMode)
    case blocked(id: Int, name: String)
    case unblocked(id: Int)
    case roomEnded
    case offer(sdp: String)
    case answer(sdp: String)
    case iceCandidate(candidate: String)
    case control(cmd: JSONValue)
    case peerLeft
    case error(code: String, message: String)

    /// Parses one server text frame. nil for anything that is not a valid server message.
    public static func decode(_ text: String) -> ServerMessage? {
        guard case .object(let o)? = JSON.parse(text), case .string(let type)? = o["type"] else { return nil }
        func str(_ k: String) -> String? { o[k]?.stringValue }
        // ids and check numbers are echoed back or shown as integers: anything else is refused
        func int(_ k: String) -> Int? {
            guard let d = o[k]?.integerValue, abs(d) < 9_007_199_254_740_992 else { return nil }
            return Int(d)
        }
        func mode(_ k: String) -> JoinMode? { str(k).flatMap(JoinMode.init(rawValue:)) }
        // `room_name` is optional: absent, null, not a string, or only white space all mean "no room name"
        // (the message itself stays valid). The server already cleaned it; it is shown as plain text only.
        func roomName() -> String? {
            guard let s = str("room_name"), !s.allSatisfy({ $0.isWhitespace }) else { return nil }
            return s
        }
        switch type {
        case "hello":
            guard let v = o["proto_version"]?.numberValue else { return nil }
            return .hello(protoVersion: v)
        case "room_created":
            guard let c = str("code"), let m = mode("join_mode") else { return nil }
            return .roomCreated(code: c, joinMode: m)
        case "joined": return .joined(roomName: roomName())
        case "peer_joined": return str("name").map { .peerJoined(name: $0, device: parseJoinDevice(o["device"])) }
        case "peer_left": return .peerLeft
        case "room_ended": return .roomEnded
        case "join_pending":
            guard let t = o["timeout_secs"]?.numberValue, t.isFinite, let c = int("check") else { return nil }
            return .joinPending(timeoutSecs: t, check: c, roomName: roomName())
        case "join_request":
            guard let id = int("id"), let n = str("name"), let c = int("check") else { return nil }
            return .joinRequest(id: id, name: n, check: c, device: parseJoinDevice(o["device"]))
        case "join_request_cancelled":
            guard let id = int("id"), let r = str("reason") else { return nil }
            return .joinRequestCancelled(id: id, reason: r)
        case "join_mode_changed": return mode("join_mode").map { .joinModeChanged($0) }
        case "blocked":
            guard let id = int("id"), let n = str("name") else { return nil }
            return .blocked(id: id, name: n)
        case "unblocked": return int("id").map { .unblocked(id: $0) }
        case "offer": return str("sdp").map { .offer(sdp: $0) }
        case "answer": return str("sdp").map { .answer(sdp: $0) }
        case "ice_candidate": return str("candidate").map { .iceCandidate(candidate: $0) }
        case "control": return o["cmd"].map { .control(cmd: $0) }
        case "error": return str("code").map { .error(code: $0, message: str("message") ?? "") }
        default: return nil
        }
    }
}

// MARK: display names

public let maxDisplayNameChars = 40

private func isUnsafeNameScalar(_ s: Unicode.Scalar) -> Bool {
    let v = s.value
    return v < 0x20 || (0x7F...0x9F).contains(v)           // control characters
        || v == 0x061C || (0x200B...0x200F).contains(v)    // zero width, bidi marks
        || (0x2028...0x202E).contains(v)                   // separators, bidi embeddings and overrides
        || (0x2060...0x206F).contains(v)                   // invisible formatting, bidi isolates
        || v == 0xFEFF || (0xFFF9...0xFFFB).contains(v)    // BOM, interlinear annotation
        || (0xE000...0xF8FF).contains(v)                   // private use
}

/// Makes a display name that somebody else chose safe to show (same rules as `cc_proto::sanitize_display_name`,
/// vectors in tests/fixtures/join-approval/name-sanitising.json): control, bidi, zero-width and invisible characters
/// are removed (a tab or line break counts as a space), white space is collapsed and trimmed, the result is cut at
/// 40 characters (Unicode scalars) and `Guest N` is used when nothing is left. The server does this already; this is
/// defence in depth. The result is still untrusted text: show it as plain text, never as markup.
public func sanitizeDisplayName(_ raw: String?, guestNumber: Int) -> String {
    sanitizedDisplayNameOrNil(raw) ?? "Guest \(guestNumber)"
}

/// The same cleaning without the fallback: nil when nothing is left. The design shows `common.noName` then (spec
/// 4.4), so the screens need to know that the name was empty.
public func sanitizedDisplayNameOrNil(_ raw: String?) -> String? {
    var out: [Unicode.Scalar] = []
    var pendingSpace = false
    for c in (raw ?? "").unicodeScalars {
        let spacing = c == "\t" || c == "\n" || c == "\r"
        if !spacing && isUnsafeNameScalar(c) { continue }
        if c.properties.isWhitespace {
            pendingSpace = !out.isEmpty
            continue
        }
        if pendingSpace { out.append(" "); pendingSpace = false }
        out.append(c)
        if out.count >= maxDisplayNameChars { break }
    }
    var cut = Array(out.prefix(maxDisplayNameChars))
    while let l = cut.last, l.properties.isWhitespace { cut.removeLast() }
    if cut.isEmpty { return nil }
    var r = String.UnicodeScalarView()
    r.append(contentsOf: cut)
    return String(r)
}

/// Error codes the server documents plus the client side ones. Unknown codes are passed through as strings and
/// get the generic message.
public let knownErrorCodes: [String] = [
    "bad_hello", "proto_mismatch", "wrong_role", "room_not_found", "room_full", "not_in_room", "bad_message",
    "altcha_required", "altcha_invalid", "rate_limited", "banned", "forbidden", "server_full", "altcha_failed",
    "altcha_too_hard", "altcha_expired", "connection_lost", "peer_failed",
    // Apple TV only
    "no_server", "local_network_denied",
]

/// Error codes added by join approval (protocol version 2; tests/fixtures/join-approval/error-codes.json). They have
/// no entry in the string catalog yet, so `errorMessageKey` gives them the generic message.
/// TODO(design): which of these a person on the TV should see, and the wording, is a UI decision. The machine handles
/// `open_rooms_disabled`, `no_such_request`, `no_such_block` and `block_list_full` without ending the session
/// (they come back as `ReceiverState.notice` where a person needs to know).
public let joinApprovalErrorCodes: [String] = [
    "join_pending", "join_denied", "room_locked", "approval_timeout", "removed_by_host", "open_rooms_disabled",
    "no_such_request", "no_such_block", "block_list_full",
]

/// Translation key of the message for an error code (`errors.<code>`, else `errors.unknown`).
public func errorMessageKey(_ code: String) -> String {
    knownErrorCodes.contains(code) ? "errors.\(code)" : "errors.unknown"
}

/// An ICE candidate in the protocol's form: the JSON text of an RTCIceCandidateInit.
public struct IceCandidateInit: Equatable, Sendable {
    public var candidate: String
    public var sdpMid: String?
    public var sdpMLineIndex: Int

    public init(candidate: String, sdpMid: String?, sdpMLineIndex: Int) {
        self.candidate = candidate; self.sdpMid = sdpMid; self.sdpMLineIndex = sdpMLineIndex
    }

    /// Parses the text of `ice_candidate.candidate`. Native clients send `sdpMid: null` and `sdpMLineIndex: 0`.
    public static func parse(_ text: String) -> IceCandidateInit? {
        guard text.utf16.count <= 4096, case .object(let o)? = JSON.parse(text), case .string(let c)? = o["candidate"] else { return nil }
        let mid = o["sdpMid"]?.stringValue
        var idx = 0
        if let i = o["sdpMLineIndex"]?.integerValue, i >= 0, i < 64 { idx = Int(i) }
        return IceCandidateInit(candidate: c, sdpMid: mid, sdpMLineIndex: idx)
    }

    public func encode() -> String {
        JSON.serialize(.object([
            "candidate": .string(candidate),
            "sdpMid": sdpMid.map { .string($0) } ?? .null,
            "sdpMLineIndex": .number(Double(sdpMLineIndex)),
        ]))
    }
}

// MARK: ICE servers

public struct IceServerConfig: Equatable, Sendable {
    public var urls: [String]
    public var username: String?
    public var credential: String?
}

/// UNCERTAIN: placeholders, the same as the native client (crates/cc-transport/src/ice.rs) and the web client.
public let maxIceServers = 16
public let maxIceURLs = 8
public let maxIceField = 512

private func iceURLIsSane(_ u: String) -> Bool {
    guard u.utf16.count <= maxIceField else { return false }
    let l = u.lowercased()
    guard ["stun:", "stuns:", "turn:", "turns:"].contains(where: { l.hasPrefix($0) }) else { return false }
    return !u.unicodeScalars.contains { $0.value <= 0x1F || $0.value == 0x7F }
}

/// Keeps only the fields in use; drops entries that cannot be valid. Bounded like the web client's `parseIceServers`.
public func parseIceServers(_ text: String) -> [IceServerConfig] {
    guard case .array(let arr)? = JSON.parse(text) else { return [] }
    var out: [IceServerConfig] = []
    for e in arr.prefix(maxIceServers * 4) {
        if out.count >= maxIceServers { break }
        guard case .object(let o) = e else { continue }
        var raw: [String]
        switch o["urls"] {
        case .string(let s)?: raw = [s]
        case .array(let a)?:
            guard !a.isEmpty, a.allSatisfy({ $0.stringValue != nil }) else { continue }
            raw = a.compactMap { $0.stringValue }
        default: continue
        }
        let urls = Array(raw.filter(iceURLIsSane).prefix(maxIceURLs))
        if urls.isEmpty { continue }
        let user = o["username"]?.stringValue
        let cred = o["credential"]?.stringValue
        if (user?.utf16.count ?? 0) > maxIceField || (cred?.utf16.count ?? 0) > maxIceField { continue }
        // A TURN entry without credentials would make the peer connection reject the whole list.
        if urls.contains(where: { $0.lowercased().hasPrefix("turn:") || $0.lowercased().hasPrefix("turns:") }),
           (user ?? "").isEmpty || (cred ?? "").isEmpty { continue }
        out.append(IceServerConfig(urls: urls, username: user, credential: cred))
    }
    return out
}
