// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Swift toolchain here); checked by reading only.
//
// Presenter device identity on the display (docs/design/presenter-identity.md): the proof in `join_request` and
// `peer_joined`, and the display's memory of who used a name. A port of `cc_transport::device` (join proof) and
// `cc_receiver::DeviceMemory`. The shared vectors tests/fixtures/join-proof.json pin every byte (made by
// tools/gen-join-proof-vectors.py with an implementation independent of Rust, TypeScript and Swift); the tests read
// that file.
//
// The proof is `device {pk, ts, sig}`: `sig` is Ed25519 over `cometcast-join-v1` 0x00 <room code, upper case> 0x00
// <ts, decimal>. The name is not signed. The display accepts it when `|ts - now| <= 120 s` and the signature is
// valid for the current room code; the device id is the lower-case hex of the first 16 bytes of SHA-256(pk).
// Verification is strict like Rust's `verify_strict`: a public key of small order and a signature whose S is not
// reduced are refused here explicitly, because the library's own check is not documented to refuse them.

import Foundation
#if canImport(CryptoKit)
import CryptoKit
#else
import Crypto
#endif

/// The domain string at the start of the signed bytes.
public let joinProofDomain = "cometcast-join-v1"
/// How far `ts` may be from the display's clock, in milliseconds (either direction).
public let joinProofWindowMs: UInt64 = 120_000

/// The `device` object of `join_request` / `peer_joined`.
public struct JoinDevice: Equatable, Sendable {
    public var pk: String
    public var ts: UInt64
    public var sig: String
    public init(pk: String, ts: UInt64, sig: String) {
        self.pk = pk
        self.ts = ts
        self.sig = sig
    }
}

/// The bytes that are signed: the domain, 0x00, the code in upper case, 0x00, `ts` in decimal.
public func joinSignedBytes(code: String, ts: UInt64) -> Data {
    var out = Data(joinProofDomain.utf8)
    out.append(0)
    out.append(contentsOf: Array(code.uppercased().utf8))
    out.append(0)
    out.append(contentsOf: Array(String(ts).utf8))
    return out
}

/// Reads `device` of a server message: exactly the keys `pk`, `ts` and `sig`, `pk` and `sig` text, `ts` a whole
/// number from 0 to 2^53 - 1 (what JSON numbers keep exactly; the web display has the same limit). Anything else is
/// "no proof" (nil) and the rest of the message stays valid. The shape of `pk` and `sig` is checked by
/// `verifyJoinProof`.
public func parseJoinDevice(_ v: JSONValue?) -> JoinDevice? {
    guard case .object(let o)? = v, o.count == 3,
          let pk = o["pk"]?.stringValue, let sig = o["sig"]?.stringValue,
          let ts = o["ts"]?.integerValue, ts >= 0, ts < 9_007_199_254_740_992 else { return nil }
    return JoinDevice(pk: pk, ts: UInt64(ts), sig: sig)
}

/// The encodings of the points of small order (and their non-canonical spellings): a public key like that "verifies"
/// signatures of any message without strict checks.
private let smallOrderPublicKeys: [String] = [
    "0100000000000000000000000000000000000000000000000000000000000000",
    "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
    "0000000000000000000000000000000000000000000000000000000000000080",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a",
    "c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac03fa",
    "26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc05",
    "26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc85",
    "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
    "eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
    "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
]

/// The order L of the Ed25519 base point group, 32 bytes little-endian.
private let groupOrderLE: [UInt8] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
]

private func hexString(_ d: Data) -> String { d.map { String(format: "%02x", $0) }.joined() }

/// Is the 32-byte little-endian number `s` smaller than the group order?
private func isReduced(_ s: [UInt8]) -> Bool {
    for i in stride(from: 31, through: 0, by: -1) {
        if s[i] < groupOrderLE[i] { return true }
        if s[i] > groupOrderLE[i] { return false }
    }
    return false // equal to L
}

/// Checks a proof for room `code` against the clock `nowMs`: the shape (43 and 86 canonical base64url characters),
/// `|ts - now| <= 120 s`, and the signature (strict). Returns the device id, or nil for every kind of failure: the
/// request is then shown without any device note (never as "verified").
public func verifyJoinProof(_ device: JoinDevice, code: String, nowMs: UInt64) -> String? {
    guard device.pk.count == 43, device.sig.count == 86,
          let pk = decodeBase64URLNoPadding(device.pk), pk.count == 32,
          let sig = decodeBase64URLNoPadding(device.sig), sig.count == 64 else { return nil }
    let distance = device.ts > nowMs ? device.ts - nowMs : nowMs - device.ts
    guard distance <= joinProofWindowMs else { return nil }
    guard !smallOrderPublicKeys.contains(hexString(pk)), isReduced(Array(sig[32..<64])) else { return nil }
    guard let key = try? Curve25519.Signing.PublicKey(rawRepresentation: pk),
          key.isValidSignature(sig, for: joinSignedBytes(code: code, ts: device.ts)) else { return nil }
    return displayKeyId(publicKey: pk)
}

// MARK: the display's memory of names and devices

/// Most pairs kept.
public let deviceMemoryMaxPairs = 200
private let deviceMemoryMaxNameChars = 40

/// The label that the server gives somebody who sent no name: `Guest ` and digits. Never stored or compared.
public func isGeneratedGuestName(_ name: String) -> Bool {
    guard name.hasPrefix("Guest ") else { return false }
    let digits = name.dropFirst(6)
    return !digits.isEmpty && digits.allSatisfy { $0.isASCII && $0.isNumber }
}

/// The pairs `(cleaned name, device id)` of every request the display allowed and every presenter that joined in open
/// mode with a valid proof, the least recently used first, at most 200. A name stored only with other device ids
/// gets the note; a name stored with this id, or never seen, gets none.
public struct DeviceMemory: Equatable, Sendable {
    public private(set) var pairs: [(name: String, id: String)] = []

    public init() {}

    /// Reads what `json` wrote. Lenient: anything that is not a list of `[name, id]` pairs of text is skipped.
    public init(json: String?) {
        guard let json, case .array(let items)? = JSON.parse(json) else { return }
        for item in items {
            if case .array(let p) = item, p.count == 2, let n = p[0].stringValue, let i = p[1].stringValue {
                _ = remember(name: n, id: i)
            }
        }
    }

    public static func == (a: DeviceMemory, b: DeviceMemory) -> Bool {
        a.pairs.count == b.pairs.count && zip(a.pairs, b.pairs).allSatisfy { $0.name == $1.name && $0.id == $1.id }
    }

    /// The text for the app's settings: `[["name","id"], ...]`, the least recently used first.
    public var json: String {
        JSON.serialize(.array(pairs.map { .array([.string($0.name), .string($0.id)]) }))
    }

    private static func key(_ name: String) -> String { name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() }

    /// The note: the name is stored, but only with device ids other than `id`. Never for a generated name.
    public func usedByOtherDevice(name: String, id: String) -> Bool {
        if isGeneratedGuestName(name) { return false }
        let key = Self.key(name)
        var seen = false
        for p in pairs where Self.key(p.name) == key {
            if p.id == id { return false }
            seen = true
        }
        return seen
    }

    /// Remember that `id` was allowed under `name`. True when the memory changed. A generated name, an empty name or
    /// an id that is not 32 lower-case hex digits changes nothing; a known pair becomes the newest.
    @discardableResult
    public mutating func remember(name: String, id: String) -> Bool {
        let clean = String(name.trimmingCharacters(in: .whitespacesAndNewlines).prefix(deviceMemoryMaxNameChars))
        guard !clean.isEmpty, !isGeneratedGuestName(clean), isValidInstanceId(id) else { return false }
        let before = json
        let key = Self.key(clean)
        pairs.removeAll { $0.id == id && Self.key($0.name) == key }
        pairs.append((clean, id))
        if pairs.count > deviceMemoryMaxPairs { pairs.removeFirst(pairs.count - deviceMemoryMaxPairs) }
        return json != before
    }
}
