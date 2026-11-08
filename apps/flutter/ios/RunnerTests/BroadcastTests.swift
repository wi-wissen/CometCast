// SPDX-License-Identifier: GPL-3.0-or-later
// Tests of the pure logic of the iOS broadcast: App Group files, Annex B conversion, frame size.
// UNCERTAIN: never compiled or run (no Xcode here). The byte vectors are the same as in
// tools/ios-annexb-selfcheck.py, which was run.

import ImageIO
import XCTest
@testable import Runner

final class BroadcastStoreTests: XCTestCase {
    private func makeStore() -> BroadcastStore {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("cometcast-test-\(UUID().uuidString)", isDirectory: true)
        return BroadcastStore(directory: dir)
    }

    func testConfigRoundTripAndDelete() throws {
        let store = makeStore()
        XCTAssertNil(store.readConfig())
        let c = BroadcastConfig(code: "ABC234", relay: "relay.example:8443", requestedAt: 100,
                                maxDimension: 1280, fps: 30, bitrate: 2_500_000)
        try store.writeConfig(c)
        XCTAssertEqual(store.readConfig(), c)
        store.deleteConfig()
        XCTAssertNil(store.readConfig())
    }

    func testConfigCarriesNameAndHandoverAndOldFilesStillDecode() throws {
        let store = makeStore()
        let c = BroadcastConfig(code: "ABC234", relay: "r", requestedAt: 1, maxDimension: 1280, fps: 30,
                                bitrate: 1_000_000, name: "Lena", handover: "q3Zx9_-Lm0AbCdEfGhIjKl")
        try store.writeConfig(c)
        XCTAssertEqual(store.readConfig()?.name, "Lena")
        XCTAssertEqual(store.readConfig()?.handover, "q3Zx9_-Lm0AbCdEfGhIjKl")
        // A config written by the app before the hand-over existed has neither field.
        let old = #"{"version":1,"code":"ABC234","relay":"r","requestedAt":1,"maxDimension":1280,"fps":30,"bitrate":1}"#
        let decoded = try JSONDecoder().decode(BroadcastConfig.self, from: Data(old.utf8))
        XCTAssertNil(decoded.name)
        XCTAssertNil(decoded.handover)
        XCTAssertNil(decoded.deviceKey)
    }

    func testConfigCarriesTheDeviceKeyOfTheApp() throws {
        let store = makeStore()
        let key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
        let c = BroadcastConfig(code: "ABC234", relay: "r", requestedAt: 1, maxDimension: 1280, fps: 30,
                                bitrate: 1_000_000, deviceKey: key)
        try store.writeConfig(c)
        XCTAssertEqual(store.readConfig()?.deviceKey, key)
    }

    func testStatusCarriesTheFallbackFieldsAndTheWayBack() throws {
        let store = makeStore()
        var s = BroadcastStatus(seq: 1, state: .pending, error: .none, sent: 0, dropped: 0, bytes: 0, keyframes: 0,
                                width: 0, height: 0, freeMemoryMB: -1, updatedAt: 5)
        s.check = 42; s.timeoutSecs = 60; s.roomName = "Room 204"; s.sas = "171 852"; s.nextHandover = "abc"
        try store.writeStatus(s)
        XCTAssertEqual(store.readStatus(), s)
        XCTAssertTrue(s.isStale(now: Date(timeIntervalSince1970: 100)), "pending counts as active")
        // A status written without the new fields still decodes.
        let old = #"{"seq":1,"state":"waiting","error":"none","sent":0,"dropped":0,"bytes":0,"keyframes":0,"width":0,"height":0,"freeMemoryMB":-1,"updatedAt":5}"#
        let d = try JSONDecoder().decode(BroadcastStatus.self, from: Data(old.utf8))
        XCTAssertNil(d.check)
        XCTAssertNil(d.nextHandover)
    }

    func testHandoverTokens() {
        // 16 bytes make 22 base64url characters without padding.
        let t = HandoverToken.make(bytes: [UInt8](repeating: 0xfb, count: 16))
        XCTAssertEqual(t.count, 22)
        XCTAssertEqual(t, "-_v7-_v7-_v7-_v7-_v7-w")
        XCTAssertTrue(HandoverToken.isValid(t))
        XCTAssertTrue(HandoverToken.isValid(HandoverToken.make()))
        XCTAssertNotEqual(HandoverToken.make(), HandoverToken.make())
        XCTAssertFalse(HandoverToken.isValid(""))
        XCTAssertFalse(HandoverToken.isValid(String(repeating: "a", count: 65)))
        XCTAssertTrue(HandoverToken.isValid(String(repeating: "a", count: 64)))
        for bad in ["a b", "a+b", "a/b", "a=", "ä"] { XCTAssertFalse(HandoverToken.isValid(bad), bad) }
    }

    func testConfigFreshness() {
        let now = Date(timeIntervalSince1970: 1_000_000)
        func config(age: Double) -> BroadcastConfig {
            BroadcastConfig(code: "ABC234", relay: "r", requestedAt: now.timeIntervalSince1970 - age,
                            maxDimension: 1280, fps: 30, bitrate: 1)
        }
        XCTAssertTrue(config(age: 0).isFresh(now: now))
        XCTAssertTrue(config(age: 299).isFresh(now: now))
        XCTAssertFalse(config(age: 301).isFresh(now: now))
        XCTAssertFalse(config(age: -60).isFresh(now: now), "a timestamp far in the future is not fresh")
    }

    func testConfigIsClamped() {
        let c = BroadcastConfig(code: "X", relay: "r", requestedAt: 0, maxDimension: 99_999, fps: 240,
                                bitrate: 1_000_000_000).clamped
        XCTAssertEqual(c.maxDimension, 1920)
        XCTAssertEqual(c.fps, 30)
        XCTAssertEqual(c.bitrate, 8_000_000)
        let low = BroadcastConfig(code: "X", relay: "r", requestedAt: 0, maxDimension: 1, fps: 0, bitrate: 0).clamped
        XCTAssertEqual(low.maxDimension, 320)
        XCTAssertEqual(low.fps, 5)
    }

    func testStatusStaleness() {
        let now = Date(timeIntervalSince1970: 1_000_000)
        func status(_ state: BroadcastStateName, age: Double) -> BroadcastStatus {
            BroadcastStatus(seq: 1, state: state, error: .none, sent: 0, dropped: 0, bytes: 0, keyframes: 0,
                            width: 0, height: 0, freeMemoryMB: -1,
                            updatedAt: now.timeIntervalSince1970 - age)
        }
        XCTAssertFalse(status(.streaming, age: 1).isStale(now: now))
        XCTAssertTrue(status(.streaming, age: 6).isStale(now: now))
        XCTAssertFalse(status(.ended, age: 600).isStale(now: now), "a finished broadcast is not 'lost'")
        XCTAssertTrue(status(.failed, age: 0).isTerminal)
    }

    func testStatusAndStopFlag() throws {
        let store = makeStore()
        XCTAssertNil(store.readStatus())
        XCTAssertFalse(store.stopRequested())
        store.requestStop()
        XCTAssertTrue(store.stopRequested())
        store.clearStopRequest()
        XCTAssertFalse(store.stopRequested())
        let s = BroadcastStatus(seq: 7, state: .streaming, error: .none, sent: 10, dropped: 1, bytes: 1000,
                                keyframes: 2, width: 1280, height: 720, freeMemoryMB: 23, updatedAt: 5)
        try store.writeStatus(s)
        XCTAssertEqual(store.readStatus(), s)
    }
}

final class AnnexBTests: XCTestCase {
    // Same vectors as tools/ios-annexb-selfcheck.py.
    let sps: [UInt8] = [0x67, 0x42, 0xc0, 0x1f, 0xda, 0x05]
    let pps: [UInt8] = [0x68, 0xce, 0x3c, 0x80]
    let idr: [UInt8] = [0x65, 0x88, 0x84, 0x00, 0x10]
    let delta: [UInt8] = [0x41, 0x9a, 0x02, 0x03]

    private func avcc(_ nals: [[UInt8]], lengthSize: Int = 4) -> [UInt8] {
        var out = [UInt8]()
        for n in nals {
            for shift in stride(from: (lengthSize - 1) * 8, through: 0, by: -8) {
                out.append(UInt8((n.count >> shift) & 0xff))
            }
            out += n
        }
        return out
    }

    func testAvccToAnnexB() {
        let data = avcc([idr, delta])
        var out = [UInt8]()
        let units = data.withUnsafeBytes { AnnexB.appendAnnexB(fromAVCC: $0, lengthSize: 4, into: &out) }
        XCTAssertEqual(units, 2)
        XCTAssertEqual(out, [0, 0, 0, 1] + idr + [0, 0, 0, 1] + delta)
        XCTAssertTrue(AnnexB.containsIDR(out))
        XCTAssertEqual(AnnexB.firstNalType(out), 5)
    }

    func testMalformedAvccIsRejected() {
        var out = [UInt8]()
        let truncated: [UInt8] = [0, 0, 0, 9, 0x65, 1]
        XCTAssertEqual(truncated.withUnsafeBytes { AnnexB.appendAnnexB(fromAVCC: $0, lengthSize: 4, into: &out) }, -1)
        let zero: [UInt8] = [0, 0, 0, 0]
        XCTAssertEqual(zero.withUnsafeBytes { AnnexB.appendAnnexB(fromAVCC: $0, lengthSize: 4, into: &out) }, -1)
    }

    func testParameterSets() {
        XCTAssertEqual(AnnexB.annexB(parameterSets: [Data(sps), Data(pps)]),
                       [0, 0, 0, 1] + sps + [0, 0, 0, 1] + pps)
    }

    func testReceiverSplit() {
        let annexB: [UInt8] = [0, 0, 0, 1] + sps + [0, 0, 1] + pps + [0, 0, 0, 1] + idr
        let unit = H264Parsing.split(annexB: annexB)
        XCTAssertEqual(unit?.sps, sps)
        XCTAssertEqual(unit?.pps, pps)
        XCTAssertEqual(unit?.isKey, true)
        XCTAssertEqual(unit?.avcc, avcc([idr]))
        XCTAssertNil(H264Parsing.split(annexB: [0, 0, 0, 1] + sps + [0, 0, 0, 1] + pps), "no slice")
    }
}

final class FrameSizeTests: XCTestCase {
    func testScalesDownKeepsAspectAndEvenEdges() {
        let r = FrameSize.fitted(width: 2732, height: 2048, orientation: .up, maxDimension: 1280)
        XCTAssertEqual(r.width, 1280)
        XCTAssertEqual(r.height, 960)
        let phone = FrameSize.fitted(width: 1170, height: 2532, orientation: .up, maxDimension: 1280)
        XCTAssertEqual(phone.height, 1280)
        XCTAssertEqual(phone.width % 2, 0)
        XCTAssertEqual(phone.width, 592)
    }

    func testRotationSwapsTheEdges() {
        let r = FrameSize.fitted(width: 1170, height: 2532, orientation: .left, maxDimension: 1280)
        XCTAssertEqual(r.width, 1280)
        XCTAssertLessThan(r.height, r.width)
    }

    func testNeverScalesUp() {
        let r = FrameSize.fitted(width: 640, height: 480, orientation: .up, maxDimension: 1280)
        XCTAssertEqual(r.width, 640)
        XCTAssertEqual(r.height, 480)
    }
}
