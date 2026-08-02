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


def to_oklch(color: RGBA) -> tuple[float, float, float]:
    red, green, blue, _ = color
    r, g, b = (srgb_to_linear(c) for c in (red, green, blue))
    l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b) ** (1 / 3)
    m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b) ** (1 / 3)
    s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b) ** (1 / 3)
    lightness = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s
    green_red = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s
    blue_yellow = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s
    return (
        lightness,
        math.hypot(green_red, blue_yellow),
        math.degrees(math.atan2(blue_yellow, green_red)) % 360,
    )


def oklch_channels(lightness: float, chroma: float, hue: float) -> list[float]:
    radians = math.radians(hue)
    green_red = chroma * math.cos(radians)
    blue_yellow = chroma * math.sin(radians)
    l = (lightness + 0.3963377774 * green_red + 0.2158037573 * blue_yellow) ** 3
    m = (lightness - 0.1055613458 * green_red - 0.0638541728 * blue_yellow) ** 3
    s = (lightness - 0.0894841775 * green_red - 1.2914855480 * blue_yellow) ** 3
    return [
        linear_to_srgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        linear_to_srgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        linear_to_srgb(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s),
    ]


def from_oklch(lightness: float, chroma: float, hue: float, alpha: float) -> RGBA:
    """El mismo mapeo de gamut que el tema: bajar croma, nunca recortar canales.

    Recortar gira el tono, y esta receta existe justamente para que un ámbar no
    se vuelva oliva al oscurecerlo.
    """
    channels = oklch_channels(lightness, chroma, hue)
    if not all(-0.002 <= c <= 1.002 for c in channels):
        low, high = 0.0, chroma
        for _ in range(12):
            middle = (low + high) / 2
            if all(-0.002 <= c <= 1.002 for c in oklch_channels(lightness, middle, hue)):
                low = middle
            else:
                high = middle
        channels = oklch_channels(lightness, low, hue)
    red, green, blue = (min(1.0, max(0.0, c)) for c in channels)
    return red, green, blue, alpha


def icon_wash(color: RGBA, lift: float, drop: float, turn: float, chroma: float):
    lightness, saturation, hue = to_oklch(color)
    top = from_oklch(min(1.0, lightness + lift), saturation * (1 - chroma / 2),
                     (hue - turn) % 360, color[3])
    bottom = from_oklch(max(0.0, lightness - drop), saturation * (1 + chroma),
                        (hue + turn) % 360, color[3])
    return top, bottom


def srgb_to_linear(channel: float) -> float:
    return channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4


def linear_to_srgb(channel: float) -> float:
    return channel * 12.92 if channel <= 0.0031308 else 1.055 * (channel ** (1 / 2.4)) - 0.055


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

    # Los tonos de glifo sobre sus superficies: un icono no es texto, así que su
    # suelo es el 3:1 de gráficos no textuales.
    glyph_tones = {
        "carpeta": mixed_role(
            "glyphDirectory", "accent", "accentLift", accent, accent_lift,
            "accentLinkMix", link_mix,
        ),
        "archivo": literal("glyphFile"),
        "enlace": literal("glyphSymlink"),
        "navegación": literal("glyphNavigation"),
        "dispositivo": literal("glyphDevice"),
        "favorito": literal("favorite"),
    }
    for tone_name, tone in glyph_tones.items():
        for surface_name, surface in {
            "canvas": night,
            "tarjeta": card,
            "elevada": elevated,
        }.items():
            require(f"glifo {tone_name}/{surface_name}", tone, surface, 3.0)

    # El lavado de los iconos de contenido: lo que se pinta son los dos extremos
    # y el fondo de la carpeta, no el token, así que es a ellos a quienes se les
    # exige el 3:1 de gráficos no textuales sobre las superficies reales.
    wash_lift = scalar("iconGradientLift")
    wash_drop = scalar("iconGradientDrop")
    wash_turn = scalar("iconGradientTurn")
    wash_chroma = scalar("iconGradientChroma")
    backdrop_drop = scalar("iconBackdropDrop")
    for tone_name, tone in glyph_tones.items():
        top, bottom = icon_wash(tone, wash_lift, wash_drop, wash_turn, wash_chroma)
        lightness, saturation, hue = to_oklch(tone)
        backdrop = from_oklch(max(0.0, lightness - backdrop_drop),
                              saturation * (1 + wash_chroma / 2), hue, tone[3])
        for surface_name, surface in {
            "canvas": night,
            "tarjeta": card,
            "elevada": elevated,
        }.items():
            require(f"icono {tone_name}/{surface_name}/lavado alto",
                    top, surface, 3.0)
            require(f"icono {tone_name}/{surface_name}/lavado bajo",
                    bottom, surface, 3.0)
            require(f"icono {tone_name}/{surface_name}/fondo carpeta",
                    backdrop, surface, 3.0)

    # La hoja y el emblema no se pintan sobre la ventana sino sobre el propio
    # icono, así que su par es el bolsillo y el fondo de la carpeta. La tinta del
    # emblema no es fija: sobre un tono claro se cambia por una profunda, y aquí
    # se reproduce esa misma decisión.
    sheet = literal("iconSheet")
    ink_threshold = scalar("iconEmblemInkThreshold")
    ink_lightness = scalar("iconEmblemInkLightness")
    for tone_name, tone in glyph_tones.items():
        _, bottom = icon_wash(tone, wash_lift, wash_drop, wash_turn, wash_chroma)
        pocket_l, pocket_c, pocket_h = to_oklch(bottom)
        ink = (
            sheet
            if pocket_l <= ink_threshold
            else from_oklch(ink_lightness, pocket_c * 0.85, pocket_h, 1.0)
        )
        require(f"emblema/{tone_name}", ink, bottom, 3.0)

        lightness, saturation, hue = to_oklch(tone)
        backdrop = from_oklch(max(0.0, lightness - backdrop_drop),
                              saturation * (1 + wash_chroma / 2), hue, tone[3])
        backdrop_l, backdrop_c, backdrop_h = to_oklch(backdrop)
        paper = (
            sheet
            if backdrop_l <= ink_threshold
            else from_oklch(ink_lightness, backdrop_c * 0.85, backdrop_h, 1.0)
        )
        require(f"hoja/{tone_name}", paper, backdrop, 3.0)

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
