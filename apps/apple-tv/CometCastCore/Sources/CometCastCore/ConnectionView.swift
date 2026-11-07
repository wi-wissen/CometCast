// SPDX-License-Identifier: GPL-3.0-or-later
//
// What the idle screen shows about the connection and when the display tries again (spec 4.2, decisions 13.30,
// 13.44, 13.59). A port of apps/web/src/core/connectionView.ts, busyMemory.ts and the watch in
// components/display/DisplayRunning.vue, with the clock passed in (seconds, any monotonic origin), so the whole
// schedule is tested without timers.
//
//   online      a code is there
//   connecting  no code yet: the code line is empty (it keeps its height), the QR box is empty, no caption
//   offline     the box "No connection to {dom}"; the QR box stays empty
//
// After a loss the display tries again at 1 s, 2 s and 3 s, then every 4 s. The box appears when the attempt at
// 3 s has failed, so a drop shorter than 3 s never shows it. Busy refusals show the box at once and retry after
// 30 s. `proto_mismatch` and `bad_hello` show the box at once and never retry (the app must be updated).

import Foundation

public enum ConnectionView: Equatable, Sendable { case online, connecting, offline }

/// Which body text the offline box shows (spec 4.2).
public enum OfflineCause: Equatable, Sendable {
    case network, busy, outdated

    /// `idle.offline.body`, `.bodyBusy`, `.bodyOutdatedApp` (the apps use the App variant, decision N14).
    public var bodyKey: String {
        switch self {
        case .network: return "idle.offline.body"
        case .busy: return "idle.offline.bodyBusy"
        case .outdated: return "idle.offline.bodyOutdatedApp"
        }
    }
}

/// Retries after a loss, seconds since the loss: 1 s, 2 s, 3 s; then every `retryEvery`.
public let retryAt: [Double] = [1, 2, 3]
public let retryEvery = 4.0
/// The box shows once this many attempts failed in a row since the last code: the failure itself plus the retries.
public let failedAttemptsForBox = retryAt.count + 1
/// Retry after a busy refusal.
public let busyRetrySeconds = 30.0
/// A refusal is remembered this long: a closed connection within it is the same refusal (decision 13.44).
public let busyMemorySeconds = 60.0
public let busyCodes = ["rate_limited", "banned", "server_full"]
private let busyCauseCodes = ["rate_limited", "banned", "server_full", "forbidden"]
private let outdatedCodes = ["proto_mismatch", "bad_hello"]

/// The cause of the last error (`forbidden` reads as busy; anything else, nil included, is the network).
public func offlineCause(_ code: String?) -> OfflineCause {
    guard let code else { return .network }
    if busyCauseCodes.contains(code) { return .busy }
    if outdatedCodes.contains(code) { return .outdated }
    return .network
}

public struct ConnectionInput: Equatable, Sendable {
    public var code: String?
    public var phase: ReceiverPhase
    /// An error happened since the display last had a code (a retry that is running does not clear it).
    public var failed: Bool
    public var failedAttempts: Int
    public var cause: OfflineCause
    /// Seconds since the display had no code.
    public var withoutCode: Double

    public init(code: String?, phase: ReceiverPhase, failed: Bool, failedAttempts: Int, cause: OfflineCause, withoutCode: Double) {
        self.code = code; self.phase = phase; self.failed = failed; self.failedAttempts = failedAttempts
        self.cause = cause; self.withoutCode = withoutCode
    }
}

public func connectionView(_ i: ConnectionInput) -> ConnectionView {
    if i.code != nil, i.phase != .error { return .online }
    if i.phase == .error || i.failed {
        if i.cause != .network { return .offline }
        return i.failedAttempts >= failedAttemptsForBox ? .offline : .connecting
    }
    return i.withoutCode >= DisplayTiming.connectingToOfflineSeconds ? .offline : .connecting
}

/// Seconds until the next attempt after an error, or nil when the app has to be updated. `failedAttempts` counts
/// the failures in a row (the first one is the loss itself); `sinceLoss` is the time since the loss. The retries
/// come at 1, 2 and 3 s after it, then every 4 s (7 s, 11 s ...), measured from the loss, so a retry that took
/// a while to fail does not push the next one back.
public func retryDelay(code: String?, failedAttempts: Int = 1, sinceLoss: Double = 0) -> Double? {
    switch offlineCause(code) {
    case .outdated: return nil
    case .busy: return busyRetrySeconds
    case .network:
        let n = max(0, failedAttempts - 1)
        let at = n < retryAt.count ? retryAt[n] : retryAt[retryAt.count - 1] + Double(n - retryAt.count + 1) * retryEvery
        return max(0, at - sinceLoss)
    }
}

/// Remembers a busy refusal: a closed connection right after it counts as the same refusal.
public struct BusyMemory: Sendable {
    private var code: String?
    private var until = 0.0
    public init() {}

    /// Takes the code of a failure and returns the code to report.
    public mutating func apply(_ failure: String, now: Double) -> String {
        if busyCodes.contains(failure) {
            code = failure
            until = now + busyMemorySeconds
            return failure
        }
        if failure == "connection_lost", let c = code, now < until { return c }
        return failure
    }

    public mutating func reset() { code = nil; until = 0 }
}

public enum RetryDirective: Equatable, Sendable {
    /// Nothing changed since the last call: leave a running timer alone.
    case unchanged
    /// Cancel a running retry timer (the display is not in an error).
    case cancel
    /// Start the next attempt after this many seconds, replacing a running timer.
    case after(Double)
    /// An error that no retry helps (the app is out of date).
    case never
}

/// The bookkeeping of the watch in DisplayRunning.vue: since when there is no code, how many attempts failed,
/// what the last error was.
public struct ConnectionTracker: Sendable {
    public private(set) var failed = false
    public private(set) var failedAttempts = 0
    public private(set) var lastError: String?
    public private(set) var lostAt: Double
    private var memory = BusyMemory()
    private var previous: (hadCode: Bool, phase: ReceiverPhase, code: String?)?

    public init(now: Double) { lostAt = now }

    private static func hasCode(_ code: String?, _ phase: ReceiverPhase) -> Bool { code != nil && phase != .error }

    /// Call after every change of the receiver state. Returns what to do with the retry timer.
    public mutating func update(code: String?, phase: ReceiverPhase, errorCode: String?, now: Double) -> RetryDirective {
        if let p = previous, p.code == code, p.phase == phase { return .unchanged }
        let had = Self.hasCode(code, phase)
        if had {
            failed = false
            failedAttempts = 0
            lastError = nil
            memory.reset()
        } else if let p = previous, p.hadCode {
            lostAt = now
        }
        if phase == .error, previous?.phase != .error {
            failed = true
            failedAttempts += 1
            lastError = memory.apply(errorCode ?? "unknown", now: now)
        }
        previous = (had, phase, code)
        guard phase == .error else { return .cancel }
        guard let d = retryDelay(code: lastError, failedAttempts: failedAttempts, sinceLoss: now - lostAt) else { return .never }
        return .after(d)
    }

    /// What the idle screen shows now.
    public func view(code: String?, phase: ReceiverPhase, now: Double) -> ConnectionView {
        connectionView(ConnectionInput(code: code, phase: phase, failed: failed, failedAttempts: failedAttempts,
                                       cause: offlineCause(lastError), withoutCode: now - lostAt))
    }

    public var cause: OfflineCause { offlineCause(lastError) }
}
