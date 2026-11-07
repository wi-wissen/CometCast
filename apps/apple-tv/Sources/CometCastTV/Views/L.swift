// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
// Translation helpers. Keys are the dotted keys of the spec (docs/design/first-run-and-display.md 11) and live in
// Localizable.xcstrings (English and German "du", shipped). Localizable-sie.xcstrings is the formal German set: it is
// kept complete and is not loaded unless the build setting CC_STRINGS_TABLE (Info.plist key CCStringsTable) names it
// (native-apps.md 16). Placeholders are `{name}` and are replaced by `fillPlaceholders` (tested in CometCastCore).

import CometCastCore
import Foundation

enum L {
    /// The table to read: `Localizable` unless the build asks for the Sie set.
    static let table: String = {
        let t = Bundle.main.object(forInfoDictionaryKey: "CCStringsTable") as? String
        return (t?.isEmpty == false) ? t! : "Localizable"
    }()

    /// Looks the key up in the app's string catalog. Falls back to the key itself (visible in a test, never empty).
    static func s(_ key: String) -> String { NSLocalizedString(key, tableName: table, bundle: .main, value: key, comment: "") }

    /// With `{placeholder}` values (names already quoted with `quoted`).
    static func t(_ key: String, _ values: [String: String]) -> String { fillPlaceholders(s(key), values) }

    /// A person's name for a sentence: `common.quoted` around the shortened name, or `common.noName` (spec 4.4).
    static func quoted(_ raw: String, unnamed: Bool) -> String {
        guard let n = shortPersonName(raw, unnamed: unnamed) else { return s("common.noName") }
        return t("common.quoted", ["name": n])
    }
}
