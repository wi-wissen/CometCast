// SPDX-License-Identifier: GPL-3.0-or-later
//
// What the display remembers (spec 4.1 "Finish stores room name, default join mode and server", 4.3 "What is
// remembered: the appearance, and Open or Approval on"). Stored as one JSON text in the app's UserDefaults; read
// leniently like `loadSetup` of apps/web/src/core/displaySetup.ts: anything unusable means "not set up" (the wizard
// shows again) or the default. Locked is never stored (spec 4.3).

import Foundation

public struct DisplayConfig: Equatable, Sendable {
    /// The cleaned room name chosen in step 1 (never empty).
    public var room: String
    /// Open or Approval on; replaced by the choice in Settings.
    public var mode: JoinMode
    /// The server chosen in step 3: its signalling URL and where it came from.
    public var wsUrl: String
    public var source: RelaySource
    public var appearance: AppearanceChoice

    public init(room: String, mode: JoinMode, wsUrl: String, source: RelaySource, appearance: AppearanceChoice = .default) {
        self.room = room
        self.mode = storableDefaultMode(mode) ?? .open
        self.wsUrl = wsUrl
        self.source = source
        self.appearance = appearance
    }

    /// The relay again (re-validated: a stored address that the relay rules refuse now means "not set up").
    public var relay: Relay? { parseRelay(wsUrl, strictWs: true).relay(source: source) }

    public func encode() -> String {
        JSON.serialize(.object([
            "room": .string(room), "mode": .string(mode.rawValue), "server": .string(wsUrl),
            "source": .string(source.rawValue), "appearance": .string(appearance.rawValue),
        ]))
    }

    /// nil: nothing stored, damaged, no usable room name or server.
    public static func decode(_ text: String?) -> DisplayConfig? {
        guard let text, case .object(let o)? = JSON.parse(text) else { return nil }
        let room = cleanRoomName(o["room"]?.stringValue ?? "")
        guard !room.isEmpty, let ws = o["server"]?.stringValue, ws.utf16.count < maxServerLen,
              parseRelay(ws, strictWs: true).relay(source: .user) != nil else { return nil }
        let mode: JoinMode = o["mode"]?.stringValue == "confirm" ? .confirm : .open
        let source = o["source"]?.stringValue.flatMap(RelaySource.init(rawValue:)) ?? .defaultList
        let appearance = o["appearance"]?.stringValue.flatMap(AppearanceChoice.init(rawValue:)) ?? .default
        return DisplayConfig(room: room, mode: mode, wsUrl: ws, source: source, appearance: appearance)
    }
}
