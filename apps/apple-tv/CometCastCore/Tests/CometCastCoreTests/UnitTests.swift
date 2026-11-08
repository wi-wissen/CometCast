// SPDX-License-Identifier: GPL-3.0-or-later
// Tests that are not driven by the shared fixtures: JSON parser, PBKDF2, state machine, server list rules,
// content controller, small helpers.

import Foundation
import XCTest
@testable import CometCastCore

final class JSONTests: XCTestCase {
    func testParsesLikeJavaScript() {
        XCTAssertEqual(JSON.parse(" {\"a\" : [1, 2.5, -0, 1e2, true, null, \"x\"]} "),
                       .object(["a": .array([.number(1), .number(2.5), .number(0), .number(100), .bool(true), .null, .string("x")])]))
        XCTAssertNil(JSON.parse("{\"a\":1,}"))
        XCTAssertNil(JSON.parse("[1,]"))
        XCTAssertNil(JSON.parse("01"))
        XCTAssertNil(JSON.parse("1."))
        XCTAssertNil(JSON.parse("\"\t\""))
        XCTAssertNil(JSON.parse("{'a':1}"))
        XCTAssertNil(JSON.parse("nul"))
        XCTAssertNil(JSON.parse(""))
        XCTAssertEqual(JSON.parse("\"\\ud83d\\ude00\""), .string("\u{1F600}"))
        XCTAssertEqual(JSON.parse("\"\\ud800\""), .string("\u{FFFD}")) // documented difference
        XCTAssertEqual(JSON.parse("{\"a\":1,\"a\":2}"), .object(["a": .number(2)]))
    }

    func testDepthLimit() {
        XCTAssertNotNil(JSON.parse(String(repeating: "[", count: 900) + String(repeating: "]", count: 900)))
        XCTAssertNil(JSON.parse(String(repeating: "[", count: 1100) + String(repeating: "]", count: 1100)))
    }

    func testSerializeIsCanonicalAndRoundTrips() {
        let v: JSONValue = .object(["b": .number(1), "a": .array([.string("x\n\"y\u{1}"), .null, .bool(false), .number(2.5)])])
        let s = JSON.serialize(v)
        XCTAssertEqual(s, "{\"a\":[\"x\\n\\\"y\\u0001\",null,false,2.5],\"b\":1}")
        XCTAssertEqual(JSON.parse(s), v)
        XCTAssertEqual(JSON.serialize(.number(4102444800)), "4102444800")
    }
}

final class PBKDF2Tests: XCTestCase {
    // Vectors computed with Node's crypto.pbkdf2Sync (OpenSSL).
    func hex(_ b: [UInt8]) -> String { hexEncode(b) }
    let pw = Array("password".utf8), salt = Array("salt".utf8)

    func testSha256() {
        XCTAssertEqual(hex(PBKDF2.derivePortable(hash: .sha256, password: pw, salt: salt, iterations: 1, keyLength: 32)), "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b")
        XCTAssertEqual(hex(PBKDF2.derivePortable(hash: .sha256, password: pw, salt: salt, iterations: 2, keyLength: 32)), "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43")
        XCTAssertEqual(hex(PBKDF2.derivePortable(hash: .sha256, password: pw, salt: salt, iterations: 4096, keyLength: 32)), "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a")
    }
    func testSha512And384() {
        XCTAssertEqual(hex(PBKDF2.derivePortable(hash: .sha512, password: pw, salt: salt, iterations: 1, keyLength: 64)), "867f70cf1ade02cff3752599a3a53dc4af34c7a669815ae5d513554e1c8cf252c02d470a285a0501bad999bfe943c08f050235d7d68b1da55e63f73b60a57fce")
        XCTAssertEqual(hex(PBKDF2.derivePortable(hash: .sha384, password: pw, salt: salt, iterations: 3, keyLength: 48)), "1bfb451b6087e5d24eeffb57d7284448da95b581d35a1887d9a6e756af559b1fb4798a4529502f87cdf71f435d66d25e")
    }
    func testKeyLongerThanBlockAndMultipleBlocks() {
        let long = Array(String(repeating: "p", count: 200).utf8)
        XCTAssertEqual(hex(PBKDF2.derivePortable(hash: .sha256, password: long, salt: salt, iterations: 3, keyLength: 32)), "c3746a40fee94fff56265bb39a06f4217558d915d0dd91256d893330a23ba3f4")
        XCTAssertEqual(hex(PBKDF2.derivePortable(hash: .sha512, password: long, salt: salt, iterations: 3, keyLength: 32)), "0499d2f52a54272883fc04786c49cbfe54771bb53aa16a2f8d3fdad6a6ef15fa")
        // longer than one hash block: 48 bytes with SHA-256 needs two blocks; the first 32 bytes equal the 32-byte result
        let a = PBKDF2.derivePortable(hash: .sha256, password: pw, salt: salt, iterations: 2, keyLength: 48)
        XCTAssertEqual(a.count, 48)
        XCTAssertEqual(hex(Array(a.prefix(32))), "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43")
    }
    func testPlatformPathEqualsPortable() {
        XCTAssertEqual(PBKDF2.derive(hash: .sha256, password: pw, salt: salt, iterations: 100, keyLength: 32),
                       PBKDF2.derivePortable(hash: .sha256, password: pw, salt: salt, iterations: 100, keyLength: 32))
    }
}

final class AltchaBehaviourTests: XCTestCase {
    func challenge(prefix: String = "ab", expires: Double? = 4102444800) -> JSONValue {
        var p: [String: JSONValue] = ["algorithm": .string("PBKDF2/SHA-256"), "cost": .number(1), "keyLength": .number(8), "keyPrefix": .string(prefix),
                                      "nonce": .string("00112233"), "salt": .string("aabb")]
        if let e = expires { p["expiresAt"] = .number(e) }
        return .object(["parameters": .object(p)])
    }
    func testCancelAndTimeout() {
        var calls = 0
        XCTAssertThrowsError(try solveAltcha(challenge(prefix: "ffffffffffff"), isCancelled: { calls += 1; return calls > 3 })) {
            XCTAssertEqual(($0 as? AltchaError)?.reason, "aborted")
        }
        var t = 0.0
        XCTAssertThrowsError(try solveAltcha(challenge(prefix: "ffffffffffff"), timeoutSeconds: 1, now: { t += 0.4; return t })) {
            XCTAssertEqual(($0 as? AltchaError)?.reason, "timeout")
        }
    }
    func testExpiredIsRefusedBeforeWork() {
        var worked = false
        XCTAssertThrowsError(try solveAltcha(challenge(expires: 1), derive: { _, _, _, _, _ in worked = true; return [] })) {
            XCTAssertEqual(($0 as? AltchaError)?.reason, "expired")
        }
        XCTAssertFalse(worked)
    }
    func testErrorCodeMapping() {
        XCTAssertEqual(altchaErrorCode(AltchaError("too_hard")), "altcha_too_hard")
        XCTAssertEqual(altchaErrorCode(AltchaError("expired")), "altcha_expired")
        XCTAssertEqual(altchaErrorCode(AltchaError("timeout")), "altcha_failed")
        XCTAssertEqual(altchaErrorCode(AltchaError("bad_challenge")), "altcha_failed")
    }
    func testParseChallengeNeedsParameters() {
        XCTAssertNotNil(parseAltchaChallenge("{\"parameters\":{}}"))
        XCTAssertNil(parseAltchaChallenge("{\"parameters\":5}"))
        XCTAssertNil(parseAltchaChallenge("[]"))
        XCTAssertNil(parseAltchaChallenge("nope"))
    }
}

final class SignalingTests: XCTestCase {
    func testClientMessagesHaveTheProtocolShape() {
        XCTAssertEqual(JSON.parse(ClientMessage.hello(role: .receiver).encode()), .object(["type": .string("hello"), "proto_version": .number(2), "role": .string("receiver")]))
        XCTAssertEqual(JSON.parse(ClientMessage.createRoom(altcha: nil, joinMode: nil).encode()), .object(["type": .string("create_room")]))
        XCTAssertEqual(JSON.parse(ClientMessage.createRoom(altcha: "abc=", joinMode: nil).encode()), .object(["type": .string("create_room"), "altcha": .string("abc=")]))
        XCTAssertEqual(JSON.parse(ClientMessage.answer(sdp: "v=0\r\n").encode()), .object(["type": .string("answer"), "sdp": .string("v=0\r\n")]))
        XCTAssertEqual(JSON.parse(ClientMessage.iceCandidate(candidate: "{}").encode()), .object(["type": .string("ice_candidate"), "candidate": .string("{}")]))
    }
    func testRoomNameIsOptionalAndStrict() {
        // create_room: written only when present
        XCTAssertEqual(JSON.parse(ClientMessage.createRoom(altcha: nil, joinMode: .open, roomName: "Meeting room Elbe").encode()),
                       .object(["type": .string("create_room"), "join_mode": .string("open"), "room_name": .string("Meeting room Elbe")]))
        XCTAssertEqual(JSON.parse(ClientMessage.createRoom(altcha: nil, joinMode: .open, roomName: nil).encode()),
                       .object(["type": .string("create_room"), "join_mode": .string("open")]))
        // joined and join_pending: a string is kept; absent, null, blank, not a string mean "no room name" and the message stays valid
        XCTAssertEqual(ServerMessage.decode("{\"type\":\"joined\",\"room_name\":\"Wohnzimmer\"}"), .joined(roomName: "Wohnzimmer"))
        XCTAssertEqual(ServerMessage.decode("{\"type\":\"joined\"}"), .joined(roomName: nil))
        for bad in ["null", "\"\"", "\"  \\t \"", "5", "[\"x\"]", "false"] {
            XCTAssertEqual(ServerMessage.decode("{\"type\":\"joined\",\"room_name\":\(bad)}"), .joined(roomName: nil), bad)
            XCTAssertEqual(ServerMessage.decode("{\"type\":\"join_pending\",\"timeout_secs\":60,\"check\":42,\"room_name\":\(bad)}"),
                           .joinPending(timeoutSecs: 60, check: 42, roomName: nil), bad)
        }
        XCTAssertEqual(ServerMessage.decode("{\"type\":\"join_pending\",\"timeout_secs\":60,\"check\":42,\"room_name\":\"Raum 204\"}"),
                       .joinPending(timeoutSecs: 60, check: 42, roomName: "Raum 204"))
        // the rest of the message is still checked
        XCTAssertNil(ServerMessage.decode("{\"type\":\"join_pending\",\"timeout_secs\":60,\"room_name\":\"x\"}"))
    }
    func testErrorKeys() {
        for code in ["altcha_required", "altcha_invalid", "rate_limited", "banned", "room_not_found", "connection_lost", "peer_failed", "altcha_too_hard"] {
            XCTAssertEqual(errorMessageKey(code), "errors.\(code)")
        }
        XCTAssertEqual(errorMessageKey("something_new"), "errors.unknown")
        // the catalog key for every known code exists in the string catalog (checked in CatalogTests)
    }
    func testIceCandidateJSON() {
        let c = IceCandidateInit(candidate: "candidate:1 1 udp 2122260223 192.168.1.2 54321 typ host", sdpMid: "0", sdpMLineIndex: 0)
        XCTAssertEqual(IceCandidateInit.parse(c.encode()), c)
        // a native peer sends sdpMid null
        let n = IceCandidateInit.parse("{\"candidate\":\"candidate:x\",\"sdpMid\":null,\"sdpMLineIndex\":0}")
        XCTAssertEqual(n, IceCandidateInit(candidate: "candidate:x", sdpMid: nil, sdpMLineIndex: 0))
        XCTAssertNil(IceCandidateInit.parse("{}"))
        XCTAssertNil(IceCandidateInit.parse("junk"))
        XCTAssertEqual(IceCandidateInit.parse("{\"candidate\":\"c\",\"sdpMLineIndex\":99}")?.sdpMLineIndex, 0)
    }
}

final class MachineTests: XCTestCase {
    func start(_ m: inout ReceiverMachine) -> [ReceiverEffect] { m.handle(.start) }

    func testHappyPath() {
        var m = ReceiverMachine()
        XCTAssertEqual(m.handle(.start), [.cancelAltcha, .resetContent, .closePeer, .closeSocket, .openSocket, .fetchIceServers])
        XCTAssertEqual(m.state.phase, .connecting)
        XCTAssertEqual(m.handle(.serverReady), [.send(.createRoom(altcha: nil, joinMode: .open))])
        XCTAssertEqual(m.handle(.message(.roomCreated(code: "ABC234", joinMode: .open))), [])
        XCTAssertEqual(m.state.phase, .waitingSender)
        XCTAssertEqual(m.state.code, "ABC234")
        // messages before the ICE list are held
        XCTAssertEqual(m.handle(.message(.peerJoined(name: "Ana"))), [])
        XCTAssertEqual(m.handle(.message(.offer(sdp: "O"))), [])
        XCTAssertEqual(m.handle(.message(.iceCandidate(candidate: "C"))), [])
        XCTAssertEqual(m.state.phase, .waitingSender)
        XCTAssertEqual(m.handle(.iceLoaded), [.createPeer, .acceptOffer(sdp: "O"), .addIceCandidate("C")])
        XCTAssertEqual(m.state.phase, .negotiating)
        XCTAssertEqual(m.handle(.answerReady(sdp: "A")), [.send(.answer(sdp: "A"))])
        XCTAssertEqual(m.handle(.peerConnected), [])
        XCTAssertEqual(m.state.phase, .receiving)
        XCTAssertEqual(m.handle(.sasReady("123 456")), [])
        XCTAssertEqual(m.state.sas, "123 456")
        _ = m.handle(.videoArrived)
        XCTAssertTrue(m.state.hasVideo)
        XCTAssertEqual(m.handle(.message(.peerLeft)), [.closePeer, .resetContent])
        XCTAssertEqual(m.state.phase, .peerLeft)
        XCTAssertNil(m.state.sas)
        XCTAssertFalse(m.state.hasVideo)
        XCTAssertEqual(m.state.code, "ABC234") // the code stays for the next sender
        // a new sender: a new peer
        XCTAssertEqual(m.handle(.message(.peerJoined(name: "Ana"))), [.createPeer])
        XCTAssertEqual(m.state.phase, .negotiating)
    }

    func testAltchaRequiredThenSolved() {
        var m = ReceiverMachine()
        m.fetchesIce = false
        _ = m.handle(.start)
        _ = m.handle(.serverReady)
        XCTAssertEqual(m.handle(.message(.error(code: "altcha_required", message: ""))), [.solveAltcha])
        XCTAssertEqual(m.state.phase, .checking)
        XCTAssertEqual(m.handle(.altchaSolved("PAYLOAD")), [.send(.createRoom(altcha: "PAYLOAD", joinMode: .open))])
        XCTAssertEqual(m.state.phase, .connecting)
        // a second altcha_required (the payload was refused) is an error: no endless loop
        let fx = m.handle(.message(.error(code: "altcha_required", message: "")))
        XCTAssertEqual(m.state.phase, .error)
        XCTAssertEqual(m.state.errorCode, "altcha_required")
        XCTAssertTrue(fx.contains(.closeSocket))
    }

    func testAltchaFailures() {
        for code in ["altcha_too_hard", "altcha_expired", "rate_limited", "banned", "forbidden", "server_full", "altcha_failed"] {
            var m = ReceiverMachine()
            m.fetchesIce = false
            _ = m.handle(.start)
            _ = m.handle(.message(.error(code: "altcha_required", message: "")))
            let fx = m.handle(.altchaFailed(code: code))
            XCTAssertEqual(m.state.phase, .error, code)
            XCTAssertEqual(m.state.errorCode, code)
            XCTAssertTrue(fx.contains(.closeSocket))
            XCTAssertNotEqual(errorMessageKey(code), "errors.unknown", code)
        }
    }

    func testServerErrorsAreTerminal() {
        for code in ["rate_limited", "banned", "bad_hello", "proto_mismatch", "bad_message"] {
            var m = ReceiverMachine()
            m.fetchesIce = false
            _ = m.handle(.start)
            _ = m.handle(.message(.roomCreated(code: "ABC234", joinMode: .open)))
            _ = m.handle(.message(.error(code: code, message: "x")))
            XCTAssertEqual(m.state.phase, .error, code)
            XCTAssertEqual(m.state.errorCode, code)
        }
    }

    func testNotInRoomIsIgnoredAfterTheRoomExists() {
        var m = ReceiverMachine()
        m.fetchesIce = false
        _ = m.handle(.start)
        _ = m.handle(.message(.roomCreated(code: "ABC234", joinMode: .open)))
        XCTAssertEqual(m.handle(.message(.error(code: "not_in_room", message: ""))), [])
        XCTAssertEqual(m.state.phase, .waitingSender)
    }

    func testConnectionLostAndPeerFailed() {
        var m = ReceiverMachine()
        m.fetchesIce = false
        _ = m.handle(.start)
        _ = m.handle(.message(.roomCreated(code: "ABC234", joinMode: .open)))
        _ = m.handle(.socketClosed)
        XCTAssertEqual(m.state.errorCode, "connection_lost")
        XCTAssertEqual(m.handle(.socketClosed), []) // already in error
        // retry
        _ = m.handle(.start)
        XCTAssertEqual(m.state.phase, .connecting)
        XCTAssertNil(m.state.errorCode)
        _ = m.handle(.message(.roomCreated(code: "ABC234", joinMode: .open)))
        _ = m.handle(.message(.peerJoined(name: "Ana")))
        _ = m.handle(.peerFailed)
        XCTAssertEqual(m.state.errorCode, "peer_failed")
        XCTAssertNil(m.state.sas)
        _ = m.handle(.stop)
        XCTAssertEqual(m.state, ReceiverState())
        XCTAssertEqual(m.handle(.socketClosed), []) // idle: nothing
    }

    func testAnswerFailure() {
        var m = ReceiverMachine()
        m.fetchesIce = false
        _ = m.handle(.start)
        _ = m.handle(.message(.roomCreated(code: "ABC234", joinMode: .open)))
        _ = m.handle(.message(.offer(sdp: "O")))
        _ = m.handle(.answerFailed)
        XCTAssertEqual(m.state.errorCode, "peer_failed")
    }

    func testStaleSasAfterPeerLeftIsIgnored() {
        var m = ReceiverMachine()
        m.fetchesIce = false
        _ = m.handle(.start)
        _ = m.handle(.message(.roomCreated(code: "ABC234", joinMode: .open)))
        _ = m.handle(.message(.peerJoined(name: "Ana")))
        _ = m.handle(.message(.peerLeft))
        _ = m.handle(.sasReady("111 222"))
        XCTAssertNil(m.state.sas)
    }

    func testUnexpectedMessagesAreIgnored() {
        var m = ReceiverMachine()
        m.fetchesIce = false
        _ = m.handle(.start)
        for msg: ServerMessage in [.hello(protoVersion: 2), .joined(roomName: nil), .joinPending(timeoutSecs: 60, check: 42), .joined(roomName: "Room"), .joinPending(timeoutSecs: 60, check: 42, roomName: "Room"), .answer(sdp: "x"), .control(cmd: .string("stop"))] {
            XCTAssertEqual(m.handle(.message(msg)), [])
        }
        XCTAssertEqual(m.state.phase, .connecting)
        XCTAssertEqual(m.handle(.altchaSolved("late")), []) // not in checking
    }
}

final class ServerListTests: XCTestCase {
    func cand(_ a: String, _ s: RelaySource) -> ServerCandidate { ServerCandidate(address: a, source: s) }
    var srv: [ServerCandidate] { [cand("wss://s1.example:443", .dnsSrv), cand("wss://s2.example:443", .dnsSrv)] }
    var ann: [ServerCandidate] { [cand("ws://192.168.1.9:8443", .displayAnnouncement)] }
    var def: [ServerCandidate] { [cand("relay.cometcast.de:443", .defaultList)] }
    func policy(_ f: (inout ServerPolicy) -> Void) -> PolicyState {
        var p = ServerPolicy()
        p.servers = ["wss://m1.example:443", "wss://m2.example:443"]
        f(&p)
        return .active(p)
    }
    func addrs(_ m: MergedServers) -> [String] { m.servers.map(\.address) }

    func testOrderWithoutPolicy() {
        let m = mergeServers(policy: .absent, srv: srv, announced: ann, defaults: def)
        XCTAssertEqual(addrs(m), ["wss://s1.example:443", "wss://s2.example:443", "ws://192.168.1.9:8443", "relay.cometcast.de:443"])
        XCTAssertFalse(m.locked)
    }
    func testMdmFirstAndFlags() {
        var m = mergeServers(policy: policy { _ in }, srv: srv, announced: ann, defaults: def)
        XCTAssertEqual(Array(addrs(m).prefix(2)), ["wss://m1.example:443", "wss://m2.example:443"])
        XCTAssertEqual(m.servers.count, 6)
        m = mergeServers(policy: policy { $0.forbidDefaultList = true }, srv: srv, announced: ann, defaults: def)
        XCTAssertFalse(addrs(m).contains("relay.cometcast.de:443"))
        m = mergeServers(policy: policy { $0.forbidAnnouncement = true }, srv: srv, announced: ann, defaults: def)
        XCTAssertFalse(addrs(m).contains("ws://192.168.1.9:8443"))
        m = mergeServers(policy: policy { $0.forbidUserOverride = true }, srv: srv, announced: ann, defaults: def)
        XCTAssertEqual(addrs(m), ["wss://m1.example:443", "wss://m2.example:443"])
        XCTAssertTrue(m.locked)
    }
    func testInvalidPolicyFailsClosed() {
        let m = mergeServers(policy: .invalid, srv: srv, announced: ann, defaults: def)
        XCTAssertTrue(m.servers.isEmpty)
        XCTAssertTrue(m.locked)
        XCTAssertTrue(m.policyUnusable)
    }
    func testDuplicatesKeepTheBestPosition() {
        let m = mergeServers(policy: .absent, srv: [cand("WSS://A.example:443", .dnsSrv)], announced: [cand("wss://a.example:443", .displayAnnouncement)], defaults: [])
        XCTAssertEqual(m.servers.count, 1)
        XCTAssertEqual(m.servers[0].source, .dnsSrv)
    }

    func testPolicyParsing() {
        func p(_ s: String) -> ServerPolicy? { JSON.parse(s).flatMap(ServerPolicy.parse) }
        XCTAssertNotNil(p("{}"))
        XCTAssertEqual(p("{\"version\":1,\"servers\":[\"wss://relay.example.org:443\",\"10.0.0.5:8443\"],\"forbid_default_list\":true,\"mode\":\"internal\"}")?.servers,
                       ["wss://relay.example.org:443", "10.0.0.5:8443"])
        XCTAssertEqual(p("{\"forbid_user_override\":true}")?.forbidUserOverride, true)
        XCTAssertNil(p("{\"serverz\":[]}"), "unknown key: the whole policy is invalid")
        XCTAssertNil(p("{\"version\":2}"))
        XCTAssertNil(p("{\"servers\":\"x\"}"))
        XCTAssertNil(p("{\"servers\":[5]}"))
        XCTAssertNil(p("{\"servers\":[\"relay.example.org\"]}"), "a port is required without a scheme")
        XCTAssertNil(p("{\"servers\":[\"wss://relay.example.org/ws\"]}"), "no path")
        XCTAssertNil(p("{\"forbid_default_list\":\"yes\"}"))
        XCTAssertNil(p("{\"mode\":\"nope\"}"))
        XCTAssertNil(p("[]"))
        let many = (0..<17).map { "\"a\($0).example:1\"" }.joined(separator: ",")
        XCTAssertNil(p("{\"servers\":[\(many)]}"))
        XCTAssertEqual(p("{\"servers\":[\"a.example:1\",\"A.example:1\"]}")?.servers, ["a.example:1"], "duplicates are removed")
        XCTAssertNotNil(p("{\"servers\":[\"ws://127.0.0.1:8443\"]}"), "an administrator may name loopback")
    }

    func testNormalizeServer() {
        XCTAssertEqual(normalizeServer("Relay.Example.org:443", trust: .network), "relay.example.org:443")
        XCTAssertNil(normalizeServer("wss://relay.example.org:443", trust: .network), "no scheme from the network")
        XCTAssertNil(normalizeServer("relay.example.org", trust: .network), "port required")
        XCTAssertNil(normalizeServer("127.0.0.1:1", trust: .network))
        XCTAssertNil(normalizeServer("localhost:1", trust: .network))
        XCTAssertNil(normalizeServer("169.254.169.254:80", trust: .network))
        XCTAssertNil(normalizeServer("8.8.8.8:80", trust: .network), "public IP literal must be a name")
        XCTAssertEqual(normalizeServer("192.168.1.5:8443", trust: .network), "192.168.1.5:8443")
        XCTAssertEqual(normalizeServer("100.64.0.1:8443", trust: .network), "100.64.0.1:8443")
        XCTAssertNil(normalizeServer("100.100.100.200:80", trust: .network))
        XCTAssertEqual(normalizeServer("[fd12::1]:8443", trust: .network), "[fd12::1]:8443")
        XCTAssertNil(normalizeServer("[fe80::1]:8443", trust: .network))
        XCTAssertNil(normalizeServer("[::ffff:192.168.1.1]:8443", trust: .network))
        XCTAssertNil(normalizeServer("[2001:db8::1]:8443", trust: .network))
        XCTAssertNil(normalizeServer("0x7f000001:80", trust: .admin), "no spelling of an IPv4 address as a name")
        XCTAssertNil(normalizeServer("127.1:80", trust: .admin))
        XCTAssertNil(normalizeServer("2130706433:80", trust: .admin))
        XCTAssertNil(normalizeServer("0177.0.0.1:80", trust: .admin))
        XCTAssertNil(normalizeServer("a_b.example:80", trust: .admin))
        XCTAssertNil(normalizeServer("exa mple:80", trust: .admin))
        XCTAssertNil(normalizeServer("a.example:0", trust: .admin))
        XCTAssertNil(normalizeServer("a.example:65536", trust: .admin))
        XCTAssertNil(normalizeServer("user@a.example:80", trust: .admin))
        XCTAssertEqual(normalizeServer("WSS://A.example", trust: .admin), "wss://a.example")
        XCTAssertEqual(normalizeServer("ws://127.0.0.1:8443", trust: .admin), "ws://127.0.0.1:8443")
        XCTAssertNil(normalizeServer(String(repeating: "a", count: 301) + ".example:1", trust: .admin))
    }

    func testNetworkServerTlsRule() {
        XCTAssertEqual(networkServer("relay.example.org:443", tls: nil)?.address, "wss://relay.example.org:443")
        XCTAssertEqual(networkServer("192.168.1.5:8443", tls: nil)?.address, "ws://192.168.1.5:8443")
        XCTAssertEqual(networkServer("nas.local:8443", tls: nil)?.address, "ws://nas.local:8443")
        XCTAssertEqual(networkServer("192.168.1.5:8443", tls: true)?.address, "wss://192.168.1.5:8443")
        XCTAssertNil(networkServer("relay.example.org:443", tls: false), "plain ws only inside the local network")
        XCTAssertEqual(networkServer("nas.local:8443", tls: false)?.address, "ws://nas.local:8443")
    }

    func testSrvOrderingIsRFC2782() {
        let recs = [SrvRecord(priority: 20, weight: 0, port: 1, target: "c.example"),
                    SrvRecord(priority: 10, weight: 5, port: 1, target: "a.example"),
                    SrvRecord(priority: 10, weight: 0, port: 1, target: "z.example"),
                    SrvRecord(priority: 10, weight: 10, port: 1, target: "b.example")]
        // always pick the smallest number: weight 0 first, then in list order
        XCTAssertEqual(orderSrv(recs, randBelow: { _ in 0 }).map(\.target), ["z.example", "a.example", "b.example", "c.example"])
        // always pick the largest: the last heavy record first
        XCTAssertEqual(orderSrv(recs, randBelow: { $0 - 1 }).map(\.target), ["b.example", "a.example", "z.example", "c.example"])
        // priority order is never broken
        XCTAssertEqual(orderSrv(recs, randBelow: { _ in 7 }).last?.target, "c.example")
    }

    func testSrvCandidates() {
        let recs = [SrvRecord(priority: 1, weight: 1, port: 443, target: "relay.example.org."),
                    SrvRecord(priority: 2, weight: 1, port: 8443, target: "10.0.0.5"),
                    SrvRecord(priority: 3, weight: 1, port: 443, target: "localhost"),
                    SrvRecord(priority: 4, weight: 1, port: 0, target: "zero.example"),
                    SrvRecord(priority: 5, weight: 1, port: 443, target: "relay.example.org")]
        XCTAssertEqual(srvCandidates(records: recs, randBelow: { _ in 0 }).map(\.address), ["wss://relay.example.org:443", "ws://10.0.0.5:8443"])
        XCTAssertEqual(srvCandidates(records: [SrvRecord(priority: 0, weight: 0, port: 0, target: ".")], randBelow: { _ in 0 }), [])
        let many = (0..<20).map { SrvRecord(priority: UInt16($0), weight: 1, port: 443, target: "s\($0).example.org") }
        XCTAssertEqual(srvCandidates(records: many, randBelow: { _ in 0 }).count, maxSrvServers)
    }

    func testSrvRdata() {
        var rd: [UInt8] = [0, 10, 0, 5, 0x20, 0xFB]
        for l in ["relay", "example", "org"] { rd.append(UInt8(l.utf8.count)); rd += Array(l.utf8) }
        rd.append(0)
        XCTAssertEqual(parseSrvRdata(rd), SrvRecord(priority: 10, weight: 5, port: 8443, target: "relay.example.org"))
        XCTAssertEqual(parseSrvRdata([0, 0, 0, 0, 0, 0, 0]), SrvRecord(priority: 0, weight: 0, port: 0, target: "."))
        XCTAssertNil(parseSrvRdata(Array(rd.dropLast())), "unterminated name")
        XCTAssertNil(parseSrvRdata(rd + [0]), "trailing bytes")
        XCTAssertNil(parseSrvRdata([0, 0, 0, 0, 0, 0, 0xC0, 0x0C]), "compression pointer")
        XCTAssertNil(parseSrvRdata([0, 0, 0, 0, 0, 1]), "too short")
        XCTAssertNil(parseSrvRdata([0, 0, 0, 0, 0, 1, 70] + [UInt8](repeating: 97, count: 70) + [0]), "label over 63 bytes")
        var long: [UInt8] = [0, 0, 0, 0, 0, 1]
        for _ in 0..<5 { long.append(60); long += [UInt8](repeating: 97, count: 60) }
        long.append(0)
        XCTAssertNil(parseSrvRdata(long), "name over 253 bytes")
    }

    func testAnnouncement() {
        let r = announcedReceiver(instanceName: "Lobby TV", txt: [("v", "1"), ("srv", "192.168.1.9:8443"), ("name", "Lobby\u{202E} Display")])
        XCTAssertEqual(r?.announcedServer, "ws://192.168.1.9:8443")
        XCTAssertEqual(r?.name, "Lobby Display")
        XCTAssertEqual(announcedReceiver(instanceName: "x", txt: [("srv", "relay.example.org:443"), ("tls", "1")])?.announcedServer, "wss://relay.example.org:443")
        XCTAssertNil(announcedReceiver(instanceName: "x", txt: [("srv", "relay.example.org:443")])?.announcedServer, "no tls=1: plain ws for a public host is dropped")
        XCTAssertNil(announcedReceiver(instanceName: "x", txt: [("v", "2"), ("srv", "192.168.1.9:1")])?.announcedServer)
        XCTAssertNil(announcedReceiver(instanceName: "x", txt: [("srv", "wss://x.example:1")])?.announcedServer)
        XCTAssertNil(announcedReceiver(instanceName: "\u{200B}", txt: []))
        XCTAssertEqual(announcedReceiver(instanceName: "x", txt: [("name", String(repeating: "n", count: 100))])?.name.count, maxNameChars)
        XCTAssertEqual(announcedReceiver(instanceName: "x", txt: [("srv", "192.168.1.9:1"), ("srv", "10.0.0.1:2")])?.announcedServer, "ws://192.168.1.9:1", "first key wins")
    }

    func testChooseRelayNeverPrefillsAnAnnouncement() {
        let m = mergeServers(policy: .absent, srv: [], announced: ann, defaults: def)
        let c = chooseRelay(merged: m, lastUsedWsUrl: nil)
        XCTAssertEqual(c.selected?.wsUrl, "wss://relay.cometcast.de/ws")
        XCTAssertEqual(c.relays.count, 2)
        // a saved announced server is not trusted as pre-fill either
        let c2 = chooseRelay(merged: m, lastUsedWsUrl: "ws://192.168.1.9:8443/ws")
        XCTAssertEqual(c2.selected?.wsUrl, "wss://relay.cometcast.de/ws")
        // only announcements: nothing is pre-selected
        let only = chooseRelay(merged: mergeServers(policy: .absent, srv: [], announced: ann, defaults: []), lastUsedWsUrl: nil)
        XCTAssertNil(only.selected)
    }

    func testChooseRelayLastUsedAndLock() {
        let m = mergeServers(policy: .absent, srv: srv, announced: [], defaults: def)
        XCTAssertEqual(chooseRelay(merged: m, lastUsedWsUrl: "wss://s2.example/ws").selected?.wsUrl, "wss://s2.example/ws")
        let custom = Relay(wsUrl: "wss://own.example/ws", httpBase: "https://own.example", host: "own.example", insecure: false, source: .user)
        XCTAssertEqual(chooseRelay(merged: m, lastUsedWsUrl: custom.wsUrl, customConfirmed: custom).selected, custom)
        let locked = mergeServers(policy: policy { $0.forbidUserOverride = true }, srv: srv, announced: [], defaults: def)
        let lc = chooseRelay(merged: locked, lastUsedWsUrl: custom.wsUrl, customConfirmed: custom)
        XCTAssertEqual(lc.selected?.wsUrl, "wss://m1.example/ws", "a saved custom relay is ignored under a lock")
        XCTAssertTrue(lc.locked)
    }

    func testConfirmationRule() {
        let a = Relay(wsUrl: "ws://192.168.1.9:8443/ws", httpBase: "http://192.168.1.9:8443", host: "192.168.1.9:8443", insecure: true, source: .displayAnnouncement)
        XCTAssertTrue(needsRelayConfirmation(a, confirmed: []))
        XCTAssertFalse(needsRelayConfirmation(a, confirmed: [a.wsUrl]))
        var u = a; u.source = .user
        XCTAssertTrue(needsRelayConfirmation(u, confirmed: []))
        for s in [RelaySource.mdm, .dnsSrv, .defaultList] { var r = a; r.source = s; XCTAssertFalse(needsRelayConfirmation(r, confirmed: [])) }
    }
}

final class ContentControllerTests: XCTestCase {
    func testImageLinkDocumentFlow() {
        let c = ContentController()
        let img = c.showImage(name: "a.png", mime: "image/png")
        XCTAssertEqual(c.content, .image(id: img, name: "a.png", mime: "image/png"))
        c.showLink(url: "https://example.com:8443/x")
        XCTAssertEqual(c.content, .link(url: "https://example.com:8443/x", host: "example.com:8443"))
        let id = c.openDocument(name: "d.pdf")
        c.goToPage(3) // ignored while opening
        c.documentOpened(id: id, pages: 5)
        c.onCmd(.nextPage)
        c.onCmd(.nextPage)
        XCTAssertEqual(c.content, .document(id: id, name: "d.pdf", pages: 5, page: 3))
        c.onCmd(.goToPage(99))
        XCTAssertEqual(c.content, .document(id: id, name: "d.pdf", pages: 5, page: 5))
        c.onCmd(.nextPage)
        XCTAssertEqual(c.content, .document(id: id, name: "d.pdf", pages: 5, page: 5))
        c.onCmd(.goToPage(1))
        c.onCmd(.prevPage)
        XCTAssertEqual(c.content, .document(id: id, name: "d.pdf", pages: 5, page: 1))
        c.onCmd(.stop)
        XCTAssertNil(c.content)
    }

    func testStaleDocumentResultsAreIgnored() {
        let c = ContentController()
        let old = c.openDocument(name: "old.pdf")
        let new = c.openDocument(name: "new.pdf")
        c.documentOpened(id: old, pages: 9)
        XCTAssertEqual(c.content, .document(id: new, name: "new.pdf", pages: 0, page: 1))
        c.documentFailed(id: old, code: "invalid")
        XCTAssertNotNil(c.content)
        c.documentFailed(id: new, code: "encrypted")
        XCTAssertNil(c.content)
        XCTAssertEqual(c.contentError, .documentEncrypted)
    }

    func testErrorMapping() {
        XCTAssertEqual(documentErrorCode("unavailable"), .documentUnsupported)
        XCTAssertEqual(documentErrorCode("too_many_pages"), .documentTooManyPages)
        XCTAssertEqual(documentErrorCode("whatever"), .documentFailed)
        XCTAssertEqual(documentErrorCode("timeout"), .documentTimeout)
    }

    func testCommandWithoutDocumentIsIgnored() {
        let c = ContentController()
        c.onCmd(.nextPage)
        c.onCmd(.goToPage(2))
        XCTAssertNil(c.content)
    }

    func testClearReportsChange() {
        let c = ContentController()
        XCTAssertFalse(c.clear())
        c.setError(.stalled)
        XCTAssertTrue(c.clear())
    }
}

/// Pause from either side (docs/design/pause.md): the same rules as `PauseState` in crates/cc-files and the web client.
final class PauseTests: XCTestCase {
    func testEitherFlagFreezesAndEachSideClearsOnlyItsOwn() {
        var s = PauseState()
        XCTAssertFalse(s.isFrozen)
        XCTAssertTrue(s.setPresenter(true))
        XCTAssertFalse(s.setPresenter(true)) // idempotent
        XCTAssertTrue(s.setDisplay(true))
        XCTAssertTrue(s.setPresenter(false))
        XCTAssertTrue(s.isFrozen) // the display flag keeps the picture frozen
        XCTAssertTrue(s.setDisplay(false))
        XCTAssertFalse(s.isFrozen)
    }

    func testClearResetsBothAndReportsTheChange() {
        var s = PauseState(presenter: true, display: true)
        XCTAssertTrue(s.clear())
        XCTAssertEqual(s, PauseState())
        XCTAssertFalse(s.clear())
    }

    func testTheMessageIsTheWireFormat() {
        XCTAssertEqual(pauseStateMessageJSON(PauseState(presenter: true, display: false)),
                       "{\"display\":false,\"kind\":\"pause_state\",\"presenter\":true}")
        // what the display sends parses back, like the presenter's parser reads it
        XCTAssertEqual(parseControlMessage(pauseStateMessageJSON(PauseState(presenter: false, display: true))),
                       .pauseState(presenter: false, display: true))
    }

    func testPauseCommandsParseAndDoNotTouchTheContent() {
        XCTAssertEqual(parseControlMessage("{\"kind\":\"cmd\",\"cmd\":\"pause\"}"), .cmd(.pause))
        XCTAssertEqual(parseControlMessage("{\"kind\":\"cmd\",\"cmd\":\"resume\"}"), .cmd(.resume))
        XCTAssertNil(parseControlMessage("{\"kind\":\"cmd\",\"cmd\":\"Pause\"}"))
        let c = ContentController()
        let id = c.openDocument(name: "d.pdf")
        c.documentOpened(id: id, pages: 3)
        c.onCmd(.pause)
        c.onCmd(.resume)
        XCTAssertEqual(c.content, .document(id: id, name: "d.pdf", pages: 3, page: 1))
    }

    func testPauseStateNeedsTwoBooleans() {
        XCTAssertNil(parseControlMessage("{\"kind\":\"pause_state\",\"presenter\":true}"))
        XCTAssertNil(parseControlMessage("{\"kind\":\"pause_state\",\"presenter\":1,\"display\":0}"))
        XCTAssertNil(parseControlMessage("{\"kind\":\"pause_state\",\"presenter\":null,\"display\":false}"))
        XCTAssertEqual(parseControlMessage("{\"kind\":\"pause_state\",\"presenter\":false,\"display\":true,\"x\":1}"),
                       .pauseState(presenter: false, display: true))
    }

    func testTheBadgeSaysWhoPausedAndThePresenterWinsWhenBothDid() {
        XCTAssertNil(frozenBadge(PauseState()))
        XCTAssertEqual(frozenBadge(PauseState(presenter: true, display: false)), .byPresenter)
        XCTAssertEqual(frozenBadge(PauseState(presenter: false, display: true)), .frozenHere)
        XCTAssertEqual(frozenBadge(PauseState(presenter: true, display: true)), .byPresenter)
    }

    func testAReceiverIgnoresPauseStateWithoutAnError() {
        final class R: ControlReceiverHandlers {
            var events: [String] = []
            func onCmd(_ cmd: ControlCmd) { events.append("cmd:" + JSON.serialize(cmd.json)) }
            func onProgress(_ p: (name: String, received: Int, size: Int)?) {}
            func onImage(bytes: [UInt8], mime: String, name: String) {}
            func onDocument(bytes: [UInt8], name: String) {}
            func onLink(url: String) {}
            func onError(_ code: TransferErrorCode) { events.append("error:" + code.rawValue) }
        }
        let r = R()
        let rx = ControlReceiver(handlers: r)
        rx.handleText("{\"kind\":\"pause_state\",\"presenter\":true,\"display\":false}", nowMs: 0)
        rx.handleText("{\"kind\":\"cmd\",\"cmd\":\"pause\"}", nowMs: 1)
        XCTAssertEqual(r.events, ["cmd:\"pause\""])
    }
}

final class MediaTests: XCTestCase {
    func testPlayerNeverStartsByItself() {
        var s = PlayerState.card
        s = playerReduce(s, .playing)
        XCTAssertEqual(s, .card)
        s = playerReduce(s, .error(.network))
        XCTAssertEqual(s, .card)
        s = playerReduce(s, .play)
        XCTAssertEqual(s, .loading)
        s = playerReduce(s, .playing)
        XCTAssertEqual(s, .playing)
        s = playerReduce(s, .play)
        XCTAssertEqual(s, .playing)
        s = playerReduce(s, .error(.decode))
        XCTAssertEqual(s, .error(.decode))
        s = playerReduce(s, .play)
        XCTAssertEqual(s, .loading)
        s = playerReduce(s, .stop)
        XCTAssertEqual(s, .card)
    }
    func testTvOSPlayable() {
        XCTAssertTrue(isPlayableOnTvOS("https://a.example/v.mp4"))
        XCTAssertTrue(isPlayableOnTvOS("https://a.example/live.M3U8?x=1"))
        XCTAssertFalse(isPlayableOnTvOS("https://a.example/v.webm"))
        XCTAssertFalse(isPlayableOnTvOS("https://a.example/v.ogv"))
        XCTAssertFalse(isPlayableOnTvOS("https://a.example/noext"))
    }
    func testCleartextRule() {
        XCTAssertTrue(isCleartextBlocked("http://cdn.example.com/a.mp4"))
        XCTAssertFalse(isCleartextBlocked("https://cdn.example.com/a.mp4"))
        XCTAssertFalse(isCleartextBlocked("http://192.168.1.2/a.mp4"))
        XCTAssertFalse(isCleartextBlocked("http://nas.local/a.mp4"))
    }
    func testRoomCodeDisplay() {
        XCTAssertEqual(displayRoomCode("ABC234"), "ABC 234")
        XCTAssertEqual(displayRoomCode("AB"), "AB")
    }
}

final class StatsTests: XCTestCase {
    func stat(_ id: String, _ type: String, _ v: [String: JSONValue]) -> RtcStat { RtcStat(id: id, type: type, values: v) }

    func sample(local: String, remote: String) -> [RtcStat] {
        [
            stat("RTCInboundRTPVideoStream_1", "inbound-rtp", ["kind": .string("video"), "frameWidth": .number(1920), "frameHeight": .number(1080), "framesPerSecond": .number(29),
                                                              "framesDecoded": .number(900), "framesDropped": .number(3), "keyFramesDecoded": .number(4), "bytesReceived": .number(2_500_000),
                                                              "packetsLost": .number(2), "jitter": .number(0.004), "decoderImplementation": .string("VideoToolbox"), "codecId": .string("C1")]),
            stat("C1", "codec", ["mimeType": .string("video/H264")]),
            stat("P1", "candidate-pair", ["state": .string("succeeded"), "nominated": .bool(true), "localCandidateId": .string("L1"), "remoteCandidateId": .string("R1"), "currentRoundTripTime": .number(0.012)]),
            stat("P0", "candidate-pair", ["state": .string("failed"), "nominated": .bool(false)]),
            stat("L1", "local-candidate", ["candidateType": .string(local), "protocol": .string("udp")]),
            stat("R1", "remote-candidate", ["candidateType": .string(remote)]),
        ]
    }

    func testSummary() {
        let s = summarizeStats(sample(local: "host", remote: "host"))
        XCTAssertEqual(s.width, 1920); XCTAssertEqual(s.height, 1080)
        XCTAssertEqual(s.framesPerSecond, 29)
        XCTAssertEqual(s.framesDecoded, 900); XCTAssertEqual(s.framesDropped, 3); XCTAssertEqual(s.keyFramesDecoded, 4)
        XCTAssertEqual(s.codec, "video/H264"); XCTAssertEqual(s.decoder, "VideoToolbox")
        XCTAssertEqual(s.route, .host); XCTAssertEqual(s.transport, "udp")
        XCTAssertEqual(s.roundTripMs ?? 0, 12, accuracy: 0.001)
        XCTAssertEqual(s.jitterMs ?? 0, 4, accuracy: 0.001)
    }
    func testRoutes() {
        XCTAssertEqual(summarizeStats(sample(local: "srflx", remote: "host")).route, .srflx)
        XCTAssertEqual(summarizeStats(sample(local: "host", remote: "prflx")).route, .srflx)
        XCTAssertEqual(summarizeStats(sample(local: "host", remote: "relay")).route, .relay)
        XCTAssertEqual(summarizeStats(sample(local: "relay", remote: "host")).route, .relay)
        XCTAssertEqual(summarizeStats([]).route, .unknown)
    }
    func testBitrate() {
        XCTAssertEqual(bitrateKbps(previousBytes: 0, currentBytes: 125_000, seconds: 1), 1000)
        XCTAssertNil(bitrateKbps(previousBytes: 10, currentBytes: 5, seconds: 1))
        XCTAssertNil(bitrateKbps(previousBytes: 0, currentBytes: 5, seconds: 0))
    }
}
