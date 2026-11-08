// SPDX-License-Identifier: GPL-3.0-or-later
// The numbers and rules of docs/design/first-run-and-display.md that the display's views rely on.

import XCTest
@testable import CometCastCore
#if canImport(CryptoKit)
import CryptoKit
#else
import Crypto
#endif

final class DisplayDesignTests: XCTestCase {
    func testPalettesAreTheSpecHexValues() {
        XCTAssertEqual(DisplayPaletteSpec.dark.bg, Rgb(0x0E1630))
        XCTAssertEqual(DisplayPaletteSpec.dark.acc, Rgb(0xFFC266))
        XCTAssertEqual(DisplayPaletteSpec.dark.focus, Rgb(0xFFC94D))
        XCTAssertEqual(DisplayPaletteSpec.light.fg, Rgb(0x0A0F1E))
        XCTAssertEqual(DisplayPaletteSpec.light.focus, Rgb(0xB35C00))
        XCTAssertEqual(DisplayPaletteSpec.oledBlack.line, Rgb(0x2C313C))
        XCTAssertEqual(DisplayPaletteSpec.oledBlack.qrbg, Rgb(0xE6E6E6))
        XCTAssertEqual(DisplayPaletteSpec.qrModule, Rgb(0x0E1630))
        XCTAssertEqual(DisplayPaletteSpec.onBlack, Rgb(0xA1A9BA))
        for p in [DisplayPaletteSpec.oledBlack] {
            XCTAssertEqual([p.bg, p.panel, p.panel2, p.btn], Array(repeating: Rgb(0), count: 4))
        }
    }

    func testSelectedTileFillIsTheSpecBlend() {
        XCTAssertEqual(DisplayPaletteSpec.dark.selectedTileFill, Rgb(0x3E4053)) // native-apps 10
        XCTAssertEqual(DisplayPaletteSpec.light.selectedTileFill, Rgb(0xD7D3CF))
        XCTAssertEqual(DisplayPaletteSpec.oledBlack.selectedTileFill, Rgb(0x000000)) // no fill in OLED black
    }

    func testAutomaticNeverPicksOled() {
        XCTAssertEqual(AppearanceChoice.auto.palette(systemIsDark: true), .dark)
        XCTAssertEqual(AppearanceChoice.auto.palette(systemIsDark: false), .light)
        XCTAssertEqual(AppearanceChoice.oled.palette(systemIsDark: false), .oledBlack)
        XCTAssertEqual(AppearanceChoice.default, .dark) // D3
        XCTAssertFalse(AppearanceChoice.dark.usesLightLogo(systemIsDark: false))
        XCTAssertFalse(AppearanceChoice.oled.usesLightLogo(systemIsDark: false))
        XCTAssertTrue(AppearanceChoice.light.usesLightLogo(systemIsDark: true))
        XCTAssertTrue(AppearanceChoice.auto.usesLightLogo(systemIsDark: false))
        XCTAssertEqual(AppearanceChoice.allCases.map(\.rawValue), ["dark", "light", "oled", "auto"]) // tile order
    }

    func testScaleAndSafePadding() {
        XCTAssertEqual(canvasScale(width: 1920, height: 1080), 2)
        XCTAssertEqual(canvasScale(width: 3840, height: 2160), 4)
        XCTAssertEqual(canvasScale(width: 1920, height: 1200), 2) // 16:10: letterboxed by width... height wins
        XCTAssertEqual(canvasScale(width: 1280, height: 1080), 1280 / 960)
        // tvOS safe area at 1920 x 1080 points: top and bottom 60, sides 80 -> 30, 30, 40 ref (native-apps 7)
        XCTAssertEqual(safePadding(spec: IdlePadding.top, inset: 60, scale: 2), 30)
        XCTAssertEqual(safePadding(spec: IdlePadding.bottom, inset: 60, scale: 2), 30)
        XCTAssertEqual(safePadding(spec: IdlePadding.side, inset: 80, scale: 2), 40)
    }

    func testTypeTableAndFonts() {
        XCTAssertEqual(DisplayType.code, TypeStyle(80, .monoBold, line: 1.0, tracking: 0.06))
        XCTAssertEqual(DisplayType.address.tracking, -0.01)
        XCTAssertEqual(DisplayType.heading.size, 32)
        XCTAssertEqual(FontFace.monoBold.rawValue, "AtkinsonHyperlegibleMono-Bold")
        XCTAssertEqual(FontFace.medium.fileName, "AtkinsonHyperlegibleNext-500.ttf")
        XCTAssertEqual(DisplayType.wizardBody.lineSpacing, 18 * 0.4, accuracy: 1e-9)
        XCTAssertEqual(DisplayType.code.lineSpacing, 0)
    }

    func testTimings() {
        XCTAssertEqual(DisplayTiming.controlsVisibleSeconds, 5)
        XCTAssertEqual(DisplayTiming.controlsFadeSeconds, 0.4)
        XCTAssertEqual(DisplayTiming.toastSeconds, 3.2)
        XCTAssertEqual(DisplayTiming.idleDimAfterSeconds, 420)
        XCTAssertEqual(DisplayTiming.idleDimOverlayAlpha, 0.6)
        XCTAssertEqual(DisplayTiming.inputGuardSeconds, 1)
        XCTAssertEqual(DisplayTiming.connectingToOfflineSeconds, 8)
        XCTAssertEqual(FocusRing.scale, 1.03)
    }

    func testLockedIsNeverStored() {
        XCTAssertEqual(storableDefaultMode(.open), .open)
        XCTAssertEqual(storableDefaultMode(.confirm), .confirm)
        XCTAssertNil(storableDefaultMode(.locked))
        XCTAssertEqual(joinModeKeys(.confirm).title, "mode.approval")
        XCTAssertEqual(joinModeKeys(.locked).text, "settings.join.lockedText")
        XCTAssertEqual(joinModeOrder, [.open, .confirm, .locked])
    }
}

final class DisplayTextTests: XCTestCase {
    func testRoomNameIsCleanedLikeTheRustCore() {
        XCTAssertEqual(cleanRoomName("  Room \u{202E}204\u{200B}  "), "Room 204")
        XCTAssertEqual(cleanRoomName("A\u{7}  B\tC"), "A B C")
        XCTAssertEqual(cleanRoomName("\u{200B}  "), "")
        XCTAssertEqual(cleanRoomName("\u{E000}x\u{F0001}"), "x")
        XCTAssertEqual(cleanRoomName(String(repeating: "x", count: 100)).count, 32)
        XCTAssertEqual(cleanRoomName("Besprechungsraum Elbe"), "Besprechungsraum Elbe")
        // cut, then trimmed: a blank at the cut position goes
        XCTAssertEqual(cleanRoomName(String(repeating: "a", count: 31) + " b"), String(repeating: "a", count: 31))
    }

    func testPersonNames() {
        XCTAssertEqual(shortPersonName("Lena"), "Lena")
        XCTAssertEqual(shortPersonName("  Lena \u{202E} Meier "), "Lena Meier")
        XCTAssertEqual(shortPersonName("Abcdefghijklmnopqrstuvwxyz"), "Abcdefghijklmnopqr…") // 18 + …
        XCTAssertEqual(shortPersonName("123456789012345678"), "123456789012345678")
        XCTAssertNil(shortPersonName("\u{200B}"))
        XCTAssertNil(shortPersonName("Guest 3", unnamed: true)) // common.noName
        XCTAssertEqual(fullPersonName("Abcdefghijklmnopqrstuvwxyz"), "Abcdefghijklmnopqrstuvwxyz")
        XCTAssertNil(fullPersonName("", unnamed: true))
    }

    func testJoinAddress() {
        XCTAssertEqual(joinAddress(host: "relay.cometcast.de"), "cometcast.de/app") // N85
        XCTAssertEqual(joinAddress(host: "relay.cometcast.de:443"), "cometcast.de/app")
        XCTAssertEqual(joinAddress(host: "cast.musterfirma.de"), "cast.musterfirma.de")
        XCTAssertEqual(joinAddress(host: "cast.musterfirma.de:443"), "cast.musterfirma.de") // N12
        XCTAssertEqual(joinAddress(host: "cast.musterfirma.de:8443"), "cast.musterfirma.de:8443")
        XCTAssertEqual(joinAddress(host: "relay.cometcast.de:8443"), "relay.cometcast.de:8443")
        XCTAssertEqual(joinAddress(host: "10.0.0.5:8443"), "10.0.0.5:8443")
        XCTAssertEqual(joinAddress(host: "[::1]:8443"), "[::1]:8443")
    }

    func testIdleAddressWithBox() {
        let a = idleAddress(relayHost: "cast-box.local", fromBox: true, boxIPv4: "192.168.1.20")
        XCTAssertEqual(a, IdleAddress(dom: "cast-box.local", altIp: "192.168.1.20", qrHost: "192.168.1.20"))
        XCTAssertEqual(a.qrUrl(code: "K7QM2W"), "https://192.168.1.20/K7QM2W")
        // a port other than 443 stays on both
        XCTAssertEqual(idleAddress(relayHost: "cast-box.local:8443", fromBox: true, boxIPv4: "192.168.1.20").altIp, "192.168.1.20:8443")
        // only when `{dom}` is a host name
        XCTAssertNil(idleAddress(relayHost: "192.168.1.20", fromBox: true, boxIPv4: "192.168.1.20").altIp)
        // only for a server that came from a box announcement, only with a valid IPv4
        XCTAssertNil(idleAddress(relayHost: "cast-box.local", fromBox: false, boxIPv4: "192.168.1.20").altIp)
        XCTAssertNil(idleAddress(relayHost: "cast-box.local", fromBox: true, boxIPv4: "fe80::1").altIp)
        let d = idleAddress(relayHost: "relay.cometcast.de", fromBox: false, boxIPv4: nil)
        XCTAssertEqual(d.qrUrl(code: "K7QM2W"), "https://cometcast.de/app/K7QM2W")
        XCTAssertEqual(qrUrl(dom: "cast.musterfirma.de", code: "ABC234"), "https://cast.musterfirma.de/ABC234")
    }

    func testSecurityCodeAndTransferPill() {
        XCTAssertEqual(groupedSas("171852"), "171 852")
        XCTAssertEqual(groupedSas("171 852"), "171 852")
        XCTAssertEqual(groupedSas("12"), "12")
        XCTAssertEqual(shortenMiddle("short.pdf", keep: 40), "short.pdf")
        let s = shortenMiddle("Quarterly-report-final-version-2026.pdf", keep: 20)
        XCTAssertTrue(s.hasSuffix(".pdf"))
        XCTAssertTrue(s.contains("…"))
        XCTAssertEqual(s.count, 20 + 1 - 0) // head + … + tail = keep + 1
        XCTAssertEqual(transferPercent(received: 50, size: 200), 25)
        XCTAssertEqual(transferPercent(received: 500, size: 200), 100)
        XCTAssertEqual(transferPercent(received: 5, size: 0), 0)
    }

    func testPlaceholders() {
        XCTAssertEqual(fillPlaceholders("{name} zeigt", ["name": "„Lena“"]), "„Lena“ zeigt")
        XCTAssertEqual(fillPlaceholders("+{n} warten", ["n": "2"]), "+2 warten")
        XCTAssertEqual(fillPlaceholders("{a}{b} {a}", ["a": "x", "b": "y"]), "xy x")
        XCTAssertEqual(fillPlaceholders("no {value} here", [:]), "no {value} here")
        XCTAssertEqual(fillPlaceholders("{", [:]), "{")
    }

    func testPageMessageAndAnnouncementName() {
        XCTAssertEqual(pageMessageJSON(page: 3, pages: 12), #"{"kind":"page","page":3,"pages":12}"#)
        XCTAssertNil(pageMessageJSON(page: 0, pages: 12))
        XCTAssertNil(pageMessageJSON(page: 13, pages: 12))
        XCTAssertNil(pageMessageJSON(page: 1, pages: 0))
        // The announced record is signed (docs/design/signed-announcements.md): the keys come in the order of
        // the signed bytes, `sig` last; details are in DisplayAnnouncementTests.
        let key = try! Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x42, count: 32))
        let t = displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://relay.cometcast.de:443/ws", mode: .open, code: "K7QM2W", roomName: "  Room\u{202E} 204 ")
        XCTAssertEqual(t?.map(\.0), ["v", "id", "pk", "seq", "state", "srv", "tls", "mode", "code", "name", "sig"])
        XCTAssertEqual(t?.first { $0.0 == "name" }?.1, "Room 204")
        // no name set: no key, no fallback to the device name
        XCTAssertEqual(displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://a.de/ws", mode: .open, code: "K7QM2W", roomName: "\u{200B}")?.map(\.0), ["v", "id", "pk", "seq", "state", "srv", "tls", "mode", "code", "sig"])
        // Locked keeps announcing name and mode, without a code
        XCTAssertEqual(displayAnnouncementTxt(key: key, seq: 1, wsUrl: "wss://a.de/ws", mode: .locked, code: nil, roomName: "Elbe")?.map(\.0), ["v", "id", "pk", "seq", "state", "srv", "tls", "mode", "name", "sig"])
        XCTAssertEqual(builtInRelayHost, defaultServers[0].split(separator: ":")[0].description)
    }
}

final class DisplayConfigTests: XCTestCase {
    func testRoundTripAndLockedIsNeverStored() {
        let c = DisplayConfig(room: "Meeting room Elbe", mode: .locked, wsUrl: "wss://relay.cometcast.de/ws", source: .dnsSrv, appearance: .oled)
        XCTAssertEqual(c.mode, .open) // Locked is never stored
        let d = DisplayConfig.decode(c.encode())
        XCTAssertEqual(d, c)
        XCTAssertEqual(d?.relay?.host, "relay.cometcast.de")
        XCTAssertEqual(d?.relay?.source, .dnsSrv)
    }

    func testLenientRead() {
        XCTAssertNil(DisplayConfig.decode(nil))
        XCTAssertNil(DisplayConfig.decode("not json"))
        XCTAssertNil(DisplayConfig.decode(#"{"room":"  ","server":"wss://a.de/ws"}"#)) // no room: the wizard shows again
        XCTAssertNil(DisplayConfig.decode(#"{"room":"A","server":"ftp://a.de"}"#))
        XCTAssertNil(DisplayConfig.decode(#"{"room":"A"}"#))
        let c = DisplayConfig.decode(#"{"room":"Room‮ 7","mode":"locked","server":"wss://a.de/ws","source":"bogus","appearance":"neon"}"#)
        XCTAssertEqual(c?.room, "Room 7")
        XCTAssertEqual(c?.mode, .open) // anything but confirm is open
        XCTAssertEqual(c?.source, .defaultList)
        XCTAssertEqual(c?.appearance, .dark)
        XCTAssertEqual(DisplayConfig.decode(#"{"room":"A","mode":"confirm","server":"ws://10.0.0.5:8443/ws"}"#)?.mode, .confirm)
    }
}
