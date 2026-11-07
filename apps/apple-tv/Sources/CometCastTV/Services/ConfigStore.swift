// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Where the app finds its configuration. Order of server sources: docs/discovery.md and docs/appletv.md.
//   * Managed app configuration (MDM): UserDefaults key "com.apple.configuration.managed", a dictionary with the
//     keys of the MDM policy of the desktop apps: servers, forbid_default_list, forbid_user_override,
//     forbid_announcement, mode, version. Unknown keys make the whole configuration invalid (fail closed), like
//     the policy file of cc-discovery. UNCERTAIN: how a real MDM product delivers it to tvOS was not tried.
//   * Build default: Info.plist key CCDefaultRelay (the counterpart of VITE_DEFAULT_RELAY of the web app).
//   * Saved in UserDefaults: the server used last, the custom server the person typed and confirmed, and the
//     servers the person confirmed.

import CometCastCore
import Foundation

enum ConfigStore {
    static let managedKey = "com.apple.configuration.managed"
    private static let lastKey = "cc.lastRelay"
    private static let customKey = "cc.customRelay"
    private static let confirmedKey = "cc.confirmedRelays"
    private static let maxConfirmed = 16

    static func policyState(_ defaults: UserDefaults = .standard) -> PolicyState {
        guard let dict = defaults.dictionary(forKey: managedKey), !dict.isEmpty else { return .absent }
        guard JSONSerialization.isValidJSONObject(dict),
              let data = try? JSONSerialization.data(withJSONObject: dict),
              let text = String(data: data, encoding: .utf8),
              let json = JSON.parse(text),
              let policy = ServerPolicy.parse(json) else { return .invalid }
        return .active(policy)
    }

    /// The build default: replaces the built-in list when it is a valid relay address.
    static func defaultCandidates(bundle: Bundle = .main) -> [ServerCandidate] {
        if let raw = bundle.object(forInfoDictionaryKey: "CCDefaultRelay") as? String, !raw.isEmpty,
           case .ok = parseRelay(raw, strictWs: false) {
            return [ServerCandidate(address: raw, source: .defaultList)]
        }
        return defaultServers.map { ServerCandidate(address: $0, source: .defaultList) }
    }

    static var lastUsedWsUrl: String? {
        get { UserDefaults.standard.string(forKey: lastKey) }
        set { UserDefaults.standard.set(newValue, forKey: lastKey) }
    }

    /// The custom relay that was typed and confirmed (one entry).
    static var customRelay: Relay? {
        get {
            guard let raw = UserDefaults.standard.string(forKey: customKey) else { return nil }
            return parseRelay(raw, strictWs: true).relay(source: .user)
        }
        set { UserDefaults.standard.set(newValue?.wsUrl, forKey: customKey) }
    }

    static var confirmed: Set<String> {
        get { Set(UserDefaults.standard.stringArray(forKey: confirmedKey) ?? []) }
        set { UserDefaults.standard.set(Array(newValue.sorted().prefix(maxConfirmed)), forKey: confirmedKey) }
    }
}
