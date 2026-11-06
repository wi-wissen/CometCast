#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate crates/cc-sender/assets/test-frames.ivf: a few tiny VP8 key frames.

Why: the native sender has no video encoder yet (no GPL encoders allowed), so for
tests it replays pre-made VP8 key frames. A lossy WebP file contains exactly one VP8
key frame (the "VP8 " chunk), so we let Pillow's libwebp write the images once and
copy the VP8 chunk into an IVF container. This is a dev-time tool only: the result is
committed, nothing here runs at build or test time.

Provenance of the committed file: generated with Python 3.13.18, Pillow 12.3.0 and its
bundled libwebp 1.6.0. The frames are plain rectangles drawn by this script (no font, no
third-party image), so the result is our own work; the lossy encoding was done by libwebp (BSD-3-Clause).

Usage: python3 tools/gen-test-frames.py
"""
import io
import struct
from pathlib import Path

from PIL import Image, ImageDraw

W, H, N, FPS = 320, 180, 12, 15
out = Path(__file__).resolve().parent.parent / "crates/cc-sender/assets/test-frames.ivf"


def vp8_chunk(webp: bytes) -> bytes:
    # RIFF header (12 bytes), then chunks: fourcc(4) + size(4, little endian) + data.
    pos = 12
    while pos < len(webp):
        fourcc, size = webp[pos : pos + 4], struct.unpack("<I", webp[pos + 4 : pos + 8])[0]
        if fourcc == b"VP8 ":
            return webp[pos + 8 : pos + 8 + size]
        pos += 8 + size + (size & 1)
    raise SystemExit("no VP8 chunk found (is the WebP lossy?)")


frames = []
for i in range(N):
    img = Image.new("RGB", (W, H))
    d = ImageDraw.Draw(img)
    hue = int(255 * i / N)
    d.rectangle([0, 0, W, H], fill=(hue, 90, 255 - hue))
    d.rectangle([10 + i * 20, 70, 70 + i * 20, 130], fill=(255, 255, 255))
    # No text is drawn on purpose: no font is used, so no font licence applies.
    d.rectangle([10, 10, 10 + 10 * (i + 1), 25], fill=(255, 255, 255))  # progress bar
    buf = io.BytesIO()
    img.save(buf, "WEBP", lossless=False, quality=60, method=4)
    frames.append(vp8_chunk(buf.getvalue()))

header = b"DKIF" + struct.pack("<HH4sHHIII", 0, 32, b"VP80", W, H, FPS, 1, N) + b"\0" * 4
body = b"".join(struct.pack("<IQ", len(f), n) + f for n, f in enumerate(frames))
out.write_bytes(header + body)
print(f"wrote {out} ({len(header) + len(body)} bytes, {N} key frames)")
