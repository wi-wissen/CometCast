// SPDX-License-Identifier: GPL-3.0-or-later
// Key tables of spec 7.1, controls reveal, idle dimming, input guard, toasts, reconnect schedule, wizard.

import XCTest
@testable import CometCastCore

final class InteractionTests: XCTestCase {
    func testBackAndMenuPerContext() {
        XCTAssertEqual(intentForBack(.idle), .system) // Menu leaves the app on Apple TV (native-apps 5)
        XCTAssertEqual(intentForMenu(.idle), .openSettings) // Play/Pause = Menu
        XCTAssertEqual(intentForBack(.settings), .closeSettings)
        XCTAssertEqual(intentForMenu(.settings), .closeSettings)
        XCTAssertEqual(intentForBack(.about), .closeAbout)
        XCTAssertEqual(intentForBack(.licences), .closeLicences)
        XCTAssertEqual(intentForBack(.joinRequest), .consume)
        XCTAssertEqual(intentForMenu(.joinRequest), .consume)
        XCTAssertEqual(intentForBack(.linkCard), .dismissLink)
        XCTAssertEqual(intentForBack(.setup(step: 0)), .system)
        XCTAssertEqual(intentForBack(.setup(step: 3)), .wizardBack)
        XCTAssertEqual(intentForMenu(.setup(step: 3)), .consume)
        XCTAssertEqual(intentForBack(.presenting(controlsShown: false)), .showControls)
        XCTAssertEqual(intentForBack(.presenting(controlsShown: true)), .hideControls)
        XCTAssertEqual(intentForMenu(.presenting(controlsShown: false)), .showControls)
        XCTAssertEqual(intentForMenu(.presenting(controlsShown: true)), .consume)
    }

    func testPresentingKeysTable() {
        // hidden: the first input only reveals; arrows also focus End; nothing activates
        for k: RemoteKey in [.up, .down, .left, .right] {
            let r = presentingResponse(key: k, controlsShown: false)
            XCTAssertTrue(r.reveal && r.consumed && r.focusEnd && !r.activateEnd, "\(k)")
        }
        for k: RemoteKey in [.select, .back, .menu] {
            let r = presentingResponse(key: k, controlsShown: false)
            XCTAssertTrue(r.reveal && r.consumed && !r.focusEnd && !r.activateEnd, "\(k)")
        }
        // shown
        XCTAssertTrue(presentingResponse(key: .left, controlsShown: true).reveal)
        XCTAssertTrue(presentingResponse(key: .left, controlsShown: true).focusEnd)
        XCTAssertTrue(presentingResponse(key: .select, controlsShown: true).activateEnd)
        XCTAssertTrue(presentingResponse(key: .back, controlsShown: true).hide)
        XCTAssertEqual(presentingResponse(key: .menu, controlsShown: true), PresentingResponse(consumed: true))
    }

    func testControlsRevealFiveSeconds() {
        var c = ControlsReveal()
        XCTAssertFalse(c.shown)
        XCTAssertTrue(c.poke(now: 10)) // was hidden: the input only reveals
        XCTAssertTrue(c.shown)
        XCTAssertEqual(c.hideAt, 15)
        XCTAssertFalse(c.tick(now: 14.9))
        XCTAssertFalse(c.poke(now: 14.9)) // restarts the 5 s
        XCTAssertEqual(c.hideAt, 19.9)
        XCTAssertFalse(c.tick(now: 19.8))
        XCTAssertTrue(c.tick(now: 19.9))
        XCTAssertFalse(c.shown)
        c.poke(now: 30); c.hideNow()
        XCTAssertFalse(c.shown)
        XCTAssertNil(c.hideAt)
    }

    func testIdleDimAfterSevenMinutes() {
        var d = IdleDimmer()
        XCTAssertFalse(d.tick(now: 1000)) // not enabled
        d.setEnabled(true, now: 0)
        XCTAssertEqual(d.nextDimAt, 420)
        XCTAssertFalse(d.tick(now: 419))
        XCTAssertTrue(d.tick(now: 420))
        XCTAssertTrue(d.dimmed)
        XCTAssertEqual(d.overlayAlpha, 0.6) // 40 % brightness
        XCTAssertTrue(d.activity(now: 500)) // the first key press only restores
        XCTAssertFalse(d.dimmed)
        XCTAssertEqual(d.overlayAlpha, 0)
        XCTAssertFalse(d.activity(now: 501)) // not dimmed: the key activates as usual
        XCTAssertFalse(d.tick(now: 900))
        XCTAssertTrue(d.tick(now: 921))
        d.setEnabled(false, now: 930) // a presentation starts: restored, never dims
        XCTAssertFalse(d.dimmed)
        XCTAssertFalse(d.tick(now: 5000))
        XCTAssertNil(d.nextDimAt)
    }

    func testInputGuardOneSecond() {
        var g = InputGuard()
        XCTAssertTrue(g.isArmed(now: 0)) // nothing shown yet
        g.arm(now: 100)
        XCTAssertFalse(g.isArmed(now: 100.5))
        XCTAssertFalse(g.isArmed(now: 100.99))
        XCTAssertTrue(g.isArmed(now: 101))
        g.arm(now: 110) // the next request
        XCTAssertFalse(g.isArmed(now: 110.2))
    }

    func testToastReplacesAndHides() {
        var t = ToastState()
        t.show("A", now: 0)
        XCTAssertEqual(t.text, "A")
        t.show("B", now: 1)
        XCTAssertEqual(t.text, "B")
        XCTAssertEqual(t.serial, 2)
        XCTAssertFalse(t.tick(now: 4.1))
        XCTAssertTrue(t.tick(now: 4.2))
        XCTAssertNil(t.text)
    }
}

final class ConnectionViewTests: XCTestCase {
    func testScheduleOneTwoThreeThenEveryFour() {
        XCTAssertEqual(retryDelay(code: "connection_lost", failedAttempts: 1, sinceLoss: 0), 1)
        XCTAssertEqual(retryDelay(code: "connection_lost", failedAttempts: 2, sinceLoss: 1.1)!, 0.9, accuracy: 1e-9)
        XCTAssertEqual(retryDelay(code: "connection_lost", failedAttempts: 3, sinceLoss: 2.1)!, 0.9, accuracy: 1e-9)
        XCTAssertEqual(retryDelay(code: "connection_lost", failedAttempts: 4, sinceLoss: 3.1)!, 3.9, accuracy: 1e-9) // 7 s
        XCTAssertEqual(retryDelay(code: "connection_lost", failedAttempts: 5, sinceLoss: 7.1)!, 3.9, accuracy: 1e-9) // 11 s
        XCTAssertEqual(retryDelay(code: "connection_lost", failedAttempts: 2, sinceLoss: 9), 0) // never negative
        XCTAssertEqual(retryDelay(code: "rate_limited"), 30)
        XCTAssertEqual(retryDelay(code: "forbidden"), 30)
        XCTAssertNil(retryDelay(code: "proto_mismatch"))
        XCTAssertNil(retryDelay(code: "bad_hello"))
        XCTAssertEqual(offlineCause("server_full"), .busy)
        XCTAssertEqual(offlineCause("peer_failed"), .network)
        XCTAssertEqual(offlineCause(nil), .network)
        XCTAssertEqual(OfflineCause.outdated.bodyKey, "idle.offline.bodyOutdatedApp")
    }

    func testBoxOnlyAfterTheAttemptAtThreeSecondsFailed() {
        var t = ConnectionTracker(now: 0)
        XCTAssertEqual(t.update(code: nil, phase: .connecting, errorCode: nil, now: 0), .cancel)
        XCTAssertEqual(t.view(code: nil, phase: .connecting, now: 1), .connecting)
        // the first attempt fails at 0.2 s
        XCTAssertEqual(t.update(code: nil, phase: .error, errorCode: "connection_lost", now: 0.2), .after(0.8))
        XCTAssertEqual(t.view(code: nil, phase: .error, now: 0.2), .connecting)
        XCTAssertEqual(t.update(code: nil, phase: .connecting, errorCode: nil, now: 1.0), .cancel)
        XCTAssertEqual(t.update(code: nil, phase: .error, errorCode: "connection_lost", now: 1.1), .after(2 - 1.1))
        XCTAssertEqual(t.update(code: nil, phase: .connecting, errorCode: nil, now: 2.0), .cancel)
        XCTAssertEqual(t.update(code: nil, phase: .error, errorCode: "connection_lost", now: 2.1), .after(3 - 2.1))
        XCTAssertEqual(t.view(code: nil, phase: .connecting, now: 2.5), .connecting) // retry running: no flicker
        XCTAssertEqual(t.update(code: nil, phase: .connecting, errorCode: nil, now: 3.0), .cancel)
        // the attempt at 3 s fails: now the box
        guard case .after(let next) = t.update(code: nil, phase: .error, errorCode: "connection_lost", now: 3.1) else { return XCTFail() }
        XCTAssertEqual(next, 7 - 3.1, accuracy: 1e-9)
        XCTAssertEqual(t.view(code: nil, phase: .error, now: 3.1), .offline)
        XCTAssertEqual(t.view(code: nil, phase: .connecting, now: 4), .offline) // does not flicker away while a retry runs
        // the code comes back by itself
        XCTAssertEqual(t.update(code: "K7QM2W", phase: .waitingSender, errorCode: nil, now: 5), .cancel)
        XCTAssertEqual(t.view(code: "K7QM2W", phase: .waitingSender, now: 5), .online)
        XCTAssertEqual(t.failedAttempts, 0)
    }

    func testShortDropNeverShowsTheBox() {
        var t = ConnectionTracker(now: 0)
        _ = t.update(code: "K7QM2W", phase: .waitingSender, errorCode: nil, now: 1)
        _ = t.update(code: nil, phase: .error, errorCode: "connection_lost", now: 10) // lost
        XCTAssertEqual(t.lostAt, 10)
        XCTAssertEqual(t.view(code: nil, phase: .error, now: 10), .connecting)
        _ = t.update(code: nil, phase: .connecting, errorCode: nil, now: 11)
        _ = t.update(code: "B4N9HT", phase: .waitingSender, errorCode: nil, now: 11.5)
        XCTAssertEqual(t.view(code: "B4N9HT", phase: .waitingSender, now: 11.5), .online)
    }

    func testStartWithoutAnyFailureEndsInTheBoxAfterEightSeconds() {
        var t = ConnectionTracker(now: 0)
        _ = t.update(code: nil, phase: .connecting, errorCode: nil, now: 0)
        XCTAssertEqual(t.view(code: nil, phase: .connecting, now: 7.9), .connecting)
        XCTAssertEqual(t.view(code: nil, phase: .connecting, now: 8), .offline)
        // an unchanged state asks for nothing
        XCTAssertEqual(t.update(code: nil, phase: .connecting, errorCode: nil, now: 9), .unchanged)
    }

    func testBusyAndOutdatedShowTheBoxAtOnce() {
        var t = ConnectionTracker(now: 0)
        XCTAssertEqual(t.update(code: nil, phase: .error, errorCode: "rate_limited", now: 0.1), .after(30))
        XCTAssertEqual(t.view(code: nil, phase: .error, now: 0.1), .offline)
        XCTAssertEqual(t.cause, .busy)
        // a connection that closes within 60 s counts as the same refusal
        _ = t.update(code: nil, phase: .connecting, errorCode: nil, now: 30)
        XCTAssertEqual(t.update(code: nil, phase: .error, errorCode: "connection_lost", now: 31), .after(30))
        XCTAssertEqual(t.cause, .busy)
        // after 60 s a closed connection is the network again
        _ = t.update(code: nil, phase: .connecting, errorCode: nil, now: 61)
        guard case .after = t.update(code: nil, phase: .error, errorCode: "connection_lost", now: 95) else { return XCTFail() }
        XCTAssertEqual(t.cause, .network)
        var o = ConnectionTracker(now: 0)
        XCTAssertEqual(o.update(code: nil, phase: .error, errorCode: "proto_mismatch", now: 1), .never)
        XCTAssertEqual(o.view(code: nil, phase: .error, now: 1), .offline)
        XCTAssertEqual(o.cause, .outdated)
    }
}

final class WizardTests: XCTestCase {
    func relay(_ host: String, _ source: RelaySource) -> Relay {
        parseRelay(host).relay(source: source)!
    }

    func make(_ sources: [(String, RelaySource)] = [("relay.cometcast.de", .defaultList)], locked: Bool = false) -> WizardState {
        WizardState(firstSuggestion: "Meeting room Elbe", choices: sources.map { AddressChoice(relay: relay($0.0, $0.1)) }, policyLocked: locked)
    }

    func testStepsAndBack() {
        var w = make()
        XCTAssertEqual(w.step, 0)
        w.reduce(.back); XCTAssertEqual(w.step, 0) // on step 0 nothing
        for want in 1...4 { w.reduce(.next); XCTAssertEqual(w.step, want) }
        w.reduce(.next); XCTAssertEqual(w.step, 4)
        w.reduce(.back); XCTAssertEqual(w.step, 3)
    }

    func testNameChipsAndFieldDecision39() {
        var w = make()
        XCTAssertEqual(w.room, "Meeting room Elbe")
        XCTAssertTrue(w.chipSelected("Meeting room Elbe"))
        w.reduce(.chip("Workshop"))
        XCTAssertEqual(w.room, "Workshop")
        w.reduce(.custom("Raum 7"))
        XCTAssertEqual(w.room, "Raum 7")
        XCTAssertFalse(w.chipSelected("Workshop")) // a custom name deselects all chips
        w.reduce(.custom("   ")) // only spaces counts as empty: the chip selected before typing returns
        XCTAssertEqual(w.room, "Workshop")
        XCTAssertTrue(w.chipSelected("Workshop"))
        w.reduce(.custom("Raum 7")); w.reduce(.chip("Clubhouse"))
        XCTAssertEqual(w.custom, "") // choosing a chip clears the field
        w.reduce(.custom(String(repeating: "x", count: 50)))
        XCTAssertEqual(w.custom.count, 32)
    }

    func testModeDefaultsToOpenAndLockedIsNotOffered() {
        var w = make()
        XCTAssertEqual(w.mode, .open) // D1
        w.reduce(.mode(.confirm)); XCTAssertEqual(w.mode, .confirm)
        w.reduce(.mode(.locked)); XCTAssertEqual(w.mode, .confirm)
    }

    func testAddressSourceLines() {
        XCTAssertEqual(AddressChoice(relay: relay("relay.cometcast.de", .defaultList)).sourceKey, "setup.address.sourceDefault")
        XCTAssertEqual(AddressChoice(relay: relay("cast.musterfirma.de", .dnsSrv)).sourceKey, "setup.address.sourceOwn")
        XCTAssertEqual(AddressChoice(relay: relay("cast-box.local", .displayAnnouncement)).sourceKey, "setup.address.sourceBox")
        XCTAssertEqual(AddressChoice(relay: relay("cast.example.org", .mdm)).sourceKey, "setup.address.sourcePolicy")
        XCTAssertEqual(AddressChoice(relay: relay("cast.example.org", .user)).sourceKey, "setup.address.sourceTyped")
    }

    func testOtherAddressCyclesThenOpensTheFieldThenKeepsOpeningIt() {
        var w = make([("cast.musterfirma.de", .dnsSrv), ("relay.cometcast.de", .defaultList)])
        for _ in 0..<3 { w.reduce(.next) }
        XCTAssertEqual(w.step, 3)
        XCTAssertEqual(w.address?.relay.host, "cast.musterfirma.de")
        w.reduce(.changeAddress)
        XCTAssertEqual(w.address?.relay.host, "relay.cometcast.de")
        XCTAssertFalse(w.fieldOpen)
        w.reduce(.changeAddress) // the found ones are through: the field
        XCTAssertTrue(w.fieldOpen)
        XCTAssertFalse(w.canChangeAddress) // "Other address" is not drawn while the field is open
        XCTAssertEqual(wizardRows(step: 3, fieldOpen: true, canChangeAddress: false), [[.field], [.back, .looksRight]])
        w.reduce(.back) // closes the field, stays on step 3
        XCTAssertEqual(w.step, 3)
        XCTAssertFalse(w.fieldOpen)
        w.reduce(.changeAddress) // after the field was used or cancelled it opens again (13.38)
        XCTAssertTrue(w.fieldOpen)
    }

    func testSingleCandidateOpensTheFieldAndTypedAddressIsValidated() {
        var w = make()
        for _ in 0..<3 { w.reduce(.next) }
        w.reduce(.changeAddress)
        XCTAssertTrue(w.fieldOpen)
        w.reduce(.fieldText("bad address"))
        w.reduce(.fieldConfirm)
        XCTAssertEqual(w.fieldError, .invalid)
        XCTAssertTrue(w.fieldOpen)
        w.reduce(.fieldText("ws://example.org")) // ws only for local hosts
        w.reduce(.fieldConfirm)
        XCTAssertEqual(w.fieldError, .insecure)
        w.reduce(.fieldText("cast.example.org:8443"))
        XCTAssertNil(w.fieldError)
        w.reduce(.fieldConfirm)
        XCTAssertFalse(w.fieldOpen)
        XCTAssertEqual(w.address?.relay.host, "cast.example.org:8443")
        XCTAssertEqual(w.address?.sourceKey, "setup.address.sourceTyped")
        w.reduce(.next) // Looks right
        XCTAssertEqual(w.step, 4)
        let r = w.result
        XCTAssertEqual(r?.relay.wsUrl, "wss://cast.example.org:8443/ws")
        XCTAssertEqual(r?.room, "Meeting room Elbe")
    }

    func testPolicyLockHidesOtherAddress() {
        var w = make([("cast.example.org", .mdm)], locked: true)
        for _ in 0..<3 { w.reduce(.next) }
        XCTAssertFalse(w.canChangeAddress)
        w.reduce(.changeAddress)
        XCTAssertFalse(w.fieldOpen)
        XCTAssertEqual(wizardRows(step: 3, fieldOpen: false, canChangeAddress: false), [[.back, .looksRight]])
        XCTAssertEqual(wizardRows(step: 3, fieldOpen: false, canChangeAddress: true), [[.back, .changeAddress, .looksRight]])
    }

    func testLateDiscoveryReplacesThePreselectionUntilTheUserMovesOn() {
        var w = make()
        w.setChoices([AddressChoice(relay: relay("cast.musterfirma.de", .dnsSrv)), AddressChoice(relay: relay("relay.cometcast.de", .defaultList))])
        XCTAssertEqual(w.address?.relay.host, "cast.musterfirma.de")
        w.reduce(.next); w.reduce(.next); w.reduce(.next)
        w.reduce(.changeAddress)
        XCTAssertEqual(w.address?.relay.host, "relay.cometcast.de")
        w.setChoices([AddressChoice(relay: relay("cast.musterfirma.de", .dnsSrv)), AddressChoice(relay: relay("cast-box.local", .displayAnnouncement)), AddressChoice(relay: relay("relay.cometcast.de", .defaultList))])
        XCTAssertEqual(w.address?.relay.host, "relay.cometcast.de") // what the person chose stays
    }

    func testFocusRowsMatchSpec() {
        XCTAssertEqual(wizardRows(step: 0, fieldOpen: false, canChangeAddress: true), [[.start]])
        XCTAssertEqual(wizardRows(step: 1, fieldOpen: false, canChangeAddress: true).count, 3)
        XCTAssertEqual(wizardRows(step: 2, fieldOpen: false, canChangeAddress: true), [[.modeOpen, .modeApproval], [.back, .next]])
        XCTAssertEqual(wizardRows(step: 4, fieldOpen: false, canChangeAddress: true), [[.finish]])
        XCTAssertEqual(wizardInitialFocus(step: 1), .next)
        XCTAssertEqual(wizardInitialFocus(step: 3), .looksRight)
    }
}
