#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Derives the Android launcher icons and the Android TV banner from assets/logo.

The originals in assets/logo are only read, never changed. Needs Pillow:
    python3 -m pip install pillow
Run from anywhere:
    python3 apps/flutter/android/tools/make_android_icons.py

Outputs (committed):
    app/src/main/res/mipmap-*/ic_launcher.png   48/72/96/144/192 px, from the square app icon
    app/src/main/res/drawable-{mdpi,hdpi,xhdpi}/tv_banner.png   320x180 dp (320/480/640 px wide)
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


# The Android TV banner is 320 x 180 **dp**: 320x180 px in drawable-mdpi (1x), 480x270 in hdpi
# (1.5x) and 640x360 in xhdpi (2x). (An earlier version put 320x180 px into drawable-xhdpi, which
# is only 160x90 dp and gets scaled up, blurry, by the launcher.)
BANNER_SCALES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0}


def _font(size: int):
    # Brand name (a name, not a translatable UI string). UNCERTAIN: the font file is looked up
    # on this machine only; the script falls back to Pillow's built-in font.
    for candidate in (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
    ):
        if Path(candidate).exists():
            return ImageFont.truetype(candidate, size)
    return ImageFont.load_default(size)


def tv_banner() -> None:
    logo_src = Image.open(LOGO / "cometcast-logo-dark-1024.png").convert("RGBA")
    for density, k in BANNER_SCALES.items():
        w, h = int(320 * k), int(180 * k)
        banner = Image.new("RGBA", (w, h), BG + (255,))
        side = int(132 * k)
        # The dark-background logo (transparent) on the navy of the icon.
        banner.alpha_composite(logo_src.resize((side, side), Image.LANCZOS), (int(24 * k), (h - side) // 2))
        draw = ImageDraw.Draw(banner)
        font = _font(int(30 * k))
        draw.text((int(166 * k), int(62 * k)), "Comet", font=font, fill=(255, 255, 255, 255))
        draw.text((int(166 * k), int(98 * k)), "Cast", font=font, fill=(247, 183, 74, 255))
        out = RES / f"drawable-{density}" / "tv_banner.png"
        out.parent.mkdir(parents=True, exist_ok=True)
        banner.convert("RGB").save(out, optimize=True)
        print("wrote", out.relative_to(REPO), banner.size)


if __name__ == "__main__":
    launcher_icons()
    tv_banner()
