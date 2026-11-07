// SPDX-License-Identifier: GPL-3.0-or-later
// The presenter tokens of spec 3.3 / native-apps.md section 10 as colours (sRGB, the same hex
// values as lib/src/design/tokens.dart). Light and dark follow the device; there is no OLED
// black on presenter pages.

import SwiftUI
import UIKit

enum PresenterPalette {
    private static func dynamic(light: UInt32, dark: UInt32) -> Color {
        Color(UIColor { $0.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
    }

    static let bg = dynamic(light: 0xF4F5F8, dark: 0x14171D)
    static let surface = dynamic(light: 0xFCFCFD, dark: 0x1D2128)
    static let border = dynamic(light: 0xCDD1D9, dark: 0x343944)
    static let borderInput = dynamic(light: 0xCDD1D9, dark: 0x3B404B)
    static let divider = dynamic(light: 0xD4D8E0, dark: 0x343944)
    static let fg = dynamic(light: 0x1C2027, dark: 0xE8EAEF)
    static let fg2 = dynamic(light: 0x3F4652, dark: 0xBCC3CF)
    static let fg3 = dynamic(light: 0x646C7B, dark: 0x9EA6B3)
    static let primary = dynamic(light: 0x2D4A8A, dark: 0x93AAE6)
    static let onPrimary = dynamic(light: 0xFFFFFF, dark: 0x0F1420)
    static let primarySoft = dynamic(light: 0xE7ECF6, dark: 0x222A3D)
    static let disabled = dynamic(light: 0xDDE1E8, dark: 0x2A2F38)
    static let onDisabled = dynamic(light: 0x4A5160, dark: 0x9EA6B3)
    static let error = dynamic(light: 0xB5483C, dark: 0xF08C80)
    static let errorDisc = dynamic(light: 0xF6E1DE, dark: 0x342A2C)
    static let ok = dynamic(light: 0x2C7A51, dark: 0x6CC597)
    static let okDisc = dynamic(light: 0xDDF0E5, dark: 0x21312F)
    static let warn = dynamic(light: 0x93650C, dark: 0xE3B45A)
    static let warnDisc = dynamic(light: 0xF8EFDC, dark: 0x332F26)

    /// Atkinson Hyperlegible Next (UI) and Mono (codes), section 11.
    static func ui(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        let name: String
        switch weight {
        case .bold, .heavy, .black: name = "AtkinsonHyperlegibleNext-Bold"
        case .medium, .semibold: name = "AtkinsonHyperlegibleNext-Medium"
        default: name = "AtkinsonHyperlegibleNext-Regular"
        }
        return .custom(name, size: size)
    }

    static func mono(_ size: CGFloat, bold: Bool = false) -> Font {
        .custom(bold ? "AtkinsonHyperlegibleMono-Bold" : "AtkinsonHyperlegibleMono-Medium", size: size)
    }
}

extension UIColor {
    convenience init(hex: UInt32) {
        self.init(
            red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255,
            alpha: 1
        )
    }
}
