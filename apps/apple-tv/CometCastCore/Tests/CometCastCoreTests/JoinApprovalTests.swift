// SPDX-License-Identifier: GPL-3.0-or-later
// Join approval (protocol version 2): the shared vectors in tests/fixtures/join-approval against the Swift
// messages and the receiver state machine, plus unit tests of the machine's join logic.

import Foundation
import XCTest
@testable import CometCastCore

final class JoinApprovalFixtureTests: XCTestCase {
    func list(_ file: String, _ key: String) -> [[String: JSONValue]] {
        (Fixtures.load("join-approval/" + file)[key]?.arrayValue ?? []).compactMap { $0.objectValue }
    }

    func testNameSanitisingVectors() {
        let f = Fixtures.load("join-approval/name-sanitising.json")
        XCTAssertEqual(f["max_chars"]?.numberValue, Double(maxDisplayNameChars))
        let vs = list("name-sanitising.json", "vectors")
        XCTAssertEqual(vs.count, 10)
        for v in vs {
            let input = v["input"]?.stringValue
            let n = Int(v["guest_number"]?.numberValue ?? 1)
            XCTAssertEqual(sanitizeDisplayName(input, guestNumber: n), v["expected"]?.stringValue, "name: \(v["note"]?.stringValue ?? "")")
        }
    }

    /// Server messages decode and come back as exactly the fixture's JSON; client messages built from the fixture
    /// JSON encode to exactly that JSON (only the ones this client can produce).
    func testMessageVectors() throws {
        let ms = list("messages.json", "messages")
        XCTAssertGreaterThan(ms.count, 30)
        var server = 0, client = 0
        for m in ms {
            let id = m["id"]?.stringValue ?? "?"
            var msg = try XCTUnwrap(m["message"], id)
            // Optional fields this core does not use (the hand-over token of the iOS app) may be dropped.
            let droppable = (m["decoders_may_drop"]?.arrayValue ?? []).compactMap { $0.stringValue }
            if !droppable.isEmpty, case .object(var o) = msg {
                for k in droppable { o[k] = nil }
                msg = .object(o)
            }
            if m["direction"]?.stringValue == "server_to_client" {
                server += 1
                let got = ServerMessage.decode(JSON.serialize(msg))
                if got?.type == "error" { continue } // the error text is not compared (see testErrorCodes)
                XCTAssertEqual(got.map(FixtureTests.webShape) ?? .null, msg, id)
            } else {
                client += 1
                let built = try XCTUnwrap(Self.clientMessage(from: msg), id)
                XCTAssertEqual(built.json, msg, id)
                XCTAssertEqual(JSON.parse(built.encode()), msg, id)
            }
        }
        XCTAssertGreaterThan(server, 10)
        XCTAssertGreaterThan(client, 10)
    }

    static func clientMessage(from j: JSONValue) -> ClientMessage? {
        guard case .object(let o) = j, let t = o["type"]?.stringValue else { return nil }
        let mode = o["join_mode"]?.stringValue.flatMap(JoinMode.init(rawValue:))
        switch t {
        case "hello": return o["role"]?.stringValue.flatMap(Role.init(rawValue:)).map { .hello(role: $0) }
        case "create_room": return .createRoom(altcha: o["altcha"]?.stringValue, joinMode: mode, roomName: o["room_name"]?.stringValue)
        case "join_room": return o["code"]?.stringValue.map { .joinRoom(code: $0, name: o["name"]?.stringValue) }
        case "set_join_mode": return mode.map { .setJoinMode($0) }
        case "join_decision": return .joinDecision(id: o["id"]?.integerValue.map { Int($0) }, allow: o["allow"]?.boolValue ?? false, block: o["block"]?.boolValue ?? false)
        case "remove_peer": return .removePeer(block: o["block"]?.boolValue ?? false)
        case "unblock": return o["id"]?.integerValue.map { .unblock(id: Int($0)) }
        case "end_room": return .endRoom
        case "cancel_join": return .cancelJoin
        default: return nil
        }
    }

    func testErrorCodes() {
        let cs = list("error-codes.json", "codes")
        XCTAssertGreaterThanOrEqual(cs.count, 11)
        let codes = Set(cs.compactMap { $0["code"]?.stringValue })
        // every code of the fixture is known to the Swift side, in one of the two lists
        for c in codes { XCTAssertTrue(joinApprovalErrorCodes.contains(c) || knownErrorCodes.contains(c), c) }
        for c in joinApprovalErrorCodes { XCTAssertTrue(codes.contains(c), "fixture lacks \(c)") }
        // `closes`: a receiver ends its session on a closing code
        for c in cs where c["closes"]?.boolValue == true {
            var m = ReceiverMachine(); m.fetchesIce = false
            _ = m.handle(.start)
            _ = m.handle(.message(.roomCreated(code: "K7M2QX", joinMode: .open)))
            _ = m.handle(.message(.error(code: c["code"]?.stringValue ?? "", message: "")))
            XCTAssertEqual(m.state.phase, .error, c["code"]?.stringValue ?? "")
        }
        // the error messages of the fixture messages decode with their code
        for m in list("messages.json", "messages") where m["message"]?.objectValue?["type"]?.stringValue == "error" {
            guard case .error(let code, let text)? = ServerMessage.decode(JSON.serialize(m["message"] ?? .null)) else { return XCTFail("error decode") }
            XCTAssertTrue(codes.contains(code), code)
            XCTAssertFalse(text.isEmpty)
        }
    }

    /// Replays the flows from the receiver's side: server-to-receiver messages are fed in, receiver-to-server
    /// messages must be what the machine sends for the matching host action.
    func testFlows() throws {
        let flows = list("flows.json", "flows")
        XCTAssertEqual(flows.count, 11)
        var replayed = 0
        for flow in flows {
            let id = flow["id"]?.stringValue ?? "?"
            let steps = (flow["steps"]?.arrayValue ?? []).compactMap { $0.objectValue }
            guard steps.contains(where: { $0["from"]?.stringValue == "receiver" || $0["to"]?.stringValue == "receiver" }) else { continue }
            var m = ReceiverMachine(); m.fetchesIce = false
            var pending: [ClientMessage] = []
            var started = false
            func boot(mode: JoinMode?) {
                var fx = m.handle(.start)
                fx += m.handle(.serverReady)
                started = true
                pending += fx.compactMap { if case .send(let c) = $0 { return c } else { return nil } }
            }
            func run(_ input: ReceiverInput) -> [ClientMessage] {
                m.handle(input).compactMap { if case .send(let c) = $0 { return c } else { return nil } }
            }
            // flows that start in a running room
            if let first = steps.first(where: { $0["from"]?.stringValue == "receiver" || $0["to"]?.stringValue == "receiver" }),
               first["message"]?.objectValue?["type"]?.stringValue != "create_room" {
                boot(mode: nil)
                _ = m.handle(.message(.roomCreated(code: "K7M2QX", joinMode: .confirm)))
                pending = []
            }
            for step in steps {
                guard let msg = step["message"], case .object(let o) = msg, let type = o["type"]?.stringValue else { continue }
                if step["to"]?.stringValue == "receiver" {
                    let decoded = try XCTUnwrap(ServerMessage.decode(JSON.serialize(msg)), "\(id): \(type)")
                    pending += run(.message(decoded))
                } else if step["from"]?.stringValue == "receiver" {
                    replayed += 1
                    let want = try XCTUnwrap(JoinApprovalFixtureTests.clientMessage(from: msg), "\(id): \(type)")
                    switch want {
                    case .createRoom(_, let mode, let roomName):
                        if !started {
                            m = ReceiverMachine(joinMode: mode ?? .open, roomName: roomName); m.fetchesIce = false
                            boot(mode: mode)
                        }
                        XCTAssertEqual(pending.first, want, "\(id): create_room")
                        pending.removeAll()
                    case .joinDecision(let rid, _, _):
                        if rid == nil || !m.state.requests.contains(where: { $0.id == rid }) {
                            _ = m.handle(.message(.joinRequest(id: rid ?? 1, name: "x", check: 10)))
                        }
                        let sent: [ClientMessage]
                        if case .joinDecision(let rid, let allow, let block) = want {
                            sent = run(rid.map { .decide(id: $0, allow: allow, block: block) } ?? .denyAll)
                        } else { sent = [] }
                        XCTAssertEqual(sent, [want], "\(id): \(type)")
                    case .setJoinMode(let mode): XCTAssertEqual(run(.setJoinMode(mode)), [want], "\(id): \(type)")
                    case .removePeer(let b): XCTAssertEqual(run(.removePeer(block: b)), [want], "\(id): \(type)")
                    case .unblock(let uid): XCTAssertEqual(run(.unblock(id: uid)), [want], "\(id): \(type)")
                    case .endRoom: XCTAssertEqual(run(.endRoom), [want], "\(id): \(type)")
                    default: XCTFail("\(id): unexpected receiver message \(type)")
                    }
                }
            }
        }
        XCTAssertEqual(replayed, 11)
    }
}

extension ServerMessage {
    var type: String {
        switch self {
        case .error: return "error"
        default: return "other"
        }
    }
}

final class JoinMachineTests: XCTestCase {
    func room(_ mode: JoinMode = .confirm) -> ReceiverMachine {
        var m = ReceiverMachine(); m.fetchesIce = false
        _ = m.handle(.start)
        _ = m.handle(.serverReady)
        _ = m.handle(.message(.roomCreated(code: "K7M2QX", joinMode: mode)))
        return m
    }

    func testDefaultModeIsOpenAndSentWithCreateRoom() {
        var m = ReceiverMachine()
        XCTAssertEqual(m.state.joinMode, .open)
        _ = m.handle(.start)
        XCTAssertEqual(m.handle(.serverReady), [.send(.createRoom(altcha: nil, joinMode: .open))])
        XCTAssertEqual(m.handle(.message(.roomCreated(code: "K7M2QX", joinMode: .open))), [])
        XCTAssertEqual(m.state.joinMode, .open)
    }

    func testRoomNameIsSentWithEveryCreateRoom() {
        var m = ReceiverMachine(joinMode: .confirm, roomName: "Meeting room Elbe")
        _ = m.handle(.start)
        XCTAssertEqual(m.handle(.serverReady), [.send(.createRoom(altcha: nil, joinMode: .confirm, roomName: "Meeting room Elbe"))])
        _ = m.handle(.message(.roomCreated(code: "K7M2QX", joinMode: .confirm)))
        // the next room after End carries it too
        let fx = m.handle(.message(.roomEnded))
        XCTAssertTrue(fx.contains(.send(.createRoom(altcha: nil, joinMode: .confirm, roomName: "Meeting room Elbe"))), "\(fx)")
        // no name: the field is absent
        var n = ReceiverMachine()
        _ = n.handle(.start)
        XCTAssertEqual(n.handle(.serverReady), [.send(.createRoom(altcha: nil, joinMode: .open, roomName: nil))])
    }

    func testModeChosenBeforeTheRoomExistsIsAppliedAfterRoomCreated() {
        var m = ReceiverMachine(joinMode: .confirm)
        _ = m.handle(.start)
        XCTAssertEqual(m.handle(.serverReady), [.send(.createRoom(altcha: nil, joinMode: .confirm))])
        XCTAssertEqual(m.handle(.setJoinMode(.locked)), []) // no room yet: remembered
        XCTAssertEqual(m.handle(.message(.roomCreated(code: "K7M2QX", joinMode: .confirm))), [.send(.setJoinMode(.locked))])
        XCTAssertEqual(m.state.joinMode, .confirm) // changes when the server confirms
        XCTAssertEqual(m.handle(.message(.joinModeChanged(.locked))), [])
        XCTAssertEqual(m.state.joinMode, .locked)
        // the mode survives stop and start
        _ = m.handle(.stop)
        XCTAssertEqual(m.state.joinMode, .locked)
    }

    func testRequestsQueueWithCleanedNames() {
        var m = room()
        _ = m.handle(.message(.joinRequest(id: 1, name: "A\u{202E}na\u{200B}", check: 42)))
        _ = m.handle(.message(.joinRequest(id: 2, name: "\u{202E}", check: 17)))
        _ = m.handle(.message(.joinRequest(id: 1, name: "dup", check: 99))) // same id: ignored
        XCTAssertEqual(m.state.requests, [JoinRequest(id: 1, name: "Ana", check: 42), JoinRequest(id: 2, name: "Guest 2", check: 17)])
        XCTAssertEqual(m.state.phase, .waitingSender) // a request does not change the phase
    }

    func testDecisions() {
        var m = room()
        for i in 1...3 { _ = m.handle(.message(.joinRequest(id: i, name: "n\(i)", check: 10 + i))) }
        XCTAssertEqual(m.handle(.decide(id: 1, allow: true, block: true)), [.send(.joinDecision(id: 1, allow: true, block: false))]) // block only with a denial
        XCTAssertEqual(m.handle(.decide(id: 2, allow: false, block: true)), [.send(.joinDecision(id: 2, allow: false, block: true))])
        XCTAssertEqual(m.state.requests.map(\.id), [3])
        XCTAssertEqual(m.handle(.decide(id: 2, allow: true, block: false)), []) // gone already
        XCTAssertEqual(m.handle(.denyAll), [.send(.joinDecision(id: nil, allow: false, block: false))])
        XCTAssertEqual(m.state.requests, [])
        XCTAssertEqual(m.handle(.denyAll), []) // nothing waits
    }

    func testCancelledAndLocked() {
        var m = room()
        _ = m.handle(.message(.joinRequest(id: 1, name: "a", check: 11)))
        _ = m.handle(.message(.joinRequest(id: 2, name: "b", check: 12)))
        _ = m.handle(.message(.joinRequest(id: 3, name: "c", check: 13)))
        _ = m.handle(.message(.joinRequestCancelled(id: 1, reason: "timeout")))
        XCTAssertEqual(m.state.requests.map(\.id), [2, 3])
        XCTAssertNil(m.state.notice)
        _ = m.handle(.message(.joinRequestCancelled(id: 2, reason: "locked")))
        _ = m.handle(.message(.joinRequestCancelled(id: 3, reason: "locked")))
        XCTAssertEqual(m.state.requests, [])
        XCTAssertEqual(m.state.notice, .lockedDiscarded(count: 2))
        _ = m.handle(.message(.joinModeChanged(.locked)))
        XCTAssertEqual(m.state.notice, .lockedDiscarded(count: 2)) // kept while locked
        _ = m.handle(.message(.joinModeChanged(.confirm)))
        XCTAssertNil(m.state.notice)
        _ = m.handle(.message(.joinRequestCancelled(id: 9, reason: "left"))) // unknown id: harmless
        _ = m.handle(.dismissNotice)
    }

    func testPeerNameAndEndingThePresentation() {
        var m = room()
        XCTAssertEqual(m.handle(.message(.peerJoined(name: " A\u{202E}na  M "))), [.createPeer])
        XCTAssertEqual(m.state.peerName, "Ana M")
        XCTAssertEqual(m.state.phase, .negotiating)
        XCTAssertEqual(m.handle(.removePeer(block: true)), [.send(.removePeer(block: true))])
        XCTAssertEqual(m.handle(.message(.blocked(id: 2, name: "Ana"))), [])
        XCTAssertEqual(m.state.blocks, [BlockedEntry(id: 2, name: "Ana")])
        _ = m.handle(.message(.blocked(id: 2, name: "again")))
        XCTAssertEqual(m.state.blocks.count, 1)
        XCTAssertEqual(m.handle(.message(.peerLeft)), [.closePeer, .resetContent])
        XCTAssertNil(m.state.peerName)
        XCTAssertEqual(m.state.phase, .peerLeft)
        XCTAssertEqual(m.state.code, "K7M2QX") // the room stays
        XCTAssertEqual(m.handle(.unblock(id: 2)), [.send(.unblock(id: 2))])
        _ = m.handle(.message(.unblocked(id: 2)))
        XCTAssertEqual(m.state.blocks, [])
        // a sender without a usable name gets a number
        _ = m.handle(.message(.peerJoined(name: "")))
        XCTAssertEqual(m.state.peerName, "Guest 2")
    }

    func testEndRoomStartsANewRoomOnTheSameConnection() {
        var m = room(.confirm)
        _ = m.handle(.setJoinMode(.locked))
        _ = m.handle(.message(.joinModeChanged(.locked)))
        _ = m.handle(.message(.joinRequest(id: 1, name: "a", check: 11)))
        _ = m.handle(.message(.blocked(id: 5, name: "b")))
        _ = m.handle(.message(.peerJoined(name: "Ana")))
        XCTAssertEqual(m.handle(.endRoom), [.send(.endRoom)])
        XCTAssertEqual(m.state.code, "K7M2QX") // nothing changes before the server answers
        let fx = m.handle(.message(.roomEnded))
        XCTAssertEqual(fx, [.closePeer, .resetContent, .send(.createRoom(altcha: nil, joinMode: .locked))])
        XCTAssertEqual(m.state.phase, .connecting)
        XCTAssertNil(m.state.code)
        XCTAssertNil(m.state.peerName)
        XCTAssertEqual(m.state.requests, [])
        XCTAssertEqual(m.state.blocks, []) // the block list lives as long as the room
        XCTAssertEqual(m.handle(.message(.roomCreated(code: "B4N9HT", joinMode: .locked))), [])
        XCTAssertEqual(m.state.code, "B4N9HT")
        XCTAssertEqual(m.state.phase, .waitingSender)
    }

    func testActionsWithoutARoomDoNothing() {
        var m = ReceiverMachine()
        for i: ReceiverInput in [.decide(id: 1, allow: true, block: false), .denyAll, .removePeer(block: false), .unblock(id: 1), .endRoom] {
            XCTAssertEqual(m.handle(i), [], "\(i)")
        }
        _ = m.handle(.start)
        XCTAssertEqual(m.handle(.endRoom), []) // connecting: no code yet
    }

    func testOpenRoomsDisabledFallsBackToConfirm() {
        var m = ReceiverMachine()
        m.fetchesIce = false
        _ = m.handle(.start)
        _ = m.handle(.serverReady)
        let fx = m.handle(.message(.error(code: "open_rooms_disabled", message: "x")))
        XCTAssertEqual(fx, [.send(.createRoom(altcha: nil, joinMode: .confirm))])
        XCTAssertEqual(m.state.phase, .connecting)
        XCTAssertEqual(m.state.joinMode, .confirm)
        XCTAssertEqual(m.state.notice, .openDisabled)
        // later (room exists): only a notice
        _ = m.handle(.message(.roomCreated(code: "K7M2QX", joinMode: .confirm)))
        XCTAssertEqual(m.handle(.message(.error(code: "open_rooms_disabled", message: ""))), [])
        XCTAssertEqual(m.state.phase, .waitingSender)
    }

    func testOpenRoomsDisabledKeepsTheAltchaPayload() {
        var m = ReceiverMachine()
        m.fetchesIce = false
        _ = m.handle(.start)
        _ = m.handle(.message(.error(code: "altcha_required", message: "")))
        _ = m.handle(.altchaSolved("P"))
        XCTAssertEqual(m.handle(.message(.error(code: "open_rooms_disabled", message: ""))), [.send(.createRoom(altcha: "P", joinMode: .confirm))])
    }

    func testNonFatalOwnerErrors() {
        var m = room()
        _ = m.handle(.message(.joinRequest(id: 1, name: "a", check: 11)))
        XCTAssertEqual(m.handle(.message(.error(code: "no_such_request", message: ""))), [])
        XCTAssertEqual(m.handle(.message(.error(code: "no_such_block", message: ""))), [])
        XCTAssertEqual(m.state.phase, .waitingSender)
        XCTAssertNil(m.state.notice)
        XCTAssertEqual(m.handle(.message(.error(code: "block_list_full", message: ""))), [])
        XCTAssertEqual(m.state.notice, .blockListFull)
        XCTAssertEqual(m.state.phase, .waitingSender)
        // anything else is terminal and clears the join state
        _ = m.handle(.message(.error(code: "banned", message: "")))
        XCTAssertEqual(m.state.phase, .error)
        XCTAssertEqual(m.state.requests, [])
        XCTAssertEqual(m.state.blocks, [])
    }

    func testStrictDecoding() {
        let bad = [
            #"{"type":"join_request","id":1,"name":"a"}"#,
            #"{"type":"join_request","id":"1","name":"a","check":42}"#,
            #"{"type":"join_request","id":1.5,"name":"a","check":42}"#,
            #"{"type":"join_request","id":1,"name":5,"check":42}"#,
            #"{"type":"join_request_cancelled","id":1}"#,
            #"{"type":"join_mode_changed","join_mode":"nope"}"#,
            #"{"type":"join_mode_changed"}"#,
            #"{"type":"blocked","id":1}"#,
            #"{"type":"unblocked"}"#,
            #"{"type":"room_created","code":"ABC234","join_mode":"Open"}"#,
            #"{"type":"join_pending","timeout_secs":60}"#,
        ]
        for t in bad { XCTAssertNil(ServerMessage.decode(t), t) }
        XCTAssertEqual(ServerMessage.decode(#"{"type":"room_ended","x":1}"#), .roomEnded)
    }
}
