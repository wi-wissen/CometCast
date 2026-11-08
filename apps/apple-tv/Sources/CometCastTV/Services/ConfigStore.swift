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
import CryptoKit
import Foundation
import Security

enum ConfigStore {
    static let managedKey = "com.apple.configuration.managed"
    private static let lastKey = "cc.lastRelay"
    private static let customKey = "cc.customRelay"
    private static let confirmedKey = "cc.confirmedRelays"
    private static let maxConfirmed = 16
    private static let keyAccount = "cc.display.key.v2"
    private static let seqKey = "cc.display.seq"
    private static let displayKey = "cc.display.config"
    private static let deviceMemoryKey = "cc.display.deviceMemory"

    /// The Ed25519 key of this display (signed announcements, docs/design/signed-announcements.md): made on first use
    /// and kept in the Keychain; the display's id derives from it. UNCERTAIN: whether the Keychain keeps an item for
    /// this app on tvOS between launches (it is not documented to be unavailable, but was never tried on a device);
    /// when saving fails the key is kept in UserDefaults, and when that fails too the key lives for this run only (the
    /// screen then shows up as a new one; nothing breaks). Not an extractable-key rule: the raw 32 bytes are stored.
    static let displayKey: Curve25519.Signing.PrivateKey = {
        if let raw = loadKeychain() ?? UserDefaults.standard.data(forKey: keyAccount),
           let key = try? Curve25519.Signing.PrivateKey(rawRepresentation: raw) {
            return key
        }
        let key = Curve25519.Signing.PrivateKey()
        if !saveKeychain(key.rawRepresentation) {
            UserDefaults.standard.set(key.rawRepresentation, forKey: keyAccount)
        }
        return key
    }()

    /// The id of this display (derived from the key) and its short tag (`ID 3F7A`).
    static var displayId: String { displayKeyId(publicKey: displayKey.publicKey.rawRepresentation) }
    static var displayTag: String { displayKeyTag(id: displayId) }

    /// What the display remembers about names and devices (docs/design/presenter-identity.md): the text of
    /// `DeviceMemory.json`, at most 200 pairs, kept in UserDefaults like the other display settings. Empty when nothing
    /// is stored.
    static var deviceMemory: DeviceMemory {
        get { DeviceMemory(json: UserDefaults.standard.string(forKey: deviceMemoryKey)) }
        set { UserDefaults.standard.set(newValue.json, forKey: deviceMemoryKey) }
    }

    /// The highest `seq` that may have been used with the key, with a reserve (persisted by the announcer), so a clock
    /// that goes back after a restart cannot reuse values.
    static var announcementSeqFloor: UInt64 {
        get { UInt64(UserDefaults.standard.string(forKey: seqKey) ?? "") ?? 0 }
        set { UserDefaults.standard.set(String(newValue), forKey: seqKey) }
    }

    private static func keychainQuery() -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: "de.cometcast.display",
         kSecAttrAccount as String: keyAccount]
    }

    private static func loadKeychain() -> Data? {
        var query = keychainQuery()
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess else { return nil }
        return item as? Data
    }

    private static func saveKeychain(_ data: Data) -> Bool {
        var query = keychainQuery()
        query[kSecValueData as String] = data
        query[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        SecItemDelete(keychainQuery() as CFDictionary)
        return SecItemAdd(query as CFDictionary, nil) == errSecSuccess
    }

    /// Room name, default join mode, server and appearance (spec 4.1 Finish, 4.3). nil until the wizard finished.
    static var displayConfig: DisplayConfig? {
        get { DisplayConfig.decode(UserDefaults.standard.string(forKey: displayKey)) }
        set {
            if let n = newValue { UserDefaults.standard.set(n.encode(), forKey: displayKey) }
            else { UserDefaults.standard.removeObject(forKey: displayKey) }
        }
    }

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
