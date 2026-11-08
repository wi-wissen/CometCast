// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The join request dialog (spec 4.4, mode Approval on): a card 600 wide over a 55 % scrim, an alert dialog, over Idle
// or over the running presentation. Eyebrow `request.title` and, when more wait, `request.queue` ("+2 waiting"); the
// name in quotes (32/700, wraps anywhere, cut to 18 characters + "…") with `request.subtitle` below; the check number
// box on the right (OLED black: plus a 1 px `--line` border); buttons [Yes, No] and the ghost "No and block". Input is
// ignored for 1 s after the dialog appears and after it switches to the next request (the buttons stay focusable, only
// their action is ignored). Initial focus: Yes, again after every decision. Back and Menu do nothing (AppModel).

import CometCastCore
import SwiftUI

struct JoinRequestView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    let request: JoinRequest
    let more: Int
    private enum Focus: Hashable { case yes, no, block }
    @FocusState private var focus: Focus?

    private var shownName: String { L.quoted(request.name, unnamed: request.unnamed) }
    private var ariaName: String {
        guard let n = fullPersonName(request.name, unnamed: request.unnamed) else { return L.s("common.noName") }
        return L.t("common.quoted", ["name": n])
    }

    var body: some View {
        DialogCard {
            HStack {
                SectionLabel(text: L.s("request.title"))
                Spacer(minLength: 0)
                if more > 0 {
                    Text(L.t("request.queue", ["n": String(more)])).dtype(DisplayType.queue).foregroundStyle(p.mut)
                }
            }
            HStack(alignment: .center, spacing: ref(20)) {
                VStack(alignment: .leading, spacing: ref(4)) {
                    Text(shownName).dtype(DisplayType.heading).foregroundStyle(p.fg).fixedSize(horizontal: false, vertical: true)
                    Text(L.s("request.subtitle")).dtype(DisplayType.wizardBody).foregroundStyle(p.mut)
                    // Neutral information, not a warning: secondary text with an info icon (presenter-identity.md).
                    if request.otherDevice {
                        HStack(alignment: .firstTextBaseline, spacing: ref(6)) {
                            IconView(name: "info", size: 16, color: p.mut)
                            Text(L.s("request.otherDevice")).dtype(DisplayType.queue).foregroundStyle(p.mut)
                        }
                        .padding(.top, ref(2))
                    }
                }
                Spacer(minLength: 0)
                checkBox
            }
            HStack(spacing: ref(12)) {
                DButton(L.s("request.yes"), kind: .primary) { model.guarded { model.allowCurrent() } }
                    .focused($focus, equals: .yes)
                DButton(L.s("request.no"), kind: .secondary) { model.guarded { model.denyCurrent(block: false) } }
                    .focused($focus, equals: .no)
                Spacer(minLength: 0)
                DButton(kind: .ghost, action: { model.guarded { model.denyCurrent(block: true) } }) {
                    Text(L.s("request.noBlock")).dtype(DisplayType.noBlock)
                }
                .focused($focus, equals: .block)
            }
            .focusSection()
        }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(.isModal)
        .accessibilityLabel(L.t("request.aria", ["name": ariaName]))
        .defaultFocus($focus, Focus.yes)
        .onAppear { focus = .yes; announce() }
        .onChange(of: request.id) { _, _ in focus = .yes; announce() }
    }

    private func announce() { AccessibilityNotification.ScreenChanged(L.t("request.aria", ["name": ariaName])).post() }

    private var checkBox: some View {
        VStack(spacing: 0) {
            Text(L.s("request.check")).dtype(DisplayType.smallCaps).foregroundStyle(p.mut)
            Text(String(request.check)).dtype(DisplayType.checkNumber).foregroundStyle(p.fg)
        }
        .padding(.vertical, ref(3)).padding(.horizontal, ref(10))
        .frame(minWidth: ref(58))
        .background(RoundedRectangle(cornerRadius: ref(10)).fill(p.bg))
        .overlay { if p.oled { RoundedRectangle(cornerRadius: ref(10)).strokeBorder(p.line, lineWidth: ref(1)) } }
        .accessibilityElement(children: .combine)
    }
}
