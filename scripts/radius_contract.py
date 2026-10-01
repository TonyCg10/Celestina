#!/usr/bin/env python3
"""The radius guard: text out of the corner's curve, concentric nesting, no margin literals.

Reads CelestinaTheme.qml for the token values, walks every QML file under the
given roots, and inspects each object whose `radius`/`cornerRadius` is a theme
token (`CelestinaTheme.radius*`). For that object's direct children it applies:

  inset      a glyph-bearing child (Text, Label, CelestinaIcon, CelestinaSectionLabel,
             Image) touching a corner — anchored to the parent's left/right/top/
             bottom/fill, or positioned with x/y — sits at least
             cornerInset(radius) = ceil(0.3 * radius) from it; the inset is the
             child's own token margin, or the parent's token padding, or 0.
             A child anchored to verticalCenter/horizontalCenter with a token
             side margin, or to centerIn, is not a corner contact. An Image
             anchored with anchors.fill is full-bleed clipped artwork, not a
             glyph, and is exempt; an Image with any other anchor is still
             inspected.
             CelestinaFocusRing may enter. A radiusPill owner is exempt.
  concentric a child with its own token radius satisfies
             parent = child + inset, unless either radius is radiusPill or the
             child radius is radiusNone.
  literal    a numeric literal bound to padding, leftPadding, rightPadding,
             topPadding, bottomPadding, anchors.margins, anchors.leftMargin,
             anchors.rightMargin, anchors.topMargin, anchors.bottomMargin, x or y
             of a direct child.

Findings count per project against scripts/radius-baseline.tsv, a shrink-only
ratchet: a project over its row fails, and a row over reality fails too, so the
commit that pays the debt is the commit that lowers the floor.

    radius_contract.py --theme THEME --baseline TSV [--write-baseline] LABEL=ROOT ...
"""

from __future__ import annotations

import argparse
import dataclasses
import math
import pathlib
import re
import sys
from typing import Iterable, Iterator

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from architecture_scanners import qml_files, strip_qml_comments  # noqa: E402

GLYPH_TYPES = {"Text", "Label", "CelestinaIcon", "CelestinaSectionLabel", "Image"}
EXEMPT_TYPES = {"CelestinaFocusRing"}
INSET_PROPERTIES = (
    "padding", "leftPadding", "rightPadding", "topPadding", "bottomPadding",
    "anchors.margins", "anchors.leftMargin", "anchors.rightMargin",
    "anchors.topMargin", "anchors.bottomMargin", "x", "y",
)
CORNER_ANCHORS = {
    "anchors.fill", "anchors.left", "anchors.right",
    "anchors.top", "anchors.bottom",
}
CENTRE_ANCHORS = {"anchors.verticalCenter", "anchors.horizontalCenter"}
TOKEN = re.compile(r"^CelestinaTheme\.([A-Za-z0-9_]+)$")
NUMBER = re.compile(r"^-?[0-9]+(?:\.[0-9]+)?$")
OBJECT_START = re.compile(r"(?<![\w.])([A-Z][A-Za-z0-9_]*)\s*\{")
BINDING = re.compile(r"(?:(?<=[;{}\n])|^)[ \t]*([A-Za-z_][\w.]*)[ \t]*:(?!:)[ \t]*")


def corner_inset(radius: int) -> int:
    return math.ceil(radius * 0.3)


@dataclasses.dataclass(frozen=True)
class Finding:
    path: pathlib.Path
    line: int
    rule: str
    detail: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.rule}: {self.detail}"


@dataclasses.dataclass
class Node:
    type_name: str
    start: int
    end: int
    bindings: dict[str, tuple[str, int]] = dataclasses.field(default_factory=dict)
    children: list["Node"] = dataclasses.field(default_factory=list)


def theme_tokens(theme: pathlib.Path) -> dict[str, int]:
    text = strip_qml_comments(theme.read_text(encoding="utf-8"))
    tokens: dict[str, int] = {}
    aliases: dict[str, str] = {}
    for match in re.finditer(
        r"readonly property int (\w+):\s*([A-Za-z_]\w*|-?[0-9]+)", text
    ):
        name, value = match.group(1), match.group(2)
        if NUMBER.match(value):
            tokens[name] = int(value)
        else:
            aliases[name] = value
    for name, target in aliases.items():
        if target in tokens:
            tokens[name] = tokens[target]
    return tokens


def parse(text: str) -> list[Node]:
    """A bounded object tree over comment-free QML: every `TypeName {` opens an
    object (an object-valued binding such as `background: Rectangle {` included),
    any other `{` a plain block (a JS body); bindings are then read per object."""
    roots: list[Node] = []
    stack: list[Node | None] = []
    index = 0
    while index < len(text):
        character = text[index]
        if character in "\"'`":
            index = _skip_string(text, index)
            continue
        if character == "{":
            name_start = _word_start(text, index)
            match = OBJECT_START.match(text, name_start)
            if match and match.end() == index + 1 and (name_start == 0 or not (
                    text[name_start - 1].isalnum() or text[name_start - 1] in "_.")):
                node = Node(match.group(1), name_start, -1)
                owner = next((frame for frame in reversed(stack) if frame), None)
                (owner.children if owner else roots).append(node)
                stack.append(node)
            else:
                stack.append(None)
        elif character == "}" and stack:
            frame = stack.pop()
            if frame:
                frame.end = index
        index += 1
    for frame in stack:
        if frame:
            frame.end = len(text)
    _collect_bindings(text, roots)
    return roots


def _skip_string(text: str, index: int) -> int:
    quote = text[index]
    index += 1
    while index < len(text) and text[index] != quote:
        index += 2 if text[index] == "\\" else 1
    return index + 1


def _word_start(text: str, brace: int) -> int:
    cursor = brace - 1
    while cursor >= 0 and text[cursor].isspace():
        cursor -= 1
    while cursor >= 0 and (text[cursor].isalnum() or text[cursor] == "_"):
        cursor -= 1
    return cursor + 1


def _collect_bindings(text: str, nodes: list[Node]) -> None:
    for node in nodes:
        start = text.index("{", node.start) + 1
        segments = []
        for child in node.children:
            segments.append((start, child.start))
            start = child.end + 1
        segments.append((start, node.end))
        for segment_start, segment_end in segments:
            for match in BINDING.finditer(text, segment_start, segment_end):
                # An object-valued binding reads through its child's braces.
                value_end = _value_end(text, match.end(), node.end)
                value = " ".join(text[match.end():value_end].split())
                line = text.count("\n", 0, match.start(1)) + 1
                node.bindings[match.group(1)] = (value, line)
        _collect_bindings(text, node.children)


def _value_end(text: str, start: int, limit: int) -> int:
    depth = 0
    index = start
    while index < limit:
        character = text[index]
        if character in "\"'`":
            index = _skip_string(text, index)
            continue
        if character in "([{":
            depth += 1
        elif character in ")]}":
            depth -= 1
        elif character in ";\n" and depth <= 0:
            return index
        index += 1
    return limit


def token_value(value: str, tokens: dict[str, int]) -> tuple[str, int] | None:
    match = TOKEN.match(value)
    if not match or match.group(1) not in tokens:
        return None
    return match.group(1), tokens[match.group(1)]


def radius_of(node: Node, tokens: dict[str, int]) -> tuple[str, int] | None:
    for name in ("radius", "cornerRadius"):
        if name in node.bindings:
            return token_value(node.bindings[name][0], tokens)
    return None


def own_inset(node: Node, tokens: dict[str, int]) -> tuple[str, int] | None:
    """The smallest token inset the child declares on a side, or (name, 0) when
    it touches the parent with no margin at all."""
    smallest: tuple[str, int] | None = None
    for name in INSET_PROPERTIES:
        if name in node.bindings:
            resolved = token_value(node.bindings[name][0], tokens)
            if resolved and (smallest is None or resolved[1] < smallest[1]):
                smallest = resolved
    return smallest


def touches_corner(node: Node) -> bool:
    keys = set(node.bindings)
    if "anchors.centerIn" in keys:
        return False
    if keys & CENTRE_ANCHORS and "anchors.fill" not in keys:
        return False
    return bool(keys & CORNER_ANCHORS) or "x" in keys or "y" in keys


def parent_padding(node: Node, tokens: dict[str, int]) -> int:
    smallest: int | None = None
    for name in ("padding", "leftPadding", "rightPadding", "topPadding", "bottomPadding"):
        if name in node.bindings:
            resolved = token_value(node.bindings[name][0], tokens)
            if resolved and (smallest is None or resolved[1] < smallest):
                smallest = resolved[1]
    return smallest or 0


def rounded_nodes(nodes: Iterable[Node], tokens: dict[str, int]) -> Iterator[tuple[Node, Node, str, int]]:
    """Yield (owner, rounded, token, radius): `rounded` carries the radius and
    `owner` is the node whose children the rule inspects — the rounded node
    itself, or the control whose `background:` it is."""
    for node in nodes:
        radius = radius_of(node, tokens)
        if radius:
            yield node, node, radius[0], radius[1]
        for child in node.children:
            child_radius = radius_of(child, tokens)
            if child_radius and _is_background(node, child):
                yield node, child, child_radius[0], child_radius[1]
        yield from rounded_nodes(node.children, tokens)


def _is_background(parent: Node, child: Node) -> bool:
    value = parent.bindings.get("background")
    return bool(value) and value[0].startswith(child.type_name + " {")


def scan_file(path: pathlib.Path, tokens: dict[str, int]) -> list[Finding]:
    text = strip_qml_comments(path.read_text(encoding="utf-8"))
    findings: list[Finding] = []
    for owner, rounded, token, radius in rounded_nodes(parse(text), tokens):
        needed = corner_inset(radius)
        padding = parent_padding(owner, tokens)
        for child in owner.children:
            if child is rounded or child.type_name in EXEMPT_TYPES:
                continue
            line = text.count("\n", 0, child.start) + 1
            literal = False
            for name in INSET_PROPERTIES:
                if name in child.bindings and NUMBER.match(child.bindings[name][0]):
                    literal = True
                    findings.append(Finding(path, child.bindings[name][1], "literal",
                                            f"{name}: {child.bindings[name][0]}; use a space* token"))
            if literal:
                continue  # one finding per defect: the literal already names it
            inset = own_inset(child, tokens)
            effective = (inset[1] if inset else 0) + padding
            child_radius = radius_of(child, tokens)
            if child_radius and token != "radiusPill" and child_radius[0] not in ("radiusPill", "radiusNone"):
                if radius != child_radius[1] + effective:
                    inset_text = f"{inset[0]} ({inset[1]})" if inset else f"no inset ({padding})"
                    findings.append(Finding(path, line, "concentric",
                                            f"{token} ({radius}) != {child_radius[0]} ({child_radius[1]}) + {inset_text}"))
            # A full-bleed Image (anchors.fill) is clipped artwork, not a glyph.
            is_full_bleed_image = child.type_name == "Image" and "anchors.fill" in child.bindings
            if (token != "radiusPill" and child.type_name in GLYPH_TYPES and not is_full_bleed_image
                    and touches_corner(child) and effective < needed):
                if inset:
                    detail = f"{child.type_name} inset {inset[0]} ({inset[1]}) is below cornerInset({token}) = {needed}"
                else:
                    detail = f"{child.type_name} at {effective} px needs cornerInset({token}) = {needed}"
                findings.append(Finding(path, line, "inset", detail))
    return findings


class BaselineError(ValueError):
    pass


def read_baseline(path: pathlib.Path) -> dict[str, int]:
    rows: dict[str, int] = {}
    if not path.is_file():
        return rows
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        count, tab, project = line.partition("\t")
        if not tab or not project.strip() or not count.strip().isdigit():
            raise BaselineError(f"{path}: malformed row: {raw}")
        rows[project.strip()] = int(count)
    return rows


def write_baseline(path: pathlib.Path, counts: dict[str, int]) -> None:
    lines = [
        "# Radius-contract debt ratchet. A row may only fall, and it falls in the",
        "# same commit that earns the reduction. Registered in docs/projects.toml as",
        "# a shared ratchet file so every project prefix may lower its own row.",
        "# findings<TAB>project",
    ]
    lines += [f"{count}\t{project}" for project, count in sorted(counts.items())]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--theme", required=True, type=pathlib.Path)
    parser.add_argument("--baseline", required=True, type=pathlib.Path)
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("roots", nargs="+", metavar="LABEL=ROOT")
    arguments = parser.parse_args(argv)

    if not arguments.theme.is_file():
        print(f"missing theme: {arguments.theme}", file=sys.stderr)
        return 2
    tokens = theme_tokens(arguments.theme)
    if "radiusLg" not in tokens:
        print(f"{arguments.theme}: no radius tokens found", file=sys.stderr)
        return 2

    counts: dict[str, int] = {}
    for spec in arguments.roots:
        label, _, root = spec.partition("=")
        if not label or not root:
            print(f"expected LABEL=ROOT, got {spec}", file=sys.stderr)
            return 2
        findings = []
        for path in qml_files([root]):
            if path.resolve() == arguments.theme.resolve():
                continue
            findings.extend(scan_file(path, tokens))
        for finding in sorted(findings, key=lambda f: (str(f.path), f.line)):
            print(finding)
        counts[label] = len(findings)

    if arguments.write_baseline:
        write_baseline(arguments.baseline, counts)
        print(f"Radius contract: baseline written to {arguments.baseline}")
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
            print(f"{label}: baseline {floor} exceeds the {count} finding(s) found; lower it in this commit", file=sys.stderr)
            status = 1
    if status == 0:
        print("Radius contract: OK")
    return status


if __name__ == "__main__":
    sys.exit(main())
