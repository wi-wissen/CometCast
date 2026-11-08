// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Swift toolchain here); checked by reading only.
//
// What a display announces in the local network (spec 14.8 of docs/design/first-run-and-display.md,
// docs/discovery.md section 3, docs/design/signed-announcements.md, decision N50). A port of `txt_records`,
// `sign_record` and the id derivation of crates/cc-discovery (display.rs, signed.rs): the same keys, the same
// signed bytes, so the presenter apps list an Apple TV like any other screen and verify it. Pure: the wire part
// (NWListener) and the key storage (Keychain) are in the app. The shared vectors tests/fixtures/announce-v2.json
// pin every byte; the tests read that file.

import Foundation
#if canImport(CryptoKit)
import CryptoKit
#else
import Crypto
#endif

/// Layout version of the record (`v`): signed records.
public let announcementLayoutVersion = "2"
/// The most bytes of one TXT record, DNS-SD lengths included.
public let announcementMaxTxtBytes = 512
/// The domain string at the start of the signed bytes.
public let announcementSignDomain = "cometcast-announce-v2"
/// The signed keys in the order of the signed bytes (`sig` is not part of it).
public let announcementSignedKeys = ["v", "id", "pk", "seq", "state", "srv", "tls", "mode", "code", "name"]
/// Added to the last `seq` when the app persists it, so that a persist that lags behind the last publish is safe.
public let announcementSeqReserve: UInt64 = 120_000

/// `live` while the display announces; `gone` in the record that is published before the goodbye.
public enum AnnounceState: String {
    case live
    case gone
}

/// Is `id` a valid id (exactly 32 lower-case hex digits)?
public func isValidInstanceId(_ id: String) -> Bool {
    id.count == 32 && id.allSatisfy { $0.isHexDigit && !$0.isUppercase }
}

/// base64url without padding (`pk`, `sig`).
public func base64URLNoPadding(_ data: Data) -> String {
    data.base64EncodedString()
        .replacingOccurrences(of: "+", with: "-")
        .replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
}

/// The id of a public key: lower-case hex of the first 16 bytes of SHA-256(public key).
public func displayKeyId(publicKey: Data) -> String {
    SHA256.hash(data: publicKey).prefix(16).map { String(format: "%02x", $0) }.joined()
}

/// The short tag shown to people: the first 4 characters of the id in upper case (`3F7A`).
public func displayKeyTag(id: String) -> String { String(id.prefix(4)).uppercased() }

/// The DNS-SD instance name: `CometCast-` plus the first 8 characters of the id, never a host or room name.
public func announcementInstanceName(id: String) -> String { "CometCast-" + id.prefix(8) }

/// The mode as announced: `confirm` is `approval`.
public func announcedMode(_ mode: JoinMode) -> String {
    switch mode {
    case .open: return "open"
    case .confirm: return "approval"
    case .locked: return "locked"
    }
}

/// `seq = max(unix time in ms, last + 1)`; `floor` is the highest value that may have been used before (the app
/// persists it with the key), so a clock that goes back after a restart cannot reuse values.
public struct AnnouncementSeq {
    public private(set) var last: UInt64
    public init(floor: UInt64) { last = floor }

    public mutating func next(nowMs: UInt64) -> UInt64 {
        let n = max(nowMs, last == UInt64.max ? last : last + 1)
        last = n
        return n
    }
}

/// The bytes that are signed: the domain string, 0x00, then `key=value` and 0x00 for every signed key in the fixed
/// order; an absent optional key is written with an empty value.
public func announcementSignedBytes(_ value: (String) -> String?) -> Data {
    var out = Data(announcementSignDomain.utf8)
    out.append(0)
    for key in announcementSignedKeys {
        out.append(contentsOf: Array("\(key)=".utf8))
        out.append(contentsOf: Array((value(key) ?? "").utf8))
        out.append(0)
    }
    return out
}

/// Sign a record. `fields` holds `srv`, `tls`, `mode`, `code`, `name` (absent keys left out). Returns the TXT entries
/// in the order of the signed keys (empty values left out) with `sig` last, or nil when it is too large or signing
/// failed. UNCERTAIN: whether CryptoKit's Ed25519 signature is deterministic (RFC 8032, as in Rust) is not known;
/// a verifier does not care, and the tests compare the bytes only when two signatures of one message are equal.
public func signAnnouncement(key: Curve25519.Signing.PrivateKey, seq: UInt64, state: AnnounceState, fields: [String: String]) -> [(String, String)]? {
    let pk = key.publicKey.rawRepresentation
    var values = fields
    values["v"] = announcementLayoutVersion
    values["id"] = displayKeyId(publicKey: pk)
    values["pk"] = base64URLNoPadding(pk)
    values["seq"] = String(seq)
    values["state"] = state.rawValue
    let message = announcementSignedBytes { values[$0] }
    guard let signature = try? key.signature(for: message) else { return nil }
    var txt: [(String, String)] = []
    for k in announcementSignedKeys {
        if let v = values[k], !v.isEmpty { txt.append((k, v)) }
    }
    txt.append(("sig", base64URLNoPadding(Data(signature))))
    let size = txt.reduce(0) { $0 + 1 + $1.0.utf8.count + 1 + $1.1.utf8.count }
    return size <= announcementMaxTxtBytes ? txt : nil
}

/// The signed TXT entries of a display, or nil when the state cannot be announced (a code that is not valid, a server
/// that is not a `ws://` or `wss://` URL, too large). While `locked` there is no code and no token. `wsUrl` is the
/// signalling URL of the relay the display uses (`wss://host:port/ws`); the announced `srv` is its `host:port`, and
/// `tls=1` is written for `wss://`. `roomName` is the name chosen in the setup wizard: it is cleaned and cut like
/// `room_name` and written as `name` unless empty; there is no fallback to the device name.
public func displayAnnouncementTxt(
    key: Curve25519.Signing.PrivateKey, seq: UInt64, state: AnnounceState = .live,
    wsUrl: String, mode: JoinMode, code: String?, roomName: String? = nil
) -> [(String, String)]? {
    let tls: Bool
    var rest: Substring
    if wsUrl.hasPrefix("wss://") { tls = true; rest = wsUrl.dropFirst(6) }
    else if wsUrl.hasPrefix("ws://") { tls = false; rest = wsUrl.dropFirst(5) }
    else { return nil }
    if let slash = rest.firstIndex(of: "/") { rest = rest[..<slash] }
    let authority = String(rest)
    guard !authority.isEmpty, !authority.contains("@") else { return nil }
    var fields: [String: String] = ["srv": authority, "mode": announcedMode(mode)]
    if tls { fields["tls"] = "1" }
    if mode != .locked {
        guard let code, isValidRoomCode(code) else { return nil }
        fields["code"] = code
    }
    if let n = roomName.map(cleanRoomName), !n.isEmpty { fields["name"] = n }
    return signAnnouncement(key: key, seq: seq, state: state, fields: fields)
}

/// base64url without padding, canonical only (decoding and encoding again gives the same text).
public func decodeBase64URLNoPadding(_ s: String) -> Data? {
    guard !s.contains("="), !s.contains("+"), !s.contains("/") else { return nil }
    var b = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
    while b.count % 4 != 0 { b += "=" }
    guard let d = Data(base64Encoded: b), base64URLNoPadding(d) == s else { return nil }
    return d
}

/// The values of a record whose signature verified.
public struct VerifiedAnnouncement: Equatable {
    public var id: String
    public var seq: UInt64
    public var state: AnnounceState
    public var srv: String
    public var tls: Bool
    public var mode: String
    public var code: String
    public var name: String
}

/// Check a record like `signed::verify` in Rust: `v=2`, well-formed values, `id` equal to the id derived from `pk`, and
/// the signature over the signed bytes. The first occurrence of a key wins; only the first 32 entries are read. nil
/// when anything fails (a v1 record included: there is no compatibility). UNCERTAIN: Rust uses `verify_strict`;
/// CryptoKit's `isValidSignature` may accept signatures that are not canonical, and it is not known whether it does.
public func verifyAnnouncement(_ txt: [(String, String)]) -> VerifiedAnnouncement? {
    let entries = Array(txt.prefix(32))
    func get(_ k: String) -> String? { entries.first { $0.0 == k }?.1 }
    guard get("v") == announcementLayoutVersion else { return nil }
    for k in announcementSignedKeys + ["sig"] {
        if let v = get(k), v.utf8.count > 255 || v.contains("\0") { return nil }
    }
    guard let id = get("id"), let pkText = get("pk"), let seqText = get("seq"),
          let state = get("state").flatMap({ AnnounceState(rawValue: $0) }),
          let srv = get("srv"), !srv.isEmpty, let mode = get("mode"), !mode.isEmpty,
          let sigText = get("sig") else { return nil }
    let tls: Bool
    switch get("tls") {
    case nil, "": tls = false
    case "1": tls = true
    default: return nil
    }
    guard isValidInstanceId(id), pkText.count == 43, sigText.count == 86 else { return nil }
    // A canonical decimal u64: digits only, no leading zero (except "0" itself).
    guard seqText == "0" || (!seqText.hasPrefix("0") && seqText.allSatisfy { ("0"..."9").contains($0) }),
          let seq = UInt64(seqText) else { return nil }
    guard let pk = decodeBase64URLNoPadding(pkText), pk.count == 32,
          let sig = decodeBase64URLNoPadding(sigText), sig.count == 64 else { return nil }
    guard displayKeyId(publicKey: pk) == id,
          let publicKey = try? Curve25519.Signing.PublicKey(rawRepresentation: pk),
          publicKey.isValidSignature(sig, for: announcementSignedBytes { get($0) }) else { return nil }
    return VerifiedAnnouncement(id: id, seq: seq, state: state, srv: srv, tls: tls, mode: mode,
                                code: get("code") ?? "", name: get("name") ?? "")
}

/// May this display announce? Not when the policy forbids announcements (or user overrides), and not when the
/// policy file is unusable (fail closed, like the policy of cc-discovery).
public func announcementsAllowed(policy: PolicyState) -> Bool {
    switch policy {
    case .absent: return true
    case .invalid: return false
    case .active(let p): return !p.forbidAnnouncement && !p.forbidUserOverride
    }
}
