// SPDX-License-Identifier: GPL-3.0-or-later
//
// ALTCHA proof of work ("PoW v2", as the Rust `altcha` crate 1.1 and the server use it; port of
// apps/web/src/core/altcha.ts and altchaClient.ts, scheme in docs/transport.md and docs/server.md):
//   password   = nonce (hex-decoded bytes) || counter as 4 bytes big endian
//   derivedKey = PBKDF2-HMAC-<hash>(password, salt (hex-decoded), cost iterations, keyLength bytes)
//   solved     = hex(derivedKey) starts with keyPrefix (hex, case-insensitive)
// The server's puzzles are deterministic: the search from counter 0 upwards ends at a secret counter.
// The answer for `create_room` is base64(JSON {challenge, solution:{counter, derivedKey, time}}).
// The challenge is sent back unchanged (all its fields, also unknown ones).

import Foundation

public struct AltchaError: Error, Equatable {
    /// unsupported_algorithm | bad_challenge | too_hard | expired | timeout | aborted
    public let reason: String
    public init(_ reason: String) { self.reason = reason }
}

/// UNCERTAIN: 60 s is a placeholder (as in the web and native clients); solving time on an Apple TV is not measured.
public let altchaDefaultSolveTimeoutSeconds: Double = 60
/// Largest PBKDF2 cost (iterations per attempt) solved. The server default is 1000. UNCERTAIN: 10x is a placeholder.
public let altchaMaxCost = 10_000
/// Largest derived key length in bytes. UNCERTAIN: placeholder.
public let altchaMaxKeyLength = 64

public struct AltchaSolution: Equatable, Sendable {
    public var counter: Int
    public var derivedKey: String
    /// Milliseconds the search took (the server accepts and ignores it).
    public var time: Int
}

private func params(_ challenge: JSONValue) -> [String: JSONValue]? {
    guard case .object(let o) = challenge, case .object(let p)? = o["parameters"] else { return nil }
    return p
}

/// Minimal shape check of the server's answer (`parameters` must be an object).
public func parseAltchaChallenge(_ text: String) -> JSONValue? {
    guard let v = JSON.parse(text), params(v) != nil else { return nil }
    return v
}

/// Refuses a puzzle that is too expensive, oddly sized or already expired, before any work is done.
public func checkChallengeLimits(_ challenge: JSONValue, nowSeconds: Double = Date().timeIntervalSince1970) throws {
    guard let p = params(challenge) else { throw AltchaError("bad_challenge") }
    guard let cost = p["cost"]?.integerValue, cost >= 1, let keyLength = p["keyLength"]?.integerValue, keyLength >= 1 else {
        throw AltchaError("bad_challenge")
    }
    if cost > Double(altchaMaxCost) || keyLength > Double(altchaMaxKeyLength) { throw AltchaError("too_hard") }
    if case .string(let prefix)? = p["keyPrefix"], Double(prefix.utf16.count) > keyLength * 2 { throw AltchaError("bad_challenge") }
    if let exp = p["expiresAt"], exp != .null {
        guard let e = exp.numberValue, e.isFinite else { throw AltchaError("bad_challenge") }
        if e <= nowSeconds { throw AltchaError("expired") }
    }
}

func hexDecode(_ s: String) -> [UInt8]? {
    let u = Array(s.utf8)
    if u.count % 2 != 0 { return nil }
    var out: [UInt8] = []
    out.reserveCapacity(u.count / 2)
    func v(_ c: UInt8) -> UInt8? {
        switch c {
        case 0x30...0x39: return c - 0x30
        case 0x41...0x46: return c - 0x41 + 10
        case 0x61...0x66: return c - 0x61 + 10
        default: return nil
        }
    }
    var i = 0
    while i < u.count {
        guard let a = v(u[i]), let b = v(u[i + 1]) else { return nil }
        out.append(a << 4 | b)
        i += 2
    }
    return out
}

func hexEncode(_ b: [UInt8]) -> String {
    let digits = Array("0123456789abcdef".utf8)
    var out = [UInt8]()
    out.reserveCapacity(b.count * 2)
    for x in b { out.append(digits[Int(x >> 4)]); out.append(digits[Int(x & 15)]) }
    return String(decoding: out, as: UTF8.self)
}

private func hashFor(_ algorithm: String) -> PBKDF2Hash? {
    switch algorithm.uppercased() {
    case "PBKDF2/SHA-256": return .sha256
    case "PBKDF2/SHA-384": return .sha384
    case "PBKDF2/SHA-512": return .sha512
    default: return nil
    }
}

/// nonce || counter (uint32, big endian).
public func altchaPassword(nonce: [UInt8], counter: UInt32) -> [UInt8] {
    nonce + [UInt8(counter >> 24 & 255), UInt8(counter >> 16 & 255), UInt8(counter >> 8 & 255), UInt8(counter & 255)]
}

/// Searches the counter, starting at 0, whose derived key starts with the key prefix. This is CPU bound and
/// synchronous: call it from a background task. `isCancelled` is polled before every attempt; `now` returns seconds
/// (injectable for tests).
public func solveAltcha(_ challenge: JSONValue, timeoutSeconds: Double = altchaDefaultSolveTimeoutSeconds,
                        isCancelled: () -> Bool = { false },
                        now: () -> Double = { Date().timeIntervalSince1970 },
                        derive: (PBKDF2Hash, [UInt8], [UInt8], Int, Int) -> [UInt8] = { PBKDF2.derive(hash: $0, password: $1, salt: $2, iterations: $3, keyLength: $4) }) throws -> AltchaSolution {
    guard let p = params(challenge), case .string(let algorithm)? = p["algorithm"], case .string(let nonceHex)? = p["nonce"],
          case .string(let saltHex)? = p["salt"] else { throw AltchaError("bad_challenge") }
    try checkChallengeLimits(challenge)
    guard let hash = hashFor(algorithm) else { throw AltchaError("unsupported_algorithm") }
    guard case .string(let prefixRaw)? = p["keyPrefix"],
          prefixRaw.unicodeScalars.allSatisfy({ $0.isASCII && Character($0).isHexDigit }) else { throw AltchaError("bad_challenge") }
    guard let nonce = hexDecode(nonceHex), let salt = hexDecode(saltHex) else { throw AltchaError("bad_challenge") }
    let cost = Int(p["cost"]!.integerValue!)
    let keyLength = Int(p["keyLength"]!.integerValue!)
    let prefix = prefixRaw.lowercased()
    let start = now()
    var counter: UInt64 = 0
    while counter <= 0xffff_ffff {
        if isCancelled() { throw AltchaError("aborted") }
        if timeoutSeconds > 0, now() - start > timeoutSeconds { throw AltchaError("timeout") }
        let key = derive(hash, altchaPassword(nonce: nonce, counter: UInt32(counter)), salt, cost, keyLength)
        let hex = hexEncode(key)
        if hex.hasPrefix(prefix) {
            return AltchaSolution(counter: Int(counter), derivedKey: hex, time: Int(((now() - start) * 1000).rounded()))
        }
        counter += 1
    }
    throw AltchaError("timeout")
}

/// base64 of the JSON `{challenge, solution}`; the value of `create_room.altcha`.
public func encodeAltchaPayload(challenge: JSONValue, solution: AltchaSolution) -> String {
    let sol: JSONValue = .object([
        "counter": .number(Double(solution.counter)),
        "derivedKey": .string(solution.derivedKey),
        "time": .number(Double(solution.time)),
    ])
    let json = JSON.serialize(.object(["challenge": challenge, "solution": sol]))
    return Data(json.utf8).base64EncodedString()
}

// MARK: HTTP side

/// Maps a non-200 answer to the vocabulary of the signalling errors (same as the web client's `httpErrorCode`).
public func httpErrorCode(status: Int, retryAfter: String?) -> String {
    if status == 429 { return "rate_limited" }
    // The server sends Retry-After with a ban and not with a plain "not allowed" (docs/server.md).
    if status == 403 { return retryAfter != nil ? "banned" : "forbidden" }
    if status == 503 { return "server_full" }
    return "altcha_failed"
}

/// Maps an AltchaError reason to a translated error code.
public func altchaErrorCode(_ e: AltchaError) -> String {
    switch e.reason {
    case "too_hard": return "altcha_too_hard"
    case "expired": return "altcha_expired"
    default: return "altcha_failed"
    }
}
