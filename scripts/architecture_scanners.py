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
import sys
from collections.abc import Iterable, Iterator


class ScannerError(RuntimeError):
    """A scanner could not inspect its complete declared input."""


def qml_files(inputs: Iterable[str]) -> Iterator[pathlib.Path]:
    seen: set[pathlib.Path] = set()
    for raw in inputs:
        source = pathlib.Path(raw)
        if not source.exists() and not source.is_symlink():
            raise ScannerError(f"ruta QML ausente: {source}")
        if source.is_symlink() and not source.exists():
            raise ScannerError(f"symlink QML roto: {source}")

        candidates = [source] if source.is_file() else source.rglob("*.qml")
        for path in sorted(candidates):
            if path.is_symlink() and not path.exists():
                raise ScannerError(f"symlink QML roto: {path}")
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
        raise ScannerError(f"arbol de estilo ausente: {style_root}")

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
        "literal hexadecimal fuera del tema",
        re.compile(r"#[0-9a-fA-F]{3,8}(?![0-9a-fA-F])"),
    ),
    (
        "transformacion de color local",
        re.compile(r"\bQt\s*\.\s*(?:rgba|darker|lighter|tint)\s*\("),
    ),
    (
        "acceso directo a CelestinaTheme.ref",
        re.compile(r"\bCelestinaTheme\s*\.\s*ref(?:\W|$)"),
    ),
    ("duracion numerica directa", re.compile(r"\bduration\s*:\s*[0-9]", re.S)),
    (
        "curva de animacion directa",
        re.compile(r"\beasing\s*\.\s*type\s*:\s*Easing\s*\.", re.S),
    ),
    (
        "tamano tipografico numerico directo",
        re.compile(r"\bfont\s*\.\s*pixelSize\s*:\s*[0-9]", re.S),
    ),
    (
        "peso tipografico directo",
        re.compile(
            r"\bfont\s*\.\s*weight\s*:\s*[0-9]"
            r"|\bFont\s*\.\s*(?:Normal|Medium|DemiBold|Bold)"
        ),
    ),
    (
        "tracking tipografico numerico directo",
        re.compile(r"\bfont\s*\.\s*letterSpacing\s*:\s*[0-9]", re.S),
    ),
    ("radio numerico directo", re.compile(r"\bradius\s*:\s*[0-9]", re.S)),
    (
        "grosor de borde numerico directo",
        re.compile(
            r"\bborder\s*\.\s*width\s*:\s*[1-9][0-9]*(?:\.[0-9]+)?",
            re.S,
        ),
    ),
    (
        "padding numerico directo",
        re.compile(
            r"\b(?:leftPadding|rightPadding|topPadding|bottomPadding|padding)"
            r"\s*:\s*[1-9][0-9]*(?:\.[0-9]+)?",
            re.S,
        ),
    ),
    (
        "opacidad fraccional directa",
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
                f"{path}:{index + 1}: color nominal fuera del tema: "
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
        raise ScannerError(f"tema QML ausente: {theme}")
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
        raise ScannerError(f"arbol de estilo ausente: {style_root}")

    canonical: dict[bytes, list[pathlib.Path]] = {}
    for path in sorted(style_root.glob("*.qml")):
        if not path.is_file():
            continue
        digest = hashlib.sha256(
            normalized_qml(path.read_text(encoding="utf-8"))
        ).digest()
        canonical.setdefault(digest, []).append(path)

    if not canonical:
        raise ScannerError(f"no hay QML canonico en {style_root}")

    for path in qml_files(inputs):
        if canonical_style_link(path, style_root):
            continue
        digest = hashlib.sha256(
            normalized_qml(path.read_text(encoding="utf-8"))
        ).digest()
        matches = canonical.get(digest, [])
        for source in matches:
            print(f"{path}: copia estructural de {source}; use el enlace compartido")


def check_shared_style_links(style_root_raw: str, inputs: Iterable[str]) -> None:
    style_root = pathlib.Path(style_root_raw)
    if not style_root.is_dir():
        raise ScannerError(f"arbol de estilo ausente: {style_root}")

    errors: list[str] = []
    for path in qml_files(inputs):
        if not path.is_symlink():
            continue

        target = path.readlink()
        canonical = style_root / path.name
        if target.is_absolute():
            errors.append(f"{path}: el symlink QML compartido debe ser relativo")
            continue
        if not canonical.is_file():
            errors.append(
                f"{path}: no existe el componente homonimo {canonical}"
            )
            continue
        if path.resolve(strict=True) != canonical.resolve(strict=True):
            errors.append(f"{path}: symlink compartido apunta fuera de {canonical}")

    if errors:
        raise ScannerError("\n".join(errors))


def cmake_call(text: str, command: str, target: str) -> str:
    text = re.sub(r"(?m)#.*$", "", text)
    start = re.search(
        rf"\b{re.escape(command)}\s*\(\s*{re.escape(target)}(?:\s|$)", text
    )
    if not start:
        raise ScannerError(f"no se encontro {command}({target} ...)")

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
    raise ScannerError(f"llamada {command}({target} ...) incompleta")


def check_cmake_qml_registration(cmake_raw: str, qml_root_raw: str, target: str) -> None:
    cmake_path = pathlib.Path(cmake_raw)
    qml_root = pathlib.Path(qml_root_raw)
    if not cmake_path.is_file():
        raise ScannerError(f"CMakeLists ausente: {cmake_path}")
    if not qml_root.is_dir():
        raise ScannerError(f"arbol QML ausente: {qml_root}")

    body = cmake_call(cmake_path.read_text(encoding="utf-8"), "qt_add_qml_module", target)
    try:
        tokens = shlex.split(body, comments=False, posix=True)
    except ValueError as error:
        raise ScannerError(f"{cmake_path}: argumentos CMake invalidos: {error}") from error

    if tokens.count("QML_FILES") != 1:
        raise ScannerError(f"{cmake_path}: se esperaba una unica seccion QML_FILES")
    start = tokens.index("QML_FILES") + 1

    registered: list[str] = []
    for token in tokens[start:]:
        if re.fullmatch(r"[A-Z][A-Z0-9_]*", token):
            break
        if not re.fullmatch(r"qml/[^\s$()]+\.qml", token):
            raise ScannerError(f"{cmake_path}: entrada QML no literal: {token}")
        registered.append(token)

    if not registered:
        raise ScannerError(f"{cmake_path}: QML_FILES esta vacio")
    if len(registered) != len(set(registered)):
        raise ScannerError(f"{cmake_path}: hay rutas QML duplicadas")

    base = cmake_path.parent
    actual = sorted(
        path.relative_to(base).as_posix() for path in qml_files([str(qml_root)])
    )
    errors = []
    for path in sorted(set(actual) - set(registered)):
        errors.append(f"{path}: QML regular ausente de {cmake_path}")
    for path in sorted(set(registered) - set(actual)):
        errors.append(f"{cmake_path}: registra '{path}', pero el fichero no existe")
    if errors:
        raise ScannerError("\n".join(errors))


def scan_dependency_metadata() -> None:
    raw = sys.stdin.read()
    if not raw.strip():
        raise ScannerError("cargo metadata no produjo JSON")
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ScannerError(f"cargo metadata produjo JSON invalido: {error}") from error

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
        else:
            scan_dependency_metadata()
    except (OSError, UnicodeError, ScannerError) as error:
        print(f"architecture scanner: ERROR: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
