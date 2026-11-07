// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
// Translation helpers. Keys live in Localizable.xcstrings (en + de); formal address ("Sie") in German.

import Foundation

enum L {
    /// Looks the key up in the app's string catalog. Falls back to the key itself (visible in a test, never empty).
    static func s(_ key: String) -> String { NSLocalizedString(key, bundle: .main, value: key, comment: "") }
    /// With format arguments (`%@`, `%lld`).
    static func f(_ key: String, _ args: CVarArg...) -> String { String(format: s(key), arguments: args) }
}
