#!/usr/bin/env python3
"""Verify contrast floors that depend on compositing Celestina theme tokens.

The QML guard catches literals outside the theme, but a semantic token can still
be unsafe when it is translucent over wallpaper or album artwork.  This script
reads the canonical values from CelestinaTheme.qml and checks the hostile black
and white backdrop extremes.  No duplicated palette snapshot lives here.
"""

from __future__ import annotations

import math
import pathlib
import re
import sys
from typing import Iterable


THEME = pathlib.Path(__file__).resolve().parents[1] / "CelestinaTheme.qml"
TEXT = THEME.read_text(encoding="utf-8")
RGBA = tuple[float, float, float, float]


def literal(name: str) -> RGBA:
    match = re.search(
        rf"(?m)^\s*(?:readonly\s+property\s+color\s+)?{re.escape(name)}"
        rf"\s*:\s*\"(#[0-9a-fA-F]{{6}}|#[0-9a-fA-F]{{8}})\"\s*(?://.*)?$",
        TEXT,
    )
    if not match:
        raise ValueError(f"no se encontró un literal directo para {name}")
    value = match.group(1)[1:]
    if len(value) == 6:
        alpha = 255
        red, green, blue = (int(value[index : index + 2], 16) for index in (0, 2, 4))
    else:
        alpha, red, green, blue = (
            int(value[index : index + 2], 16) for index in (0, 2, 4, 6)
        )
    return red / 255, green / 255, blue / 255, alpha / 255


def scalar(name: str) -> float:
    match = re.search(
        rf"(?m)^\s*readonly\s+property\s+real\s+{re.escape(name)}\s*:\s*([0-9.]+)",
        TEXT,
    )
    if not match:
        raise ValueError(f"no se encontró el escalar {name}")
    return float(match.group(1))


def mixed_role(
    name: str,
    source_name: str,
    target_name: str,
    source: RGBA,
    target: RGBA,
    amount_name: str,
    amount: float,
) -> RGBA:
    pattern = (
        rf"\b{re.escape(name)}\s*:\s*theme\.mixColors\(\s*"
        rf"theme\.ref\.{re.escape(source_name)}\s*,\s*"
        rf"theme\.ref\.{re.escape(target_name)}\s*,\s*"
        rf"theme\.{re.escape(amount_name)}\s*\)"
    )
    if not re.search(pattern, TEXT, flags=re.S):
        raise ValueError(f"el rol {name} ya no usa la receta contractual")
    return mix(source, target, amount)


def mix(source: RGBA, target: RGBA, amount: float) -> RGBA:
    return tuple(
        start + (end - start) * amount for start, end in zip(source, target)
    )  # type: ignore[return-value]


def composite(foreground: RGBA, background: RGBA) -> RGBA:
    alpha = foreground[3] + background[3] * (1 - foreground[3])
    if math.isclose(alpha, 0):
        return 0, 0, 0, 0
    channels = tuple(
        (foreground[index] * foreground[3]
         + background[index] * background[3] * (1 - foreground[3]))
        / alpha
        for index in range(3)
    )
    return channels[0], channels[1], channels[2], alpha


def relative_luminance(color: RGBA) -> float:
    def linear(channel: float) -> float:
        return channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4

    red, green, blue = (linear(channel) for channel in color[:3])
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast(first: RGBA, second: RGBA) -> float:
    high, low = sorted(
        (relative_luminance(first), relative_luminance(second)), reverse=True
    )
    return (high + 0.05) / (low + 0.05)


failures: list[str] = []


def require(label: str, foreground: RGBA, background: RGBA, minimum: float) -> None:
    ratio = contrast(foreground, background)
    if ratio + 1e-9 < minimum:
        failures.append(f"{label}: {ratio:.2f}:1; mínimo {minimum:.1f}:1")


def extremes() -> Iterable[tuple[str, RGBA]]:
    yield "negro", (0, 0, 0, 1)
    yield "blanco", (1, 1, 1, 1)


try:
    accent = literal("accent")
    accent_ink = literal("accentInk")
    accent_lift = literal("accentLift")
    night = literal("night")
    text_hi = literal("textHi")
    text_lo = literal("textLo")
    text_faint = literal("textFaint")
    card = literal("card")
    elevated = literal("elevated")
    danger = literal("danger")
    warning = literal("warning")
    link_mix = scalar("accentLinkMix")
    hover_mix = scalar("accentHoverMix")
    pressed_mix = scalar("accentPressedMix")
    accent_link = mixed_role(
        "accentLink", "accent", "accentLift", accent, accent_lift,
        "accentLinkMix", link_mix,
    )
    focus_ring = mixed_role(
        "focusRing", "accent", "accentLift", accent, accent_lift,
        "accentLinkMix", link_mix,
    )

    panel_inks = {
        "texto": text_hi,
        "texto secundario": text_lo,
        "enlace/acento": accent_link,
        "peligro": danger,
        "aviso": warning,
    }
    for tint_name in ("compositorGlassTint", "compositorGlassFallback"):
        tint = literal(tint_name)
        for backdrop_name, backdrop in extremes():
            surface = composite(tint, backdrop)
            require(f"{tint_name}/{backdrop_name}/foco exterior",
                    focus_ring, surface, 3.0)
            for ink_name, ink in panel_inks.items():
                require(
                    f"{tint_name}/{backdrop_name}/{ink_name}", ink, surface, 4.5
                )

    media_scrim = literal("mediaScrim")
    progress = literal("mediaProgress")
    progress_track = literal("mediaProgressTrack")
    for artwork_name, artwork in extremes():
        media_surface = composite(media_scrim, artwork)
        require(f"media/{artwork_name}/texto", text_hi, media_surface, 4.5)
        require(f"media/{artwork_name}/foco exterior",
                focus_ring, media_surface, 3.0)
        rendered_track = composite(progress_track, media_surface)
        rendered_progress = composite(progress, media_surface)
        require(
            f"media/{artwork_name}/progreso", rendered_progress, rendered_track, 3.0
        )

    primary_states = {
        "normal": accent,
        "hover": mix(accent, accent_lift, hover_mix),
        "pulsado": mix(accent, night, pressed_mix),
    }
    for state, fill in primary_states.items():
        require(f"botón primario/{state}", accent_ink, fill, 4.5)

    for surface_name, surface in {
        "canvas": night,
        "tarjeta": card,
        "elevada": elevated,
    }.items():
        require(f"foco exterior/{surface_name}", focus_ring, surface, 3.0)

    for state in ("normal", "hover", "pulsado"):
        require(f"foco/botón destructivo/{state}/exterior",
                focus_ring, card, 3.0)

    require(
        "texto seleccionado/campo",
        accent_ink,
        primary_states["pulsado"],
        4.5,
    )

    require("botón destructivo/pulsado", night, danger, 4.5)
    require("metadata tenue/tarjeta", text_faint, card, 4.5)
except (OSError, ValueError) as error:
    print(f"contraste: ERROR: {error}", file=sys.stderr)
    raise SystemExit(2) from error

if failures:
    for failure in failures:
        print(f"contraste: ERROR: {failure}", file=sys.stderr)
    raise SystemExit(1)

print("Contrato de contraste: OK")
