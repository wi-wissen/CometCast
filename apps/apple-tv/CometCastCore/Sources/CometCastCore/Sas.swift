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

/// The `a=fingerprint:sha-256 XX:..` of an SDP, normalised to lower case hex, or nil. Same rule as
/// `sdp_fingerprint` in crates/cc-transport/src/sas.rs (vectors: tests/fixtures/sas.json): lines end at LF, one CR
/// right before it belongs to the line end; an SDP with any other CR or with U+2028 / U+2029 is refused; every
/// `a=fingerprint:` line whose algorithm is `sha-256` (in any case) must hold 32 hex pairs and all must agree.
public func sdpFingerprint(_ sdp: String) -> String? {
    let scalars = Array(sdp.unicodeScalars)
    if scalars.contains(where: { $0 == "\u{2028}" || $0 == "\u{2029}" }) { return nil }
    var lines: [ArraySlice<Unicode.Scalar>] = []
    var start = 0
    for (i, u) in scalars.enumerated() where u == "\n" {
        lines.append(scalars[start..<i])
        start = i + 1
    }
    lines.append(scalars[start..<scalars.count])
    var found: String?
    for raw in lines {
        var line = raw
        if line.last == "\r" { line = line.dropLast() }
        if line.contains("\r") { return nil }
        let prefix = Array("a=fingerprint:".unicodeScalars)
        guard line.count >= prefix.count, Array(line.prefix(prefix.count)) == prefix else { continue }
        let rest = line.dropFirst(prefix.count)
        let alg = rest.prefix(while: { $0 != " " && $0 != "\t" })
        guard String(String.UnicodeScalarView(alg)).lowercased() == "sha-256" else { continue }
        var value = rest.dropFirst(alg.count)
        while let f = value.first, f == " " || f == "\t" { value = value.dropFirst() }
        while let l = value.last, l == " " || l == "\t" { value = value.dropLast() }
        let chars = Array(value)
        guard chars.count == 95 else { return nil }
        for (i, c) in chars.enumerated() {
            if i % 3 == 2 { if c != ":" { return nil } }
            else if !(c.isASCII && Character(c).isHexDigit) { return nil }
        }
        let fp = String(String.UnicodeScalarView(chars)).lowercased()
        if let f = found, f != fp { return nil }
        found = fp
    }
    return found
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
