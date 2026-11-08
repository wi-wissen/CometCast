// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Swift toolchain here).
// The announced record follows docs/discovery.md section 3 (the same rules as crates/cc-discovery display.rs and
// signed.rs). The shared vectors are tests/fixtures/announce-v2.json (made by tools/gen-announce-v2-vectors.py with
// an implementation that is independent of Rust and Swift); Rust reads the same file.

import Foundation
import XCTest
@testable import CometCastCore
#if canImport(CryptoKit)
import CryptoKit
#else
import Crypto
#endif

final class DisplayAnnouncementTests: XCTestCase {
    let key = try! Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x42, count: 32))

    func dict(_ t: [(String, String)]?) -> [String: String] { Dictionary(uniqueKeysWithValues: t ?? []) }

    func testOpenRecordCarriesTheCodeAndIsSigned() {
        let t = displayAnnouncementTxt(key: key, seq: 100, wsUrl: "wss://relay.cometcast.de:443/ws", mode: .open, code: "K7QM2W", roomName: "Elbe")
        let d = dict(t)
        XCTAssertEqual(d["v"], "2")
        XCTAssertEqual(d["id"], displayKeyId(publicKey: key.publicKey.rawRepresentation))
        XCTAssertEqual(d["seq"], "100")
        XCTAssertEqual(d["state"], "live")
        XCTAssertEqual(d["srv"], "relay.cometcast.de:443")
        XCTAssertEqual(d["tls"], "1")
        XCTAssertEqual(d["mode"], "open")
        XCTAssertEqual(d["code"], "K7QM2W")
        XCTAssertEqual(d["name"], "Elbe")
        // The fixed key order, `sig` last.
        XCTAssertEqual(t?.map(\.0), ["v", "id", "pk", "seq", "state", "srv", "tls", "mode", "code", "name", "sig"])
        XCTAssertEqual(d["sig"]?.count, 86)
        XCTAssertEqual(d["pk"]?.count, 43)
    }

    func testTheSignatureVerifiesOverTheSignedBytes() {
        let t = displayAnnouncementTxt(key: key, seq: 7, wsUrl: "ws://10.0.0.5:8443/ws", mode: .confirm, code: "K7QM2W")!
        let d = dict(t)
        let message = announcementSignedBytes { d[$0] }
        let sig = Data(base64URLDecoded: d["sig"]!)!
        XCTAssertTrue(key.publicKey.isValidSignature(sig, for: message))
        // A changed code does not verify.
        var tampered = d
        tampered["code"] = "AAAAAA"
        XCTAssertFalse(key.publicKey.isValidSignature(sig, for: announcementSignedBytes { tampered[$0] }))
    }

    func testConfirmIsAnnouncedAsApproval() {
        let t = displayAnnouncementTxt(key: key, seq: 1, wsUrl: "ws://10.0.0.5:8443/ws", mode: .confirm, code: "K7QM2W")
        XCTAssertEqual(dict(t)["mode"], "approval")
        XCTAssertNil(dict(t)["tls"])
        XCTAssertEqual(dict(t)["srv"], "10.0.0.5:8443")
    }

    func testLockedNeverCarriesACode() {
        let t = displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://relay.cometcast.de/ws", mode: .locked, code: "K7QM2W")
        XCTAssertEqual(dict(t)["mode"], "locked")
        XCTAssertNil(dict(t)["code"])
    }

    func testGoneRecord() {
        let t = displayAnnouncementTxt(key: key, seq: 9, state: .gone, wsUrl: "wss://a.de/ws", mode: .open, code: "K7QM2W")
        XCTAssertEqual(dict(t)["state"], "gone")
        XCTAssertEqual(dict(t)["seq"], "9")
    }

    func testUnusableStatesAreNotAnnounced() {
        XCTAssertNil(displayAnnouncementTxt(key: key, seq: 1, wsUrl: "https://a.de/ws", mode: .open, code: "K7QM2W"))
        XCTAssertNil(displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://a.de/ws", mode: .open, code: nil))
        XCTAssertNil(displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://a.de/ws", mode: .open, code: "K7QM2O"))
        XCTAssertNil(displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://u@a.de/ws", mode: .open, code: "K7QM2W"))
    }

    func testNameIsCleanedAndNeverFallsBack() {
        let t = displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://relay.cometcast.de:443/ws", mode: .open, code: "K7QM2W", roomName: "  Room\u{202E} 204 ")
        XCTAssertEqual(t?.map(\.0), ["v", "id", "pk", "seq", "state", "srv", "tls", "mode", "code", "name", "sig"])
        XCTAssertEqual(dict(t)["name"], "Room 204")
        // No name set: no key.
        let none = displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://a.de/ws", mode: .open, code: "K7QM2W", roomName: "\u{200B}")
        XCTAssertNil(dict(none)["name"])
        XCTAssertEqual(none?.map(\.0), ["v", "id", "pk", "seq", "state", "srv", "tls", "mode", "code", "sig"])
    }

    func testInstanceNameIdAndTag() {
        let id = displayKeyId(publicKey: key.publicKey.rawRepresentation)
        XCTAssertTrue(isValidInstanceId(id))
        XCTAssertEqual(id.count, 32)
        XCTAssertEqual(announcementInstanceName(id: id), "CometCast-" + id.prefix(8))
        XCTAssertEqual(displayKeyTag(id: id), String(id.prefix(4)).uppercased())
        XCTAssertFalse(isValidInstanceId("0123456789abcdef")) // v1 ids (16 digits) are not ids any more
        XCTAssertFalse(isValidInstanceId(id.uppercased()))
    }

    func testSeqIsMaxOfClockAndLastPlusOne() {
        var s = AnnouncementSeq(floor: 0)
        XCTAssertEqual(s.next(nowMs: 1000), 1000)
        XCTAssertEqual(s.next(nowMs: 1000), 1001)
        XCTAssertEqual(s.next(nowMs: 5), 1002) // the clock went back: no value is reused
        XCTAssertEqual(s.next(nowMs: 5000), 5000)
        var r = AnnouncementSeq(floor: 9000)
        XCTAssertEqual(r.next(nowMs: 10), 9001)
    }

    func testPolicy() {
        XCTAssertTrue(announcementsAllowed(policy: .absent))
        XCTAssertFalse(announcementsAllowed(policy: .invalid))
    }
}

/// tests/fixtures/announce-v2.json: the same vectors as crates/cc-discovery/tests/announce_v2_vectors.rs.
final class AnnouncementVectorTests: XCTestCase {
    func hex(_ s: String) -> Data {
        var out = Data()
        var i = s.startIndex
        while i < s.endIndex {
            let j = s.index(i, offsetBy: 2)
            out.append(UInt8(s[i..<j], radix: 16)!)
            i = j
        }
        return out
    }

    func pairs(_ v: JSONValue?) -> [(String, String)] {
        (v?.arrayValue ?? []).map { p in
            let a = p.arrayValue ?? []
            return (a[0].stringValue ?? "", a[1].stringValue ?? "")
        }
    }

    func key(_ f: [String: JSONValue], _ name: String) -> (Curve25519.Signing.PrivateKey, [String: JSONValue]) {
        let k = (f["keys"]?.arrayValue ?? []).compactMap { $0.objectValue }.first { $0["name"]?.stringValue == name }!
        let priv = try! Curve25519.Signing.PrivateKey(rawRepresentation: hex(k["seed_hex"]!.stringValue!))
        return (priv, k)
    }

    func testKeysIdsAndTags() {
        let f = Fixtures.load("announce-v2.json")
        for k in (f["keys"]?.arrayValue ?? []).compactMap({ $0.objectValue }) {
            let priv = try! Curve25519.Signing.PrivateKey(rawRepresentation: hex(k["seed_hex"]!.stringValue!))
            let pk = priv.publicKey.rawRepresentation
            XCTAssertEqual(pk, hex(k["public_key_hex"]!.stringValue!))
            XCTAssertEqual(base64URLNoPadding(pk), k["pk"]!.stringValue!)
            XCTAssertEqual(displayKeyId(publicKey: pk), k["id"]!.stringValue!)
            XCTAssertEqual(displayKeyTag(id: displayKeyId(publicKey: pk)), k["tag"]!.stringValue!)
        }
    }

    func testValidRecordsHaveTheSameBytes() {
        let f = Fixtures.load("announce-v2.json")
        let valid = (f["valid"]?.arrayValue ?? []).compactMap { $0.objectValue }
        XCTAssertGreaterThanOrEqual(valid.count, 7)
        for v in valid {
            let name = v["name"]?.stringValue ?? "?"
            let (priv, _) = key(f, v["key"]!.stringValue!)
            var fields: [String: String] = [:]
            for (k, x) in v["fields"]?.objectValue ?? [:] { fields[k] = x.stringValue }
            let seq = UInt64(v["seq"]!.numberValue!)
            let state = AnnounceState(rawValue: v["state"]!.stringValue!)!
            let expected = pairs(v["txt"])
            let built = signAnnouncement(key: priv, seq: seq, state: state, fields: fields)!
            // Everything but the signature is deterministic and must be identical.
            XCTAssertEqual(built.dropLast().map { "\($0.0)=\($0.1)" }, expected.dropLast().map { "\($0.0)=\($0.1)" }, name)
            XCTAssertEqual(built.last?.0, "sig", name)
            // The signed bytes equal the vector.
            let d = Dictionary(uniqueKeysWithValues: built)
            XCTAssertEqual(announcementSignedBytes { d[$0] }, hex(v["signed_bytes_hex"]!.stringValue!), name)
            // The signature of the vector verifies with the public key; so does ours.
            let msg = announcementSignedBytes { d[$0] }
            XCTAssertTrue(priv.publicKey.isValidSignature(Data(base64URLDecoded: expected.last!.1)!, for: msg), name)
            XCTAssertTrue(priv.publicKey.isValidSignature(Data(base64URLDecoded: built.last!.1)!, for: msg), name)
            // UNCERTAIN: Ed25519 is deterministic in RFC 8032 (and in Rust); whether CryptoKit's `signature(for:)` is
            // too was not checked. When two signatures of one message are equal, ours must equal the vector.
            let again = try! priv.signature(for: msg)
            let first = try! priv.signature(for: msg)
            if again == first {
                XCTAssertEqual(built.last?.1, expected.last?.1, "\(name): deterministic signature")
            }
        }
    }

    func testRefusedRecordsAreRefused() {
        let f = Fixtures.load("announce-v2.json")
        let refused = (f["refused"]?.arrayValue ?? []).compactMap { $0.objectValue }
        XCTAssertGreaterThanOrEqual(refused.count, 15)
        for r in refused {
            XCTAssertNil(verifyAnnouncement(pairs(r["txt"])), "refused: \(r["name"]?.stringValue ?? "?")")
            XCTAssertNil(announcedReceiver(instanceName: "x", txt: pairs(r["txt"]))?.announcedServer)
        }
        // And every valid live record verifies.
        for v in (f["valid"]?.arrayValue ?? []).compactMap({ $0.objectValue }) {
            let ok = verifyAnnouncement(pairs(v["txt"]))
            XCTAssertNotNil(ok, v["name"]?.stringValue ?? "?")
            XCTAssertEqual(ok?.state.rawValue, v["state"]?.stringValue)
        }
    }

    func testSeqCounterVectors() {
        let f = Fixtures.load("announce-v2.json")
        for c in (f["seq_counter"]?.arrayValue ?? []).compactMap({ $0.objectValue }) {
            var counter = AnnouncementSeq(floor: UInt64(c["floor"]!.numberValue!))
            let got = (c["clock"]?.arrayValue ?? []).map { counter.next(nowMs: UInt64($0.numberValue!)) }
            let expect = (c["expect"]?.arrayValue ?? []).map { UInt64($0.numberValue!) }
            XCTAssertEqual(got, expect)
        }
    }
}

private extension Data {
    /// base64url without padding.
    init?(base64URLDecoded s: String) {
        var b = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        while b.count % 4 != 0 { b += "=" }
        self.init(base64Encoded: b)
    }
}
