#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Checks the byte logic of the iOS code with the same test vectors as
apps/flutter/ios/RunnerTests/BroadcastTests.swift.

The Swift files (Shared/AnnexB.swift, Runner/Video/H264Parsing.swift, Shared/FrameSize.swift)
cannot be compiled here (no Swift toolchain, no Mac). This script is a line-by-line Python port of
their algorithms; it proves that the VECTORS and the algorithm are right, not that the Swift
text is. Run: python3 tools/ios-annexb-selfcheck.py
"""

START = bytes([0, 0, 0, 1])


def append_annexb_from_avcc(data: bytes, length_size: int):
    """Port of AnnexB.appendAnnexB(fromAVCC:lengthSize:into:); returns (bytes, units) or (None, -1)."""
    if not 1 <= length_size <= 4:
        return None, -1
    out = bytearray()
    offset = 0
    units = 0
    while offset < len(data):
        if offset + length_size > len(data):
            return None, -1
        length = int.from_bytes(data[offset:offset + length_size], "big")
        offset += length_size
        if length <= 0 or offset + length > len(data):
            return None, -1
        out += START + data[offset:offset + length]
        offset += length
        units += 1
    return bytes(out), units


def nal_units(data: bytes):
    """Port of H264Parsing.nalUnits(in:)."""
    starts = []
    i = 0
    while i + 2 < len(data):
        if data[i] == 0 and data[i + 1] == 0 and data[i + 2] == 1:
            starts.append(i + 3)
            i += 3
        else:
            i += 1
    ranges = []
    for n, start in enumerate(starts):
        end = starts[n + 1] - 3 if n + 1 < len(starts) else len(data)
        while end > start and data[end - 1] == 0:
            end -= 1
        if end > start:
            ranges.append((start, end))
    return ranges


def split(annexb: bytes):
    """Port of H264Parsing.split(annexB:)."""
    sps = pps = None
    avcc = bytearray()
    key = False
    slices = 0
    for a, b in nal_units(annexb):
        t = annexb[a] & 0x1F
        if t == 7:
            sps = annexb[a:b]
        elif t == 8:
            pps = annexb[a:b]
        elif t == 9:
            pass
        else:
            if t == 5:
                key = True
            if t in (1, 5):
                slices += 1
            avcc += (b - a).to_bytes(4, "big") + annexb[a:b]
    return (sps, pps, bytes(avcc), key) if slices else None


def fitted(width, height, orientation_swaps, max_dim):
    """Port of FrameSize.fitted."""
    w, h = (height, width) if orientation_swaps else (width, height)
    longest = max(w, h)
    scale = max_dim / longest if longest > max_dim else 1.0

    def even(v):
        # Swift: Int((v / 2).rounded()) * 2 ("schoolbook" rounding: halves away from zero)
        import math
        x = v / 2
        r = math.floor(x + 0.5)
        return max(2, int(r) * 2)

    return even(w * scale), even(h * scale)


SPS = bytes([0x67, 0x42, 0xC0, 0x1F, 0xDA, 0x05])
PPS = bytes([0x68, 0xCE, 0x3C, 0x80])
IDR = bytes([0x65, 0x88, 0x84, 0x00, 0x10])
DELTA = bytes([0x41, 0x9A, 0x02, 0x03])


def avcc(nals, size=4):
    return b"".join(len(n).to_bytes(size, "big") + n for n in nals)


def main():
    out, units = append_annexb_from_avcc(avcc([IDR, DELTA]), 4)
    assert units == 2 and out == START + IDR + START + DELTA, (units, out)
    assert append_annexb_from_avcc(bytes([0, 0, 0, 9, 0x65, 1]), 4)[1] == -1
    assert append_annexb_from_avcc(bytes([0, 0, 0, 0]), 4)[1] == -1
    two, n = append_annexb_from_avcc(avcc([IDR], 2), 2)
    assert n == 1 and two == START + IDR

    stream = START + SPS + bytes([0, 0, 1]) + PPS + START + IDR
    sps, pps, av, key = split(stream)
    assert sps == SPS and pps == PPS and key and av == avcc([IDR]), (sps, pps, av, key)
    assert split(START + SPS + START + PPS) is None

    assert fitted(2732, 2048, False, 1280) == (1280, 960)
    assert fitted(1170, 2532, False, 1280) == (592, 1280)
    w, h = fitted(1170, 2532, True, 1280)
    assert w == 1280 and h < w
    assert fitted(640, 480, False, 1280) == (640, 480)
    print("ios-annexb-selfcheck: all vectors match")


if __name__ == "__main__":
    main()
