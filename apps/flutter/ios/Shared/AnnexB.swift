// SPDX-License-Identifier: GPL-3.0-or-later
// H.264 bit stream helpers: VideoToolbox writes AVCC (every NAL unit preceded by a big-endian
// length) and keeps SPS/PPS in the format description; `cc_ios_push_frame` wants Annex B (start
// codes), the form the Rust RTP packetiser uses (crates/cc-sender/src/external.rs accepts AVCC
// too, but Annex B needs no guess about the length size). Foundation only, so the tests of
// RunnerTests can run these without VideoToolbox. Compiled into both targets.
//
// UNCERTAIN: never compiled or run; the byte logic is mirrored by a Python check in
// tools/ios-annexb-selfcheck.py that WAS run (docs/ios.md, "What was verified").

import Foundation

enum AnnexB {
    static let startCode: [UInt8] = [0, 0, 0, 1]

    /// Appends `startCode + nal` for every NAL unit of the AVCC buffer to `out`.
    /// `lengthSize` is 1...4 (VideoToolbox: 4). Returns the number of NAL units written, or -1
    /// if the buffer is malformed (a length that runs past the end, or a zero length); `out` is
    /// then left with whatever was appended before the error and must be discarded.
    @discardableResult
    static func appendAnnexB(fromAVCC data: UnsafeRawBufferPointer, lengthSize: Int,
                             into out: inout [UInt8]) -> Int {
        guard (1...4).contains(lengthSize) else { return -1 }
        var offset = 0
        var units = 0
        let bytes = data.bindMemory(to: UInt8.self)
        while offset < bytes.count {
            guard offset + lengthSize <= bytes.count else { return -1 }
            var length = 0
            for i in 0..<lengthSize { length = (length << 8) | Int(bytes[offset + i]) }
            offset += lengthSize
            guard length > 0, offset + length <= bytes.count else { return -1 }
            out.append(contentsOf: startCode)
            out.append(contentsOf: UnsafeBufferPointer(rebasing: bytes[offset..<(offset + length)]))
            offset += length
            units += 1
        }
        return units
    }

    /// `startCode + SPS + startCode + PPS ...` for the parameter sets of a format description.
    static func annexB(parameterSets: [Data]) -> [UInt8] {
        var out = [UInt8]()
        for set in parameterSets where !set.isEmpty {
            out.append(contentsOf: startCode)
            out.append(contentsOf: set)
        }
        return out
    }

    /// NAL unit type of the first NAL unit after a start code, or nil if there is none.
    static func firstNalType(_ annexB: [UInt8]) -> Int? {
        var i = 0
        while i + 3 < annexB.count {
            if annexB[i] == 0, annexB[i + 1] == 0, annexB[i + 2] == 1 { return Int(annexB[i + 3] & 0x1f) }
            i += 1
        }
        return nil
    }

    /// True if the Annex B unit contains an IDR slice (NAL type 5).
    static func containsIDR(_ annexB: [UInt8]) -> Bool {
        var i = 0
        while i + 3 < annexB.count {
            if annexB[i] == 0, annexB[i + 1] == 0, annexB[i + 2] == 1 {
                if annexB[i + 3] & 0x1f == 5 { return true }
                i += 3
            } else {
                i += 1
            }
        }
        return false
    }
}
