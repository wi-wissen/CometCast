// SPDX-License-Identifier: GPL-3.0-or-later
// The announced record follows docs/discovery.md section 3 (the same rules as crates/cc-discovery display.rs).

import XCTest
@testable import CometCastCore

final class DisplayAnnouncementTests: XCTestCase {
    let id = "0123456789abcdef"

    func dict(_ t: [(String, String)]?) -> [String: String] { Dictionary(uniqueKeysWithValues: t ?? []) }

    func testOpenRecordCarriesTheCode() {
        let t = displayAnnouncementTxt(id: id, wsUrl: "wss://relay.cometcast.de:443/ws", mode: .open, code: "K7QM2W")
        XCTAssertEqual(dict(t), ["v": "1", "id": id, "srv": "relay.cometcast.de:443", "tls": "1", "mode": "open", "code": "K7QM2W"])
    }

    func testConfirmIsAnnouncedAsApproval() {
        let t = displayAnnouncementTxt(id: id, wsUrl: "ws://10.0.0.5:8443/ws", mode: .confirm, code: "K7QM2W")
        XCTAssertEqual(dict(t)["mode"], "approval")
        XCTAssertNil(dict(t)["tls"])
        XCTAssertEqual(dict(t)["srv"], "10.0.0.5:8443")
    }

    func testLockedNeverCarriesACode() {
        let t = displayAnnouncementTxt(id: id, wsUrl: "wss://relay.cometcast.de/ws", mode: .locked, code: "K7QM2W")
        XCTAssertEqual(dict(t)["mode"], "locked")
        XCTAssertNil(dict(t)["code"])
    }

    func testUnusableStatesAreNotAnnounced() {
        XCTAssertNil(displayAnnouncementTxt(id: "XYZ", wsUrl: "wss://a.de/ws", mode: .open, code: "K7QM2W"))
        XCTAssertNil(displayAnnouncementTxt(id: id, wsUrl: "https://a.de/ws", mode: .open, code: "K7QM2W"))
        XCTAssertNil(displayAnnouncementTxt(id: id, wsUrl: "wss://a.de/ws", mode: .open, code: nil))
        XCTAssertNil(displayAnnouncementTxt(id: id, wsUrl: "wss://a.de/ws", mode: .open, code: "K7QM2O"))
    }

    func testInstanceNameAndId() {
        XCTAssertEqual(announcementInstanceName(id: id), "CometCast-01234567")
        XCTAssertTrue(isValidInstanceId(newInstanceId()))
        XCTAssertFalse(isValidInstanceId("0123456789ABCDEF"))
    }

    func testPolicy() {
        XCTAssertTrue(announcementsAllowed(policy: .absent))
        XCTAssertFalse(announcementsAllowed(policy: .invalid))
    }
}
