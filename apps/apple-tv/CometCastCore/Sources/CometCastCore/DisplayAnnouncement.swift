// SPDX-License-Identifier: GPL-3.0-or-later
//
// What a display announces in the local network (spec 14.8 of docs/design/first-run-and-display.md,
// docs/discovery.md section 3, decision N50). A port of `txt_records` and `instance_name` of
// crates/cc-discovery/src/display.rs: the same keys, the same rules, so the presenter apps list an
// Apple TV like any other screen. Pure: the wire part (NWListener) is in the app.

import Foundation

/// Layout version of the record (`v`).
public let announcementLayoutVersion = "1"
/// Spec 14.8, point 6: well under 400 bytes, DNS-SD lengths included.
public let announcementMaxTxtBytes = 400

/// Is `id` a valid instance id (8 to 32 lower-case hex digits)?
public func isValidInstanceId(_ id: String) -> Bool {
    (8...32).contains(id.count) && id.allSatisfy { $0.isHexDigit && !$0.isUppercase }
}

/// A new stable random instance id (16 lower-case hex digits). Not the host name, not a hardware id.
public func newInstanceId() -> String {
    var generator = SystemRandomNumberGenerator()
    return String(format: "%016llx", UInt64.random(in: UInt64.min...UInt64.max, using: &generator))
}

/// The DNS-SD instance name: `CometCast-` plus the first 8 characters of the id, never a host or room name.
public func announcementInstanceName(id: String) -> String { "CometCast-" + id.prefix(8) }

/// The mode as announced: `confirm` is `approval`.
public func announcedMode(_ mode: JoinMode) -> String {
    switch mode {
    case .open: return "open"
    case .confirm: return "approval"
    case .locked: return "locked"
    }
}

/// The TXT entries of a display with a code, or nil when the state cannot be announced (bad id or code, a
/// server that is not a `ws://` or `wss://` URL, too large). While `locked` there is no code and no token.
/// `wsUrl` is the signalling URL of the relay the display uses (`wss://host:port/ws`); the announced `srv` is
/// its `host:port`, and `tls=1` is written for `wss://`. `roomName` is the name chosen in the setup wizard: it is
/// cleaned and cut like `room_name` and written as `name` (last key, like cc-discovery) unless empty; there is
/// no fallback to the device name.
public func displayAnnouncementTxt(id: String, wsUrl: String, mode: JoinMode, code: String?, roomName: String? = nil) -> [(String, String)]? {
    guard isValidInstanceId(id) else { return nil }
    let tls: Bool
    var rest: Substring
    if wsUrl.hasPrefix("wss://") { tls = true; rest = wsUrl.dropFirst(6) }
    else if wsUrl.hasPrefix("ws://") { tls = false; rest = wsUrl.dropFirst(5) }
    else { return nil }
    if let slash = rest.firstIndex(of: "/") { rest = rest[..<slash] }
    let authority = String(rest)
    guard !authority.isEmpty, !authority.contains("@") else { return nil }
    var txt: [(String, String)] = [("v", announcementLayoutVersion), ("id", id), ("srv", authority)]
    if tls { txt.append(("tls", "1")) }
    txt.append(("mode", announcedMode(mode)))
    if mode != .locked {
        guard let code, isValidRoomCode(code) else { return nil }
        txt.append(("code", code))
    }
    if let n = roomName.map(cleanRoomName), !n.isEmpty { txt.append(("name", n)) }
    let size = txt.reduce(0) { $0 + 1 + $1.0.utf8.count + 1 + $1.1.utf8.count }
    return size <= announcementMaxTxtBytes ? txt : nil
}

/// May this display announce? Not when the policy forbids announcements (or user overrides), and not when the
/// policy file is unusable (fail closed, like the policy of cc-discovery).
public func announcementsAllowed(policy: PolicyState) -> Bool {
    switch policy {
    case .absent: return true
    case .invalid: return false
    case .active(let p): return !p.forbidAnnouncement && !p.forbidUserOverride
    }
}
