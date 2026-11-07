// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The setup wizard (spec 4.1): a card 780 wide on `--bg`. Step 0 is centred vertically; steps 1 to 4 keep the card's
// top edge at 40 so the logo and the step dots do not jump. Top row: logo 40 and, from step 1, four dots (done =
// `--acc`, todo = `--line`) with "Step n of 4". All state and rules are in CometCastCore (`WizardState`); this file
// only draws it and moves the focus. Text fields use the full-screen tvOS keyboard on Select; when it closes with
// Done the focus moves to the row's primary button (native-apps.md 8).

import CometCastCore
import SwiftUI

struct SetupView: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    let wizard: WizardState
    @FocusState private var focus: WizardFocus?

    var body: some View {
        VStack(spacing: 0) {
            if wizard.step == 0 { Spacer(minLength: 0) } else { Spacer().frame(height: ref(Metrics.wizardCardTop)) }
            card
            Spacer(minLength: 0)
        }
        .frame(width: ref(canvasWidth), height: ref(canvasHeight))
        .defaultFocus($focus, initialFocus)
        .onAppear { focus = initialFocus }
        .onChange(of: wizard.step) { _, _ in focus = initialFocus }
        .onChange(of: wizard.fieldOpen) { _, open in focus = open ? .field : (wizard.step == 3 ? .changeAddress : initialFocus) }
    }

    private var initialFocus: WizardFocus {
        if wizard.step == 2 { return wizard.mode == .confirm ? .modeApproval : .modeOpen }
        if wizard.step == 3, wizard.fieldOpen { return .field }
        return wizardInitialFocus(step: wizard.step)
    }

    private func act(_ a: WizardAction) { model.wizardAct(a) }

    // MARK: card

    private var card: some View {
        DialogCard(width: Metrics.wizardWidth, shadow: false) {
            HStack {
                LogoView(size: Metrics.logoWizard)
                Spacer(minLength: 0)
                if wizard.step > 0 { dots }
            }
            switch wizard.step {
            case 0: welcome
            case 1: name
            case 2: mode
            case 3: address
            default: done
            }
        }
    }

    private var dots: some View {
        HStack(spacing: ref(8)) {
            ForEach(1...4, id: \.self) { n in
                Circle().fill(n <= wizard.step ? p.acc : p.line).frame(width: ref(10), height: ref(10))
            }
            Text("\u{A0}" + L.t("common.stepOf", ["n": String(wizard.step)])).dtype(DisplayType.dots).foregroundStyle(p.mut)
        }
        .accessibilityElement(children: .combine)
    }

    private func heading(_ key: String) -> some View {
        Text(L.s(key)).dtype(DisplayType.heading).foregroundStyle(p.fg).fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
    }
    private func bodyText(_ key: String) -> some View {
        Text(L.s(key)).dtype(DisplayType.wizardBody).foregroundStyle(p.mut).fixedSize(horizontal: false, vertical: true)
    }
    private func actions<C: View>(@ViewBuilder _ content: () -> C) -> some View {
        HStack(spacing: ref(12)) { Spacer(minLength: 0); content() }.focusSection()
    }

    // MARK: steps

    private var welcome: some View {
        Group {
            heading("setup.welcome.title")
            bodyText("setup.welcome.body")
            actions { DButton(L.s("setup.welcome.start"), kind: .primary) { act(.next) }.focused($focus, equals: .start) }
        }
    }

    private var name: some View {
        Group {
            heading("setup.name.title")
            bodyText("setup.name.body")
            FlowLayout(spacing: ref(10)) {
                ForEach(Array(nameSuggestionKeys.enumerated()), id: \.offset) { i, key in
                    let text = L.s(key)
                    let on = wizard.chipSelected(text)
                    DButton(kind: .chip, selected: on, action: { act(.chip(text)) }) {
                        Text(text).dtype(on ? DisplayType.chipSelected : DisplayType.chip)
                    }
                    .focused($focus, equals: .chip(i))
                    .accessibilityAddTraits(on ? .isSelected : [])
                }
            }
            .focusSection()
            VStack(alignment: .leading, spacing: ref(6)) {
                Text(L.s("setup.name.custom")).dtype(DisplayType.fieldLabel).foregroundStyle(p.mut)
                DField(text: Binding(get: { wizard.custom }, set: { act(.custom($0)) }), focused: focus == .field)
                    .focused($focus, equals: .field)
                    .onSubmit { focus = .next }
            }
            actions {
                DButton(L.s("common.back"), kind: .secondary) { act(.back) }.focused($focus, equals: .back)
                DButton(L.s("common.next"), kind: .primary) { act(.next) }.focused($focus, equals: .next)
            }
        }
    }

    private var mode: some View {
        Group {
            heading("setup.mode.title")
            HStack(alignment: .top, spacing: ref(14)) {
                modeCard(.open, eyebrow: "setup.mode.fewPeople", text: "setup.mode.openText", focus: .modeOpen)
                modeCard(.confirm, eyebrow: "setup.mode.manyPeople", text: "setup.mode.approvalText", focus: .modeApproval)
            }
            .focusSection()
            bodyText("setup.mode.later")
            actions {
                DButton(L.s("common.back"), kind: .secondary) { act(.back) }.focused($focus, equals: .back)
                DButton(L.s("common.next"), kind: .primary) { act(.next) }.focused($focus, equals: .next)
            }
        }
    }

    private func modeCard(_ m: JoinMode, eyebrow: String, text: String, focus target: WizardFocus) -> some View {
        let keys = joinModeKeys(m)
        let on = wizard.mode == m
        return DButton(kind: .card, selected: on, action: { act(.mode(m)) }) {
            VStack(alignment: .leading, spacing: ref(6)) {
                Text(L.s(eyebrow)).dtype(DisplayType.eyebrow).foregroundStyle(p.acc)
                HStack(spacing: ref(10)) {
                    IconView(name: keys.icon, size: 24, color: p.fg)
                    Text(L.s(keys.title)).dtype(DisplayType.choiceTitle).foregroundStyle(p.fg)
                }
                Text(L.s(text)).dtype(DisplayType.cardBody).foregroundStyle(p.mut).fixedSize(horizontal: false, vertical: true)
            }
            .padding(ref(16))
            .frame(maxWidth: .infinity, minHeight: ref(120), alignment: .topLeading)
        }
        .focused($focus, equals: target)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private var address: some View {
        Group {
            heading("setup.address.title")
            if wizard.fieldOpen {
                VStack(alignment: .leading, spacing: ref(6)) {
                    Text(L.s("setup.address.fieldLabel")).dtype(DisplayType.fieldLabel).foregroundStyle(p.mut)
                    DField(text: Binding(get: { wizard.fieldText }, set: { act(.fieldText($0)) }), focused: focus == .field)
                        .focused($focus, equals: .field)
                        .onSubmit { focus = .looksRight }
                    if let e = wizard.fieldError { InfoLine(text: L.s("relay.errors.\(e.rawValue)")) }
                }
            } else if let a = wizard.address {
                HStack(spacing: ref(16)) {
                    IconView(name: "globe", size: 34, color: p.acc)
                    VStack(alignment: .leading, spacing: 0) {
                        Text(joinAddress(host: a.relay.host)).dtype(DisplayType.wizardAddress).foregroundStyle(p.fg)
                        Text(L.s(a.sourceKey)).dtype(DisplayType.hint).foregroundStyle(p.mut)
                    }
                    Spacer(minLength: 0)
                }
                .padding(.vertical, ref(16)).padding(.horizontal, ref(18))
                .background(RoundedRectangle(cornerRadius: ref(16)).fill(p.panel2))
                .accessibilityElement(children: .combine)
            }
            bodyText("setup.address.body")
            actions {
                DButton(L.s("common.back"), kind: .secondary) { act(.back) }.focused($focus, equals: .back)
                if wizard.canChangeAddress {
                    DButton(L.s("setup.address.change"), kind: .secondary) { act(.changeAddress) }.focused($focus, equals: .changeAddress)
                }
                DButton(L.s("setup.address.ok"), kind: .primary) { act(wizard.fieldOpen ? .fieldConfirm : .next) }
                    .focused($focus, equals: .looksRight)
            }
        }
    }

    private var done: some View {
        Group {
            heading("setup.done.title")
            bodyText("setup.done.bodyRemote")
            actions { DButton(L.s("setup.done.finish"), kind: .primary) { model.finishSetup() }.focused($focus, equals: .finish) }
        }
    }
}

/// A text field in the display's look (spec 3.5): 50 high, radius 10, 2 px `--line` border, `--bg` fill, 22 px text; the
/// focus ring without scale. tvOS opens its full-screen keyboard on Select.
struct DField: View {
    @Binding var text: String
    let focused: Bool
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: ref(10))
        TextField("", text: $text)
            .textFieldStyle(.plain)
            .autocorrectionDisabled()
            .dtype(DisplayType.field)
            .foregroundStyle(p.fg)
            .padding(.horizontal, ref(14))
            .frame(height: ref(50))
            .background(shape.fill(p.bg))
            .overlay(shape.strokeBorder(p.line, lineWidth: ref(2)))
            .focusEffectDisabled()
            .dfocusRing(focused: focused, radius: 10, scales: false)
    }
}

/// Chips that wrap onto the next line (the row of chips is one D-pad row even when it wraps, spec 4.1).
struct FlowLayout: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(width: proposal.width ?? .infinity, subviews: subviews).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let a = arrange(width: bounds.width, subviews: subviews)
        for (i, f) in a.frames.enumerated() {
            subviews[i].place(at: CGPoint(x: bounds.minX + f.minX, y: bounds.minY + f.minY), proposal: ProposedViewSize(f.size))
        }
    }

    private func arrange(width: CGFloat, subviews: Subviews) -> (frames: [CGRect], size: CGSize) {
        var frames: [CGRect] = []
        var x: CGFloat = 0, y: CGFloat = 0, rowH: CGFloat = 0, maxX: CGFloat = 0
        for s in subviews {
            let size = s.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width { x = 0; y += rowH + spacing; rowH = 0 }
            frames.append(CGRect(origin: CGPoint(x: x, y: y), size: size))
            x += size.width + spacing
            rowH = max(rowH, size.height)
            maxX = max(maxX, x - spacing)
        }
        return (frames, CGSize(width: maxX, height: y + rowH))
    }
}
