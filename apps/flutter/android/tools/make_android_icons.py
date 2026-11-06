#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Derives the Android launcher icons and the Android TV banner from assets/logo.

The originals in assets/logo are only read, never changed. Needs Pillow:
    python3 -m pip install pillow
Run from anywhere:
    python3 apps/flutter/android/tools/make_android_icons.py

Outputs (committed):
    app/src/main/res/mipmap-*/ic_launcher.png   48/72/96/144/192 px, from the square app icon
    app/src/main/res/drawable-xhdpi/tv_banner.png   320x180 px, Android TV launcher banner
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
ANDROID = HERE.parent
REPO = ANDROID.parents[2]
LOGO = REPO / "assets" / "logo"
RES = ANDROID / "app" / "src" / "main" / "res"

# Background colour taken from the app icon (dark navy), sampled at a corner-inset pixel.
ICON = Image.open(LOGO / "cometcast-app-icon-1024.png").convert("RGBA")
BG = ICON.getpixel((512, 40))[:3]

LAUNCHER_SIZES = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}


def launcher_icons() -> None:
    for density, size in LAUNCHER_SIZES.items():
        out = RES / f"mipmap-{density}" / "ic_launcher.png"
        ICON.resize((size, size), Image.LANCZOS).save(out, optimize=True)
        print("wrote", out.relative_to(REPO))


def tv_banner() -> None:
    w, h = 320, 180
    banner = Image.new("RGBA", (w, h), BG + (255,))
    # The dark-background logo (transparent) on the navy of the icon.
    logo = Image.open(LOGO / "cometcast-logo-dark-1024.png").convert("RGBA")
    logo = logo.resize((132, 132), Image.LANCZOS)
    banner.alpha_composite(logo, (24, (h - 132) // 2))
    # Brand name (a name, not a translatable UI string). UNCERTAIN: the font file is looked up
    # on this machine only; the script falls back to Pillow's built-in font.
    draw = ImageDraw.Draw(banner)
    font = None
    for candidate in (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
    ):
        if Path(candidate).exists():
            font = ImageFont.truetype(candidate, 30)
            break
    if font is None:
        font = ImageFont.load_default(30)
    draw.text((166, 62), "Comet", font=font, fill=(255, 255, 255, 255))
    draw.text((166, 98), "Cast", font=font, fill=(247, 183, 74, 255))
    out = RES / "drawable-xhdpi" / "tv_banner.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    banner.convert("RGB").save(out, optimize=True)
    print("wrote", out.relative_to(REPO))


if __name__ == "__main__":
    launcher_icons()
    tv_banner()
