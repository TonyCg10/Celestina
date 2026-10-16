#!/usr/bin/env python3
"""Tests for gen-tokens.py, the generator of the style's token outputs.

Run: python3 celestina-style/scripts/test-gen-tokens.py
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import pathlib
import re
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
STYLE = HERE.parent
SPEC = importlib.util.spec_from_file_location("gen_tokens", HERE / "gen-tokens.py")
assert SPEC is not None and SPEC.loader is not None
gen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gen)

# How each scheme role the theme used to derive is computed from the reference
# palette and the recipe dials, so a change to `ref.accent` that forgets a
# derived role fails here instead of shipping two accents. A role missing from
# this table is a literal of its own.
RECIPES = {
    "canvas": ("ref", "night"),
    "canvasInk": ("ref", "textHi"),
    "card": ("ref", "card"),
    "cardInk": ("ref", "textHi"),
    "elevated": ("ref", "elevated"),
    "elevatedInk": ("ref", "textHi"),
    "text": ("ref", "textHi"),
    "textMuted": ("ref", "textLo"),
    "textFaint": ("ref", "textFaint"),
    "divider": ("ref", "divider"),
    "dividerStrong": ("ref", "d4"),
    "accent": ("ref", "accent"),
    "accentInk": ("ref", "accentInk"),
    "accentLink": ("mix", "accent", "accentLift", ("accent", "linkMix")),
    "accentHover": ("mix", "accent", "accentLift", ("accent", "hoverMix")),
    "accentPressed": ("mix", "accent", "night", ("accent", "pressedMix")),
    "focusRing": ("mix", "accent", "accentLift", ("accent", "linkMix")),
    "danger": ("ref", "danger"),
    "dangerInk": ("ref", "night"),
    "success": ("ref", "success"),
    "successInk": ("ref", "night"),
    "warning": ("ref", "warning"),
    "warningInk": ("ref", "night"),
    "codeComment": ("ref", "codeComment"),
    "codeString": ("ref", "codeString"),
    "codeNumber": ("ref", "codeNumber"),
    "codeKeyword": ("ref", "codeKeyword"),
    "surfaceSelected": ("alpha", "accent", ("accent", "selectedOpacity")),
    "contentHover": ("alpha", "accent", ("accent", "contentHoverOpacity")),
    "pressedWash": ("alpha", "accent", ("accent", "pressedWashOpacity")),
    "selectionMarquee": ("alpha", "accent", ("accent", "marqueeOpacity")),
    "badgeAccentFill": ("alpha", "accent", ("accent", "badgeOpacity")),
    "accentSoft": ("alpha", "accent", ("accent", "softOpacity")),
    "accentSoftBorder": (
        "mix-alpha", "accent", "accentLift", ("accent", "linkMix"),
        ("accent", "softBorderOpacity"),
    ),
    "swatchOutline": ("alpha", "textHi", ("opacity", "swatchOutline")),
    "accentDisabledFill": ("alpha", "accent", ("accent", "selectedOpacity")),
    "accentDisabledInk": ("alpha", "accent", ("accent", "disabledInkOpacity")),
    "mediaScrimInk": ("ref", "textHi"),
    "mediaSurfaceInk": ("ref", "textHi"),
    "mediaArtworkInk": ("ref", "textHi"),
    "glyphDirectory": ("mix", "accent", "accentLift", ("accent", "linkMix")),
    "favorite": ("ref", "favorite"),
    "panelTint": ("alpha", "elevated", ("opacity", "panelTint")),
}


def channels(value: object) -> list[float]:
    red, green, blue, alpha = gen.rgba(value)
    return [red / 255, green / 255, blue / 255, alpha]


def resolved(colour: list[float]) -> str:
    """The theme's arithmetic: each 0..1 channel rounded half-up to 8 bits."""
    red, green, blue, alpha = (gen.byte(channel) for channel in colour)
    if alpha == 255:
        return f"#{red:02x}{green:02x}{blue:02x}"
    return f"#{alpha:02x}{red:02x}{green:02x}{blue:02x}"


def mix(source: list[float], target: list[float], amount: float) -> list[float]:
    return [start + (end - start) * amount for start, end in zip(source, target)]


def generate(tokens: pathlib.Path, root: pathlib.Path, *extra: str) -> tuple[int, str]:
    output = io.StringIO()
    with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output):
        status = gen.main([
            "--tokens", str(tokens),
            "--out-qml", str(root / "CelestinaDesignTokens.qml"),
            "--out-rs", str(root / "generated.rs"),
            *extra,
        ])
    return status, output.getvalue()


class GenTokensTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tokens = gen.load(gen.TOKENS)
        self.data = self.tokens.data

    def test_every_scheme_role_of_celestina_theme_is_a_key(self) -> None:
        theme = (STYLE / "CelestinaTheme.qml").read_text(encoding="utf-8")
        block = re.search(r"component ColorScheme: QtObject \{(.*?)\n    \}", theme, re.S)
        self.assertIsNotNone(block)
        roles = re.findall(r"required property color (\w+)", block.group(1))
        self.assertGreater(len(roles), 80)
        self.assertEqual(list(self.data["scheme"]["dark"]), roles)

    def test_hex_colours_parse_to_rgba(self) -> None:
        self.assertEqual(gen.rgba("#3e91ff"), (62, 145, 255, 1.0))
        self.assertEqual(gen.rgba("#16ffffff"), (255, 255, 255, 22 / 255))
        self.assertEqual(
            gen.rgba({"color": "#050608", "alpha": 0.70}), (5, 6, 8, 0.70)
        )
        for bad in ("#3e91f", "3e91ff", "#3e91fg", {"color": "#050608"},
                    {"color": "#050608", "alpha": 1.5}, 12):
            with self.assertRaises(gen.TokenError, msg=repr(bad)):
                gen.rgba(bad)

    def test_qml_colours_are_eight_bit_literals(self) -> None:
        self.assertEqual(gen.qml_colour("#3e91ff"), '"#3e91ff"')
        self.assertEqual(gen.qml_colour("#16ffffff"), '"#16ffffff"')
        # 0.70 × 255 = 178.5, rounded half-up to 179 = 0xb3.
        self.assertEqual(
            gen.qml_colour({"color": "#050608", "alpha": 0.70}), '"#b3050608"'
        )

    def test_rust_alpha_keeps_each_spelling(self) -> None:
        self.assertIn("a: 22.0 / 255.0,", gen.rust_colour("#16ffffff", "    "))
        self.assertIn("a: 1.0,", gen.rust_colour("#3e91ff", "    "))
        self.assertIn(
            "a: 0.7,", gen.rust_colour({"color": "#050608", "alpha": 0.70}, "    ")
        )

    def test_curves_gain_the_bezier_end_point_in_qml(self) -> None:
        qml = gen.render_qml(self.tokens)
        self.assertIn(
            "readonly property var motionCurveStandard: [0.22, 0.25, 0.0, 1.0, 1, 1]",
            qml,
        )
        rust = gen.render_rust(self.tokens)
        self.assertIn(
            "pub const CURVE_STANDARD: [f32; 4] = [0.22, 0.25, 0.0, 1.0];", rust
        )

    def test_derived_roles_follow_their_recipes(self) -> None:
        ref = {name: channels(value) for name, value in self.data["ref"].items()}

        def dial(path: tuple[str, str]) -> float:
            return float(self.data[path[0]][path[1]])

        for role, recipe in RECIPES.items():
            kind = recipe[0]
            if kind == "ref":
                expected = ref[recipe[1]]
            elif kind == "mix":
                expected = mix(ref[recipe[1]], ref[recipe[2]], dial(recipe[3]))
            elif kind == "alpha":
                expected = ref[recipe[1]][:3] + [dial(recipe[2])]
            else:
                mixed = mix(ref[recipe[1]], ref[recipe[2]], dial(recipe[3]))
                expected = mixed[:3] + [dial(recipe[4])]
            self.assertEqual(
                self.data["scheme"]["dark"][role], resolved(expected), role
            )

    def test_every_key_needs_its_comment(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = pathlib.Path(temporary) / "tokens.toml"
            source.write_text(
                "# The radii.\n[radius]\n# Rows.\nmd = 12\nlg = 20\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(gen.TokenError, r"radius\.lg"):
                gen.load(source)

    def test_two_tokens_may_not_share_a_qml_name(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            source = pathlib.Path(temporary) / "tokens.toml"
            source.write_text(
                "# R.\n[ref]\n# N.\nnight = \"#050608\"\n"
                "# S.\n[scheme.dark]\n# C.\ncanvas = \"#050608\"\n"
                "# A.\n[spacing]\n# Row.\nrowHeight = 40\n"
                "# B.\n[spacing.row]\n# Height.\nheight = 40\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(gen.TokenError, "spacingRowHeight"):
                gen.load(source)

    def test_check_mode_fails_on_a_stale_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            status, _ = generate(gen.TOKENS, root)
            self.assertEqual(status, 0)
            status, output = generate(gen.TOKENS, root, "--check")
            self.assertEqual(status, 0, output)

            rust = root / "generated.rs"
            rust.write_text(
                rust.read_text(encoding="utf-8").replace("r: 62,", "r: 63,", 1),
                encoding="utf-8",
            )
            status, output = generate(gen.TOKENS, root, "--check")
            self.assertEqual(status, 1)
            self.assertIn(str(rust), output)
            self.assertNotIn(str(root / "CelestinaDesignTokens.qml"), output)
            self.assertIn("r: 63,", output)

            rust.unlink()
            status, output = generate(gen.TOKENS, root, "--check")
            self.assertEqual(status, 1)
            self.assertIn(str(rust), output)

    def test_generated_rust_is_stable(self) -> None:
        first = gen.render_rust(gen.load(gen.TOKENS))
        second = gen.render_rust(gen.load(gen.TOKENS))
        self.assertEqual(first, second)
        self.assertEqual(gen.render_qml(self.tokens), gen.render_qml(self.tokens))
        with tempfile.TemporaryDirectory() as one, tempfile.TemporaryDirectory() as two:
            for root in (one, two):
                status, _ = generate(gen.TOKENS, pathlib.Path(root))
                self.assertEqual(status, 0)
            for name in ("generated.rs", "CelestinaDesignTokens.qml"):
                self.assertEqual(
                    (pathlib.Path(one) / name).read_bytes(),
                    (pathlib.Path(two) / name).read_bytes(),
                    name,
                )


if __name__ == "__main__":
    unittest.main()
