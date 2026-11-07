// SPDX-License-Identifier: GPL-3.0-or-later
//
// PBKDF2-HMAC for the ALTCHA solver. CryptoKit has HMAC and the SHA-2 family but no PBKDF2, so a portable
// version is built from them (also used by the tests on Linux with swift-crypto). On Apple platforms the
// system routine `CCKeyDerivationPBKDF` is used for speed; `derivePortable` stays available so a test on a Mac
// can compare both. UNCERTAIN: the CommonCrypto path was never compiled or run (no Mac here).

import Foundation
#if canImport(CryptoKit)
import CryptoKit
#else
import Crypto
#endif
#if canImport(CommonCrypto)
import CommonCrypto
#endif

public enum PBKDF2Hash: Sendable {
    case sha256, sha384, sha512
}

public enum PBKDF2 {
    /// Fast path on Apple platforms, portable code elsewhere.
    public static func derive(hash: PBKDF2Hash, password: [UInt8], salt: [UInt8], iterations: Int, keyLength: Int) -> [UInt8] {
        #if canImport(CommonCrypto)
        let prf: CCPseudoRandomAlgorithm
        switch hash {
        case .sha256: prf = CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA256)
        case .sha384: prf = CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA384)
        case .sha512: prf = CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA512)
        }
        var out = [UInt8](repeating: 0, count: keyLength)
        let status = password.withUnsafeBufferPointer { pw in
            salt.withUnsafeBufferPointer { sl in
                out.withUnsafeMutableBufferPointer { o in
                    pw.baseAddress!.withMemoryRebound(to: Int8.self, capacity: max(pw.count, 1)) { pwp in
                        CCKeyDerivationPBKDF(CCPBKDFAlgorithm(kCCPBKDF2), pwp, pw.count, sl.baseAddress, sl.count, prf,
                                             UInt32(iterations), o.baseAddress, o.count)
                    }
                }
            }
        }
        if status == Int32(kCCSuccess) { return out }
        #endif
        return derivePortable(hash: hash, password: password, salt: salt, iterations: iterations, keyLength: keyLength)
    }

    public static func derivePortable(hash: PBKDF2Hash, password: [UInt8], salt: [UInt8], iterations: Int, keyLength: Int) -> [UInt8] {
        switch hash {
        case .sha256: return run(SHA256.self, 64, password, salt, iterations, keyLength)
        case .sha384: return run(SHA384.self, 128, password, salt, iterations, keyLength)
        case .sha512: return run(SHA512.self, 128, password, salt, iterations, keyLength)
        }
    }

    /// HMAC built from two pre-keyed hash states that are copied for every call (a hasher is a value type), which
    /// avoids hashing the key pads in every one of the `iterations` rounds.
    private static func run<H: HashFunction>(_ type: H.Type, _ block: Int, _ password: [UInt8], _ salt: [UInt8],
                                             _ iterations: Int, _ keyLength: Int) -> [UInt8] {
        var key = password
        if key.count > block { key = Array(H.hash(data: key)) }
        key += [UInt8](repeating: 0, count: block - key.count)
        var inner = H()
        inner.update(data: key.map { $0 ^ 0x36 })
        var outer = H()
        outer.update(data: key.map { $0 ^ 0x5c })
        func hmac(_ msg: [UInt8]) -> [UInt8] {
            var i = inner
            i.update(data: msg)
            var o = outer
            o.update(data: Array(i.finalize()))
            return Array(o.finalize())
        }
        let hLen = H.Digest.byteCount
        var out: [UInt8] = []
        var blockIndex: UInt32 = 1
        while out.count < keyLength {
            var u = hmac(salt + [UInt8(blockIndex >> 24 & 255), UInt8(blockIndex >> 16 & 255), UInt8(blockIndex >> 8 & 255), UInt8(blockIndex & 255)])
            var t = u
            if iterations > 1 {
                for _ in 1..<iterations {
                    u = hmac(u)
                    for k in 0..<hLen { t[k] ^= u[k] }
                }
            }
            out += t
            blockIndex += 1
        }
        return Array(out.prefix(keyLength))
    }
}
