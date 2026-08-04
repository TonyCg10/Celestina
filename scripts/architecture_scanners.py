#!/usr/bin/env python3
"""Reusable scanners for the suite architecture guard.

Each command prints findings to stdout. Scanner/configuration failures are
reported on stderr and return a non-zero status so the Bash caller can fail
closed instead of mistaking a broken scanner for a clean checkout.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import shlex
import subprocess
import sys
import tomllib
from collections.abc import Iterable, Iterator
from typing import Any


class ScannerError(RuntimeError):
    """A scanner could not inspect its complete declared input."""


def _normalized_project_path(raw: object, label: str) -> str:
    if not isinstance(raw, str):
        raise ScannerError(f"{label} must be a relative project path")
    path = pathlib.PurePosixPath(raw)
    if (
        path.is_absolute()
        or str(path) != raw
        or raw in {"", "."}
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise ScannerError(f"{label} is not a normalized project path: {raw}")
    return raw


def _normalized_commit_roots(raw: object, label: str) -> tuple[str, ...]:
    if not isinstance(raw, list) or not raw:
        raise ScannerError(f"{label} must contain directory roots")
    roots: list[str] = []
    for value in raw:
        if not isinstance(value, str) or not value.endswith("/"):
            raise ScannerError(f"{label} requires roots ending in '/': {value}")
        path = pathlib.PurePosixPath(value)
        if (
            path.is_absolute()
            or str(path) + "/" != value
            or any(part in {"", ".", ".."} for part in path.parts)
        ):
            raise ScannerError(f"{label} contains a non-normalized root: {value}")
        roots.append(value)
    return tuple(dict.fromkeys(roots))


def canonical_evidence_root_for_prefix(
    registry: dict[str, Any], prefix: str
) -> str | None:
    """Return the only evidence root allowed to close debt for a prefix.

    Suite and primary project prefixes own durable evidence. Component prefixes
    intentionally return ``None`` because their narrow atomic scope cannot own
    a ledger/evidence record.
    """

    suite = registry.get("suite")
    if not isinstance(suite, dict) or not isinstance(suite.get("commit_prefix"), str):
        raise ScannerError("registry suite.commit_prefix is missing or invalid")
    if prefix == suite["commit_prefix"]:
        return "docs/evidence"

    projects = registry.get("projects", [])
    if not isinstance(projects, list):
        raise ScannerError("registry projects must be a list")
    matches: list[str] = []
    for index, project in enumerate(projects):
        if not isinstance(project, dict):
            raise ScannerError(f"registry projects[{index}] must be a table")
        if project.get("commit_prefix") != prefix:
            continue
        owner = _normalized_project_path(
            project.get("path"), f"projects[{index}].path"
        )
        matches.append(f"{owner}/docs/evidence")
    if len(matches) > 1:
        raise ScannerError(f'registry prefix "{prefix}:" has multiple owners')
    return matches[0] if matches else None


def canonical_evidence_roots_for_source(
    registry: dict[str, Any], source: str
) -> tuple[str, ...]:
    """Return suite evidence plus registered project evidence owning source."""

    roots = ["docs/evidence"]
    projects = registry.get("projects", [])
    if not isinstance(projects, list):
        raise ScannerError("registry projects must be a list")
    for index, project in enumerate(projects):
        if not isinstance(project, dict):
            raise ScannerError(f"registry projects[{index}] must be a table")
        commit_roots = _normalized_commit_roots(
            project.get("commit_roots"), f"projects[{index}].commit_roots"
        )
        if not any(source.startswith(root) for root in commit_roots):
            continue
        owner = _normalized_project_path(
            project.get("path"), f"projects[{index}].path"
        )
        roots.append(f"{owner}/docs/evidence")
    return tuple(dict.fromkeys(roots))


def is_canonical_evidence_path(path: str, roots: Iterable[str]) -> bool:
    candidate = pathlib.PurePosixPath(path)
    if (
        candidate.is_absolute()
        or str(candidate) != path
        or candidate.suffix != ".md"
        or any(part in {"", ".", ".."} for part in candidate.parts)
    ):
        return False
    return any(path.startswith(f"{root}/") for root in roots)


def parse_architecture_baseline(
    text: str, source: str
) -> dict[tuple[str, str], int]:
    result: dict[tuple[str, str], int] = {}
    for number, raw in enumerate(text.splitlines(), 1):
        if not raw or raw.startswith("#"):
            continue
        parts = raw.split("\t")
        if (
            len(parts) != 3
            or parts[0] not in {"lines", "control"}
            or not parts[2].isdigit()
            or int(parts[2]) <= 0
        ):
            raise ScannerError(f"{source}:{number}: invalid architecture baseline row")
        key = (parts[0], parts[1])
        if key in result:
            raise ScannerError(f"{source}:{number}: duplicate architecture baseline row")
        result[key] = int(parts[2])
    return result


def check_architecture_baseline_history(
    root: pathlib.Path,
    compare_ref: str,
    current_path: pathlib.Path,
    registry_path: pathlib.Path,
) -> None:
    """Reject new/raised debt and non-canonical resolution evidence."""

    try:
        old_text = subprocess.run(
            ["git", "show", f"{compare_ref}:scripts/architecture-baseline.tsv"],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        changed_paths = set(
            subprocess.run(
                ["git", "diff", "--name-only", "--no-renames", compare_ref, "--"],
                cwd=root,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.splitlines()
        )
        changed_paths.update(
            subprocess.run(
                ["git", "ls-files", "--others", "--exclude-standard"],
                cwd=root,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.splitlines()
        )
        current_text = current_path.read_text(encoding="utf-8")
        with registry_path.open("rb") as handle:
            registry = tomllib.load(handle)
    except (OSError, UnicodeError, subprocess.CalledProcessError, tomllib.TOMLDecodeError) as error:
        raise ScannerError(f"could not inspect architecture baseline history: {error}") from error

    old = parse_architecture_baseline(old_text, compare_ref)
    current = parse_architecture_baseline(current_text, str(current_path))

    def has_resolution_evidence(source: str) -> bool:
        marker = f"- **Resolved architecture debt:** `{source}`"
        roots = canonical_evidence_roots_for_source(registry, source)
        for path in sorted(changed_paths):
            if not is_canonical_evidence_path(path, roots):
                continue
            candidate = root / path
            if candidate.is_symlink() or not candidate.is_file():
                continue
            try:
                lines = candidate.read_text(encoding="utf-8").splitlines()
            except (OSError, UnicodeError):
                continue
            if marker in (line.strip() for line in lines):
                return True
        return False

    errors: list[str] = []
    for key, maximum in current.items():
        kind, name = key
        if key not in old:
            errors.append(f"new baseline debt {kind}: {name} ({maximum})")
        elif maximum > old[key]:
            errors.append(f"baseline raised: {kind} {name} {old[key]} -> {maximum}")
    for kind, name in sorted(set(old) - set(current)):
        if kind == "lines" and (
            name not in changed_paths or not has_resolution_evidence(name)
        ):
            errors.append(
                "baseline row removed without a changed source and canonical "
                f"resolution evidence: {name}"
            )
    if errors:
        raise ScannerError("\n".join(errors))


def qml_files(inputs: Iterable[str]) -> Iterator[pathlib.Path]:
    seen: set[pathlib.Path] = set()
    for raw in inputs:
        source = pathlib.Path(raw)
        if not source.exists() and not source.is_symlink():
            raise ScannerError(f"missing QML path: {source}")
        if source.is_symlink() and not source.exists():
            raise ScannerError(f"broken QML symlink: {source}")

        candidates = [source] if source.is_file() else source.rglob("*.qml")
        for path in sorted(candidates):
            if path.is_symlink() and not path.exists():
                raise ScannerError(f"broken QML symlink: {path}")
            if not path.is_file():
                continue
            if "build" in path.parts or "target" in path.parts or path in seen:
                continue
            seen.add(path)
            yield path


def strip_qml_comments(text: str) -> str:
    result: list[str] = []
    index = 0
    state = "normal"
    quote = ""
    escaped = False
    while index < len(text):
        character = text[index]
        following = text[index + 1] if index + 1 < len(text) else ""

        if state == "line-comment":
            if character == "\n":
                state = "normal"
                result.append(character)
            else:
                result.append(" ")
        elif state == "block-comment":
            if character == "*" and following == "/":
                result.extend((" ", " "))
                index += 1
                state = "normal"
            else:
                result.append("\n" if character == "\n" else " ")
        elif state == "string":
            result.append(character)
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                state = "normal"
        elif character == "/" and following == "/":
            result.extend((" ", " "))
            index += 1
            state = "line-comment"
        elif character == "/" and following == "*":
            result.extend((" ", " "))
            index += 1
            state = "block-comment"
        else:
            result.append(character)
            if character in {'"', "'", "`"}:
                quote = character
                state = "string"
        index += 1
    return "".join(result)


def parenthesis_delta(line: str) -> int:
    delta = 0
    quote: str | None = None
    escaped = False
    for character in line:
        if quote is not None:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                quote = None
            continue
        if character in {'"', "'", "`"}:
            quote = character
        elif character == "(":
            delta += 1
        elif character == ")":
            delta -= 1
    return delta


def scan_auto_bindings(inputs: Iterable[str]) -> None:
    binding = re.compile(r"^\s*([A-Za-z_]\w*)\s*:\s*([A-Za-z_]\w*)\s*;?\s*$")
    for path in qml_files(inputs):
        depth = 0
        text = strip_qml_comments(path.read_text(encoding="utf-8"))
        for number, line in enumerate(text.splitlines(), 1):
            match = binding.match(line)
            if (
                depth == 0
                and match
                and match.group(1) == match.group(2)
                and match.group(1) != "id"
            ):
                print(f"{path}:{number}: {line.rstrip()}")
            depth = max(0, depth + parenthesis_delta(line))


CONTROL = re.compile(
    r"^[ \t]*(?:"
    r"(?:component\s+[A-Za-z_]\w*|"
    r"(?:(?:default|readonly|required)\s+)*property\s+Component\s+[A-Za-z_]\w*|"
    r"[A-Za-z_]\w*(?:\.[A-Za-z_]\w*)*)"
    r"\s*:\s*)?"
    r"(?:[A-Za-z_]\w*\.)?"
    r"(BusyIndicator|Button|CheckBox|CheckDelegate|ComboBox|Container|Control|"
    r"DelayButton|Dial|Dialog|DialogButtonBox|Drawer|Frame|GroupBox|"
    r"HorizontalHeaderView|ItemDelegate|Label|Menu|MenuBar|MenuBarItem|MenuItem|"
    r"MenuSeparator|Page|PageIndicator|Pane|Popup|ProgressBar|RadioButton|"
    r"RadioDelegate|RangeSlider|RoundButton|ScrollBar|ScrollIndicator|ScrollView|"
    r"SelectionRectangle|Slider|SpinBox|SplitView|StackView|SwipeDelegate|"
    r"SwipeView|Switch|SwitchDelegate|TabBar|TabButton|TextArea|TextField|ToolBar|"
    r"ToolButton|ToolSeparator|ToolTip|TreeViewDelegate|Tumbler|VerticalHeaderView)"
    r"[ \t\r\n]*\{",
    re.M,
)


def canonical_style_link(path: pathlib.Path, style_root: pathlib.Path) -> bool:
    if not path.is_symlink():
        return False
    canonical = style_root / path.name
    if not canonical.is_file():
        return False
    return path.resolve(strict=True) == canonical.resolve(strict=True)


def scan_local_controls(
    inputs: Iterable[str], style_root_raw: str | None = None
) -> None:
    style_root = pathlib.Path(style_root_raw) if style_root_raw else None
    if style_root is not None and not style_root.is_dir():
        raise ScannerError(f"missing style tree: {style_root}")

    for path in qml_files(inputs):
        # Shared controls are intentionally linked into each app. Exclude only
        # the canonical same-name link; renamed or foreign links remain input.
        if style_root is not None and canonical_style_link(path, style_root):
            continue
        text = strip_qml_comments(path.read_text(encoding="utf-8"))
        for match in CONTROL.finditer(text):
            print(f"{path}\t{match.group(1)}")


STYLE_RULES = (
    (
        "hexadecimal literal outside the theme",
        re.compile(r"#[0-9a-fA-F]{3,8}(?![0-9a-fA-F])"),
    ),
    (
        "local color transformation",
        re.compile(r"\bQt\s*\.\s*(?:rgba|darker|lighter|tint)\s*\("),
    ),
    (
        "direct CelestinaTheme.ref access",
        re.compile(r"\bCelestinaTheme\s*\.\s*ref(?:\W|$)"),
    ),
    ("direct numeric duration", re.compile(r"\bduration\s*:\s*[0-9]", re.S)),
    (
        "direct animation curve",
        re.compile(r"\beasing\s*\.\s*type\s*:\s*Easing\s*\.", re.S),
    ),
    (
        "direct numeric font size",
        re.compile(r"\bfont\s*\.\s*pixelSize\s*:\s*[0-9]", re.S),
    ),
    (
        "direct font weight",
        re.compile(
            r"\bfont\s*\.\s*weight\s*:\s*[0-9]"
            r"|\bFont\s*\.\s*(?:Normal|Medium|DemiBold|Bold)"
        ),
    ),
    (
        "direct numeric letter spacing",
        re.compile(r"\bfont\s*\.\s*letterSpacing\s*:\s*[0-9]", re.S),
    ),
    ("direct numeric radius", re.compile(r"\bradius\s*:\s*[0-9]", re.S)),
    (
        "direct numeric border width",
        re.compile(
            r"\bborder\s*\.\s*width\s*:\s*[1-9][0-9]*(?:\.[0-9]+)?",
            re.S,
        ),
    ),
    (
        "direct numeric padding",
        re.compile(
            r"\b(?:leftPadding|rightPadding|topPadding|bottomPadding|padding)"
            r"\s*:\s*[1-9][0-9]*(?:\.[0-9]+)?",
            re.S,
        ),
    ),
    (
        "direct fractional opacity",
        re.compile(r"\bopacity\s*:\s*0\.[0-9]+", re.S),
    ),
)

COLOR_BINDING = re.compile(
    r"(?:\b(?:readonly\s+)?property\s+color\s+[A-Za-z_]\w*"
    r"|\b(?:[A-Za-z_]\w*\s*\.\s*)*(?:color|selectionColor|selectedTextColor|"
    r"placeholderTextColor|fillColor))\s*:\s*(.*)$"
)
NAMED_COLOR_STRING = re.compile(r"[\"']([A-Za-z][A-Za-z0-9_-]*)[\"']")
QML_NAMED_COLORS = frozenset(
    """aliceblue antiquewhite aqua aquamarine azure beige bisque black
    blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse
    chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan
    darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta
    darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen
    darkslateblue darkslategray darkslategrey darkturquoise darkviolet
    darkyellow deeppink deepskyblue dimgray dimgrey dodgerblue firebrick
    floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray
    green greenyellow grey honeydew hotpink indianred indigo ivory khaki
    lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral
    lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink
    lightsalmon lightseagreen lightskyblue lightslategray lightslategrey
    lightsteelblue lightyellow lime limegreen linen magenta maroon
    mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen
    mediumslateblue mediumspringgreen mediumturquoise mediumvioletred
    midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive
    olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise
    palevioletred papayawhip peachpuff peru pink plum powderblue purple
    rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown
    seagreen seashell sienna silver skyblue slateblue slategray slategrey snow
    springgreen steelblue tan teal thistle tomato transparent turquoise violet
    wheat white whitesmoke yellow yellowgreen""".split()
)


def scan_named_color_bindings(path: pathlib.Path, text: str) -> None:
    lines = text.splitlines()
    for index, line in enumerate(lines):
        match = COLOR_BINDING.search(line)
        if not match:
            continue

        base_indent = len(line) - len(line.lstrip(" \t"))
        expression = [match.group(1)]
        needs_expression_line = not match.group(1).strip()
        continuation = index + 1
        while continuation < len(lines):
            candidate = lines[continuation]
            if not candidate.strip():
                expression.append(candidate)
                continuation += 1
                continue
            indent = len(candidate) - len(candidate.lstrip(" \t"))
            # QML permits the first expression token after a bare `color:` at
            # the binding's own indentation. Always consume that first
            # non-empty line; later sibling bindings at the same indentation
            # must still terminate the expression to avoid false positives.
            if indent <= base_indent and not needs_expression_line:
                break
            expression.append(candidate)
            needs_expression_line = False
            continuation += 1

        for named in NAMED_COLOR_STRING.finditer("\n".join(expression)):
            if named.group(1).lower() not in QML_NAMED_COLORS:
                continue
            print(
                f"{path}:{index + 1}: named color outside the theme: "
                f"{named.group(0)}"
            )


def normalized_qml(text: str) -> bytes:
    text = strip_qml_comments(text)
    result: list[str] = []
    quote: str | None = None
    escaped = False
    for character in text:
        if quote is not None:
            result.append(character)
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                quote = None
        elif character in {'"', "'", "`"}:
            quote = character
            result.append(character)
        elif not character.isspace():
            result.append(character)
    return "".join(result).encode("utf-8")


def scan_qml_style_contract(theme_raw: str, inputs: Iterable[str]) -> None:
    theme = pathlib.Path(theme_raw)
    if not theme.is_file():
        raise ScannerError(f"missing QML theme: {theme}")
    canonical_theme = theme.resolve()

    for path in qml_files(inputs):
        if path.resolve() == canonical_theme:
            continue
        text = strip_qml_comments(path.read_text(encoding="utf-8"))
        scan_named_color_bindings(path, text)
        for label, pattern in STYLE_RULES:
            for match in pattern.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                snippet = " ".join(match.group(0).split())[:120]
                print(f"{path}:{line}: {label}: {snippet}")


def scan_style_copies(style_root_raw: str, inputs: Iterable[str]) -> None:
    style_root = pathlib.Path(style_root_raw)
    if not style_root.is_dir():
        raise ScannerError(f"missing style tree: {style_root}")

    canonical: dict[bytes, list[pathlib.Path]] = {}
    for path in sorted(style_root.glob("*.qml")):
        if not path.is_file():
            continue
        digest = hashlib.sha256(
            normalized_qml(path.read_text(encoding="utf-8"))
        ).digest()
        canonical.setdefault(digest, []).append(path)

    if not canonical:
        raise ScannerError(f"no canonical QML found in {style_root}")

    for path in qml_files(inputs):
        if canonical_style_link(path, style_root):
            continue
        digest = hashlib.sha256(
            normalized_qml(path.read_text(encoding="utf-8"))
        ).digest()
        matches = canonical.get(digest, [])
        for source in matches:
            print(f"{path}: structural copy of {source}; use the shared link")


def check_shared_style_links(style_root_raw: str, inputs: Iterable[str]) -> None:
    style_root = pathlib.Path(style_root_raw)
    if not style_root.is_dir():
        raise ScannerError(f"missing style tree: {style_root}")

    errors: list[str] = []
    for path in qml_files(inputs):
        if not path.is_symlink():
            continue

        target = path.readlink()
        canonical = style_root / path.name
        if target.is_absolute():
            errors.append(f"{path}: the shared QML symlink must be relative")
            continue
        if not canonical.is_file():
            errors.append(
                f"{path}: the sibling component {canonical} does not exist"
            )
            continue
        if path.resolve(strict=True) != canonical.resolve(strict=True):
            errors.append(f"{path}: shared symlink points outside {canonical}")

    if errors:
        raise ScannerError("\n".join(errors))


def cmake_call(text: str, command: str, target: str) -> str:
    text = re.sub(r"(?m)#.*$", "", text)
    start = re.search(
        rf"\b{re.escape(command)}\s*\(\s*{re.escape(target)}(?:\s|$)", text
    )
    if not start:
        raise ScannerError(f"could not find {command}({target} ...)")

    opening = text.find("(", start.start())
    depth = 0
    quote: str | None = None
    escaped = False
    for offset in range(opening, len(text)):
        character = text[offset]
        if quote is not None:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                quote = None
            continue
        if character == '"':
            quote = character
        elif character == "(":
            depth += 1
        elif character == ")":
            depth -= 1
            if depth == 0:
                return text[opening + 1 : offset]
            if depth < 0:
                break
    raise ScannerError(f"incomplete {command}({target} ...) call")


def check_cmake_qml_registration(cmake_raw: str, qml_root_raw: str, target: str) -> None:
    cmake_path = pathlib.Path(cmake_raw)
    qml_root = pathlib.Path(qml_root_raw)
    if not cmake_path.is_file():
        raise ScannerError(f"missing CMakeLists: {cmake_path}")
    if not qml_root.is_dir():
        raise ScannerError(f"missing QML tree: {qml_root}")

    body = cmake_call(cmake_path.read_text(encoding="utf-8"), "qt_add_qml_module", target)
    try:
        tokens = shlex.split(body, comments=False, posix=True)
    except ValueError as error:
        raise ScannerError(f"{cmake_path}: invalid CMake arguments: {error}") from error

    if tokens.count("QML_FILES") != 1:
        raise ScannerError(f"{cmake_path}: expected exactly one QML_FILES section")
    start = tokens.index("QML_FILES") + 1

    registered: list[str] = []
    for token in tokens[start:]:
        if re.fullmatch(r"[A-Z][A-Z0-9_]*", token):
            break
        if not re.fullmatch(r"qml/[^\s$()]+\.qml", token):
            raise ScannerError(f"{cmake_path}: non-literal QML entry: {token}")
        registered.append(token)

    if not registered:
        raise ScannerError(f"{cmake_path}: QML_FILES is empty")
    if len(registered) != len(set(registered)):
        raise ScannerError(f"{cmake_path}: there are duplicate QML paths")

    base = cmake_path.parent
    actual = sorted(
        path.relative_to(base).as_posix() for path in qml_files([str(qml_root)])
    )
    errors = []
    for path in sorted(set(actual) - set(registered)):
        errors.append(f"{path}: plain QML missing from {cmake_path}")
    for path in sorted(set(registered) - set(actual)):
        errors.append(f"{cmake_path}: registers '{path}', but the file does not exist")
    if errors:
        raise ScannerError("\n".join(errors))


def scan_dependency_metadata() -> None:
    raw = sys.stdin.read()
    if not raw.strip():
        raise ScannerError("cargo metadata produced no JSON")
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ScannerError(f"cargo metadata produced invalid JSON: {error}") from error

    banned = re.compile(
        r"^(?:cxx[-_]qt.*|qmetaobject.*|qml.*|qt(?:6)?(?:[-_].*|types)?|"
        r"niri(?:[-_].*)?|gtk4?(?:[-_].*)?|gdk4?(?:[-_].*)?|libadwaita.*|"
        r"iced(?:[-_].*)?|slint(?:[-_].*)?|egui(?:[-_].*)?|eframe(?:[-_].*)?|"
        r"winit(?:[-_].*)?|tao(?:[-_].*)?|relm4(?:[-_].*)?|smithay(?:[-_].*)?|"
        r"wayland[-_]client|wayland[-_]protocols(?:[-_].*)?)$"
    )
    for package in data.get("packages", []):
        package_name = package.get("name", "<unknown>")
        for dependency in package.get("dependencies", []):
            name = dependency.get("name", "")
            if banned.match(name):
                alias = dependency.get("rename")
                suffix = f" (alias {alias})" if alias else ""
                print(f"{package_name}: {name}{suffix}")


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    auto_bindings = subparsers.add_parser("qml-auto-bindings")
    auto_bindings.add_argument("inputs", nargs="+")

    local_controls = subparsers.add_parser("local-controls")
    local_controls.add_argument("--style-root")
    local_controls.add_argument("inputs", nargs="+")

    style = subparsers.add_parser("qml-style-contract")
    style.add_argument("theme")
    style.add_argument("inputs", nargs="+")

    copies = subparsers.add_parser("style-copies")
    copies.add_argument("style_root")
    copies.add_argument("inputs", nargs="+")

    links = subparsers.add_parser("shared-style-links")
    links.add_argument("style_root")
    links.add_argument("inputs", nargs="+")

    cmake = subparsers.add_parser("cmake-qml-registration")
    cmake.add_argument("cmake")
    cmake.add_argument("qml_root")
    cmake.add_argument("target")

    history = subparsers.add_parser("baseline-history")
    history.add_argument("compare_ref")
    history.add_argument("baseline", type=pathlib.Path)
    history.add_argument("registry", type=pathlib.Path)
    history.add_argument("--root", type=pathlib.Path, default=pathlib.Path.cwd())

    subparsers.add_parser("dependency-metadata")
    return parser.parse_args()


def main() -> int:
    arguments = parse_arguments()
    try:
        if arguments.command == "qml-auto-bindings":
            scan_auto_bindings(arguments.inputs)
        elif arguments.command == "local-controls":
            scan_local_controls(arguments.inputs, arguments.style_root)
        elif arguments.command == "qml-style-contract":
            scan_qml_style_contract(arguments.theme, arguments.inputs)
        elif arguments.command == "style-copies":
            scan_style_copies(arguments.style_root, arguments.inputs)
        elif arguments.command == "shared-style-links":
            check_shared_style_links(arguments.style_root, arguments.inputs)
        elif arguments.command == "cmake-qml-registration":
            check_cmake_qml_registration(
                arguments.cmake, arguments.qml_root, arguments.target
            )
        elif arguments.command == "baseline-history":
            check_architecture_baseline_history(
                arguments.root.resolve(),
                arguments.compare_ref,
                arguments.baseline.resolve(),
                arguments.registry.resolve(),
            )
        else:
            scan_dependency_metadata()
    except (OSError, UnicodeError, ScannerError) as error:
        print(f"architecture scanner: ERROR: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
