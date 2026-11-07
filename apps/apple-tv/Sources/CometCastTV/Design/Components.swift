// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The shared pieces of the display: the focus ring of spec 3.5 (drawn by us, the tvOS focus effect is switched off
// with `.focusEffectDisabled()`), the button looks of 3.5 / 4.x, the dialog card, the scrim, the toast.

import CometCastCore
import SwiftUI

// MARK: focus ring (spec 3.5, native-apps.md 6.1)

/// Outline 4 `--focus`, offset 3, glow 9 at 18 %, scale 1.03 in 120 ms (text fields: no scale). Reduced motion:
/// no scale, no animation (native-apps.md 14). The parent must not clip.
struct FocusRingModifier: ViewModifier {
    let focused: Bool
    let radius: Double
    var scales = true
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content
            .background {
                if focused {
                    RoundedRectangle(cornerRadius: ref(radius + FocusRing.glow))
                        .fill(DisplayPalette.glow)
                        .padding(-ref(FocusRing.glow))
                }
            }
            .overlay {
                if focused {
                    RoundedRectangle(cornerRadius: ref(radius + FocusRing.offset + FocusRing.outline / 2))
                        .strokeBorder(p.focus, lineWidth: ref(FocusRing.outline))
                        .padding(-ref(FocusRing.offset + FocusRing.outline))
                }
            }
            .scaleEffect(focused && scales && !reduceMotion ? FocusRing.scale : 1)
            .animation(reduceMotion ? nil : .easeInOut(duration: DisplayTiming.focusSeconds), value: focused)
            .zIndex(focused ? 1 : 0)
    }
}

extension View {
    func dfocusRing(focused: Bool, radius: Double, scales: Bool = true) -> some View {
        modifier(FocusRingModifier(focused: focused, radius: radius, scales: scales))
    }
}

// MARK: buttons

enum DButtonKind {
    /// Wizard and dialog primary: `--acc` fill, text `--bg`.
    case primary
    /// Wizard Back, request No, Dismiss: `--btn` fill, `--line` border.
    case secondary
    /// "No and block": ghost, 34 high, 14 px text.
    case ghost
    /// Name suggestion chip.
    case chip
    /// Mode and appearance tiles.
    case tile
    /// The wizard's choice cards.
    case card
    /// The Settings button on the idle screen.
    case menu
    /// The Close button of Settings: no border, `--mut`.
    case close
    /// End during a presentation: the name tag's fill (`--panel` at 92 %), 2 px `--line`, radius 10, min 46.
    case overlay

    var radius: Double {
        switch self {
        case .primary, .secondary: return Metrics.buttonRadius
        case .ghost, .menu, .overlay: return Metrics.ghostRadius
        case .chip: return 999
        case .tile: return Metrics.modeTileRadius
        case .card: return 16
        case .close: return 8
        }
    }
    var minHeight: Double {
        switch self {
        case .primary, .secondary: return Metrics.buttonMinHeight
        case .ghost: return Metrics.ghostHeight
        case .chip: return 46
        case .menu: return Metrics.settingsButtonMinHeight
        case .overlay: return Metrics.tagMinHeight
        case .close: return 34
        case .tile, .card: return 0
        }
    }
}

struct DButtonStyle: ButtonStyle {
    var kind: DButtonKind
    var selected = false

    func makeBody(configuration: Configuration) -> some View {
        DButtonBody(configuration: configuration, kind: kind, selected: selected)
    }
}

private struct DButtonBody: View {
    let configuration: ButtonStyleConfiguration
    let kind: DButtonKind
    let selected: Bool
    @Environment(\.isFocused) private var focused
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref

    private var fill: Color {
        switch kind {
        case .primary: return p.acc
        case .secondary: return p.btn
        case .ghost, .close, .menu: return kind == .menu ? p.panel : .clear
        case .overlay: return p.overlayFill
        case .chip: return p.panel2
        case .tile, .card: return selected ? p.selectedTile : p.panel2
        }
    }
    private var border: Color {
        switch kind {
        case .primary: return p.acc
        case .close: return .clear
        case .chip, .tile, .card: return selected ? p.acc : p.line
        default: return p.line
        }
    }
    private var text: Color {
        switch kind {
        case .primary: return p.bg
        case .close: return p.mut
        default: return p.fg
        }
    }
    private var borderWidth: Double { kind == .close ? 0 : 2 }
    private var padLeading: Double {
        switch kind {
        case .primary, .secondary: return Metrics.buttonPadH
        case .ghost: return 10
        case .overlay: return 14
        case .chip: return 18
        case .menu: return 12
        case .close: return 6
        case .tile, .card: return 0
        }
    }
    private var padTrailing: Double { kind == .menu ? 16 : padLeading }

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: ref(kind.radius))
        configuration.label
            .foregroundStyle(text)
            .padding(.leading, ref(padLeading))
            .padding(.trailing, ref(padTrailing))
            .frame(minHeight: ref(kind.minHeight))
            .background(shape.fill(fill))
            .overlay(shape.strokeBorder(border, lineWidth: ref(borderWidth)))
            .contentShape(shape)
            .dfocusRing(focused: focused, radius: kind.radius)
    }
}

/// A focusable button with the display's look. The caller adds `.focused(...)` / `.disabled(...)`.
struct DButton<Label: View>: View {
    var kind: DButtonKind = .secondary
    var selected = false
    let action: () -> Void
    @ViewBuilder var label: Label

    var body: some View {
        Button(action: action) { label }
            .buttonStyle(DButtonStyle(kind: kind, selected: selected))
            .focusEffectDisabled()
    }
}

extension DButton where Label == ModifiedContent<Text, DisplayTypeModifier> {
    init(_ title: String, kind: DButtonKind = .secondary, selected: Bool = false, action: @escaping () -> Void) {
        self.kind = kind
        self.selected = selected
        self.action = action
        self.label = Text(title).modifier(DisplayTypeModifier(style: DisplayType.button))
    }
}

// MARK: surfaces

/// The card of dialogs and the wizard: `--panel`, 1 px `--line`, radius 22, padding 28 x 34, gap 16, shadow
/// `0 24px 60px rgba(0,0,0,.55)` (spec 3.5).
struct DialogCard<Content: View>: View {
    var width: Double = Metrics.dialogWidth
    /// Dialogs over the scrim have the shadow; the wizard card (on plain `--bg`) has none.
    var shadow = true
    @ViewBuilder var content: Content
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: ref(Metrics.dialogRadius))
        VStack(alignment: .leading, spacing: ref(Metrics.dialogGap)) { content }
            .padding(.vertical, ref(Metrics.dialogPadV))
            .padding(.horizontal, ref(Metrics.dialogPadH))
            .frame(width: ref(width), alignment: .leading)
            .background(shape.fill(p.panel))
            .overlay(shape.strokeBorder(p.line, lineWidth: ref(1)))
            .shadow(color: .black.opacity(shadow ? 0.55 : 0), radius: ref(30), x: 0, y: ref(24))
    }
}

/// The scrim behind a dialog (55 % black); it also covers the letterbox around the canvas.
struct Scrim: View {
    var body: some View { DisplayPalette.scrim.ignoresSafeArea() }
}

/// Section label of dialogs: 14/700, uppercase, .08em, `--mut`.
struct SectionLabel: View {
    let text: String
    var color: Color?
    @Environment(\.palette) private var p
    var body: some View {
        Text(text).dtype(DisplayType.sectionLabel).foregroundStyle(color ?? p.mut)
    }
}

/// A line of warning or error text: 16/700 `--fg`, led by the `info` icon 20 in `--acc`; no red (spec 4.5).
struct InfoLine: View {
    let text: String
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    var body: some View {
        HStack(alignment: .top, spacing: ref(8)) {
            IconView(name: "info", size: 20, color: p.acc).padding(.top, ref(1))
            Text(text).dtype(DisplayType.warning).foregroundStyle(p.fg).fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// The toast: one line, `--fg` fill, `--bg` text, 17/700, capsule, shadow `0 10px 30px rgba(0,0,0,.4)`.
struct ToastView: View {
    let text: String
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref
    var body: some View {
        Text(text)
            .dtype(DisplayType.toast)
            .foregroundStyle(p.bg)
            .lineLimit(1)
            .padding(.vertical, ref(8)).padding(.horizontal, ref(20))
            .background(Capsule().fill(p.fg))
            .shadow(color: .black.opacity(0.4), radius: ref(15), x: 0, y: ref(10))
            .accessibilityAddTraits(.isStaticText)
    }
}

/// The canvas-sized frame: everything on the display except the picture is placed on the 960 x 540 canvas.
struct CanvasFrame<Content: View>: View {
    @ViewBuilder var content: Content
    @Environment(\.ref) private var ref
    var body: some View {
        content.frame(width: ref(canvasWidth), height: ref(canvasHeight))
    }
}
