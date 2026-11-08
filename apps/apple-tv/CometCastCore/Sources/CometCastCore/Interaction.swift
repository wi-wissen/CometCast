// SPDX-License-Identifier: GPL-3.0-or-later
//
// How the display reacts to the Siri Remote and to time (spec 7.1 keys table, 8 interaction rules,
// native-apps.md 5 and 6.1). Pure state with an explicit clock (seconds), so every rule is tested on Linux. The
// views translate Siri Remote events into `RemoteKey` and timer ticks into `now`.

import Foundation

/// The keys of the Siri Remote that the display uses (native-apps.md 6.1): the touch surface and ring (arrows),
/// Select (click), Menu / Back (`onExitCommand`), Play/Pause (`onPlayPauseCommand`, mapped to the spec's Menu).
/// The remote's Settings, Home, voice and app keys are not bound.
public enum RemoteKey: Equatable, Sendable {
    case up, down, left, right, select, back, menu

    public var isArrow: Bool {
        switch self { case .up, .down, .left, .right: return true; default: return false }
    }
}

/// Where the display is, for the key tables.
public enum DisplayContext: Equatable, Sendable {
    case setup(step: Int)
    case idle
    case settings
    case about
    case licences
    case joinRequest
    case linkCard
    case presenting(controlsShown: Bool)
}

/// What Back or Menu does (spec 7.1 keys table, native-apps.md 5).
public enum BackIntent: Equatable, Sendable {
    /// Nothing happens and the key is consumed (a join request needs an answer).
    case consume
    /// The system's own Back: on Apple TV on Idle, Menu leaves the app (tvOS rule).
    case system
    case openSettings
    case closeSettings
    case closeAbout
    case closeLicences
    case dismissLink
    case wizardBack
    case showControls
    case hideControls
    /// Play/Pause while a live screen is shown: toggles the display's own pause flag (docs/design/pause.md).
    case togglePause
}

/// Menu / Back (`onExitCommand`).
public func intentForBack(_ c: DisplayContext) -> BackIntent {
    switch c {
    case .setup(let step): return step > 0 ? .wizardBack : .system
    case .idle: return .system
    case .settings: return .closeSettings
    case .about: return .closeAbout
    case .licences: return .closeLicences
    case .joinRequest: return .consume
    case .linkCard: return .dismissLink
    case .presenting(let shown): return shown ? .hideControls : .showControls
    }
}

/// Play/Pause, which stands for the spec's Menu key (spec 7.1: "On Apple TV, map Play/Pause to Menu").
public func intentForMenu(_ c: DisplayContext) -> BackIntent {
    switch c {
    case .idle: return .openSettings
    case .settings: return .closeSettings
    case .presenting(let shown): return shown ? .consume : .showControls
    case .joinRequest: return .consume
    case .about, .licences, .linkCard, .setup: return .consume
    }
}

/// The Play/Pause button of the Siri Remote. While a live screen is shown it toggles the display's own pause flag
/// (docs/design/pause.md) and nothing else; in every other case it stands for the spec's Menu key as before.
public func intentForPlayPause(_ c: DisplayContext, canPause: Bool) -> BackIntent {
    if case .presenting = c, canPause { return .togglePause }
    return intentForMenu(c)
}

// MARK: presenting: name tag and End (spec 4.5, 8.1, 7.1)

/// What a key does while presenting.
public struct PresentingResponse: Equatable, Sendable {
    /// The controls are shown (or kept shown: the 5 s restart).
    public var reveal = false
    public var hide = false
    /// The focus goes to End (the only control).
    public var focusEnd = false
    public var activateEnd = false
    /// The key must not reach anything else (the first input only reveals).
    public var consumed = false
}

/// The "Presenting" columns of the keys table (spec 7.1): hidden: arrows show the controls and focus End; Select
/// shows them without activating; Back and Menu show them. Shown: arrows move focus (only End exists) and restart
/// the 5 s; Select activates End; Back hides them; Menu does nothing.
public func presentingResponse(key: RemoteKey, controlsShown: Bool) -> PresentingResponse {
    var r = PresentingResponse()
    if !controlsShown {
        r.reveal = true
        r.consumed = true
        r.focusEnd = key.isArrow
        return r
    }
    switch key {
    case .up, .down, .left, .right: r.reveal = true; r.focusEnd = true
    case .select: r.activateEnd = true
    case .back: r.hide = true
    case .menu: r.consumed = true
    }
    return r
}

/// The name tag and End: visible for 5 s after the last input, 400 ms fades (spec 3.6).
public struct ControlsReveal: Sendable {
    public private(set) var shown = false
    /// When the controls hide by themselves (nil while hidden).
    public private(set) var hideAt: Double?
    public init() {}

    /// Any input. Returns true when the controls were hidden before (the input only reveals).
    @discardableResult
    public mutating func poke(now: Double) -> Bool {
        let wasHidden = !shown
        shown = true
        hideAt = now + DisplayTiming.controlsVisibleSeconds
        return wasHidden
    }

    public mutating func hideNow() { shown = false; hideAt = nil }

    /// Call on a timer; returns true when the controls just hid.
    @discardableResult
    public mutating func tick(now: Double) -> Bool {
        if shown, let h = hideAt, now >= h { hideNow(); return true }
        return false
    }
}

// MARK: idle dimming (spec 8.6)

/// After 7 minutes on Idle without activity the whole idle screen fades to 40 % brightness (an overlay of `bg` at
/// 60 %, 2 s); any input, join request or connection restores it in 200 ms; the first key press only restores.
public struct IdleDimmer: Sendable {
    public private(set) var enabled = false
    public private(set) var dimmed = false
    private var lastActivity = 0.0
    private let after: Double

    public init(after: Double = DisplayTiming.idleDimAfterSeconds) { self.after = after }

    /// Idle (and nothing else going on) = enabled. Leaving idle restores the brightness.
    public mutating func setEnabled(_ on: Bool, now: Double) {
        guard on != enabled else { return }
        enabled = on
        if on { lastActivity = now } else { dimmed = false }
    }

    /// Any input, join, request, connection or mode change. Returns true when this restored a dimmed screen (a key
    /// press that did must not also activate something).
    @discardableResult
    public mutating func activity(now: Double) -> Bool {
        guard enabled else { return false }
        let wasDimmed = dimmed
        dimmed = false
        lastActivity = now
        return wasDimmed
    }

    /// Call on a timer; returns true when the screen just dimmed.
    @discardableResult
    public mutating func tick(now: Double) -> Bool {
        if enabled, !dimmed, now - lastActivity >= after { dimmed = true; return true }
        return false
    }

    /// When the next tick matters (for a one-shot timer); nil when nothing can change.
    public var nextDimAt: Double? { enabled && !dimmed ? lastActivity + after : nil }

    /// Overlay opacity target: 0.6 of `bg` while dimmed.
    public var overlayAlpha: Double { dimmed ? DisplayTiming.idleDimOverlayAlpha : 0 }
}

// MARK: 1 s input guard (spec 4.4)

/// Input is ignored for 1 s after a dialog appeared and after it switched to the next request. The control stays
/// focusable; only the action is ignored (native-apps.md 6.1).
public struct InputGuard: Sendable {
    private var shownAt = -Double.infinity
    public init() {}
    public mutating func arm(now: Double) { shownAt = now }
    public func isArmed(now: Double) -> Bool { now - shownAt >= DisplayTiming.inputGuardSeconds }
}

// MARK: toast (spec 4.6)

/// One line, 3.2 s; a new toast replaces the old; it never takes focus.
public struct ToastState: Equatable, Sendable {
    public private(set) var text: String?
    public private(set) var hideAt = 0.0
    /// Increases with every toast, so a view can restart its announcement for an identical text.
    public private(set) var serial = 0
    public init() {}

    public mutating func show(_ text: String, now: Double) {
        self.text = text
        hideAt = now + DisplayTiming.toastSeconds
        serial += 1
    }

    @discardableResult
    public mutating func tick(now: Double) -> Bool {
        if text != nil, now >= hideAt { text = nil; return true }
        return false
    }
}
