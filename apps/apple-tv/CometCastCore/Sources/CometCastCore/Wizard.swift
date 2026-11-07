// SPDX-License-Identifier: GPL-3.0-or-later
//
// The setup wizard of the display (spec 4.1) as a pure state machine: a port of the wizard part of
// apps/web/src/core/displaySetup.ts (steps, name chips and field, decision 13.39, mode choice) plus the address
// step of the native apps (native-apps.md 3: found addresses, the source line of decision N13, "Other address"
// cycles the found ones, then opens the field; decision 13.38 for the field). No view code, no storage.

import Foundation

public let wizardLastStep = 4
/// The five name suggestions, in order; the first is preselected (spec 4.1).
public let nameSuggestionKeys = ["setup.name.s1", "setup.name.s2", "setup.name.s3", "setup.name.s4", "setup.name.s5"]

/// One address the wizard offers: a relay and the line that says where it came from.
public struct AddressChoice: Equatable, Sendable {
    public var relay: Relay
    public init(relay: Relay) { self.relay = relay }

    /// `setup.address.source*` (spec 4.1, decision N13): an administrator's policy, a DNS entry, an in-house box
    /// found on the network, an address typed into the field, otherwise the default server.
    public var sourceKey: String {
        switch relay.source {
        case .mdm: return "setup.address.sourcePolicy"
        case .dnsSrv: return "setup.address.sourceOwn"
        case .displayAnnouncement: return "setup.address.sourceBox"
        case .user: return "setup.address.sourceTyped"
        case .defaultList: return "setup.address.sourceDefault"
        }
    }
}

public enum WizardAction: Equatable, Sendable {
    case next, back
    /// A suggestion chip was chosen: it clears the field and becomes the one to return to.
    case chip(String)
    /// The custom field changed: a non-empty text deselects all chips.
    case custom(String)
    case mode(JoinMode)
    /// "Other address".
    case changeAddress
    /// The address field's text changed.
    case fieldText(String)
    /// "Looks right" while the field is open (confirms like Enter).
    case fieldConfirm
}

public struct WizardState: Equatable, Sendable {
    public private(set) var step = 0
    /// The room name: the chosen suggestion, or the cleaned custom text, whichever is set.
    public private(set) var room: String
    /// What is typed into the custom field (as typed). It starts empty.
    public private(set) var custom = ""
    /// The suggestion selected before anything was typed; selected again when the field is emptied (13.39).
    public private(set) var chip: String
    public private(set) var mode: JoinMode = .open
    /// What can be offered, in order; the first is the pre-selection.
    public private(set) var choices: [AddressChoice]
    /// The address in use now: one of `choices`, or the one typed (source `.user`).
    public private(set) var address: AddressChoice?
    public private(set) var fieldOpen = false
    public private(set) var fieldText = ""
    public private(set) var fieldError: RelayErrorCode?
    /// The field was used or cancelled: "Other address" opens it again at once (decision 13.38).
    private var fieldUsed = false
    /// An administrator's policy locks the server: "Other address" is not offered.
    public let policyLocked: Bool

    /// `firstSuggestion`: the translated first name suggestion (preselected).
    public init(firstSuggestion: String, choices: [AddressChoice], policyLocked: Bool) {
        let name = cleanRoomName(firstSuggestion)
        room = name
        chip = name
        self.choices = choices
        address = choices.first
        self.policyLocked = policyLocked
    }

    /// Is this suggestion the selected one? Never together with a non-empty custom name.
    public func chipSelected(_ suggestion: String) -> Bool {
        cleanRoomName(custom).isEmpty && room == cleanRoomName(suggestion)
    }

    /// "Other address" is drawn: not under a policy lock, and not while the field is open.
    public var canChangeAddress: Bool { !policyLocked && !fieldOpen }

    /// The wizard finished with these values (step 4 Finish).
    public var result: (room: String, mode: JoinMode, relay: Relay)? {
        guard let a = address else { return nil }
        return (room, mode, a.relay)
    }

    /// Discovery answered late: the list is replaced. Once the person has moved on from the pre-selection (cycled or
    /// typed), their address stays; before that the pre-selection follows the better list.
    public mutating func setChoices(_ list: [AddressChoice]) {
        guard list != choices else { return }
        let touched = fieldUsed || address?.relay.source == .user || (address != nil && address != choices.first)
        choices = list
        if !touched { address = list.first }
        else if let a = address, a.relay.source != .user, !list.contains(a) { address = list.first }
    }

    public mutating func reduce(_ a: WizardAction) {
        switch a {
        case .next:
            if step == 3, fieldOpen { return } // the field has its own confirmation
            if step < wizardLastStep { step += 1 }
        case .back:
            // Back on the first screen does nothing; with the field open it closes the field (stays on step 3).
            if step == 3, fieldOpen {
                fieldOpen = false
                fieldError = nil
                fieldUsed = true
            } else if step > 0 { step -= 1 }
        case .chip(let value):
            let name = cleanRoomName(value)
            room = name; custom = ""; chip = name
        case .custom(let value):
            custom = String(value.unicodeScalars.prefix(maxRoomNameChars).map(Character.init))
            let name = cleanRoomName(custom)
            // Emptied again (or only spaces): the chip selected before typing is selected again.
            room = name.isEmpty ? chip : name
        case .mode(let m):
            if let stored = storableDefaultMode(m) { mode = stored }
        case .changeAddress:
            guard canChangeAddress else { return }
            let current = address.flatMap { a in choices.firstIndex(of: a) }
            if !fieldUsed, let i = current, i < choices.count - 1 {
                address = choices[i + 1]
            } else {
                fieldOpen = true
                fieldText = ""
                fieldError = nil
            }
        case .fieldText(let t):
            fieldText = t
            fieldError = nil
        case .fieldConfirm:
            guard fieldOpen else { return }
            switch parseRelay(fieldText, strictWs: false) {
            case .failure(let e): fieldError = e
            case .ok:
                if let r = parseRelay(fieldText).relay(source: .user) {
                    address = AddressChoice(relay: r)
                    fieldOpen = false
                    fieldError = nil
                    fieldUsed = true
                } else { fieldError = .invalid }
            }
        }
    }
}

/// The focus rows of a step (spec 7.1), as stable ids, for the tests and for the views. Chips are one row.
public enum WizardFocus: Hashable, Sendable {
    case start, chip(Int), field, back, next, modeOpen, modeApproval, changeAddress, looksRight, finish
}

/// The initial focus of a step (spec 4.1 table).
public func wizardInitialFocus(step: Int) -> WizardFocus {
    switch step {
    case 0: return .start
    case 1: return .next
    case 2: return .modeOpen // replaced by the selected card in the view
    case 3: return .looksRight
    default: return .finish
    }
}

/// The rows of a step top to bottom, left to right (spec 7.1).
public func wizardRows(step: Int, fieldOpen: Bool, canChangeAddress: Bool) -> [[WizardFocus]] {
    switch step {
    case 0: return [[.start]]
    case 1: return [[.chip(0), .chip(1), .chip(2), .chip(3), .chip(4)], [.field], [.back, .next]]
    case 2: return [[.modeOpen, .modeApproval], [.back, .next]]
    case 3:
        if fieldOpen { return [[.field], [.back, .looksRight]] }
        return [canChangeAddress ? [.back, .changeAddress, .looksRight] : [.back, .looksRight]]
    default: return [[.finish]]
    }
}
