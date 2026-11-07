// SPDX-License-Identifier: GPL-3.0-or-later
// Every message key the core can produce exists in the app's string catalog, in English and German.

import Foundation
import XCTest
@testable import CometCastCore

final class CatalogTests: XCTestCase {
    func catalog() -> [String: JSONValue] {
        var u = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { u.deleteLastPathComponent() } // file, CometCastCoreTests, Tests, CometCastCore -> apple-tv
        u.appendPathComponent("Sources/CometCastTV/Localizable.xcstrings")
        guard let d = try? Data(contentsOf: u), let t = String(data: d, encoding: .utf8), case .object(let o)? = JSON.parse(t),
              case .object(let strings)? = o["strings"] else { XCTFail("catalog not readable at \(u.path)"); return [:] }
        return strings
    }

    func has(_ cat: [String: JSONValue], _ key: String) -> Bool {
        guard case .object(let e)? = cat[key], case .object(let l)? = e["localizations"] else { return false }
        for lang in ["en", "de"] {
            guard case .object(let x)? = l[lang], case .object(let su)? = x["stringUnit"], case .string(let v)? = su["value"], !v.isEmpty else { return false }
        }
        return true
    }

    func testAllKeysExist() {
        let cat = catalog()
        XCTAssertGreaterThan(cat.count, 100)
        var keys: [String] = ["errors.unknown"]
        keys += knownErrorCodes.map { "errors.\($0)" }
        keys += TransferErrorCode.allCases.map { "transferError.\($0.rawValue)" }
        keys += ReceiverPhase.allCases.map { "receiver.status.\($0.rawValue)" }
        keys += RelaySource.allCases.map { "relay.source.\($0.rawValue)" }
        keys += RelayErrorCode.allCases.map { "relay.errors.\($0.rawValue)" }
        keys += MediaErrorCode.allCases.map { "media.error.\($0.rawValue)" }
        for k in keys { XCTAssertTrue(has(cat, k), "missing or empty translation: \(k)") }
        for k in knownErrorCodes { XCTAssertEqual(errorMessageKey(k), "errors.\(k)") }
    }

    func testPlaceholdersAreEqualInBothLanguages() {
        let re = try! NSRegularExpression(pattern: "%(?:\\d+\\$)?(?:lld|@|%)")
        for (k, v) in catalog() {
            guard case .object(let e) = v, case .object(let l)? = e["localizations"] else { continue }
            func val(_ lang: String) -> String {
                if case .object(let x)? = l[lang], case .object(let su)? = x["stringUnit"], case .string(let s)? = su["value"] { return s }
                return ""
            }
            func phs(_ s: String) -> [String] {
                re.matches(in: s, range: NSRange(s.startIndex..., in: s)).map { String(s[Range($0.range, in: s)!]) }.sorted()
            }
            XCTAssertEqual(phs(val("en")), phs(val("de")), "placeholders differ: \(k)")
        }
    }
}
