#!/usr/bin/env python3
"""The glass-canvas guard: every application window paints the Haze canvas.

An application's main window (`<qml root>/Main.qml`) is transparent, so the
compositor's blur shows through, and a `CelestinaBackdrop` paints the Haze
tint and grain over it (STYLE-G7-Q, DESIGN §5.2 L0). Two findings per window:

  opaque     the root object binds `color` to anything but
             `CelestinaTheme.clear`, or binds none (a window's default is
             opaque)
  backdrop   no `CelestinaBackdrop` object anywhere in the file

Comments never count. Findings count per project against a shrink-only
ratchet, as the radius guard's do: a project over its row fails, and a row
over reality fails too, so the unit that clears a window lowers its floor.

    glass_canvas_contract.py --baseline TSV [--write-baseline] LABEL=QML_ROOT ...
"""

from __future__ import annotations

import argparse
import dataclasses
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from architecture_scanners import strip_qml_comments  # noqa: E402
from radius_contract import BaselineError, read_baseline  # noqa: E402

COLOUR = re.compile(r"^\s*color\s*:\s*(\S.*?)\s*;?\s*$")
BACKDROP = re.compile(r"\bCelestinaBackdrop\s*\{")
CLEAR = "CelestinaTheme.clear"


@dataclasses.dataclass(frozen=True)
class Finding:
    path: pathlib.Path
    kind: str
    message: str

    def __str__(self) -> str:
        return f"{self.path}: {self.kind}: {self.message}"


def root_lines(text: str) -> list[str]:
    """The lines of the first object's own body, without nested objects."""
    start = text.find("{")
    if start < 0:
        return []
    depth = 0
    kept: list[str] = []
    for char in text[start:]:
        if char == "{":
            depth += 1
            kept.append("\n")
            continue
        if char == "}":
            depth -= 1
            kept.append("\n")
            if depth == 0:
                break
            continue
        if depth == 1:
            kept.append(char)
    return "".join(kept).splitlines()


def scan_window(path: pathlib.Path) -> list[Finding]:
    text = strip_qml_comments(path.read_text(encoding="utf-8"))
    findings: list[Finding] = []
    colours = [match.group(1) for line in root_lines(text)
               if (match := COLOUR.match(line))]
    if colours != [CLEAR]:
        shown = colours[0] if colours else "nothing"
        findings.append(Finding(path, "opaque",
                                f"the window binds color to {shown}, not {CLEAR}"))
    if not BACKDROP.search(text):
        findings.append(Finding(path, "backdrop",
                                "no CelestinaBackdrop paints the Haze canvas"))
    return findings


def write_baseline(path: pathlib.Path, counts: dict[str, int]) -> None:
    lines = [
        "# Glass-canvas debt ratchet. A row may only fall, and it falls in the",
        "# same commit that clears the window. Registered in docs/projects.toml as",
        "# a shared ratchet file so every project prefix may lower its own row.",
        "# findings<TAB>project",
    ]
    lines += [f"{count}\t{project}" for project, count in sorted(counts.items())]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--baseline", required=True, type=pathlib.Path)
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("roots", nargs="+", metavar="LABEL=QML_ROOT")
    arguments = parser.parse_args(argv)

    counts: dict[str, int] = {}
    for spec in arguments.roots:
        label, _, root = spec.partition("=")
        if not label or not root:
            print(f"expected LABEL=QML_ROOT, got {spec}", file=sys.stderr)
            return 2
        window = pathlib.Path(root) / "Main.qml"
        if not window.is_file():
            print(f"{label}: missing main window {window}", file=sys.stderr)
            return 2
        findings = scan_window(window)
        for finding in findings:
            print(finding)
        counts[label] = len(findings)

    if arguments.write_baseline:
        write_baseline(arguments.baseline, counts)
        print(f"Glass-canvas contract: baseline written to {arguments.baseline}")
        return 0

    try:
        baseline = read_baseline(arguments.baseline)
    except BaselineError as error:
        print(error, file=sys.stderr)
        return 2
    status = 0
    for label, count in counts.items():
        floor = baseline.get(label, 0)
        if count > floor:
            print(f"{label}: {count} finding(s) over the baseline {floor}", file=sys.stderr)
            status = 1
        elif count < floor:
            print(f"{label}: baseline {floor} exceeds the {count} finding(s) found; "
                  "lower it in this commit", file=sys.stderr)
            status = 1
    if status == 0:
        print("Glass-canvas contract: OK")
    return status


if __name__ == "__main__":
    sys.exit(main())
