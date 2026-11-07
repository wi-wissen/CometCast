// SPDX-License-Identifier: GPL-3.0-or-later
//
// The numbers of the design (docs/design/first-run-and-display.md section 3, docs/design/native-apps.md
// sections 11, 12, 14, 15), kept here as plain values so they are tested on Linux. The SwiftUI app only wraps
// them (Color, Font). Nothing in this file is a design decision: every value is copied from the spec.

import Foundation

// MARK: colours (spec 3.2)

/// An sRGB colour as 0xRRGGBB.
public struct Rgb: Equatable, Sendable {
    public var hex: UInt32
    public init(_ hex: UInt32) { self.hex = hex }
    public var red: Double { Double((hex >> 16) & 0xFF) / 255 }
    public var green: Double { Double((hex >> 8) & 0xFF) / 255 }
    public var blue: Double { Double(hex & 0xFF) / 255 }
    var r: Int { Int((hex >> 16) & 0xFF) }
    var g: Int { Int((hex >> 8) & 0xFF) }
    var b: Int { Int(hex & 0xFF) }

    /// `color-mix(in srgb, self alpha%, base)`: `self` at `alpha` over `base`, rounded per channel.
    public func blended(alpha: Double, over base: Rgb) -> Rgb {
        func mix(_ a: Int, _ b: Int) -> UInt32 { UInt32((Double(a) * alpha + Double(b) * (1 - alpha)).rounded()) }
        return Rgb((mix(r, base.r) << 16) | (mix(g, base.g) << 8) | mix(b, base.b))
    }
}

/// The display tokens of one appearance (spec 3.2). Names are the CSS custom properties without `--`.
public struct DisplayPaletteSpec: Equatable, Sendable {
    public var bg, panel, panel2, line, fg, mut, acc, focus, btn, qrbg: Rgb
    /// OLED black: the selected tile stays black, the check box gets a line border.
    public var oled: Bool

    /// Selected tile fill: the accent at 14 % over `panel2` (Dark `#3E4053`, Light `#D7D3CF`); OLED black: `bg`.
    public var selectedTileFill: Rgb { oled ? bg : acc.blended(alpha: 0.14, over: panel2) }

    public static let dark = DisplayPaletteSpec(
        bg: Rgb(0x0E1630), panel: Rgb(0x17213F), panel2: Rgb(0x1F2B50), line: Rgb(0x2E3C68), fg: Rgb(0xF3F5FA),
        mut: Rgb(0xB9C2D8), acc: Rgb(0xFFC266), focus: Rgb(0xFFC94D), btn: Rgb(0x24305A), qrbg: Rgb(0xFFFFFF), oled: false)
    public static let light = DisplayPaletteSpec(
        bg: Rgb(0xFFFFFF), panel: Rgb(0xF2F4F8), panel2: Rgb(0xE6EAF1), line: Rgb(0xB4BCCB), fg: Rgb(0x0A0F1E),
        mut: Rgb(0x343C51), acc: Rgb(0x7A4300), focus: Rgb(0xB35C00), btn: Rgb(0xE3E7EF), qrbg: Rgb(0xFFFFFF), oled: false)
    public static let oledBlack = DisplayPaletteSpec(
        bg: Rgb(0x000000), panel: Rgb(0x000000), panel2: Rgb(0x000000), line: Rgb(0x2C313C), fg: Rgb(0xE2E6EE),
        mut: Rgb(0xA1A9BA), acc: Rgb(0xD29E48), focus: Rgb(0xDDAE48), btn: Rgb(0x000000), qrbg: Rgb(0xE6E6E6), oled: true)

    /// QR modules are always dark on light.
    public static let qrModule = Rgb(0x0E1630)
    /// Text drawn directly on the black picture area: the same in every theme (decision 57).
    public static let onBlack = Rgb(0xA1A9BA)
    /// The picture area is black in every theme.
    public static let pictureBlack = Rgb(0x000000)
    /// Dialog scrim `rgba(0,0,0,.55)`.
    public static let scrimAlpha = 0.55
    /// Focus glow `rgba(255,201,77,.18)`.
    public static let focusGlow = Rgb(0xFFC94D)
    public static let focusGlowAlpha = 0.18
    /// Name tag, End, badges and pills: `panel` at 92 % (native-apps 10 `overlayFill`).
    public static let overlayAlpha = 0.92
}

/// What the person chose in Settings (spec 4.3).
public enum AppearanceChoice: String, CaseIterable, Sendable {
    case dark, light, oled, auto

    /// Translation key of the tile.
    public var titleKey: String {
        switch self {
        case .dark: return "appearance.dark"
        case .light: return "appearance.light"
        case .oled: return "appearance.oled"
        case .auto: return "appearance.auto"
        }
    }

    /// The palette to draw with. Automatic follows the system (dark system: Dark, light: Light) and never
    /// picks OLED black (spec 3.2).
    public func palette(systemIsDark: Bool) -> DisplayPaletteSpec {
        switch self {
        case .dark: return .dark
        case .light: return .light
        case .oled: return .oledBlack
        case .auto: return systemIsDark ? .dark : .light
        }
    }

    /// The logo file: dark on Dark and OLED black, light on Light (spec 3.2).
    public func usesLightLogo(systemIsDark: Bool) -> Bool {
        switch self {
        case .light: return true
        case .auto: return !systemIsDark
        default: return false
        }
    }

    /// Dark is the default (decision D3).
    public static let `default`: AppearanceChoice = .dark
}

// MARK: geometry and scaling (spec 3.1, 3.5, native-apps 7, 12)

public let canvasWidth = 960.0
public let canvasHeight = 540.0

/// `1 ref px = scale device points`: `min(w / 960, h / 540)` (spec 3.1).
public func canvasScale(width: Double, height: Double) -> Double {
    guard width > 0, height > 0 else { return 1 }
    return min(width / canvasWidth, height / canvasHeight)
}

/// Edge padding in ref px: `max(spec padding, safe-area inset / scale)` (native-apps 7, Apple TV). At 1920 x 1080
/// points the tvOS safe area gives top 30, bottom 30 and sides 40 ref.
public func safePadding(spec: Double, inset: Double, scale: Double) -> Double {
    guard scale > 0 else { return spec }
    return max(spec, inset / scale)
}

/// Idle screen padding in ref px (spec 3.5): top 26, sides 40, bottom 18.
public enum IdlePadding {
    public static let top = 26.0, side = 40.0, bottom = 18.0
}

// MARK: type (spec 3.4, native-apps 11)

/// The Apple PostScript names of the bundled fonts (native-apps 11).
public enum FontFace: String, Sendable {
    case regular = "AtkinsonHyperlegibleNext-Regular"
    case medium = "AtkinsonHyperlegibleNext-Medium"
    case bold = "AtkinsonHyperlegibleNext-Bold"
    case monoMedium = "AtkinsonHyperlegibleMono-Medium"
    case monoBold = "AtkinsonHyperlegibleMono-Bold"

    /// The files that go into the app bundle and `UIAppFonts` (Resources/Fonts).
    public var fileName: String {
        switch self {
        case .regular: return "AtkinsonHyperlegibleNext-400.ttf"
        case .medium: return "AtkinsonHyperlegibleNext-500.ttf"
        case .bold: return "AtkinsonHyperlegibleNext-700.ttf"
        case .monoMedium: return "AtkinsonHyperlegibleMono-500.ttf"
        case .monoBold: return "AtkinsonHyperlegibleMono-700.ttf"
        }
    }
}

/// One row of the display type table (spec 3.4). Sizes in ref px, `tracking` in em, `lineHeight` a multiplier
/// (0 = the font's own).
public struct TypeStyle: Equatable, Sendable {
    public var size: Double
    public var face: FontFace
    public var lineHeight: Double
    public var tracking: Double
    public var uppercase: Bool

    public init(_ size: Double, _ face: FontFace, line: Double = 0, tracking: Double = 0, uppercase: Bool = false) {
        self.size = size; self.face = face; self.lineHeight = line; self.tracking = tracking; self.uppercase = uppercase
    }

    /// Extra space between lines in ref px: `size * (lineHeight - 1)` (SwiftUI `lineSpacing` is added to the
    /// font's natural line height; Atkinson's natural height is taken as 1.0 em, UNCERTAIN: measured on a device).
    public var lineSpacing: Double { lineHeight > 1 ? size * (lineHeight - 1) : 0 }
}

public enum DisplayType {
    public static let code = TypeStyle(80, .monoBold, line: 1.0, tracking: 0.06)
    public static let address = TypeStyle(58, .bold, line: 1.05, tracking: -0.01)
    public static let heading = TypeStyle(32, .bold, line: 1.15)
    public static let roomName = TypeStyle(30, .bold, line: 1.05)
    public static let stepNumber = TypeStyle(21, .bold)
    public static let choiceTitle = TypeStyle(21, .bold)
    public static let modePill = TypeStyle(17, .bold)
    public static let stepLabel = TypeStyle(17, .regular)
    public static let toast = TypeStyle(17, .bold)
    public static let settingsButton = TypeStyle(17, .bold)
    public static let wizardBody = TypeStyle(18, .regular, line: 1.4)
    public static let button = TypeStyle(19, .bold, line: 1.4)
    public static let tile = TypeStyle(16, .medium)
    public static let tileSelected = TypeStyle(16, .bold)
    public static let footer = TypeStyle(15, .regular, line: 1.3)
    public static let nameTagLine1 = TypeStyle(15, .bold, line: 1.3)
    public static let nameTagLine2 = TypeStyle(13, .regular, line: 1.3)
    public static let nameTagDigits = TypeStyle(13, .monoBold, line: 1.3)
    public static let sectionLabel = TypeStyle(14, .bold, tracking: 0.08, uppercase: true)
    public static let smallCaps = TypeStyle(11, .regular, tracking: 0.06, uppercase: true)
    public static let checkNumber = TypeStyle(24, .monoBold)
    public static let linkAddress = TypeStyle(18, .monoMedium)
    public static let offlineTitle = TypeStyle(22, .bold)
    public static let offlineBody = TypeStyle(18, .regular, line: 1.35)
    public static let wizardChoiceBody = TypeStyle(16, .regular, line: 1.4)
    public static let wizardAddress = TypeStyle(30, .bold)
    public static let noBlock = TypeStyle(14, .bold)
    public static let hint = TypeStyle(16, .regular, line: 1.4)
    public static let warning = TypeStyle(16, .bold, line: 1.4)
    public static let licenceText = TypeStyle(13, .monoMedium, line: 1.4)
    public static let licenceName = TypeStyle(15, .bold)
    public static let licenceSub = TypeStyle(13, .regular)
    // wizard and dialog texts of the web display's stylesheet (apps/web/src/styles/display.css), same values
    public static let field = TypeStyle(22, .regular)
    public static let fieldLabel = TypeStyle(15, .regular)
    public static let chip = TypeStyle(18, .regular)
    public static let chipSelected = TypeStyle(18, .bold)
    public static let eyebrow = TypeStyle(13, .bold, tracking: 0.05, uppercase: true)
    public static let cardBody = TypeStyle(16, .regular, line: 1.35)
    public static let modeDescription = TypeStyle(15, .regular)
    public static let appearanceTile = TypeStyle(15, .medium)
    public static let appearanceTileSelected = TypeStyle(15, .bold)
    public static let queue = TypeStyle(15, .regular)
    public static let closeButton = TypeStyle(14, .regular)
    public static let mediaHost = TypeStyle(15, .bold)
    public static let dots = TypeStyle(15, .regular)
}

// MARK: motion and timing (spec 3.6, 8, native-apps 2.6, 15)

public enum DisplayTiming {
    /// Name tag and End stay after the last input (spec 3.6).
    public static let controlsVisibleSeconds = 5.0
    /// Name tag and End fade in and out.
    public static let controlsFadeSeconds = 0.4
    /// Toast visible.
    public static let toastSeconds = 3.2
    /// Focus ring move and scale.
    public static let focusSeconds = 0.12
    /// Input is ignored after a join request or link card appeared (spec 4.4).
    public static let inputGuardSeconds = 1.0
    /// Idle dimming: after 7 minutes without activity, to 40 % brightness (overlay of `bg` at 60 %) in 2 s; any
    /// activity restores in 200 ms.
    public static let idleDimAfterSeconds = 7.0 * 60
    public static let idleDimFadeSeconds = 2.0
    public static let idleRestoreSeconds = 0.2
    public static let idleDimOverlayAlpha = 0.6
    /// Idle, connecting turns into Idle, offline after this long without a code (spec 4.2).
    public static let connectingToOfflineSeconds = 8.0
    /// The switch knob (spec 15.10), not used on Apple TV (no switches), kept for the table.
    public static let knobSeconds = 0.12
}

// MARK: focus ring (spec 3.5)

public enum FocusRing {
    public static let outline = 4.0
    public static let offset = 3.0
    public static let glow = 9.0
    public static let scale = 1.03
}

/// Sizes of the surfaces in ref px (spec 3.5). One place, so the views carry no literals of their own.
public enum Metrics {
    public static let qrBox = 212.0, qrPadding = 10.0, qrRadius = 14.0, qrCaptionGap = 8.0
    public static let columnGap = 36.0, idleGap = 14.0, stepGap = 12.0, stepNumberGap = 16.0
    public static let stepDisc = 38.0
    public static let settingsButtonMinHeight = 46.0, settingsButtonRadius = 10.0, settingsButtonBorder = 2.0, settingsGear = 20.0
    public static let pillRadius = 999.0, pillIcon = 20.0
    public static let dialogWidth = 600.0, wizardWidth = 780.0, dialogPadV = 28.0, dialogPadH = 34.0, dialogRadius = 22.0, dialogGap = 16.0
    public static let modeTileHeight = 60.0, modeTileRadius = 12.0, modeTileBorder = 2.0, modeTileGap = 8.0
    public static let appearanceTileHeight = 46.0, appearanceTileGap = 6.0
    public static let buttonMinHeight = 50.0, buttonRadius = 12.0, buttonBorder = 2.0, buttonPadH = 22.0
    public static let ghostHeight = 34.0, ghostRadius = 10.0
    public static let tagMinHeight = 46.0, tagRadius = 10.0, tagBorder = 2.0, tagEdge = 12.0
    public static let progressPillMaxWidth = 340.0
    public static let toastTop = 14.0
    public static let wizardCardTop = 40.0, wizardMinBelow = 24.0
    public static let logoIdle = 46.0, logoWizard = 40.0
    public static let licenceListWidth = 300.0, licenceRowHeight = 40.0
    public static let linkMaxAddressLines = 6
}

// MARK: join mode and persistence (spec 4.3)

/// What is stored as the default mode: Open or Approval on. Locked is never stored (spec 4.3): it lasts until
/// End or until the display restarts.
public func storableDefaultMode(_ mode: JoinMode) -> JoinMode? {
    mode == .locked ? nil : mode
}

/// Translation keys of a join mode: pill and tile word, and the description line in Settings.
public func joinModeKeys(_ mode: JoinMode) -> (title: String, text: String, icon: String) {
    switch mode {
    case .open: return ("mode.open", "settings.join.openText", "open")
    case .confirm: return ("mode.approval", "settings.join.approvalText", "approval")
    case .locked: return ("mode.locked", "settings.join.lockedText", "locked")
    }
}

/// The three tiles of Settings, in the order of the focus row (spec 7.1).
public let joinModeOrder: [JoinMode] = [.open, .confirm, .locked]
