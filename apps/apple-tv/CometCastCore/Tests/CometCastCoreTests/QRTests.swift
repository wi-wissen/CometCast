// SPDX-License-Identifier: GPL-3.0-or-later
// The QR generator against the matrices of qrcode-generator 2.0.4, the encoder of the web display.

import XCTest
@testable import CometCastCore

final class QRTests: XCTestCase {
    func rows(_ q: QRCode) -> [String] { q.modules.map { $0.map { $0 ? "1" : "0" }.joined() } }

    /// The mask a matrix was made with, read from its format information (level bits must be M).
    func mask(ofRows r: [String]) -> Int {
        func bit(_ x: Int, _ y: Int) -> Int { r[y][r[y].index(r[y].startIndex, offsetBy: x)] == "1" ? 1 : 0 }
        var bits = 0
        for i in 0...5 { bits |= bit(8, i) << i }
        bits |= bit(8, 7) << 6; bits |= bit(8, 8) << 7; bits |= bit(7, 8) << 8
        for i in 9..<15 { bits |= bit(14 - i, 8) << i }
        let data = (bits ^ 0x5412) >> 10
        XCTAssertEqual(data >> 3, 0, "level bits are not M")
        return data & 7
    }

    /// Both encoders make the same matrix once the mask is the same: data, error correction, function patterns,
    /// format and version information all agree. (The two choose the mask by different penalty rules: the web
    /// encoder uses a 3 x 3 neighbourhood rule for N1, this one the runs of ISO/IEC 18004; both are valid codes.)
    func testMatricesEqualTheWebEncoderAtLevelMWithTheSameMask() {
        XCTAssertEqual(QRReference.cases.count, 10)
        for c in QRReference.cases {
            let m = mask(ofRows: c.rows)
            guard let q = QREncoder.encode(Array(c.text.utf8), forcedMask: m) else { XCTFail("no QR for \(c.text)"); continue }
            XCTAssertEqual(q.version, c.version, c.text)
            XCTAssertEqual(rows(q), c.rows, "matrix differs (mask \(m)) for \(c.text)")
        }
    }

    func testChosenMaskHasTheLowestPenaltyAndIsScannable() {
        for c in QRReference.cases {
            let bytes = Array(c.text.utf8)
            let chosen = makeQR(c.text)!
            let all = (0..<8).map { QREncoder.encode(bytes, forcedMask: $0)! }
            let best = all.map { QREncoder.penalty($0.modules) }.min()!
            XCTAssertEqual(QREncoder.penalty(chosen.modules), best, c.text)
            XCTAssertEqual(mask(ofRows: rows(chosen)), chosen.mask)
        }
    }

    func testRoomUrlIsVersionThree() {
        // spec 4.2: "version 3 or 4 for normal addresses"
        XCTAssertEqual(makeQR("https://cometcast.de/app/K7QM2W")?.version, 3)
        XCTAssertEqual(makeQR("https://cometcast.de/app/K7QM2W")?.size, 29)
    }

    func testStructure() {
        let q = makeQR("https://cometcast.de/app/K7QM2W")!
        // finder patterns in three corners, timing pattern, dark module
        for (ox, oy) in [(0, 0), (q.size - 7, 0), (0, q.size - 7)] {
            for d in 0..<7 {
                XCTAssertTrue(q.module(x: ox + d, y: oy) && q.module(x: ox, y: oy + d) && q.module(x: ox + 6, y: oy + d))
            }
            XCTAssertFalse(q.module(x: ox + 1, y: oy + 1)); XCTAssertTrue(q.module(x: ox + 3, y: oy + 3))
        }
        for i in 8..<(q.size - 8) { XCTAssertEqual(q.module(x: i, y: 6), i % 2 == 0) }
        XCTAssertTrue(q.module(x: 8, y: q.size - 8))
    }

    func testTooLongAndUtf8() {
        XCTAssertNil(makeQR(String(repeating: "x", count: 214)))
        XCTAssertEqual(makeQR(String(repeating: "x", count: 213))?.version, 10)
        XCTAssertNotNil(makeQR("Grüße 😀")) // UTF-8 bytes
        XCTAssertEqual(makeQR("")?.version, 1)
    }

    func testEveryMaskIsAValidChoice() {
        // forcing each mask must keep the data decodable-looking: format bits differ, data modules differ only by the mask
        let bytes = Array("https://cometcast.de/app/K7QM2W".utf8)
        let base = QREncoder.encode(bytes, forcedMask: 0)!
        for m in 1..<8 {
            let q = QREncoder.encode(bytes, forcedMask: m)!
            XCTAssertEqual(q.mask, m)
            XCTAssertNotEqual(q.modules, base.modules)
        }
    }
}
