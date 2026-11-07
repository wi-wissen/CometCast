// SPDX-License-Identifier: GPL-3.0-or-later
// H.264 helpers for the receiver: the Rust core hands over Annex B access units (start codes);
// VideoToolbox wants a format description (SPS/PPS) and length-prefixed (AVCC) samples.
// Foundation only, so RunnerTests can check them without VideoToolbox.
//
// UNCERTAIN: never compiled or run (tools/ios-annexb-selfcheck.py mirrors the byte logic).

import Foundation

enum H264Parsing {
    /// Ranges of the NAL units (without start codes) of an Annex B buffer. Accepts 3 and 4 byte
    /// start codes; trailing zero bytes before the next start code belong to no unit.
    static func nalUnits(in data: [UInt8]) -> [Range<Int>] {
        var starts = [Int]()
        var i = 0
        while i + 2 < data.count {
            if data[i] == 0, data[i + 1] == 0, data[i + 2] == 1 {
                starts.append(i + 3)
                i += 3
            } else {
                i += 1
            }
        }
        var ranges = [Range<Int>]()
        for (n, start) in starts.enumerated() {
            var end = n + 1 < starts.count ? starts[n + 1] - 3 : data.count
            while end > start, data[end - 1] == 0 { end -= 1 }
            if end > start { ranges.append(start..<end) }
        }
        return ranges
    }

    static func nalType(_ byte: UInt8) -> Int { Int(byte & 0x1f) }

    struct AccessUnit {
        var sps: [UInt8]?
        var pps: [UInt8]?
        /// The slice NAL units (types 1 and 5, plus SEI etc. that are not parameter sets or
        /// delimiters), AVCC with 4-byte lengths.
        var avcc: [UInt8]
        var isKey: Bool
    }

    /// Splits an Annex B access unit into parameter sets and slice data (AVCC, 4-byte lengths).
    /// Returns nil if the unit holds no slice.
    static func split(annexB data: [UInt8]) -> AccessUnit? {
        var unit = AccessUnit(sps: nil, pps: nil, avcc: [], isKey: false)
        var slices = 0
        for r in nalUnits(in: data) {
            switch nalType(data[r.lowerBound]) {
            case 7: unit.sps = Array(data[r])
            case 8: unit.pps = Array(data[r])
            case 9: break  // access unit delimiter
            case let t:
                if t == 5 { unit.isKey = true }
                if t == 1 || t == 5 { slices += 1 }
                let n = UInt32(r.count)
                unit.avcc.append(contentsOf: [UInt8(n >> 24), UInt8((n >> 16) & 0xff), UInt8((n >> 8) & 0xff), UInt8(n & 0xff)])
                unit.avcc.append(contentsOf: data[r])
            }
        }
        return slices > 0 ? unit : nil
    }
}
