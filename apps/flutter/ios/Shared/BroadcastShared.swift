// SPDX-License-Identifier: GPL-3.0-or-later
// Compiled into BOTH targets (Runner and BroadcastExtension): what the app and the extension
// exchange through the App Group container. The extension runs in its own process and the
// app is suspended while another app is in front, so everything here is a small file plus a
// Darwin notification as a wake-up (docs/ios.md, "Hand-over through the App Group").
//
// UNCERTAIN: never compiled (no Xcode here); logic that does not need Apple frameworks is
// covered by RunnerTests/BroadcastSharedTests.swift, which was never run either.

import Foundation
import Security

/// What the app hands to the extension before the person starts the broadcast.
struct BroadcastConfig: Codable, Equatable {
    var version: Int = 1
    /// Room code the receiver shows.
    var code: String
    /// Relay (signalling server) as `host:port` or a `ws(s)://` URL.
    var relay: String
    /// Unix time in seconds when the app wrote it.
    var requestedAt: Double
    /// Longest edge of the encoded picture in pixels.
    var maxDimension: Int
    /// Target frame rate.
    var fps: Int
    /// Target bit rate in bit/s.
    var bitrate: Int
    /// The person's name as the display shows it on its request (docs/design/native-apps.md
    /// 20.5). Nil or empty: the display says "Guest N".
    var name: String? = nil
    /// Single-use hand-over token that the display issued to the app (20.5, step 1): a display
    /// that holds it lets the extension in without a dialog. Nil: an ordinary join.
    var handover: String? = nil

    /// A configuration is only used for a few minutes: the person has to start the broadcast
    /// right after pressing the button in the app. An old file must never start a broadcast
    /// (that would send the screen to a room the person forgot about).
    static let maxAge: TimeInterval = 5 * 60

    func isFresh(now: Date = Date()) -> Bool {
        let age = now.timeIntervalSince1970 - requestedAt
        return age >= -5 && age <= BroadcastConfig.maxAge
    }

    /// Values outside these bounds are clamped, not rejected: the extension has a memory budget
    /// that a huge `maxDimension` would blow.
    var clamped: BroadcastConfig {
        var c = self
        c.maxDimension = min(max(maxDimension, 320), 1920)
        c.fps = min(max(fps, 5), 30)
        c.bitrate = min(max(bitrate, 200_000), 8_000_000)
        return c
    }
}

/// Where the broadcast is, written by the extension, read by the app.
enum BroadcastStateName: String, Codable {
    /// `pending`: the join waits for the host's decision (a join without a token, or with one
    /// the display did not accept); `check` and `timeoutSecs` of the status say what to show.
    case starting, connecting, pending, waiting, streaming, reconnecting, paused, ended, failed
}

/// Why it ended or failed. The app has a translated text for each (ARB keys `iosError*`), the
/// extension has its own (Localizable.strings) because it must show one itself.
enum BroadcastErrorName: String, Codable {
    case none
    case noConfig          // started without the app (Control Center) or config missing
    case configExpired     // config older than `maxAge`
    case invalidRoomCode
    case invalidRelay
    case roomNotFound
    case serverUnreachable
    case connectionLost
    case peerLeft
    case encoderFailed
    case memory            // memory budget exhausted
    case stoppedByApp
    case internalError
    case joinDenied        // the host denied the join
    case approvalTimeout   // nobody decided in time
    case roomLocked        // the room is locked (a hand-over join is refused like any join)
}

struct BroadcastStatus: Codable, Equatable {
    var seq: Int
    var state: BroadcastStateName
    var error: BroadcastErrorName
    var sent: Int
    var dropped: Int
    var bytes: Int
    var keyframes: Int
    var width: Int
    var height: Int
    /// `os_proc_available_memory()` in MB at the time of writing; -1 = unknown. For the
    /// memory soak of docs/ios-device-test-plan.md.
    var freeMemoryMB: Int
    /// Unix time in seconds; the app treats a status older than `staleAfter` as "lost".
    var updatedAt: Double
    // What the Presenting page and the fallbacks of 20.5 need. Optional so that a status
    // written without them still decodes.
    /// Check number while the join waits for the host (`pending`).
    var check: Int? = nil
    /// Seconds the host has to decide, with `check`.
    var timeoutSecs: Int? = nil
    /// The display's room name ("" or nil = none).
    var roomName: String? = nil
    /// Connection check code `123 456` of the broadcast, once connected.
    var sas: String? = nil
    /// The token of the way back (20.5, step 5): the display holds it; the app joins with it
    /// after the broadcast. Nil or empty: the app joins the ordinary way.
    var nextHandover: String? = nil

    /// The extension writes at least once per second while it runs.
    static let staleAfter: TimeInterval = 5

    func isStale(now: Date = Date()) -> Bool {
        let active: [BroadcastStateName] = [.starting, .connecting, .pending, .waiting, .streaming, .reconnecting, .paused]
        return active.contains(state) && now.timeIntervalSince1970 - updatedAt > BroadcastStatus.staleAfter
    }

    var isTerminal: Bool { state == .ended || state == .failed }
}

/// Names that come from Info.plist (set by Config/CometCast.xcconfig), so no identifier is
/// written twice.
enum BroadcastIdentity {
    static var appGroup: String {
        (Bundle.main.object(forInfoDictionaryKey: "CometCastAppGroup") as? String) ?? ""
    }
    static var extensionBundleId: String {
        (Bundle.main.object(forInfoDictionaryKey: "CometCastExtensionBundleId") as? String) ?? ""
    }
    static var statusNotification: String { appGroup + ".broadcast.status" }
    static var stopNotification: String { appGroup + ".broadcast.stop" }
}

/// Reads and writes the three files in a directory (the App Group container in production, a
/// temporary directory in tests). Writes are atomic (temporary file, then rename), so a reader
/// never sees half a JSON file.
final class BroadcastStore {
    let directory: URL
    private let fileManager: FileManager

    init(directory: URL, fileManager: FileManager = .default) {
        self.directory = directory
        self.fileManager = fileManager
        try? fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    /// The store in the App Group container; nil if the entitlement is missing (the app then
    /// says so: a build or signing problem, not a user problem).
    static func shared(group: String = BroadcastIdentity.appGroup) -> BroadcastStore? {
        guard !group.isEmpty,
              let url = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)
        else { return nil }
        return BroadcastStore(directory: url.appendingPathComponent("broadcast", isDirectory: true))
    }

    private var configURL: URL { directory.appendingPathComponent("config.json") }
    private var statusURL: URL { directory.appendingPathComponent("status.json") }
    private var stopURL: URL { directory.appendingPathComponent("stop") }

    /// Directory for `cc-ios.log` (the container itself, next to the three files).
    var logDirectory: URL { directory }

    func writeConfig(_ c: BroadcastConfig) throws { try write(c, to: configURL) }
    func readConfig() -> BroadcastConfig? { read(BroadcastConfig.self, from: configURL) }
    func deleteConfig() { try? fileManager.removeItem(at: configURL) }

    func writeStatus(_ s: BroadcastStatus) throws { try write(s, to: statusURL) }
    func readStatus() -> BroadcastStatus? { read(BroadcastStatus.self, from: statusURL) }
    func deleteStatus() { try? fileManager.removeItem(at: statusURL) }

    func requestStop() { fileManager.createFile(atPath: stopURL.path, contents: Data()) }
    func stopRequested() -> Bool { fileManager.fileExists(atPath: stopURL.path) }
    func clearStopRequest() { try? fileManager.removeItem(at: stopURL) }

    private func write<T: Encodable>(_ value: T, to url: URL) throws {
        let data = try JSONEncoder().encode(value)
        try data.write(to: url, options: .atomic)
    }

    private func read<T: Decodable>(_ type: T.Type, from url: URL) -> T? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(type, from: data)
    }
}

/// Darwin notifications cross process boundaries without a payload; the receiver then reads
/// the file. Delivery is not guaranteed while the app is suspended, so the app also reads the
/// status file when it becomes active and on a timer while it is in front.
enum DarwinNotifier {
    static func post(_ name: String) {
        CFNotificationCenterPostNotification(
            CFNotificationCenterGetDarwinNotifyCenter(),
            CFNotificationName(name as CFString), nil, nil, true)
    }

    /// Calls `handler` on the main queue for every notification `name`. Returns a token; keep
    /// it as long as the observation should last.
    static func observe(_ name: String, handler: @escaping () -> Void) -> DarwinObservation {
        DarwinObservation(name: name, handler: handler)
    }
}

final class DarwinObservation {
    private let name: String
    fileprivate let handler: () -> Void

    fileprivate init(name: String, handler: @escaping () -> Void) {
        self.name = name
        self.handler = handler
        let observer = Unmanaged.passUnretained(self).toOpaque()
        CFNotificationCenterAddObserver(
            CFNotificationCenterGetDarwinNotifyCenter(), observer,
            { _, observer, _, _, _ in
                guard let observer else { return }
                let me = Unmanaged<DarwinObservation>.fromOpaque(observer).takeUnretainedValue()
                DispatchQueue.main.async { me.handler() }
            },
            name as CFString, nil, .deliverImmediately)
    }

    deinit {
        let observer = Unmanaged.passUnretained(self).toOpaque()
        CFNotificationCenterRemoveObserver(
            CFNotificationCenterGetDarwinNotifyCenter(), observer, CFNotificationName(name as CFString), nil)
    }
}

/// A hand-over token: 128 random bits as base64url without padding (22 characters of
/// `A-Z a-z 0-9 - _`), the form the display accepts (docs/design/native-apps.md 20.5).
enum HandoverToken {
    static func make(bytes: [UInt8]? = nil) -> String {
        var b = bytes ?? [UInt8](repeating: 0, count: 16)
        if bytes == nil {
            let status = SecRandomCopyBytes(kSecRandomDefault, b.count, &b)
            // No secure randomness: no token (the way back then joins the ordinary way).
            if status != errSecSuccess { return "" }
        }
        return Data(b).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    /// The display's rule: 1 to 64 characters of the base64url alphabet.
    static func isValid(_ t: String) -> Bool {
        !t.isEmpty && t.utf8.count <= 64
            && t.utf8.allSatisfy { ($0 >= 0x30 && $0 <= 0x39) || ($0 >= 0x41 && $0 <= 0x5a) || ($0 >= 0x61 && $0 <= 0x7a) || $0 == 0x2d || $0 == 0x5f }
    }
}
