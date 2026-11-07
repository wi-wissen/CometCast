// SPDX-License-Identifier: GPL-3.0-or-later
// The rules of the share extension that need no UIKit (ShareExtension/ShareLogic.swift).
// UNCERTAIN: never run (no Xcode here, docs/ios.md).

import XCTest
@testable import Runner

final class ShareTests: XCTestCase {
    func testEventsOfThePoll() {
        XCTAssertEqual(ShareEvent.parse(#"{"event":"joined","room_name":"Room 1"}"#), .joined(roomName: "Room 1"))
        XCTAssertEqual(ShareEvent.parse(#"{"event":"joined","room_name":null}"#), .joined(roomName: nil))
        XCTAssertEqual(
            ShareEvent.parse(#"{"event":"waiting","check":42,"timeout_secs":60,"room_name":null}"#),
            .waiting(check: 42, timeoutSecs: 60, roomName: nil))
        XCTAssertEqual(ShareEvent.parse(#"{"event":"page","page":2,"pages":5}"#), .page(2, of: 5))
        // A page outside the range is not believed.
        XCTAssertEqual(ShareEvent.parse(#"{"event":"page","page":6,"pages":5}"#), .unknown)
        XCTAssertEqual(ShareEvent.parse(#"{"event":"ended","code":"removed_by_host"}"#), .ended(code: "removed_by_host"))
        XCTAssertEqual(ShareEvent.parse("not json"), .unknown)
        XCTAssertEqual(ShareEvent.parse(#"{"event":"something new"}"#), .unknown)
    }

    func testErrorPages() {
        XCTAssertEqual(ShareErrorPage.forCode("room_not_found"), .code)
        XCTAssertEqual(ShareErrorPage.forCode("join_denied"), .denied)
        XCTAssertEqual(ShareErrorPage.forCode("approval_timeout"), .timeout)
        XCTAssertEqual(ShareErrorPage.forCode("room_locked"), .locked)
        XCTAssertEqual(ShareErrorPage.forCode("connection_lost"), .noInternet)
        XCTAssertEqual(ShareErrorPage.forCode("peer_failed"), .displayOff)
        XCTAssertEqual(ShareErrorPage.forCode("removed_by_host"), .ended)
        XCTAssertEqual(ShareErrorPage.forCode("whatever"), .other)
    }

    func testJoinAddressAndTrust() {
        XCTAssertEqual(joinAddress("relay.cometcast.de:443"), "cometcast.de/app")
        XCTAssertEqual(joinAddress("wss://cast.example.org:443/"), "cast.example.org")
        XCTAssertEqual(joinAddress("192.168.1.5:8443"), "192.168.1.5:8443")
        XCTAssertTrue(isTrustedServer("relay.cometcast.de:443", confirmed: []))
        XCTAssertFalse(isTrustedServer("cast.example.org", confirmed: []))
        XCTAssertTrue(isTrustedServer("cast.example.org:443", confirmed: ["cast.example.org"]))
    }

    func testFileNamesAreShortenedInTheMiddleKeepingTheExtension() {
        XCTAssertEqual(shortenedFileName("a.pdf"), "a.pdf")
        let n = shortenedFileName("a-very-long-file-name-for-a-holiday-photo-2026.jpeg")
        XCTAssertTrue(n.hasSuffix(".jpeg"))
        XCTAssertTrue(n.contains("…"))
        XCTAssertLessThanOrEqual(n.count, 29)
    }

    func testSettingsFileRoundTrip() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let store = ShareSettingsStore(directory: dir)
        XCTAssertEqual(store.read(), ShareSettings())
        store.write(ShareSettings(name: "Anna", confirmedServers: ["a.example"], lastServer: "a.example"))
        store.writeName("Ben")
        let s = store.read()
        XCTAssertEqual(s.name, "Ben")
        XCTAssertEqual(s.confirmedServers, ["a.example"])
        XCTAssertEqual(s.lastServer, "a.example")
        try? FileManager.default.removeItem(at: dir)
    }

    func testRejectionKeys() {
        XCTAssertEqual(ShareModel.rejectKey("too_large"), "sendRejectTooLarge")
        XCTAssertEqual(ShareModel.rejectKey("unsupported_type"), "sendRejectType")
        XCTAssertEqual(ShareModel.rejectKey("bad_link"), "shareNoLink")
        XCTAssertEqual(ShareModel.rejectKey("?"), "sendRejectUnreadable")
    }
}
