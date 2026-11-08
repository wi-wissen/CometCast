// SPDX-License-Identifier: GPL-3.0-or-later
// The shared fixtures (tests/fixtures/*.json) against the Swift implementation.

import Foundation
import XCTest
@testable import CometCastCore

final class FixtureTests: XCTestCase {
    func testControlParse() {
        let f = Fixtures.load("control-parse.json")
        let cs = Fixtures.cases(f)
        XCTAssertGreaterThan(cs.count, 100)
        for c in cs {
            let got = parseControlMessage(Fixtures.text(c))?.json ?? .null
            XCTAssertEqual(got, Fixtures.expected(c), "control-parse: \(Fixtures.name(c))")
        }
    }

    func testLinkUrl() {
        let cs = Fixtures.cases(Fixtures.load("link-url.json"))
        XCTAssertGreaterThan(cs.count, 100)
        var overrides = 0
        for c in cs {
            if c["expect_apple_tv"] != nil { overrides += 1 }
            let got = validateLinkUrl(Fixtures.text(c)).map { JSONValue.string($0) } ?? .null
            XCTAssertEqual(got, Fixtures.expected(c), "link-url: \(Fixtures.name(c))")
        }
        print("link-url: \(overrides) documented differences from the browser")
    }

    func testMedia() {
        let f = Fixtures.load("media.json")
        for c in Fixtures.cases(f, "classify") {
            let got = classifyMediaUrl(c["url"]?.stringValue ?? "").map { JSONValue.string($0.rawValue) } ?? .null
            XCTAssertEqual(got, Fixtures.expected(c), "classify: \(c["url"]?.stringValue ?? "")")
        }
        for c in Fixtures.cases(f, "host_local_or_ip") {
            XCTAssertEqual(JSONValue.bool(hostIsLocalOrIp(c["host"]?.stringValue ?? "")), Fixtures.expected(c), "host: \(c["host"]?.stringValue ?? "")")
        }
        for c in Fixtures.cases(f, "short_host") {
            XCTAssertEqual(JSONValue.string(shortHost(c["host"]?.stringValue ?? "")), Fixtures.expected(c), "short host")
        }
    }

    func testSas() {
        let f = Fixtures.load("sas.json")
        let codes = Fixtures.cases(f, "code")
        XCTAssertGreaterThan(codes.count, 10)
        for c in codes {
            let got = connectionCode(localSdp: c["local_sdp"]?.stringValue ?? "", remoteSdp: c["remote_sdp"]?.stringValue ?? "").map { JSONValue.string($0) } ?? .null
            XCTAssertEqual(got, Fixtures.expected(c), "sas code: \(Fixtures.name(c))")
        }
        for c in Fixtures.cases(f, "fingerprint") {
            let got = sdpFingerprint(c["sdp"]?.stringValue ?? "").map { JSONValue.string($0) } ?? .null
            XCTAssertEqual(got, Fixtures.expected(c), "fingerprint: \(Fixtures.name(c))")
        }
    }

    func testAltchaPuzzles() throws {
        let f = Fixtures.load("altcha.json")
        for c in Fixtures.cases(f, "puzzles") {
            let challenge = try XCTUnwrap(c["challenge"])
            let sol = try solveAltcha(challenge, timeoutSeconds: 0)
            let exp = try XCTUnwrap(c["expect_solution"]?.objectValue)
            XCTAssertEqual(Double(sol.counter), exp["counter"]?.numberValue, "counter: \(Fixtures.name(c))")
            XCTAssertEqual(sol.derivedKey, exp["derivedKey"]?.stringValue, "derived key: \(Fixtures.name(c))")
            // payload: base64 of {challenge, solution}; the challenge comes back unchanged
            let payload = encodeAltchaPayload(challenge: challenge, solution: sol)
            let decoded = try XCTUnwrap(Data(base64Encoded: payload).flatMap { String(data: $0, encoding: .utf8) }.flatMap(JSON.parse))
            guard case .object(var o) = decoded, case .object(var s)? = o["solution"] else { return XCTFail("payload shape") }
            s["time"] = .number(0)
            o["solution"] = .object(s)
            XCTAssertEqual(JSONValue.object(o), c["expect_payload"], "payload: \(Fixtures.name(c))")
        }
    }

    func testAltchaLimits() {
        let f = Fixtures.load("altcha.json")
        for c in Fixtures.cases(f, "limits") {
            let now = c["now_seconds"]?.numberValue ?? 0
            var got = "ok"
            do { try checkChallengeLimits(c["challenge"] ?? .null, nowSeconds: now) } catch let e as AltchaError { got = e.reason } catch { got = "?" }
            XCTAssertEqual(got, c["expect"]?.stringValue, "limits: \(Fixtures.name(c))")
        }
        for c in Fixtures.cases(f, "solve_errors") {
            var got = "solved"
            do { _ = try solveAltcha(c["challenge"] ?? .null, timeoutSeconds: 0) } catch let e as AltchaError { got = e.reason } catch { got = "?" }
            XCTAssertEqual(got, c["expect"]?.stringValue, "solve error: \(Fixtures.name(c))")
        }
        for c in Fixtures.cases(f, "http") {
            let status = Int(c["status"]?.numberValue ?? 0)
            XCTAssertEqual(httpErrorCode(status: status, retryAfter: c["retry_after"]?.stringValue), c["expect"]?.stringValue, "http \(status)")
        }
    }

    func testRelay() {
        let cs = Fixtures.cases(Fixtures.load("relay.json"))
        XCTAssertGreaterThan(cs.count, 50)
        for c in cs {
            let strict = c["strict_ws"]?.boolValue ?? false
            let r = parseRelay(c["input"]?.stringValue ?? "", strictWs: strict)
            let got: JSONValue
            switch r {
            case .ok(let w, let h, let host, let insecure):
                got = .object(["ok": .bool(true), "wsUrl": .string(w), "httpBase": .string(h), "host": .string(host), "insecure": .bool(insecure)])
            case .failure(let e): got = .object(["ok": .bool(false), "error": .string(e.rawValue)])
            }
            XCTAssertEqual(got, Fixtures.expected(c), "relay: \(c["input"]?.stringValue.map { String($0.prefix(60)) } ?? "")")
        }
    }

    func testSignalingDecode() {
        let cs = Fixtures.cases(Fixtures.load("signaling.json"), "server_messages")
        XCTAssertGreaterThan(cs.count, 30)
        for c in cs {
            // protocol version 2: the Swift decoder gives the shared result, no `expect_apple_tv` deviation
            XCTAssertNil(c["expect_apple_tv"], "signaling.json must not carry a Swift deviation")
            let m = ServerMessage.decode(c["input"]?.stringValue ?? "")
            XCTAssertEqual(m.map(Self.webShape) ?? .null, c["expect"] ?? .null, "signaling: \(c["input"]?.stringValue ?? "")")
        }
    }

    static func webShape(_ m: ServerMessage) -> JSONValue {
        switch m {
        case .hello(let v): return .object(["type": .string("hello"), "proto_version": .number(v)])
        case .roomCreated(let c, let m): return .object(["type": .string("room_created"), "code": .string(c), "join_mode": .string(m.rawValue)])
        case .joined(let r):
            var o: [String: JSONValue] = ["type": .string("joined")]
            if let r { o["room_name"] = .string(r) }
            return .object(o)
        case .peerJoined(let n, _): return .object(["type": .string("peer_joined"), "name": .string(n)])
        case .peerLeft: return .object(["type": .string("peer_left")])
        case .roomEnded: return .object(["type": .string("room_ended")])
        case .joinPending(let t, let c, let r):
            var o: [String: JSONValue] = ["type": .string("join_pending"), "timeout_secs": .number(t), "check": .number(Double(c))]
            if let r { o["room_name"] = .string(r) }
            return .object(o)
        case .joinRequest(let id, let n, let c, _): return .object(["type": .string("join_request"), "id": .number(Double(id)), "name": .string(n), "check": .number(Double(c))])
        case .joinRequestCancelled(let id, let r): return .object(["type": .string("join_request_cancelled"), "id": .number(Double(id)), "reason": .string(r)])
        case .joinModeChanged(let m): return .object(["type": .string("join_mode_changed"), "join_mode": .string(m.rawValue)])
        case .blocked(let id, let n): return .object(["type": .string("blocked"), "id": .number(Double(id)), "name": .string(n)])
        case .unblocked(let id): return .object(["type": .string("unblocked"), "id": .number(Double(id))])
        case .offer(let s): return .object(["type": .string("offer"), "sdp": .string(s)])
        case .answer(let s): return .object(["type": .string("answer"), "sdp": .string(s)])
        case .iceCandidate(let c): return .object(["type": .string("ice_candidate"), "candidate": .string(c)])
        case .control(let c): return .object(["type": .string("control"), "cmd": c])
        case .error(let c, let m): return .object(["type": .string("error"), "code": .string(c), "message": .string(m)])
        }
    }

    func testIceServers() {
        for c in Fixtures.cases(Fixtures.load("ice-servers.json")) {
            let list = parseIceServers(c["input"]?.stringValue ?? "").map { s -> JSONValue in
                var o: [String: JSONValue] = ["urls": .array(s.urls.map { .string($0) })]
                if let u = s.username { o["username"] = .string(u) }
                if let k = s.credential { o["credential"] = .string(k) }
                return .object(o)
            }
            XCTAssertEqual(JSONValue.array(list), Fixtures.expected(c), "ice: \((c["input"]?.stringValue ?? "").prefix(80))")
        }
    }

    func testRoomCode() {
        for c in Fixtures.cases(Fixtures.load("room-code.json")) {
            let input = c["input"]?.stringValue ?? ""
            let n = normalizeRoomCode(input)
            XCTAssertEqual(n, c["normalized"]?.stringValue, "normalize \(input)")
            XCTAssertEqual(isValidRoomCode(input), c["valid_input"]?.boolValue, "valid \(input)")
            XCTAssertEqual(isValidRoomCode(n), c["valid_normalized"]?.boolValue, "valid normalized \(input)")
        }
    }

    // MARK: content scenarios

    final class Recorder: ControlReceiverHandlers {
        var events: [String] = []
        func onCmd(_ cmd: ControlCmd) { events.append("cmd:" + JSON.serialize(cmd.json)) }
        func onProgress(_ p: (name: String, received: Int, size: Int)?) { events.append("progress:" + (p.map { "\($0.name):\($0.received):\($0.size)" } ?? "null")) }
        func onImage(bytes: [UInt8], mime: String, name: String) { events.append("image:\(name):\(bytes.count)") }
        func onDocument(bytes: [UInt8], name: String) {
            events.append("document:\(name):\(bytes.count):\(bytes.reduce(0) { ($0 + Int($1)) % 65521 })")
        }
        func onLink(url: String) { events.append("link:" + url) }
        func onError(_ code: TransferErrorCode) { events.append("error:" + code.rawValue) }
    }

    func testContentScenarios() {
        let f = Fixtures.load("content-scenarios.json")
        let scenarios = Fixtures.cases(f, "scenarios")
        XCTAssertGreaterThan(scenarios.count, 10)
        for s in scenarios {
            let rec = Recorder()
            let rx = ControlReceiver(handlers: rec)
            for opv in s["ops"]?.arrayValue ?? [] {
                guard case .object(let op) = opv, let t = op["t"]?.numberValue else { continue }
                rx.tick(nowMs: t)
                if let text = op["text"]?.stringValue {
                    rx.handleText(text, nowMs: t)
                } else if let n = op["bin"]?.numberValue {
                    rx.handleBinary((0..<Int(n)).map { UInt8(($0 * 31 + 7) % 251) }, nowMs: t)
                }
            }
            let expected = (s["events"]?.arrayValue ?? []).compactMap { $0.stringValue }
            XCTAssertEqual(rec.events, expected, "scenario: \(Fixtures.name(s))")
        }
    }
}
