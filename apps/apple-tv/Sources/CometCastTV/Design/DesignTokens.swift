// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The design tokens as SwiftUI (docs/design/native-apps.md 11, 12, 14): the display palette of the chosen
// appearance (hex values from CometCastCore, sRGB), the reference scale of the 960 x 540 canvas, the display type
// scale with the bundled Atkinson Hyperlegible fonts, the focus ring of the spec. No SwiftUI semantic colours
// (`.primary`, `.accentColor`, materials) on our surfaces, no `.scaleEffect` for the canvas (it blurs text).

import CometCastCore
import SwiftUI

extension Color {
    /// sRGB, never Display P3 (native-apps.md 12).
    init(_ rgb: Rgb, opacity: Double = 1) {
        self.init(.sRGB, red: rgb.red, green: rgb.green, blue: rgb.blue, opacity: opacity)
    }
}

/// The colours of the current appearance (spec 3.2).
struct DisplayPalette: Equatable {
    let spec: DisplayPaletteSpec
    let lightLogo: Bool

    var bg: Color { Color(spec.bg) }
    var panel: Color { Color(spec.panel) }
    var panel2: Color { Color(spec.panel2) }
    var line: Color { Color(spec.line) }
    var fg: Color { Color(spec.fg) }
    var mut: Color { Color(spec.mut) }
    var acc: Color { Color(spec.acc) }
    var focus: Color { Color(spec.focus) }
    var btn: Color { Color(spec.btn) }
    var qrbg: Color { Color(spec.qrbg) }
    var oled: Bool { spec.oled }
    /// Selected tile fill: accent at 14 % over panel2 (OLED black: bg, no fill).
    var selectedTile: Color { Color(spec.selectedTileFill) }
    /// Name tag, End, badges and pills: panel at 92 %.
    var overlayFill: Color { Color(spec.panel, opacity: DisplayPaletteSpec.overlayAlpha) }
    static var qrModule: Color { Color(DisplayPaletteSpec.qrModule) }
    static var onBlack: Color { Color(DisplayPaletteSpec.onBlack) }
    static var picture: Color { Color(DisplayPaletteSpec.pictureBlack) }
    static var scrim: Color { Color.black.opacity(DisplayPaletteSpec.scrimAlpha) }
    static var glow: Color { Color(DisplayPaletteSpec.focusGlow, opacity: DisplayPaletteSpec.focusGlowAlpha) }

    static let dark = DisplayPalette(spec: .dark, lightLogo: false)

    init(spec: DisplayPaletteSpec, lightLogo: Bool) { self.spec = spec; self.lightLogo = lightLogo }

    init(choice: AppearanceChoice, systemIsDark: Bool) {
        self.init(spec: choice.palette(systemIsDark: systemIsDark), lightLogo: choice.usesLightLogo(systemIsDark: systemIsDark))
    }
}

/// Reference px to points: `1 ref px = scale`. Every length on the display is written `ref(26)`.
struct Ref: Equatable {
    var scale: CGFloat = 1
    func callAsFunction(_ v: Double) -> CGFloat { CGFloat(v) * scale }
}

/// The tvOS safe area at the canvas edges, in reference px (zero where the canvas is inside it). Controls and text
/// keep `max(spec padding, inset)` from each edge (native-apps.md 7).
struct CanvasInsets: Equatable {
    var top = 0.0, leading = 0.0, bottom = 0.0, trailing = 0.0
    func top(_ spec: Double) -> Double { max(spec, top) }
    func leading(_ spec: Double) -> Double { max(spec, leading) }
    func bottom(_ spec: Double) -> Double { max(spec, bottom) }
    func trailing(_ spec: Double) -> Double { max(spec, trailing) }
}

private struct PaletteKey: EnvironmentKey { static let defaultValue = DisplayPalette.dark }
private struct RefKey: EnvironmentKey { static let defaultValue = Ref() }
private struct InsetsKey: EnvironmentKey { static let defaultValue = CanvasInsets() }

extension EnvironmentValues {
    var palette: DisplayPalette { get { self[PaletteKey.self] } set { self[PaletteKey.self] = newValue } }
    var ref: Ref { get { self[RefKey.self] } set { self[RefKey.self] = newValue } }
    var canvasInsets: CanvasInsets { get { self[InsetsKey.self] } set { self[InsetsKey.self] = newValue } }
}

// MARK: type (spec 3.4)

struct DisplayTypeModifier: ViewModifier {
    let style: TypeStyle
    @Environment(\.ref) private var ref

    func body(content: Content) -> some View {
        content
            .font(.custom(style.face.rawValue, fixedSize: ref(style.size)))
            .tracking(ref(style.size * style.tracking))
            .lineSpacing(ref(style.lineSpacing))
            .textCase(style.uppercase ? .uppercase : nil)
    }
}

extension View {
    /// A row of the display type table (CometCastCore `DisplayType`), sizes in reference px.
    func dtype(_ style: TypeStyle) -> some View { modifier(DisplayTypeModifier(style: style)) }
}

// MARK: icons (spec 13, native-apps.md 13)

/// A stroke icon of the asset catalog (`Icons/icon-<name>`, template image, vector data preserved), tinted by `color`.
struct IconView: View {
    let name: String
    var size: Double = 20
    var color: Color
    @Environment(\.ref) private var ref

    var body: some View {
        Image("icon-\(name)")
            .renderingMode(.template)
            .resizable()
            .scaledToFit()
            .frame(width: ref(size), height: ref(size))
            .foregroundStyle(color)
            .accessibilityHidden(true)
    }
}

/// The logo (`Logo/logo-dark`, `logo-light`), a vector in its own colours.
struct LogoView: View {
    var size: Double
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref

    var body: some View {
        Image(p.lightLogo ? "logo-light" : "logo-dark")
            .resizable()
            .scaledToFit()
            .frame(width: ref(size), height: ref(size))
            .accessibilityHidden(true)
    }
}
