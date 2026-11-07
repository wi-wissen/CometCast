// SPDX-License-Identifier: GPL-3.0-or-later
//
// Signalling messages, version 1 (docs/protocol.md; crates/cc-proto; apps/web/src/core/signaling.ts).
// The Apple TV is a receiver: it sends hello (role receiver), create_room, answer, ice_candidate and
// control (never needed), and understands every server message.

import Foundation

public let protoVersion = 1

public enum Role: String, Sendable { case sender, receiver }

public enum ClientMessage: Equatable, Sendable {
    case hello(role: Role)
    case createRoom(altcha: String?)
    case joinRoom(code: String)
    case offer(sdp: String)
    case answer(sdp: String)
    case iceCandidate(candidate: String)

    public var json: JSONValue {
        switch self {
        case .hello(let r): return .object(["type": .string("hello"), "proto_version": .number(Double(protoVersion)), "role": .string(r.rawValue)])
        case .createRoom(let a):
            var o: [String: JSONValue] = ["type": .string("create_room")]
            if let a { o["altcha"] = .string(a) }
            return .object(o)
        case .joinRoom(let c): return .object(["type": .string("join_room"), "code": .string(c)])
        case .offer(let s): return .object(["type": .string("offer"), "sdp": .string(s)])
        case .answer(let s): return .object(["type": .string("answer"), "sdp": .string(s)])
        case .iceCandidate(let c): return .object(["type": .string("ice_candidate"), "candidate": .string(c)])
        }
    }

    public func encode() -> String { JSON.serialize(json) }
}

public enum ServerMessage: Equatable, Sendable {
    case hello(protoVersion: Double)
    case roomCreated(code: String)
    case joined
    case peerJoined
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
        switch type {
        case "hello":
            guard let v = o["proto_version"]?.numberValue else { return nil }
            return .hello(protoVersion: v)
        case "room_created": return str("code").map { .roomCreated(code: $0) }
        case "joined": return .joined
        case "peer_joined": return .peerJoined
        case "peer_left": return .peerLeft
        case "offer": return str("sdp").map { .offer(sdp: $0) }
        case "answer": return str("sdp").map { .answer(sdp: $0) }
        case "ice_candidate": return str("candidate").map { .iceCandidate(candidate: $0) }
        case "control": return o["cmd"].map { .control(cmd: $0) }
        case "error": return str("code").map { .error(code: $0, message: str("message") ?? "") }
        default: return nil
        }
    }
}

/// Error codes the server documents plus the client side ones. Unknown codes are passed through as strings and
/// get the generic message.
public let knownErrorCodes: [String] = [
    "bad_hello", "proto_mismatch", "wrong_role", "room_not_found", "room_full", "not_in_room", "bad_message",
    "altcha_required", "altcha_invalid", "rate_limited", "banned", "forbidden", "server_full", "altcha_failed",
    "altcha_too_hard", "altcha_expired", "connection_lost", "peer_failed",
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
