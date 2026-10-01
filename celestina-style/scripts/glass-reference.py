#!/usr/bin/env python3
"""Render the Haze reference the gallery shows beside the live GlassSurface.

Haze (1.6.10, the phone's glass) composes: a Gaussian blur whose sigma for a
20 dp radius is 0.57735 * 20 + 0.5 = 12.05 px, then its grain texture with
alpha 0.15, then the background colour at alpha 0.70. This script does exactly
that with Pillow over a backdrop that carries the gallery's own three colours
(accent, success, warning) as a horizontal gradient, so a person can hold the
live surface against the reference and see whether MultiEffect's blurMax is
calibrated. Run by hand; the PNGs are committed; no runtime reads them.

    python3 scripts/glass-reference.py
"""

from __future__ import annotations

import pathlib
import re
import sys

from PIL import Image, ImageFilter

HERE = pathlib.Path(__file__).resolve().parent
STYLE = HERE.parent
THEME = STYLE / "CelestinaTheme.qml"
OUT = STYLE / "gallery" / "reference"
SIZE = (300, 130)
SIGMA = 0.57735 * 20 + 0.5
NOISE_ALPHA = 0.15
TINT_ALPHA = 0.70


def literal(name: str) -> tuple[int, int, int]:
    text = THEME.read_text(encoding="utf-8")
    match = re.search(rf'readonly property color {name}: "#([0-9a-fA-F]{{6}})"', text)
    if match is None:
        match = re.search(rf'{name}: "#([0-9a-fA-F]{{6}})"', text)
    if match is None:
        sys.exit(f"{THEME}: no six-digit literal for {name}")
    value = match.group(1)
    return tuple(int(value[index : index + 2], 16) for index in (0, 2, 4))


def gradient(colours: list[tuple[int, int, int]]) -> Image.Image:
    width, height = SIZE
    image = Image.new("RGB", SIZE)
    pixels = image.load()
    stops = len(colours) - 1
    for x in range(width):
        position = x / (width - 1) * stops
        index = min(int(position), stops - 1)
        amount = position - index
        start, end = colours[index], colours[index + 1]
        colour = tuple(round(start[c] + (end[c] - start[c]) * amount) for c in range(3))
        for y in range(height):
            pixels[x, y] = colour
    return image


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    backdrop = gradient([literal("accent"), literal("success"), literal("warning")])
    backdrop.save(OUT / "backdrop.png", optimize=True)

    blurred = backdrop.filter(ImageFilter.GaussianBlur(radius=SIGMA)).convert("RGBA")

    noise = Image.open(STYLE / "icons" / "haze-noise.png").convert("RGBA")
    tiled = Image.new("RGBA", SIZE)
    for x in range(0, SIZE[0], noise.width):
        for y in range(0, SIZE[1], noise.height):
            tiled.paste(noise, (x, y))
    alpha = tiled.getchannel("A").point(lambda value: round(value * NOISE_ALPHA))
    tiled.putalpha(alpha)
    grained = Image.alpha_composite(blurred, tiled)

    canvas = literal("night")
    tint = Image.new("RGBA", SIZE, canvas + (round(255 * TINT_ALPHA),))
    tinted = Image.alpha_composite(grained, tint).convert("RGB")
    tinted.save(OUT / "haze.png", optimize=True)
    print(f"wrote {OUT / 'backdrop.png'} and {OUT / 'haze.png'} (sigma {SIGMA:.2f})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
