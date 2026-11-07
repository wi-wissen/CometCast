// SPDX-License-Identifier: GPL-3.0-or-later
// The rules of the share extension that need no UIKit (native-apps.md 21.3), so XCTest can run
// them without a sheet: the events of cc_ios_share_poll, the mapping of codes to pages, the join
// address, the trust of a server. Compiled into the extension and into RunnerTests.

import Foundation

/// One event of `cc_ios_share_poll` (the JSON shapes of `PresenterUiEvent`, docs/ios.md).
enum ShareEvent: Equatable {
    case waiting(check: Int, timeoutSecs: Int, roomName: String?)
    case joined(roomName: String?)
    case connectionCode(String)
    case page(Int, of: Int)
    case contentPrepared(name: String, kind: String, size: Int, host: String?)
    case contentRejected(reason: String)
    case progress(sent: Int, total: Int)
    case contentSent
    case presenting
    case stopped
    case ended(code: String)
    case error(code: String)
    case unknown

    /// Parses one JSON line; anything that is not one of the shapes is `.unknown`.
    static func parse(_ line: String) -> ShareEvent {
        guard let data = line.data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let name = o["event"] as? String
        else { return .unknown }
        func int(_ k: String) -> Int { (o[k] as? NSNumber)?.intValue ?? 0 }
        func str(_ k: String) -> String? {
            guard let s = o[k] as? String, !s.isEmpty else { return nil }
            return s
        }
        switch name {
        case "waiting": return .waiting(check: int("check"), timeoutSecs: int("timeout_secs"), roomName: str("room_name"))
        case "joined": return .joined(roomName: str("room_name"))
        case "connection_code": return str("sas").map(ShareEvent.connectionCode) ?? .unknown
        case "page":
            let p = int("page"), n = int("pages")
            return (p >= 1 && n >= p) ? .page(p, of: n) : .unknown
        case "content_prepared":
            return .contentPrepared(name: str("name") ?? "", kind: str("kind") ?? "", size: int("size"), host: str("host"))
        case "content_rejected": return .contentRejected(reason: str("reason") ?? "unreadable")
        case "progress": return .progress(sent: int("sent"), total: int("total"))
        case "content_sent": return .contentSent
        case "presenting": return .presenting
        case "stopped": return .stopped
        case "ended": return .ended(code: str("code") ?? "display_left")
        case "error": return .error(code: str("code") ?? "unknown")
        default: return .unknown
        }
    }
}

/// The page an error code ends on (spec 6; the same mapping as `errors.dart`). The page texts are
/// the keys `error<Name>Title/Why/Now` of the shipped set.
enum ShareErrorPage: String {
    case code, denied, timeout, locked, noInternet, displayOff, ended, busy, occupied, other

    static func forCode(_ code: String) -> ShareErrorPage {
        switch code {
        case "room_not_found": return .code
        case "join_denied": return .denied
        case "removed_by_host": return .ended
        case "approval_timeout": return .timeout
        case "room_locked", "room_locked_waiting": return .locked
        case "connection_lost": return .noInternet
        case "peer_failed", "ice_refused", "ice_failed": return .displayOff
        case "display_left": return .ended
        case "room_full": return .occupied
        case "rate_limited", "banned", "server_full": return .busy
        default: return .other
        }
    }
}

/// The join address `{dom}` of a server for the "via" line (native-apps.md 21.6): the built-in relay
/// is `cometcast.de/app`; every other server shows its host, `:443` left out.
func joinAddress(_ server: String) -> String {
    var v = server.trimmingCharacters(in: .whitespaces)
    if let r = v.range(of: "://") { v = String(v[r.upperBound...]) }
    while v.hasSuffix("/") { v.removeLast() }
    if v.lowercased() == "relay.cometcast.de:443" || v.lowercased() == "relay.cometcast.de" {
        return "cometcast.de/app"
    }
    return v.hasSuffix(":443") ? String(v.dropLast(4)) : v
}

/// Is [server] used without asking (the built-in relay, or confirmed on this device)?
/// Anything else shows the relay confirmation page first (spec 7.2; the extension has no policy or
/// DNS source of its own).
func isTrustedServer(_ server: String, confirmed: [String]) -> Bool {
    let s = joinAddress(server).lowercased()
    if s == joinAddress("relay.cometcast.de:443") { return true }
    return confirmed.contains { joinAddress($0).lowercased() == s }
}

/// A file name shortened in the middle, keeping the extension (the transfer pill rule).
func shortenedFileName(_ name: String, limit: Int = 28) -> String {
    guard name.count > limit else { return name }
    let ext = (name as NSString).pathExtension
    let dotExt = ext.isEmpty ? "" : "." + ext
    let stem = String(name.dropLast(dotExt.count))
    let room = max(limit - dotExt.count - 1, 2)
    let head = (room + 1) / 2, tail = room / 2
    return String(stem.prefix(head)) + "…" + String(stem.suffix(tail)) + dotExt
}

/// A size as "1.4 MB" for the item card's meta line.
func shareSizeText(_ bytes: Int) -> String {
    let f = ByteCountFormatter()
    f.allowedUnits = [.useKB, .useMB]
    f.countStyle = .file
    return f.string(fromByteCount: Int64(bytes))
}

/// The settings the app writes for the extension (the name, the servers confirmed on this
/// device and the last used one). The extension reads them and writes back only the name.
struct ShareSettings: Codable, Equatable {
    var name: String = ""
    var confirmedServers: [String] = []
    var lastServer: String? = nil

    enum CodingKeys: String, CodingKey {
        case name
        case confirmedServers = "confirmed_servers"
        case lastServer = "last_server"
    }
}

/// Reads and writes `share-settings.json` in the App Group container (`<container>/share/`, a
/// folder of its own: the broadcast extension deletes `broadcast/config.json` after reading it).
final class ShareSettingsStore {
    private let url: URL

    init(directory: URL) {
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        url = directory.appendingPathComponent("share-settings.json")
    }

    static func shared(group: String) -> ShareSettingsStore? {
        guard !group.isEmpty,
              let c = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
        else { return nil }
        return ShareSettingsStore(directory: c.appendingPathComponent("share", isDirectory: true))
    }

    func read() -> ShareSettings {
        guard let d = try? Data(contentsOf: url),
              let s = try? JSONDecoder().decode(ShareSettings.self, from: d)
        else { return ShareSettings() }
        return s
    }

    /// Atomic (temporary file, then rename), so a reader never sees half a file.
    func write(_ s: ShareSettings) {
        guard let d = try? JSONEncoder().encode(s) else { return }
        try? d.write(to: url, options: .atomic)
    }

    /// The extension changes the name only.
    func writeName(_ name: String) {
        var s = read()
        s.name = name
        write(s)
    }
}
