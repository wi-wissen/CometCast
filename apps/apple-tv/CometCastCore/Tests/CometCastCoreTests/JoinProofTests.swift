// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Swift toolchain here).
// Presenter device identity on the display (docs/design/presenter-identity.md). The shared vectors are
// tests/fixtures/join-proof.json (made by tools/gen-join-proof-vectors.py with an implementation that is independent
// of Rust, TypeScript and Swift); the Rust and web tests read the same file.

import Foundation
import XCTest
@testable import CometCastCore
#if canImport(CryptoKit)
import CryptoKit
#else
import Crypto
#endif

private func joinHex(_ s: String) -> Data {
    var out = Data()
    var i = s.startIndex
    while i < s.endIndex {
        let j = s.index(i, offsetBy: 2)
        out.append(UInt8(s[i..<j], radix: 16)!)
        i = j
    }
    return out
}

final class JoinProofVectorTests: XCTestCase {
    func testKeysIdsAndSignedBytesMatch() {
        let f = Fixtures.load("join-proof.json")
        XCTAssertEqual(f["window_ms"]?.integerValue, Double(joinProofWindowMs))
        XCTAssertEqual(f["domain"]?.stringValue, joinProofDomain)
        for k in (f["keys"]?.arrayValue ?? []).compactMap({ $0.objectValue }) {
            let priv = try! Curve25519.Signing.PrivateKey(rawRepresentation: joinHex(k["seed_hex"]!.stringValue!))
            XCTAssertEqual(priv.publicKey.rawRepresentation, joinHex(k["public_key_hex"]!.stringValue!))
            XCTAssertEqual(displayKeyId(publicKey: priv.publicKey.rawRepresentation), k["id"]?.stringValue)
        }
        for v in (f["valid"]?.arrayValue ?? []).compactMap({ $0.objectValue }) {
            let name = v["name"]?.stringValue ?? "?"
            let device = parseJoinDevice(v["device"])!
            let code = v["signed_for_code"]!.stringValue!
            XCTAssertEqual(joinSignedBytes(code: code, ts: device.ts), joinHex(v["signed_bytes_hex"]!.stringValue!), name)
        }
    }

    func testValidProofsVerifyToTheirDeviceId() {
        let f = Fixtures.load("join-proof.json")
        for v in (f["valid"]?.arrayValue ?? []).compactMap({ $0.objectValue }) {
            let name = v["name"]?.stringValue ?? "?"
            let device = parseJoinDevice(v["device"])
            XCTAssertNotNil(device, name)
            let id = verifyJoinProof(device!, code: v["code"]!.stringValue!, nowMs: UInt64(v["now_ms"]!.numberValue!))
            XCTAssertEqual(id, v["device_id"]?.stringValue, name)
        }
    }

    func testRefusedProofsAreRefused() {
        let f = Fixtures.load("join-proof.json")
        for r in (f["refused"]?.arrayValue ?? []).compactMap({ $0.objectValue }) {
            let name = r["name"]?.stringValue ?? "?"
            guard let device = parseJoinDevice(r["device"]) else { continue } // a malformed shape is refused here
            XCTAssertNil(verifyJoinProof(device, code: r["code"]!.stringValue!, nowMs: UInt64(r["now_ms"]!.numberValue!)), name)
        }
    }

    func testMalformedShapesNeverBecomeADevice() {
        let f = Fixtures.load("join-proof.json")
        for r in (f["refused"]?.arrayValue ?? []).compactMap({ $0.objectValue }) where r["reason"]?.stringValue == "malformed" {
            let name = r["name"]?.stringValue ?? "?"
            if let device = parseJoinDevice(r["device"]) {
                // Right keys and types: the length and spelling checks of the verification refuse it.
                XCTAssertNil(verifyJoinProof(device, code: r["code"]!.stringValue!, nowMs: UInt64(r["now_ms"]!.numberValue!)), name)
            }
        }
    }
}

final class DeviceMemoryTests: XCTestCase {
    let a = "0123456789abcdef0123456789abcdef"
    let b = "fedcba9876543210fedcba9876543210"

    func testTheNoteNeedsTheNameWithAnotherDeviceOnly() {
        var m = DeviceMemory()
        XCTAssertFalse(m.usedByOtherDevice(name: "Julian's iPad", id: a))
        XCTAssertTrue(m.remember(name: "Julian's iPad", id: a))
        XCTAssertFalse(m.usedByOtherDevice(name: "Julian's iPad", id: a))
        XCTAssertTrue(m.usedByOtherDevice(name: "Julian's iPad", id: b))
        XCTAssertTrue(m.usedByOtherDevice(name: "JULIAN'S IPAD", id: b))
        XCTAssertFalse(m.usedByOtherDevice(name: "Anna's iPad", id: b))
        m.remember(name: "Julian's iPad", id: b)
        XCTAssertFalse(m.usedByOtherDevice(name: "Julian's iPad", id: b))
        XCTAssertFalse(m.usedByOtherDevice(name: "Julian's iPad", id: a))
    }

    func testGeneratedNamesAreNeverStoredOrCompared() {
        var m = DeviceMemory()
        XCTAssertFalse(m.remember(name: "Guest 3", id: a))
        XCTAssertFalse(m.remember(name: "", id: a))
        XCTAssertFalse(m.remember(name: "Ana", id: "nope"))
        XCTAssertTrue(m.pairs.isEmpty)
        m.remember(name: "Ana", id: a)
        XCTAssertFalse(m.usedByOtherDevice(name: "Guest 3", id: b))
        XCTAssertTrue(m.remember(name: "Guest", id: a))
    }

    func testAtMost200PairsLeastRecentlyUsedFirst() {
        var m = DeviceMemory()
        for i in 0..<deviceMemoryMaxPairs { m.remember(name: "name \(i)", id: a) }
        XCTAssertEqual(m.pairs.count, deviceMemoryMaxPairs)
        m.remember(name: "name 0", id: a) // used again: now the newest
        m.remember(name: "one more", id: a)
        XCTAssertEqual(m.pairs.count, deviceMemoryMaxPairs)
        XCTAssertTrue(m.usedByOtherDevice(name: "name 0", id: b))
        XCTAssertFalse(m.usedByOtherDevice(name: "name 1", id: b))
    }

    func testJsonRoundTripsAndGarbageIsSkipped() {
        var m = DeviceMemory()
        m.remember(name: "Ana", id: a)
        m.remember(name: "Bea", id: b)
        XCTAssertEqual(DeviceMemory(json: m.json), m)
        for bad in ["", "null", "{}", "[1]", "[[\"Ana\"]]", "not json"] {
            XCTAssertTrue(DeviceMemory(json: bad).pairs.isEmpty, bad)
        }
    }
}

final class DeviceNoteMachineTests: XCTestCase {
    let now: UInt64 = 1_767_225_600_000
    let keyA = try! Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 1, count: 32))
    let keyB = try! Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 2, count: 32))

    func proof(_ key: Curve25519.Signing.PrivateKey, code: String = "K7QM2W", ts: UInt64? = nil) -> JoinDevice {
        let ts = ts ?? now
        let sig = try! key.signature(for: joinSignedBytes(code: code, ts: ts))
        return JoinDevice(pk: base64URLNoPadding(key.publicKey.rawRepresentation), ts: ts, sig: base64URLNoPadding(Data(sig)))
    }

    func machine(_ mode: JoinMode, memory: DeviceMemory = DeviceMemory()) -> ReceiverMachine {
        var m = ReceiverMachine(joinMode: mode, deviceMemory: memory)
        let t = now
        m.nowMs = { t }
        _ = m.handle(.start)
        _ = m.handle(.serverReady)
        _ = m.handle(.message(.roomCreated(code: "K7QM2W", joinMode: mode)))
        return m
    }

    func testConfirmNoteForAKnownNameWithAnotherDeviceOnly() {
        var m = machine(.confirm)
        _ = m.handle(.message(.joinRequest(id: 1, name: "Julian's iPad", check: 11, device: proof(keyA))))
        XCTAssertEqual(m.state.requests[0].otherDevice, false)
        XCTAssertNotNil(m.state.requests[0].deviceId)
        _ = m.handle(.decide(id: 1, allow: true, block: false))
        XCTAssertNil(m.takeDeviceMemoryChange(), "remembered only when the presenter is in")
        _ = m.handle(.message(.peerJoined(name: "Julian's iPad")))
        XCTAssertNotNil(m.takeDeviceMemoryChange())
        _ = m.handle(.message(.peerLeft))
        // Another key, same name: the note. Denied: not remembered.
        _ = m.handle(.message(.joinRequest(id: 2, name: "julian's ipad", check: 12, device: proof(keyB))))
        XCTAssertTrue(m.state.requests[0].otherDevice)
        _ = m.handle(.decide(id: 2, allow: false, block: false))
        XCTAssertNil(m.takeDeviceMemoryChange())
        // The first key again: no note. A new name: none.
        _ = m.handle(.message(.joinRequest(id: 3, name: "Julian's iPad", check: 13, device: proof(keyA))))
        _ = m.handle(.message(.joinRequest(id: 4, name: "Someone else", check: 14, device: proof(keyB))))
        XCTAssertEqual(m.state.requests.map(\.otherDevice), [false, false])
    }

    func testNoDeviceWithoutAValidProofAndNeverForGuestNames() {
        var memory = DeviceMemory()
        memory.remember(name: "Julian's iPad", id: displayKeyId(publicKey: keyA.publicKey.rawRepresentation))
        var m = machine(.confirm, memory: memory)
        let good = proof(keyB)
        let cases: [(String, JoinDevice?)] = [
            ("no proof", nil),
            ("another room", proof(keyB, code: "AAAAAA")),
            ("too old", proof(keyB, ts: now - 121_000)),
            ("bad signature", JoinDevice(pk: good.pk, ts: good.ts, sig: proof(keyA).sig)),
        ]
        for (i, c) in cases.enumerated() {
            _ = m.handle(.message(.joinRequest(id: 10 + i, name: "Julian's iPad", check: 20, device: c.1)))
            let r = m.state.requests.first { $0.id == 10 + i }!
            XCTAssertNil(r.deviceId, c.0)
            XCTAssertFalse(r.otherDevice, c.0)
        }
        _ = m.handle(.message(.joinRequest(id: 99, name: "", check: 20, device: good)))
        XCTAssertFalse(m.state.requests.first { $0.id == 99 }!.otherDevice)
    }

    func testOpenModeToastCarriesTheNoteAndRemembersVerifiedJoins() {
        var m = machine(.open)
        _ = m.handle(.message(.peerJoined(name: "Ana", device: proof(keyA))))
        XCTAssertEqual(m.takeEvents(), [.senderJoined(name: "Ana", unnamed: false, otherDevice: false)])
        XCTAssertNotNil(m.takeDeviceMemoryChange())
        _ = m.handle(.message(.peerLeft))
        _ = m.takeEvents()
        _ = m.handle(.message(.peerJoined(name: "ANA", device: proof(keyB))))
        XCTAssertEqual(m.takeEvents(), [.senderJoined(name: "ANA", unnamed: false, otherDevice: true)])
        _ = m.handle(.message(.peerLeft))
        _ = m.takeEvents()
        _ = m.handle(.message(.peerJoined(name: "Ana")))
        XCTAssertEqual(m.takeEvents(), [.senderJoined(name: "Ana", unnamed: false, otherDevice: false)])
    }

    func testTheDeviceOfAMessageIsReadLeniently() {
        let d = proof(keyA)
        let device: JSONValue = .object(["pk": .string(d.pk), "ts": .number(Double(d.ts)), "sig": .string(d.sig)])
        let text = JSON.serialize(.object(["type": .string("join_request"), "id": .number(1), "name": .string("A"), "check": .number(20), "device": device]))
        XCTAssertEqual(ServerMessage.decode(text), .joinRequest(id: 1, name: "A", check: 20, device: d))
        let bad = JSON.serialize(.object(["type": .string("peer_joined"), "name": .string("A"), "device": .object(["pk": .number(1)])]))
        XCTAssertEqual(ServerMessage.decode(bad), .peerJoined(name: "A", device: nil))
    }
}
