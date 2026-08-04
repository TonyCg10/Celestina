#!/usr/bin/env python3
"""Enforce English canonical sources and a decreasing legacy-language ratchet."""

# language-contract: allow-non-english
# The detector necessarily contains the non-English samples it rejects.

from __future__ import annotations

import argparse
import os
import re
import subprocess
from pathlib import Path


TEXT_SUFFIXES = {
    ".cc", ".cpp", ".desktop", ".h", ".hh", ".hpp", ".json", ".kdl",
    ".md", ".py", ".qml", ".rs", ".service", ".sh", ".toml", ".txt",
    ".yaml", ".yml",
}
ACCENTED_SPANISH = re.compile(r"[áéíóúüñÁÉÍÓÚÜÑ¿¡]")
SPANISH_WORDS = re.compile(
    r"\b(?:agente|agentes|archivo|archivos|cambio|cambios|carpeta|comando|"
    r"compilar|desplegar|despues|ejecuta|ejecutar|entrada|espanol|estado|"
    r"evidencia|falta|fallo|hito|hitos|idioma|lineas|ninguna|proyecto|prueba|"
    r"pruebas|repositorio|regla|reglas|ruta|rutas|salida|verificacion)\b",
    re.IGNORECASE,
)
LOCALE_DESKTOP = re.compile(r"^[A-Za-z][A-Za-z0-9-]*\[[A-Za-z_@.-]+\]=")
# Product copy, per ADR 0007. Only these two forms are user-visible text; the
# comments, identifiers and diagnostics around them are still development truth
# and are still scanned.
QSTR_LITERAL = re.compile(
    r"""qsTr\s*\(\s*("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')""", re.DOTALL
)
PRODUCT_COPY_MARKER = "language-contract: product-copy"
STRING_LITERAL = re.compile(r"""("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')""")


def is_localization(path: str) -> bool:
    parts = set(Path(path).parts)
    return bool(parts & {"i18n", "l10n", "locale", "locales", "translations"})


def is_history(path: str) -> bool:
    return "/docs/history/" in f"/{path}" or "/docs/plans/archive/" in f"/{path}"


def is_canonical(path: str) -> bool:
    p = Path(path)
    if p.name == "AGENTS.md" or path == "CONTRIBUTING.md":
        return True
    if path.startswith(".github/workflows/"):
        return True
    if path in {"docs/README.md", "docs/VISION.md", "docs/projects.toml"}:
        return True
    if any(path.startswith(f"docs/{name}/") for name in (
        "contracts", "decisions", "discussions", "governance", "standards", "templates"
    )):
        return True
    if "/docs/plans/active/" in f"/{path}":
        return True
    if len(p.parts) <= 2 and p.name in {"README.md", "STATUS.md", "ROADMAP.md", "VALIDATION.md"}:
        return True
    return False


def suspicious_lines(text: str, *, suffix: str = "") -> list[int]:
    head = "\n".join(text.splitlines()[:10])
    if "language-contract: allow-non-english" in head:
        return []
    # A marked file declares that its string literals are what a person reads.
    # Everything outside a literal in that file is still development truth.
    product_copy = PRODUCT_COPY_MARKER in head
    if suffix == ".qml":
        # Only the argument of qsTr() is product copy. A bare literal in QML is
        # a state token, an icon name or a path — development truth. Blanked
        # over the whole text rather than line by line, because a wrapped call
        # puts the literal on the line after `qsTr(`; the replacement keeps the
        # newlines so reported line numbers still point at the real source.
        text = QSTR_LITERAL.sub(lambda m: "\n" * m.group(0).count("\n"), text)
    result: list[int] = []
    for number, line in enumerate(text.splitlines(), 1):
        if LOCALE_DESKTOP.match(line):
            continue
        if product_copy and suffix != ".qml":
            line = STRING_LITERAL.sub("", line)
        if ACCENTED_SPANISH.search(line) or len(SPANISH_WORDS.findall(line)) >= 2:
            result.append(number)
    return result


def repository_paths(root: Path) -> list[str]:
    output = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=root,
        text=True,
    )
    return sorted(set(output.splitlines()))


def scan(root: Path) -> tuple[dict[str, int], list[str]]:
    legacy: dict[str, int] = {}
    errors: list[str] = []
    for relative in repository_paths(root):
        path = root / relative
        if path.is_symlink() or not path.is_file() or path.suffix.lower() not in TEXT_SUFFIXES:
            continue
        if is_localization(relative) or is_history(relative):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        lines = suspicious_lines(text, suffix=path.suffix.lower())
        if not lines:
            continue
        if is_canonical(relative):
            preview = ", ".join(str(item) for item in lines[:8])
            errors.append(f"{relative}: non-English canonical text at line(s) {preview}")
        else:
            legacy[relative] = len(lines)
    return legacy, errors


def read_baseline(path: Path) -> dict[str, int]:
    result: dict[str, int] = {}
    for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not raw or raw.startswith("#"):
            continue
        fields = raw.split("\t")
        if len(fields) != 2 or not fields[0].isdigit() or not fields[1]:
            raise ValueError(f"{path}:{number}: invalid language baseline row")
        count, relative = int(fields[0]), fields[1]
        if count <= 0 or relative in result:
            raise ValueError(f"{path}:{number}: invalid or duplicate language debt")
        result[relative] = count
    return result


def read_historical_baseline(root: Path, revision: str) -> dict[str, int] | None:
    result = subprocess.run(
        ["git", "show", f"{revision}:scripts/language-baseline.tsv"],
        cwd=root,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        return None
    parsed: dict[str, int] = {}
    for number, raw in enumerate(result.stdout.splitlines(), 1):
        if not raw or raw.startswith("#"):
            continue
        fields = raw.split("\t")
        if len(fields) != 2 or not fields[0].isdigit() or not fields[1]:
            raise ValueError(f"{revision}:language-baseline:{number}: invalid row")
        parsed[fields[1]] = int(fields[0])
    return parsed


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--write-baseline", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    baseline_path = root / "scripts/language-baseline.tsv"
    legacy, errors = scan(root)

    if args.write_baseline:
        rows = [
            "# Legacy non-English line ratchet. English is mandatory for new content.",
            "# suspicious_lines<TAB>path",
            *[f"{count}\t{path}" for path, count in sorted(legacy.items())],
        ]
        baseline_path.write_text("\n".join(rows) + "\n", encoding="utf-8")
        return 0

    if not baseline_path.is_file():
        errors.append("scripts/language-baseline.tsv: missing baseline")
        baseline: dict[str, int] = {}
    else:
        try:
            baseline = read_baseline(baseline_path)
        except ValueError as error:
            errors.append(str(error))
            baseline = {}

    compare_ref = os.environ.get("LANGUAGE_COMPARE_REF", "")
    if compare_ref:
        try:
            historical = read_historical_baseline(root, compare_ref)
        except ValueError as error:
            errors.append(str(error))
            historical = None
        if historical is not None:
            for path, count in baseline.items():
                old = historical.get(path)
                if old is None:
                    errors.append(f"{path}: new legacy-language baseline entry is forbidden")
                elif count > old:
                    errors.append(f"{path}: baseline increased from {old} to {count}")

    for path, count in legacy.items():
        expected = baseline.get(path)
        if expected is None:
            errors.append(f"{path}: new non-English repository text ({count} suspicious line(s))")
        elif count > expected:
            errors.append(f"{path}: language debt grew from {expected} to {count} line(s)")
        elif count < expected:
            errors.append(f"{path}: language debt fell from {expected} to {count}; lower the baseline")
    for path in sorted(set(baseline) - set(legacy)):
        errors.append(f"{path}: language debt is gone; remove its baseline row")

    if errors:
        for error in errors:
            print(f"language-contract: ERROR: {error}")
        return 1
    print(f"Language contract: OK ({len(legacy)} legacy file(s) ratcheted)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
