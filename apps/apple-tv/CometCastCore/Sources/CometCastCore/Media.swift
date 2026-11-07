// SPDX-License-Identifier: GPL-3.0-or-later
//
// Media links: classification by the URL path extension (apps/web/src/core/media.ts) and the pure parts of
// the player state. Nothing is played until the person presses Select on "Play" (card -> loading -> playing).

import Foundation

public enum MediaKind: String, Equatable, Sendable { case video, audio, hls }

private let extensions: [String: MediaKind] = [
    "mp4": .video, "webm": .video, "ogv": .video, "m3u8": .hls,
    "mp3": .audio, "ogg": .audio, "wav": .audio, "m4a": .audio,
]

/// video, audio or hls from the path extension (query and fragment ignored, case-insensitive); nil otherwise.
/// Same table as the web client. The URL is normalised first (a relative URL or another scheme gives nil).
public func classifyMediaUrl(_ url: String) -> MediaKind? {
    guard let u = parseHTTPURL(url) else { return nil }
    let last = u.path.split(separator: "/", omittingEmptySubsequences: false).last.map(String.init) ?? ""
    guard let dot = last.lastIndex(of: ".") else { return nil }
    return extensions[String(last[last.index(after: dot)...]).lowercased()]
}

/// Whether AVPlayer on tvOS can be expected to play this extension. The web table also lists webm, ogv and ogg,
/// which AVFoundation does not play (UNCERTAIN: from Apple's documented format list, not tried on a device).
public func isPlayableOnTvOS(_ url: String) -> Bool {
    guard let u = parseHTTPURL(url) else { return false }
    let last = u.path.split(separator: "/").last.map(String.init) ?? ""
    guard let dot = last.lastIndex(of: ".") else { return false }
    return ["mp4", "m4a", "mp3", "wav", "m3u8"].contains(String(last[last.index(after: dot)...]).lowercased())
}

public enum MediaErrorCode: String, Equatable, Sendable { case aborted, network, decode, unsupported, blocked }

public enum PlayerState: Equatable, Sendable {
    case card, loading, playing
    case error(MediaErrorCode)
}

public enum PlayerEvent: Equatable, Sendable {
    case play, playing, stop
    case error(MediaErrorCode)
}

/// Pure transitions. Only a deliberate `play` leaves `card`; `stop` always returns to it (player released).
public func playerReduce(_ s: PlayerState, _ e: PlayerEvent) -> PlayerState {
    switch e {
    case .play:
        if case .card = s { return .loading }
        if case .error = s { return .loading }
        return s
    case .playing:
        if case .loading = s { return .playing }
        return s
    case .error(let c):
        if case .card = s { return s }
        return .error(c)
    case .stop:
        return .card
    }
}

public let maxShortHost = 40

/// A long host for a button or label: first 24 and last 12 characters around an ellipsis.
public func shortHost(_ host: String) -> String {
    let chars = Array(host.utf16)
    if chars.count <= maxShortHost { return host }
    return String(decoding: chars[0..<24], as: UTF16.self) + "\u{2026}" + String(decoding: chars[(chars.count - 12)...], as: UTF16.self)
}

/// Loading timeout of the player. UNCERTAIN: 20 s is the web client's guess.
public let mediaLoadTimeoutSeconds: Double = 20

/// Hosts that point into the receiver's own network or at a bare address (see the web client: name based only,
/// DNS rebinding and redirects are not detected). `hostname` is the host as in `HTTPURL.host`.
public func hostIsLocalOrIp(_ hostname: String) -> Bool {
    var h = hostname.lowercased()
    if h.hasSuffix(".") { h.removeLast() }
    if h.isEmpty { return false }
    if h.hasPrefix("[") { return true }
    let parts = h.split(separator: ".", omittingEmptySubsequences: false)
    if parts.count == 4, parts.allSatisfy({ !$0.isEmpty && $0.allSatisfy { $0.isASCII && $0.isNumber } }) { return true }
    if !h.contains(".") { return true }
    for suffix in [".local", ".localhost", ".localdomain", ".lan", ".home", ".internal", ".intranet", ".corp", ".private"] where h.hasSuffix(suffix) { return true }
    return h.hasSuffix(".home.arpa")
}

public func urlHostIsLocalOrIp(_ url: String) -> Bool {
    guard let u = parseHTTPURL(url) else { return false }
    return hostIsLocalOrIp(u.host)
}

/// tvOS counterpart of the web client's mixed-content rule: with App Transport Security a plain http: media URL
/// is blocked unless the host is local (`NSAllowsLocalNetworking`), so no Play is offered for it.
public func isCleartextBlocked(_ url: String) -> Bool {
    guard let u = parseHTTPURL(url) else { return false }
    return u.scheme == "http" && !hostIsLocalOrIp(u.host)
}
