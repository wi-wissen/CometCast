// SPDX-License-Identifier: GPL-3.0-or-later
// Loads the shared JSON fixtures in <repository>/tests/fixtures (also checked against the web implementation by
// apps/web/tests/fixtures.test.ts). The path is found from this file's location (#filePath), which works for
// `swift test` and, UNCERTAIN, for an Xcode test run in the simulator (the simulator can read the host file system;
// a test run on a real device could not).

import Foundation
import XCTest
@testable import CometCastCore

enum Fixtures {
    static let dir: URL = {
        var u = URL(fileURLWithPath: #filePath)
        for _ in 0..<6 { u.deleteLastPathComponent() } // file, CometCastCoreTests, Tests, CometCastCore, apple-tv, apps
        return u.appendingPathComponent("tests/fixtures")
    }()

    static func load(_ name: String, file: StaticString = #filePath, line: UInt = #line) -> [String: JSONValue] {
        let url = dir.appendingPathComponent(name)
        guard let data = try? Data(contentsOf: url), let text = String(data: data, encoding: .utf8),
              case .object(let o)? = JSON.parse(text) else {
            XCTFail("cannot read fixture \(url.path)", file: file, line: line)
            return [:]
        }
        return o
    }

    /// `input` or `input_parts` of a case.
    static func text(_ c: [String: JSONValue]) -> String {
        if let s = c["input"]?.stringValue { return s }
        var out = ""
        for p in c["input_parts"]?.arrayValue ?? [] {
            switch p {
            case .string(let s): out += s
            case .object(let o): out += String(repeating: o["repeat"]?.stringValue ?? "", count: Int(o["count"]?.numberValue ?? 0))
            default: break
            }
        }
        return out
    }

    static func cases(_ f: [String: JSONValue], _ key: String = "cases") -> [[String: JSONValue]] {
        (f[key]?.arrayValue ?? []).compactMap { $0.objectValue }
    }

    /// `expect_apple_tv` when present (even null), else `expect`.
    static func expected(_ c: [String: JSONValue]) -> JSONValue { c["expect_apple_tv"] ?? c["expect"] ?? .null }

    static func name(_ c: [String: JSONValue]) -> String { c["name"]?.stringValue ?? c["input"]?.stringValue ?? "?" }
}
