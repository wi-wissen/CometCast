// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// Settings (spec 4.3): a card 600 wide over the scrim, only over Idle. Header: `settings.title` (section label)
// left; About and Close right (Close reads `settings.closeRemote`: "Close (Back key)"). Section "Joining": three tiles
// Open / Approval on / Locked and one line for the mode; section "Appearance": four tiles. Changes apply at once,
// the dialog stays open. Focus rows: [About, Close] · [Open, Approval on, Locked] · [Dark, Light, OLED black,
// Automatic]; initial focus is the tile of the current join mode. Nothing about AirPlay (decision N84).

import CometCastCore
import SwiftUI

struct SettingsView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref

    private enum Focus: Hashable { case about, close, mode(JoinMode), look(AppearanceChoice) }
    @FocusState private var focus: Focus?

    private var mode: JoinMode { model.state.joinMode }
    private var appearance: AppearanceChoice { model.config?.appearance ?? .default }
    /// The tile of the current join mode; after About, the About button (spec 4.3, 15.9).
    private var initialFocus: Focus { model.settingsFocusAbout ? .about : .mode(mode) }

    var body: some View {
        DialogCard {
            HStack {
                SectionLabel(text: L.s("settings.title"))
                Spacer(minLength: 0)
                HStack(spacing: ref(8)) {
                    DButton(kind: .close, action: { model.openAbout() }) {
                        Text(L.s("about.open")).dtype(DisplayType.closeButton)
                    }
                    .focused($focus, equals: .about)
                    DButton(kind: .close, action: { model.closeSettings() }) {
                        HStack(spacing: ref(6)) {
                            IconView(name: "close", size: 16, color: p.mut)
                            Text(L.s("settings.closeRemote")).dtype(DisplayType.closeButton)
                        }
                    }
                    .focused($focus, equals: .close)
                }
                .focusSection()
            }
            VStack(alignment: .leading, spacing: ref(14)) {
                SectionLabel(text: L.s("settings.join.label"))
                HStack(spacing: ref(Metrics.modeTileGap)) {
                    ForEach(joinModeOrder, id: \.self) { m in
                        let keys = joinModeKeys(m)
                        DButton(kind: .tile, selected: mode == m, action: { model.chooseMode(m) }) {
                            VStack(spacing: ref(3)) {
                                IconView(name: keys.icon, size: 22, color: p.fg)
                                Text(L.s(keys.title)).dtype(mode == m ? DisplayType.tileSelected : DisplayType.tile)
                            }
                            .frame(maxWidth: .infinity).frame(height: ref(Metrics.modeTileHeight))
                        }
                        .focused($focus, equals: .mode(m))
                        .accessibilityAddTraits(mode == m ? .isSelected : [])
                    }
                }
                .focusSection()
                Text(L.s(joinModeKeys(mode).text)).dtype(DisplayType.modeDescription).foregroundStyle(p.mut)
            }
            VStack(alignment: .leading, spacing: ref(14)) {
                SectionLabel(text: L.s("settings.appearance"))
                HStack(spacing: ref(Metrics.appearanceTileGap)) {
                    ForEach(AppearanceChoice.allCases, id: \.self) { a in
                        DButton(kind: .tile, selected: appearance == a, action: { model.chooseAppearance(a) }) {
                            Text(L.s(a.titleKey)).dtype(appearance == a ? DisplayType.appearanceTileSelected : DisplayType.appearanceTile)
                                .frame(maxWidth: .infinity).frame(height: ref(Metrics.appearanceTileHeight))
                        }
                        .focused($focus, equals: .look(a))
                        .accessibilityAddTraits(appearance == a ? .isSelected : [])
                    }
                }
                .focusSection()
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isModal)
        .accessibilityLabel(L.s("settings.title"))
        .defaultFocus($focus, initialFocus)
        .onAppear { focus = initialFocus }
    }
}
