// SPDX-License-Identifier: GPL-3.0-or-later
//
// Connection check code (short authentication string), identical to apps/web/src/core/sas.ts: both sides hash
// the two DTLS certificate fingerprints of the final offer/answer, sorted, and show 6 digits. It only helps when
// people compare the codes (not enforced). A short code can in principle be ground by an active attacker who has
// time to try many certificates: it is a check, not a proof.

import Foundation
#if canImport(CryptoKit)
import CryptoKit
#else
import Crypto
#endif

/// First `a=fingerprint:sha-256 XX:..` of an SDP, normalised to lower case hex, or nil.
/// Lines end at CR, LF, U+2028 or U+2029 (what the web client's multi-line regular expression treats as line ends).
public func sdpFingerprint(_ sdp: String) -> String? {
    var line = String.UnicodeScalarView()
    func check(_ l: String.UnicodeScalarView) -> String? {
        let s = String(l)
        let prefix = "a=fingerprint:sha-256"
        guard s.hasPrefix(prefix) else { return nil }
        var rest = Substring(s.dropFirst(prefix.count))
        let ws = rest.prefix(while: { $0 == " " || $0 == "\t" })
        if ws.isEmpty { return nil }
        rest = rest.dropFirst(ws.count)
        let fp = rest.prefix(95)
        guard fp.count == 95 else { return nil }
        let tail = rest.dropFirst(95)
        if !tail.allSatisfy({ $0 == " " || $0 == "\t" }) { return nil }
        let chars = Array(fp.unicodeScalars)
        for (i, c) in chars.enumerated() {
            if i % 3 == 2 { if c != ":" { return nil } }
            else if !(c.isASCII && Character(c).isHexDigit) { return nil }
        }
        return fp.lowercased()
    }
    for u in sdp.unicodeScalars {
        if u == "\n" || u == "\r" || u == "\u{2028}" || u == "\u{2029}" {
            if let r = check(line) { return r }
            line = String.UnicodeScalarView()
        } else { line.append(u) }
    }
    return check(line)
}

private let sasDomain = "cometcast/connection-check/v1"

/// 6 digits ("123 456") from the two fingerprints, independent of their order, or nil.
public func connectionCode(localSdp: String, remoteSdp: String) -> String? {
    guard let a = sdpFingerprint(localSdp), let b = sdpFingerprint(remoteSdp) else { return nil }
    let sorted = [a, b].sorted { Array($0.utf8).lexicographicallyPrecedes(Array($1.utf8)) }
    let data = Array("\(sasDomain)\n\(sorted[0])\n\(sorted[1])".utf8)
    let h = Array(SHA256.hash(data: data))
    let n = UInt32(h[0]) << 24 | UInt32(h[1]) << 16 | UInt32(h[2]) << 8 | UInt32(h[3])
    let d = String(format: "%06u", n % 1_000_000)
    return "\(d.prefix(3)) \(d.suffix(3))"
}
