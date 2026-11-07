// SPDX-License-Identifier: GPL-3.0-or-later
// Every message key the core can produce and every key the app's views use exists in the app's string catalogs
// (the loaded German "du" set and the Sie alternate), in English and German. The views are scanned for the keys they
// pass to `L.s` / `L.t` / `L.f` / `Text(`, because the app target cannot be compiled on Linux.

import Foundation
import XCTest
@testable import CometCastCore

final class CatalogTests: XCTestCase {
    static var appDir: URL {
        var u = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { u.deleteLastPathComponent() } // file, CometCastCoreTests, Tests, CometCastCore -> apple-tv
        return u
    }

    func catalog(_ name: String) -> [String: JSONValue] {
        let u = Self.appDir.appendingPathComponent("Sources/CometCastTV/\(name).xcstrings")
        guard let d = try? Data(contentsOf: u), let t = String(data: d, encoding: .utf8), case .object(let o)? = JSON.parse(t),
              case .object(let strings)? = o["strings"] else { XCTFail("catalog not readable at \(u.path)"); return [:] }
        return strings
    }

    func value(_ cat: [String: JSONValue], _ key: String, _ lang: String) -> String? {
        guard case .object(let e)? = cat[key], case .object(let l)? = e["localizations"], case .object(let x)? = l[lang],
              case .object(let su)? = x["stringUnit"], case .string(let v)? = su["value"], !v.isEmpty else { return nil }
        return v
    }

    /// Keys that the core's tables name.
    var coreKeys: [String] {
        var keys: [String] = []
        keys += TransferErrorCode.allCases.map { "receive.transferError.\($0.rawValue)" }
        keys += RelayErrorCode.allCases.map { "relay.errors.\($0.rawValue)" }
        keys += MediaErrorCode.allCases.map { "receive.media.error.\($0.rawValue)" }
        keys += AppearanceChoice.allCases.map(\.titleKey)
        keys += joinModeOrder.flatMap { [joinModeKeys($0).title, joinModeKeys($0).text] }
        keys += nameSuggestionKeys
        keys += [OfflineCause.network, .busy, .outdated].map(\.bodyKey)
        keys += ["setup.address.sourcePolicy", "setup.address.sourceOwn", "setup.address.sourceBox", "setup.address.sourceTyped", "setup.address.sourceDefault"]
        keys += ["receive.document.pageError.failed", "receive.document.loading", "receive.transfer"]
        return keys
    }

    func testCoreKeysExistInBothCatalogs() {
        for name in ["Localizable", "Localizable-sie"] {
            let cat = catalog(name)
            XCTAssertEqual(cat.count, 138, name)
            for k in coreKeys {
                for lang in ["en", "de"] { XCTAssertNotNil(value(cat, k, lang), "\(name): missing or empty \(lang) value: \(k)") }
            }
        }
    }

    /// `{name}` placeholders are replaced by the app (`fillPlaceholders`): they must be equal in every language and set.
    func testBracePlaceholdersAreEqual() {
        let re = try! NSRegularExpression(pattern: "\\{(\\w+)\\}")
        func phs(_ s: String) -> [String] { re.matches(in: s, range: NSRange(s.startIndex..., in: s)).map { String(s[Range($0.range(at: 1), in: s)!]) }.sorted() }
        let du = catalog("Localizable"), sie = catalog("Localizable-sie")
        XCTAssertEqual(Set(du.keys), Set(sie.keys))
        for k in du.keys {
            let en = value(du, k, "en") ?? "", de = value(du, k, "de") ?? "", s = value(sie, k, "de") ?? ""
            XCTAssertEqual(phs(en), phs(de), "placeholders differ (en/de): \(k)")
            XCTAssertEqual(phs(de), phs(s), "placeholders differ (du/Sie): \(k)")
            XCTAssertFalse(en.contains("%@") || de.contains("%@"), "use {name} placeholders: \(k)")
        }
    }

    func testPlaceholdersOfTheSpecKeys() {
        let cat = catalog("Localizable")
        func has(_ k: String, _ names: [String]) {
            let re = try! NSRegularExpression(pattern: "\\{(\\w+)\\}")
            let s = value(cat, k, "en") ?? ""
            XCTAssertEqual(re.matches(in: s, range: NSRange(s.startIndex..., in: s)).map { String(s[Range($0.range(at: 1), in: s)!]) }, names, k)
        }
        has("common.quoted", ["name"]); has("present.byline", ["name"]); has("present.sas", ["sas"]); has("request.queue", ["n"])
        has("toast.joined", ["name"]); has("receive.transfer", ["name", "percent"]); has("receive.link.title", ["host"])
        has("idle.offline.title", ["dom"]); has("idle.altAddress", ["ip"]); has("common.stepOf", ["n"]); has("about.version", ["version"])
        has("about.sourceTv", ["url"]); has("request.aria", ["name"])
    }

    /// The German du set must say du: spot checks of strings that differ between the two sets.
    func testDuIsLoadedAndSieIsTheAlternate() {
        let du = catalog("Localizable"), sie = catalog("Localizable-sie")
        XCTAssertEqual(value(du, "settings.join.approvalText", "de"), "Du lässt jede Person einzeln zu.")
        XCTAssertEqual(value(sie, "settings.join.approvalText", "de"), "Sie lassen jede Person einzeln zu.")
        XCTAssertEqual(value(du, "mode.approval", "de"), "Mit Freigabe")
        XCTAssertEqual(value(du, "request.noBlock", "de"), "Nein und sperren")
        XCTAssertEqual(value(du, "common.quoted", "de"), "„{name}“")
    }

    func appSources() -> [(String, String)] {
        let root = Self.appDir.appendingPathComponent("Sources/CometCastTV")
        var out: [(String, String)] = []
        if let en = FileManager.default.enumerator(at: root, includingPropertiesForKeys: nil) {
            for case let u as URL in en where u.pathExtension == "swift" {
                if let t = try? String(contentsOf: u, encoding: .utf8) { out.append((u.lastPathComponent, t)) }
            }
        }
        return out
    }

    func testEveryKeyUsedByTheViewsIsInTheCatalog() {
        let cat = catalog("Localizable")
        let re = try! NSRegularExpression(pattern: "(?:L\\.(?:s|t|f)|Text|Label|localized|key:)\\(?\\s*\"([a-z][A-Za-z0-9_]*(?:\\.[A-Za-z0-9_-]+)+)\"")
        var seen = Set<String>()
        for (file, text) in appSources() {
            for m in re.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
                let key = String(text[Range(m.range(at: 1), in: text)!])
                seen.insert(key)
                for lang in ["en", "de"] { XCTAssertNotNil(value(cat, key, lang), "\(file): key not in the catalog: \(key)") }
            }
        }
        XCTAssertGreaterThan(seen.count, 40, "the scan found too few keys: the app sources moved?")
    }

    func testNoUnusedOldKeysAndNoRawKeysShown() {
        let cat = catalog("Localizable")
        XCTAssertNil(cat["receiver.start"], "old pre-design key still in the catalog")
        XCTAssertNil(cat["app.name"])
    }

    func testInfoPlistCatalog() {
        let u = Self.appDir.appendingPathComponent("Sources/CometCastTV/InfoPlist.xcstrings")
        let text = (try? String(contentsOf: u, encoding: .utf8)) ?? ""
        guard case .object(let o)? = JSON.parse(text), case .object(let s)? = o["strings"] else { return XCTFail("InfoPlist.xcstrings not readable") }
        XCTAssertNotNil(value(s, "NSLocalNetworkUsageDescription", "en")); XCTAssertNotNil(value(s, "NSLocalNetworkUsageDescription", "de"))
        XCTAssertNotNil(value(s, "CFBundleDisplayName", "de"))
    }

    func testErrorKeyMappingStillWorks() {
        for k in knownErrorCodes { XCTAssertEqual(errorMessageKey(k), "errors.\(k)") }
    }
}
