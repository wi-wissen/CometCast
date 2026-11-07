// SPDX-License-Identifier: GPL-3.0-or-later
// The licence list of About, parsed from the real THIRD_PARTY files; host events of the machine.

import XCTest
@testable import CometCastCore

final class LicenceTests: XCTestCase {
    static func repoFile(_ name: String) -> String {
        var u = URL(fileURLWithPath: #filePath)
        for _ in 0..<6 { u.deleteLastPathComponent() } // file, CometCastCoreTests, Tests, CometCastCore, apple-tv, apps
        return (try? String(contentsOf: u.appendingPathComponent(name), encoding: .utf8)) ?? ""
    }

    func testSyntheticFiles() {
        let notices = """
        # Title

        ## OpenH264 notice

        Cisco text line 1
        ```text
        ## not a heading
        ```

        ## Atkinson Hyperlegible notice

        Copyright lines

        ## Open Screen Library notice

        desktop only

        ## PDFium Foxit font notice (hayro-font)

        Foxit text
        """
        let licences = """
        ## Rust crates

        #### not an entry

        ### Rust licence texts

        #### tokio 1.0, mio 1.0

        ```text
        MIT License

        ## 1. A line inside the text
        more
        ```
        #### serde 1.0
        ```text
        Apache License
        ```
        ## npm packages (apps/web, production tree)

        | Package | Version | Licence |
        | --- | --- | --- |
        | vue | 3.5.0 | MIT |
        | vue-i18n | 11.0.0 | MIT |

        ## Fonts bundled in the repository (cc-office)

        #### Liberation 2.1.5
        ```text
        x
        ```
        ## Flutter (pub) packages

        ### Flutter licence texts

        #### path 1.0
        ```text
        BSD
        ```
        """
        let e = parseLicences(notices: notices, licences: licences)
        XCTAssertEqual(e.map(\.name), ["Cisco OpenH264", "Atkinson Hyperlegible", "PDFium Foxit font notice (hayro-font)", "tokio 1.0, mio 1.0", "serde 1.0", "vue", "vue-i18n", "path 1.0"])
        XCTAssertEqual(e.map(\.featured), [true, true, true, false, false, false, false, false])
        XCTAssertEqual(e[0].text, "Cisco text line 1\n```text\n## not a heading\n```")
        XCTAssertEqual(e[0].licence, "Cisco text line 1")
        XCTAssertEqual(e[3].text, "MIT License\n\n## 1. A line inside the text\nmore")
        XCTAssertEqual(e[3].licence, "MIT License")
        XCTAssertEqual(e[5].licence, "MIT"); XCTAssertEqual(e[5].text, "MIT")
        XCTAssertFalse(e.contains { $0.name.contains("Open Screen") || $0.name.contains("Liberation") })
    }

    func testRealFiles() {
        let n = Self.repoFile("THIRD_PARTY_NOTICES.md"), l = Self.repoFile("THIRD_PARTY_LICENSES.md")
        XCTAssertGreaterThan(n.count, 10_000); XCTAssertGreaterThan(l.count, 1_000_000)
        let e = parseLicences(notices: n, licences: l)
        XCTAssertGreaterThan(e.count, 500)
        XCTAssertEqual(e.prefix(4).map(\.name), ["Cisco OpenH264", "Atkinson Hyperlegible", "PDFium Foxit font notice (hayro-font)", "Adobe CMap notice (hayro-cmap)"])
        XCTAssertTrue(e.prefix(4).allSatisfy(\.featured))
        XCTAssertTrue(e.dropFirst(4).allSatisfy { !$0.featured })
        XCTAssertTrue(e.allSatisfy { !$0.text.isEmpty && $0.licence.count <= 60 })
        // spec 15.11: the search finds "openh264", "Atkinson" and "tokio"
        for q in ["openh264", "atkinson", "tokio"] {
            XCTAssertTrue(e.contains { $0.name.lowercased().contains(q) }, q)
        }
        XCTAssertTrue(e.first { $0.name == "Atkinson Hyperlegible" }!.text.contains("SIL OPEN FONT LICENSE"))
    }

    func testBundledCopiesAreTheRootFiles() {
        for f in ["THIRD_PARTY_NOTICES.md", "THIRD_PARTY_LICENSES.md"] {
            let copy = Self.repoFile("apps/apple-tv/Resources/Licenses/" + f)
            XCTAssertFalse(copy.isEmpty, "missing bundled copy of \(f): run node tools/gen-third-party.mjs")
            XCTAssertEqual(copy, Self.repoFile(f), "\(f) differs from the bundled copy: run node tools/gen-third-party.mjs")
        }
    }
}

final class HostEventTests: XCTestCase {
    func room() -> ReceiverMachine {
        var m = ReceiverMachine(joinMode: .confirm, roomName: "Elbe"); m.fetchesIce = false
        _ = m.handle(.start); _ = m.handle(.serverReady)
        _ = m.handle(.message(.roomCreated(code: "K7M2QX", joinMode: .confirm)))
        return m
    }

    func testExpiredWithdrawnAndSilentCancels() {
        var m = room()
        _ = m.handle(.message(.joinRequest(id: 1, name: "Ana", check: 11)))
        _ = m.handle(.message(.joinRequest(id: 2, name: "Ben", check: 12)))
        _ = m.handle(.message(.joinRequest(id: 3, name: "\u{200B}", check: 13)))
        _ = m.handle(.message(.joinRequest(id: 4, name: "Dan", check: 14)))
        _ = m.handle(.message(.joinRequestCancelled(id: 1, reason: "timeout")))
        _ = m.handle(.message(.joinRequestCancelled(id: 2, reason: "left")))
        _ = m.handle(.message(.joinRequestCancelled(id: 3, reason: "cancelled")))
        _ = m.handle(.message(.joinRequestCancelled(id: 4, reason: "room_full"))) // no toast for this one
        _ = m.handle(.message(.joinRequestCancelled(id: 9, reason: "timeout"))) // unknown id: nothing
        XCTAssertEqual(m.takeEvents(), [
            .requestExpired(name: "Ana", unnamed: false),
            .requestWithdrawn(name: "Ben", unnamed: false),
            .requestWithdrawn(name: "Guest 3", unnamed: true),
        ])
        XCTAssertEqual(m.takeEvents(), []) // taken once
    }

    func testJoinLeaveAndEnd() {
        var m = room()
        _ = m.handle(.message(.peerJoined(name: "Lena")))
        XCTAssertEqual(m.state.peerUnnamed, false)
        _ = m.handle(.message(.peerLeft))
        _ = m.handle(.message(.peerJoined(name: "")))
        XCTAssertEqual(m.state.peerUnnamed, true)
        _ = m.handle(.message(.roomEnded))
        XCTAssertEqual(m.takeEvents(), [
            .senderJoined(name: "Lena", unnamed: false), .senderLeft(name: "Lena", unnamed: false),
            .senderJoined(name: "Guest 2", unnamed: true), .sessionEnded,
        ])
        XCTAssertFalse(m.state.peerUnnamed)
    }

    func testDefaultModeSurvivesEndLockedDoesNot() {
        var m = room()
        _ = m.handle(.setJoinMode(.open)) // Settings: Open becomes the default
        _ = m.handle(.message(.joinModeChanged(.open)))
        _ = m.handle(.setJoinMode(.locked))
        _ = m.handle(.message(.joinModeChanged(.locked)))
        let fx = m.handle(.message(.roomEnded))
        XCTAssertTrue(fx.contains(.send(.createRoom(altcha: nil, joinMode: .open, roomName: "Elbe"))), "\(fx)")
        XCTAssertEqual(m.state.joinMode, .open)
        // Approval on as the default
        _ = m.handle(.message(.roomCreated(code: "B4N9HT", joinMode: .open)))
        _ = m.handle(.setJoinMode(.confirm))
        _ = m.handle(.message(.joinModeChanged(.confirm)))
        _ = m.handle(.setJoinMode(.locked))
        let fx2 = m.handle(.message(.roomEnded))
        XCTAssertTrue(fx2.contains(.send(.createRoom(altcha: nil, joinMode: .confirm, roomName: "Elbe"))), "\(fx2)")
    }
}
