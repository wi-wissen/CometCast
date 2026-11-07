// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The licence list of About: parsed once per app run, on first opening of the list, off the main actor, from the two
// bundled copies of THIRD_PARTY_NOTICES.md and THIRD_PARTY_LICENSES.md (Resources/Licenses, written by
// tools/gen-third-party.mjs). No network. The rules are CometCastCore's `parseLicences` (tested on the real files).

import Combine
import CometCastCore
import Foundation

@MainActor
final class LicenceStore: ObservableObject {
    @Published private(set) var entries: [LicenceEntry]?
    private var loading = false

    func loadIfNeeded() async {
        guard entries == nil, !loading else { return }
        loading = true
        let list = await Task.detached(priority: .userInitiated) { () -> [LicenceEntry] in
            func read(_ name: String) -> String {
                Bundle.main.url(forResource: name, withExtension: "md").flatMap { try? String(contentsOf: $0, encoding: .utf8) } ?? ""
            }
            return parseLicences(notices: read("THIRD_PARTY_NOTICES"), licences: read("THIRD_PARTY_LICENSES"))
        }.value
        entries = list
        loading = false
    }
}
