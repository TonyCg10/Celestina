#!/usr/bin/env python3
"""Keeps the compositor's generated colours tied to the sealed theme.

`celestina-shell-core::niri_colours` writes Niri's focus ring and backdrop from
values it carries as constants. Those constants are a second place the sealed
palette is written down, which is exactly the drift that module exists to
prevent — so this guard reads both and refuses a mismatch.

It is deliberately literal: it compares the exact strings, because a colour that
is "nearly" the panel's is the failure nobody notices and then cannot unsee.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
THEME = ROOT / "celestina-style/CelestinaTheme.qml"
GENERATOR = ROOT / "celestina-rs/crates/celestina-shell-core/src/niri_colours.rs"

SEALED_ROW = re.compile(
    r'token:\s*"(?P<token>[A-Za-z0-9_]+)"\s*,\s*value:\s*"(?P<value>#[0-9a-fA-F]+)"',
    re.MULTILINE,
)


def theme_colours(text: str) -> dict[str, str]:
    """Every colour the theme states literally, however it states it.

    Two forms carry a sealed value: a `readonly property color` on the theme
    itself, and a plain member assignment inside the active scheme block. Both
    are read, because which one a token uses is an implementation detail of the
    theme and not something this guard should depend on.
    """
    declared: dict[str, str] = {}
    literal = re.compile(
        r'(?:readonly\s+property\s+color\s+)?(?P<name>[A-Za-z0-9_]+)\s*:\s*'
        r'"(?P<value>#[0-9a-fA-F]{6,8})"'
    )
    for match in literal.finditer(text):
        # First statement wins: the reference palette is declared before the
        # schemes that derive from it, and a later override is a different
        # token's business.
        declared.setdefault(match["name"], match["value"])
    return declared


def main() -> int:
    for path in (THEME, GENERATOR):
        if not path.is_file():
            print(f"sealed-colours: missing {path.relative_to(ROOT)}", file=sys.stderr)
            return 1

    declared = theme_colours(THEME.read_text(encoding="utf-8"))
    generated = {
        match["token"]: match["value"]
        for match in SEALED_ROW.finditer(GENERATOR.read_text(encoding="utf-8"))
    }
    if not generated:
        print("sealed-colours: the generator declares no sealed colours", file=sys.stderr)
        return 1

    failures = 0
    for token, value in sorted(generated.items()):
        if token not in declared:
            print(
                f"sealed-colours: `{token}` is generated for Niri but the theme "
                "declares no such colour",
                file=sys.stderr,
            )
            failures = 1
            continue
        if declared[token] != value:
            print(
                f"sealed-colours: `{token}` is {value} in the Niri generator and "
                f"{declared[token]} in the theme; regenerate rather than editing one",
                file=sys.stderr,
            )
            failures = 1

    if failures:
        return 1
    print(f"Sealed colour contract: OK ({len(generated)} colour(s))")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
