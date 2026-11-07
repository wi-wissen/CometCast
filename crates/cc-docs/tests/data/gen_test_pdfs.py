#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generates the small test PDFs in this folder. Our own work; no third-party documents.

Run:  python3 -I gen_test_pdfs.py   (from this folder)
Needs: reportlab (tested with 5.0.1) and Pillow (tested with 12.3.0), Python 3.13.
The hand-written files need nothing. See PROVENANCE.md for what each file is for.
"""

import io
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def build_pdf(objects, header=b"%PDF-1.4\n", with_xref=True):
    """objects: list of bytes bodies for object 1..n. Returns a valid file with xref."""
    out = bytearray(header)
    offsets = []
    for i, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
    if with_xref:
        xref = len(out)
        out += b"xref\n0 %d\n" % (len(objects) + 1)
        out += b"0000000000 65535 f \n"
        for o in offsets:
            out += b"%010d 00000 n \n" % o
        out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (
            len(objects) + 1,
            xref,
        )
    return bytes(out)


def stream(dict_extra, data):
    return b"<< " + dict_extra + b" /Length %d >>\nstream\n" % len(data) + data + b"\nendstream"


def minimal():
    # 200 x 100 pt, left half red, right half blue, black text "Hi" top left (Helvetica,
    # not embedded: hayro substitutes its built-in font).
    content = (
        b"1 0 0 rg 0 0 100 100 re f\n"
        b"0 0 1 rg 100 0 100 100 re f\n"
        b"0 g BT /F1 14 Tf 4 82 Td (Hi) Tj ET\n"
    )
    return build_pdf(
        [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R "
            b"/Resources << /Font << /F1 5 0 R >> >> >>",
            stream(b"", content),
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        ]
    )


def no_xref():
    # Same page, but the cross reference table and trailer are missing: a reader has to
    # rebuild it by scanning (damaged-file case).
    full = minimal()
    cut = full.index(b"xref\n")
    return full[:cut]


def two_pages_one_rotated():
    # Page 1: 200 x 100, yellow. Page 2: /Rotate 90, 200 x 100 unrotated, green. Rendered
    # size of page 2 must be 100 x 200.
    p1 = b"1 1 0 rg 0 0 200 100 re f\n"
    p2 = b"0 0.6 0 rg 0 0 200 100 re f\n1 0 0 rg 0 50 50 50 re f\n"
    return build_pdf(
        [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 5 0 R >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Rotate 90 /Contents 6 0 R >>",
            stream(b"", p1),
            stream(b"", p2),
        ]
    )


def jpeg_bytes():
    from PIL import Image

    im = Image.new("RGB", (64, 32))
    for x in range(64):
        for y in range(32):
            im.putpixel((x, y), (255, 128, 0) if x < 32 else (0, 128, 128))
    buf = io.BytesIO()
    im.save(buf, "JPEG", quality=95, subsampling=0)
    return buf.getvalue()


def rl_canvas(name, **kw):
    from reportlab.pdfgen import canvas

    return canvas.Canvas(os.path.join(HERE, name), invariant=1, **kw)


def slides():
    from reportlab.lib.pagesizes import A4
    from reportlab.pdfbase import pdfmetrics
    from reportlab.pdfbase.ttfonts import TTFont
    import reportlab

    vera = os.path.join(os.path.dirname(reportlab.__file__), "fonts", "Vera.ttf")
    pdfmetrics.registerFont(TTFont("Vera", vera))
    c = rl_canvas("slides.pdf", pagesize=A4)  # 595.28 x 841.89 pt
    c.setTitle("Comet Cast test document")
    # Page 1: text in standard fonts and vector graphics.
    c.setFont("Helvetica-Bold", 28)
    c.drawString(50, 780, "Comet Cast test page 1")
    c.setFont("Times-Roman", 14)
    for i in range(12):
        c.drawString(50, 740 - 18 * i, "The quick brown fox jumps over the lazy dog %d." % i)
    c.setFillColorRGB(0, 0.6, 0)  # green box: x 50..250, y 300..450
    c.rect(50, 300, 200, 150, stroke=0, fill=1)
    c.setFillColorRGB(1, 0.85, 0)  # yellow circle
    c.circle(400, 375, 75, stroke=0, fill=1)
    c.setStrokeColorRGB(0.8, 0, 0.8)
    c.setLineWidth(4)
    p = c.beginPath()
    p.moveTo(50, 200)
    p.curveTo(150, 280, 250, 120, 350, 200)
    c.drawPath(p, stroke=1, fill=0)
    c.showPage()
    # Page 2: an embedded JPEG (left half orange, right half teal), 320 x 160 pt at (100, 500).
    from reportlab.lib.utils import ImageReader

    c.drawImage(ImageReader(io.BytesIO(jpeg_bytes())), 100, 500, width=320, height=160)
    c.setFont("Helvetica", 12)
    c.drawString(100, 480, "Embedded JPEG above (64 x 32 px, DCT).")
    c.showPage()
    # Page 3: embedded TrueType font (Bitstream Vera subset) and a linear gradient.
    c.setFont("Vera", 24)
    c.drawString(50, 780, "Embedded font: Vera 123 ABC")
    c.saveState()
    p = c.beginPath()
    p.rect(50, 400, 400, 100)
    c.clipPath(p, stroke=0, fill=0)
    c.linearGradient(50, 400, 450, 400, (colors_rgb(1, 0, 0), colors_rgb(0, 0, 1)))
    c.restoreState()
    c.showPage()
    c.save()


def colors_rgb(r, g, b):
    from reportlab.lib.colors import Color

    return Color(r, g, b)


def rotated_pages():
    # Four pages, all MediaBox 200 x 300 pt, with /Rotate 0, 90, 180, 270. In unrotated page
    # space a red 50 x 50 box sits in the top-left corner, so after rotation (clockwise) it
    # must show at top-left, top-right, bottom-right, bottom-left. Rotated 90 and 270 pages
    # are 300 x 200 when displayed. (Hand-written: reportlab swaps the media box itself.)
    content = b"1 0 0 rg 0 250 50 50 re f\n"
    objs = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R] /Count 4 >>",
    ]
    for rot in (0, 90, 180, 270):
        objs.append(
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Rotate %d /Contents 7 0 R >>"
            % rot
        )
    objs.append(stream(b"", content))
    return build_pdf(objs)


def encrypted():
    from reportlab.lib.pagesizes import A4
    from reportlab.lib import pdfencrypt

    for name, user, strength in (
        ("encrypted_empty_user_password.pdf", "", 128),
        ("encrypted_password.pdf", "secret", 128),
    ):
        enc = pdfencrypt.StandardEncryption(user, ownerPassword="owner", strength=strength)
        c = rl_canvas(name, pagesize=A4, encrypt=enc)
        c.setFillColorRGB(0, 0, 1)
        c.rect(100, 400, 200, 200, stroke=0, fill=1)
        c.setFillColorRGB(0, 0, 0)
        c.drawString(100, 700, "Encrypted test page (%s)" % name)
        c.showPage()
        c.save()


def main():
    def w(name, data):
        with open(os.path.join(HERE, name), "wb") as f:
            f.write(data)

    w("minimal.pdf", minimal())
    w("no_xref.pdf", no_xref())
    w("two_pages_one_rotated.pdf", two_pages_one_rotated())
    w("rotated_pages.pdf", rotated_pages())
    try:
        slides()
        encrypted()
    except ImportError as e:
        print("reportlab/Pillow missing, richer files not regenerated:", e, file=sys.stderr)


if __name__ == "__main__":
    main()
