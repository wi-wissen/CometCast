// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The QR box of the idle screen (spec 3.2, 3.5, 4.2): 212 x 212, padding 10, radius 14. With a code it has the
// `--qrbg` fill (Light: plus a 1 px `--line` border); the modules are `#0E1630`. Locked: the code (quiet zone and
// modules) at 25 %, the Light border stays at full opacity. Empty box (connecting, offline): no modules, `--panel2`
// fill, 1 px `--line` border, full opacity: an old code must never be scannable. The matrix comes from
// CometCastCore (`makeQR`, level M, tested against the web encoder).

import CometCastCore
import SwiftUI

struct QRView: View {
    /// The URL to encode, or nil for the empty box.
    let url: String?
    var dimmed = false
    @Environment(\.palette) private var p
    @Environment(\.ref) private var ref

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: ref(Metrics.qrRadius))
        let code = url.flatMap(makeQR)
        ZStack {
            if let code {
                ZStack {
                    shape.fill(p.qrbg)
                    modules(code).padding(ref(Metrics.qrPadding))
                }
                .opacity(dimmed ? 0.25 : 1)
                if p.spec == DisplayPaletteSpec.light { shape.strokeBorder(p.line, lineWidth: ref(1)) }
            } else {
                shape.fill(p.panel2)
                shape.strokeBorder(p.line, lineWidth: ref(1))
            }
        }
        .frame(width: ref(Metrics.qrBox), height: ref(Metrics.qrBox))
        .accessibilityElement()
        .accessibilityLabel(url.map { "QR: \($0)" } ?? "")
        .accessibilityAddTraits(.isImage)
        .accessibilityHidden(code == nil)
    }

    private func modules(_ code: QRCode) -> some View {
        Canvas { ctx, size in
            let n = CGFloat(code.size)
            let cell = min(size.width, size.height) / n
            var path = Path()
            for y in 0..<code.size {
                var x = 0
                while x < code.size {
                    guard code.modules[y][x] else { x += 1; continue }
                    var run = 1
                    while x + run < code.size, code.modules[y][x + run] { run += 1 }
                    path.addRect(CGRect(x: CGFloat(x) * cell, y: CGFloat(y) * cell, width: CGFloat(run) * cell, height: cell))
                    x += run
                }
            }
            ctx.fill(path, with: .color(DisplayPalette.qrModule))
        }
    }
}
