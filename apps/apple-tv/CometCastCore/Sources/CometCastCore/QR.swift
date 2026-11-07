// SPDX-License-Identifier: GPL-3.0-or-later
//
// A QR code generator for the idle screen (spec 4.2: the URL `https://{dom}/{CODE}`, error correction level M,
// "the QR version is whatever that URL needs"). Pure Swift, so that the matrix is tested on Linux against the
// matrices of the web app's generator (`qrcode-generator` 2.0.4) instead of relying on CoreImage.
// Byte mode only, level M, versions 1 to 10 (up to 213 bytes: far more than a room URL needs). The structure
// follows ISO/IEC 18004: function patterns, Reed-Solomon over GF(256) with 0x11D, the eight masks, the penalty
// rules N1 to N4, 15 bit format information and 18 bit version information.

import Foundation

public struct QRCode: Equatable, Sendable {
    public let version: Int
    public let size: Int
    public let mask: Int
    /// `modules[y][x]`, true = dark.
    public let modules: [[Bool]]

    public func module(x: Int, y: Int) -> Bool { modules[y][x] }
}

/// The QR code of `text` (UTF-8 bytes) at level M, or nil when it does not fit version 10.
public func makeQR(_ text: String) -> QRCode? {
    QREncoder.encode(Array(text.utf8), forcedMask: nil)
}

enum QREncoder {
    /// (EC codewords per block, [(block count, data codewords per block)]) at level M, versions 1 to 10.
    static let blocksM: [Int: (ec: Int, groups: [(count: Int, data: Int)])] = [
        1: (10, [(1, 16)]), 2: (16, [(1, 28)]), 3: (26, [(1, 44)]), 4: (18, [(2, 32)]), 5: (24, [(2, 43)]),
        6: (16, [(4, 27)]), 7: (18, [(4, 31)]), 8: (22, [(2, 38), (2, 39)]), 9: (22, [(3, 36), (2, 37)]),
        10: (26, [(4, 43), (1, 44)]),
    ]
    static let alignment: [Int: [Int]] = [
        1: [], 2: [6, 18], 3: [6, 22], 4: [6, 26], 5: [6, 30], 6: [6, 34], 7: [6, 22, 38], 8: [6, 24, 42], 9: [6, 26, 46],
        10: [6, 28, 50],
    ]
    /// Format bits of level M.
    static let formatLevelBits = 0

    static func dataCapacity(_ v: Int) -> Int { blocksM[v]!.groups.reduce(0) { $0 + $1.count * $1.data } }

    static func encode(_ bytes: [UInt8], forcedMask: Int?) -> QRCode? {
        // smallest version that holds the data: 4 bits mode, 8 or 16 bits count, 8 bits per byte
        var version = 0
        for v in 1...10 {
            let countBits = v <= 9 ? 8 : 16
            if 4 + countBits + bytes.count * 8 <= dataCapacity(v) * 8 { version = v; break }
        }
        guard version > 0, bytes.count < (1 << (version <= 9 ? 8 : 16)) else { return nil }

        // data bits
        var bits: [Bool] = []
        func put(_ value: Int, _ n: Int) { for i in stride(from: n - 1, through: 0, by: -1) { bits.append((value >> i) & 1 == 1) } }
        put(0b0100, 4)
        put(bytes.count, version <= 9 ? 8 : 16)
        for b in bytes { put(Int(b), 8) }
        let capBits = dataCapacity(version) * 8
        put(0, min(4, capBits - bits.count))
        while bits.count % 8 != 0 { bits.append(false) }
        var data: [UInt8] = stride(from: 0, to: bits.count, by: 8).map { i in
            (0..<8).reduce(UInt8(0)) { ($0 << 1) | (bits[i + $1] ? 1 : 0) }
        }
        var pad: UInt8 = 0xEC
        while data.count < dataCapacity(version) { data.append(pad); pad = pad == 0xEC ? 0x11 : 0xEC }

        let all = interleave(data, version: version)
        let size = version * 4 + 17

        var modules = [[Bool]](repeating: [Bool](repeating: false, count: size), count: size)
        var isFunction = [[Bool]](repeating: [Bool](repeating: false, count: size), count: size)
        drawFunctionPatterns(&modules, &isFunction, version: version, mask: 0)
        drawCodewords(&modules, isFunction, all)

        var bestMask = 0
        if let m = forcedMask { bestMask = m } else {
            var best = Int.max
            for m in 0..<8 {
                applyMask(&modules, isFunction, m)
                drawFormatBits(&modules, &isFunction, size: size, mask: m)
                let p = penalty(modules)
                if p < best { best = p; bestMask = m }
                applyMask(&modules, isFunction, m) // undo (XOR)
            }
        }
        applyMask(&modules, isFunction, bestMask)
        drawFormatBits(&modules, &isFunction, size: size, mask: bestMask)
        return QRCode(version: version, size: size, mask: bestMask, modules: modules)
    }

    // MARK: Reed-Solomon

    static let exp: [UInt8] = {
        var t = [UInt8](repeating: 0, count: 512)
        var x = 1
        for i in 0..<255 {
            t[i] = UInt8(x)
            x <<= 1
            if x & 0x100 != 0 { x ^= 0x11D }
        }
        for i in 255..<512 { t[i] = t[i - 255] }
        return t
    }()
    static let log: [Int] = {
        var t = [Int](repeating: 0, count: 256)
        for i in 0..<255 { t[Int(exp[i])] = i }
        return t
    }()

    static func mul(_ a: UInt8, _ b: UInt8) -> UInt8 {
        a == 0 || b == 0 ? 0 : exp[log[Int(a)] + log[Int(b)]]
    }

    /// Generator polynomial of the given degree, coefficients from the highest power down, without the leading 1.
    static func generator(_ degree: Int) -> [UInt8] {
        var result = [UInt8](repeating: 0, count: degree)
        result[degree - 1] = 1
        var root: UInt8 = 1
        for _ in 0..<degree {
            for j in 0..<degree {
                result[j] = mul(result[j], root)
                if j + 1 < degree { result[j] ^= result[j + 1] }
            }
            root = mul(root, 2)
        }
        return result
    }

    static func remainder(_ data: [UInt8], _ gen: [UInt8]) -> [UInt8] {
        var result = [UInt8](repeating: 0, count: gen.count)
        for b in data {
            let factor = b ^ result.removeFirst()
            result.append(0)
            for i in 0..<gen.count { result[i] ^= mul(gen[i], factor) }
        }
        return result
    }

    static func interleave(_ data: [UInt8], version: Int) -> [UInt8] {
        let spec = blocksM[version]!
        let gen = generator(spec.ec)
        var dataBlocks: [[UInt8]] = []
        var k = 0
        for g in spec.groups { for _ in 0..<g.count { dataBlocks.append(Array(data[k..<(k + g.data)])); k += g.data } }
        let ecBlocks = dataBlocks.map { remainder($0, gen) }
        var out: [UInt8] = []
        let maxData = dataBlocks.map(\.count).max()!
        for i in 0..<maxData { for b in dataBlocks where i < b.count { out.append(b[i]) } }
        for i in 0..<spec.ec { for b in ecBlocks { out.append(b[i]) } }
        return out
    }

    // MARK: matrix

    static func drawFunctionPatterns(_ m: inout [[Bool]], _ f: inout [[Bool]], version: Int, mask: Int) {
        let size = m.count
        func set(_ x: Int, _ y: Int, _ dark: Bool) { m[y][x] = dark; f[y][x] = true }
        for i in 0..<size { set(6, i, i % 2 == 0); set(i, 6, i % 2 == 0) }
        func finder(_ cx: Int, _ cy: Int) {
            for dy in -4...4 { for dx in -4...4 {
                let dist = max(abs(dx), abs(dy))
                let x = cx + dx, y = cy + dy
                if x >= 0, x < size, y >= 0, y < size { set(x, y, dist != 2 && dist != 4) }
            } }
        }
        finder(3, 3); finder(size - 4, 3); finder(3, size - 4)
        let pos = alignment[version]!
        for (i, px) in pos.enumerated() { for (j, py) in pos.enumerated() {
            if (i == 0 && j == 0) || (i == 0 && j == pos.count - 1) || (i == pos.count - 1 && j == 0) { continue }
            for dy in -2...2 { for dx in -2...2 { set(px + dx, py + dy, max(abs(dx), abs(dy)) != 1) } }
        } }
        drawFormatBits(&m, &f, size: size, mask: mask)
        if version >= 7 {
            var rem = version
            for _ in 0..<12 { rem = (rem << 1) ^ ((rem >> 11) * 0x1F25) }
            let bits = version << 12 | rem
            for i in 0..<18 {
                let bit = (bits >> i) & 1 == 1
                let a = size - 11 + i % 3, b = i / 3
                set(a, b, bit); set(b, a, bit)
            }
        }
    }

    static func drawFormatBits(_ m: inout [[Bool]], _ f: inout [[Bool]], size: Int, mask: Int) {
        func set(_ x: Int, _ y: Int, _ dark: Bool) { m[y][x] = dark; f[y][x] = true }
        let data = formatLevelBits << 3 | mask
        var rem = data
        for _ in 0..<10 { rem = (rem << 1) ^ ((rem >> 9) * 0x537) }
        let bits = (data << 10 | rem) ^ 0x5412
        func bit(_ i: Int) -> Bool { (bits >> i) & 1 == 1 }
        for i in 0...5 { set(8, i, bit(i)) }
        set(8, 7, bit(6)); set(8, 8, bit(7)); set(7, 8, bit(8))
        for i in 9..<15 { set(14 - i, 8, bit(i)) }
        for i in 0..<8 { set(size - 1 - i, 8, bit(i)) }
        for i in 8..<15 { set(8, size - 15 + i, bit(i)) }
        set(8, size - 8, true)
    }

    static func drawCodewords(_ m: inout [[Bool]], _ f: [[Bool]], _ data: [UInt8]) {
        let size = m.count
        var i = 0
        var right = size - 1
        while right >= 1 {
            if right == 6 { right = 5 }
            for vert in 0..<size {
                for j in 0..<2 {
                    let x = right - j
                    let upward = ((right + 1) & 2) == 0
                    let y = upward ? size - 1 - vert : vert
                    if !f[y][x], i < data.count * 8 {
                        m[y][x] = (data[i >> 3] >> UInt8(7 - (i & 7))) & 1 == 1
                        i += 1
                    }
                }
            }
            right -= 2
        }
    }

    static func applyMask(_ m: inout [[Bool]], _ f: [[Bool]], _ mask: Int) {
        let size = m.count
        for y in 0..<size { for x in 0..<size where !f[y][x] {
            let invert: Bool
            switch mask {
            case 0: invert = (x + y) % 2 == 0
            case 1: invert = y % 2 == 0
            case 2: invert = x % 3 == 0
            case 3: invert = (x + y) % 3 == 0
            case 4: invert = (x / 3 + y / 2) % 2 == 0
            case 5: invert = x * y % 2 + x * y % 3 == 0
            case 6: invert = (x * y % 2 + x * y % 3) % 2 == 0
            default: invert = ((x + y) % 2 + x * y % 3) % 2 == 0
            }
            if invert { m[y][x].toggle() }
        } }
    }

    // MARK: penalty (N1 to N4)

    static func penalty(_ m: [[Bool]]) -> Int {
        let size = m.count
        var result = 0
        func addHistory(_ run: Int, _ h: inout [Int]) {
            var run = run
            if h[0] == 0 { run += size }
            h.removeLast()
            h.insert(run, at: 0)
        }
        func countPatterns(_ h: [Int]) -> Int {
            let n = h[1]
            let core = n > 0 && h[2] == n && h[3] == n * 3 && h[4] == n && h[5] == n
            return (core && h[0] >= n * 4 && h[6] >= n ? 1 : 0) + (core && h[6] >= n * 4 && h[0] >= n ? 1 : 0)
        }
        func terminate(_ color: Bool, _ run: Int, _ h: inout [Int]) -> Int {
            var run = run
            if color { addHistory(run, &h); run = 0 }
            run += size
            addHistory(run, &h)
            return countPatterns(h)
        }
        for pass in 0..<2 {
            for a in 0..<size {
                var runColor = false
                var runLen = 0
                var h = [Int](repeating: 0, count: 7)
                for b in 0..<size {
                    let c = pass == 0 ? m[a][b] : m[b][a]
                    if c == runColor {
                        runLen += 1
                        if runLen == 5 { result += 3 } else if runLen > 5 { result += 1 }
                    } else {
                        addHistory(runLen, &h)
                        if !runColor { result += countPatterns(h) * 40 }
                        runColor = c
                        runLen = 1
                    }
                }
                result += terminate(runColor, runLen, &h) * 40
            }
        }
        for y in 0..<(size - 1) { for x in 0..<(size - 1) {
            let c = m[y][x]
            if c == m[y][x + 1], c == m[y + 1][x], c == m[y + 1][x + 1] { result += 3 }
        } }
        var dark = 0
        for row in m { for c in row where c { dark += 1 } }
        let total = size * size
        let k = (abs(dark * 20 - total * 10) + total - 1) / total - 1
        result += k * 10
        return result
    }
}
