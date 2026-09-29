#!/usr/bin/env python3
"""Enforce English canonical sources and a decreasing legacy-language ratchet."""

# language-contract: allow-non-english
# The detector necessarily contains the non-English samples it rejects.

from __future__ import annotations

import argparse
import os
import re
from pathlib import Path

import repo_git


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
ALLOW_MARKER = "language-contract: allow-non-english"
STRING_LITERAL = re.compile(r"""("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')""")
# Where each exemption marker counts (TOOL-11). `product-copy` declares that a
# Rust or C++ file's string literals are what a person reads, as ADR 0007
# says; `allow-non-english` labels a fixture or detector that needs foreign
# input, which is code, never a document. Neither counts in a canonical path,
# and a marker anywhere else changes nothing, so it cannot park prose.
PRODUCT_COPY_SUFFIXES = frozenset({".rs", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp"})
DOCUMENT_SUFFIXES = frozenset({".md", ".txt"})
# A plan or evidence record cites product copy the way code holds it: as a
# string literal inside a closed fenced block or inside an inline code span
# that does not cross a blank line. Only those literals are blanked, and never
# in a canonical path; the prose around them, a qsTr() call in prose and the
# text after a fence that never closes are scanned like any other line.
FENCE = re.compile(r"^\s*(`{3,}|~{3,})")
CODE_SPAN = re.compile(r"(`+)((?:[^`\n]|\n(?![ \t]*\n))+?)\1")


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


def blank_literals(text: str) -> str:
    """`text` with every string literal emptied; newlines inside one are kept."""
    return STRING_LITERAL.sub(
        lambda match: '""' + "\n" * match.group(0).count("\n"), text
    )


def markdown_without_cited_copy(text: str) -> str:
    """Markdown with the product copy it cites in code blanked, line numbers kept."""
    lines = text.split("\n")
    prose: list[int] = []
    index = 0
    while index < len(lines):
        opening = FENCE.match(lines[index])
        if opening is None:
            prose.append(index)
            index += 1
            continue
        marker = opening.group(1)
        closing = next(
            (
                later
                for later in range(index + 1, len(lines))
                if FENCE.match(lines[later]) and lines[later].strip().startswith(marker)
            ),
            None,
        )
        if closing is None:
            # An unclosed fence cites nothing: the rest is prose.
            prose.extend(range(index, len(lines)))
            break
        for inside in range(index + 1, closing):
            lines[inside] = blank_literals(lines[inside])
        index = closing + 1
    # Inline spans, within each run of prose lines.
    runs: list[list[int]] = []
    for number in prose:
        if runs and runs[-1][-1] == number - 1:
            runs[-1].append(number)
        else:
            runs.append([number])
    for run in runs:
        joined = "\n".join(lines[number] for number in run)
        joined = CODE_SPAN.sub(
            lambda match: match.group(1) + blank_literals(match.group(2)) + match.group(1),
            joined,
        )
        for number, line in zip(run, joined.split("\n")):
            lines[number] = line
    return "\n".join(lines)


def honoured_markers(head: str, suffix: str, path: str) -> tuple[bool, bool]:
    """Whether the head's `allow-non-english` and `product-copy` markers count here."""
    if path and is_canonical(path):
        return False, False
    allow = ALLOW_MARKER in head and suffix not in DOCUMENT_SUFFIXES
    product_copy = PRODUCT_COPY_MARKER in head and suffix in PRODUCT_COPY_SUFFIXES
    return allow, product_copy


def suspicious_lines(text: str, *, suffix: str = "", path: str = "") -> list[int]:
    head = "\n".join(text.splitlines()[:10])
    allow, product_copy = honoured_markers(head, suffix, path)
    if allow:
        return []
    # A marked file declares that its string literals are what a person reads.
    # Everything outside a literal in that file is still development truth.
    if suffix == ".md" and not (path and is_canonical(path)):
        text = markdown_without_cited_copy(text)
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
        if product_copy:
            line = STRING_LITERAL.sub("", line)
        if ACCENTED_SPANISH.search(line) or len(SPANISH_WORDS.findall(line)) >= 2:
            result.append(number)
    return result


def repository_paths(root: Path) -> list[str]:
    output = repo_git.output(root, "ls-files", "-z", "--cached", "--others", "--exclude-standard")
    return sorted(set(repo_git.paths(output)))


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
        lines = suspicious_lines(text, suffix=path.suffix.lower(), path=relative)
        if not lines:
            continue
        if is_canonical(relative):
            preview = ", ".join(str(item) for item in lines[:8])
            errors.append(f"{relative}: non-English canonical text at line(s) {preview}")
        else:
            legacy[relative] = len(lines)
    return legacy, errors


def parse_baseline(text: str, source: str) -> dict[str, int]:
    """The one parser of the language ratchet: `count<TAB>path` rows (TOOL-18).

    The repository scan, the history comparison and the commit hook
    (commit_scope.py, through HEAD's copy of this module) all read it here.
    """
    result: dict[str, int] = {}
    for number, raw in enumerate(text.splitlines(), 1):
        if not raw or raw.startswith("#"):
            continue
        fields = raw.split("\t")
        if len(fields) != 2 or not fields[0].isdigit() or not fields[1]:
            raise ValueError(f"{source}:{number}: invalid language baseline row")
        count, relative = int(fields[0]), fields[1]
        if count <= 0 or relative in result:
            raise ValueError(f"{source}:{number}: invalid or duplicate language debt")
        result[relative] = count
    return result


def read_baseline(path: Path) -> dict[str, int]:
    return parse_baseline(path.read_text(encoding="utf-8"), str(path))


def compare_ref_resolves(root: Path, revision: str) -> bool:
    """Whether the comparison ref names a commit this checkout actually has."""
    return repo_git.run(root, "rev-parse", "--verify", "--quiet", f"{revision}^{{commit}}").returncode == 0


def historical_baseline_exists(root: Path, revision: str) -> bool:
    return repo_git.run(root, "cat-file", "-e", f"{revision}:scripts/language-baseline.tsv").returncode == 0


def read_historical_baseline(root: Path, revision: str) -> dict[str, int]:
    result = repo_git.run(root, "show", f"{revision}:scripts/language-baseline.tsv")
    if result.returncode != 0:
        raise ValueError(
            f"could not read scripts/language-baseline.tsv at {revision}"
        )
    return parse_baseline(
        repo_git.decode(result.stdout), f"{revision}:scripts/language-baseline.tsv"
    )


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
        # Fail closed, exactly as check_baseline_history does in the
        # architecture guard. A ref that is set but cannot be resolved used to
        # skip the monotonicity check and still print OK, so the ratchet
        # disappeared silently. CI passes `github.event.before`, which is all
        # zeros when a branch is created, so this was reachable in practice.
        if not compare_ref_resolves(root, compare_ref):
            errors.append(
                f"cannot resolve LANGUAGE_COMPARE_REF={compare_ref}; "
                "history is missing to protect the baseline"
            )
        elif not historical_baseline_exists(root, compare_ref):
            print(f"language-contract: initial baseline; no history at {compare_ref}")
        else:
            try:
                historical = read_historical_baseline(root, compare_ref)
            except ValueError as error:
                errors.append(str(error))
            else:
                for path, count in baseline.items():
                    old = historical.get(path)
                    if old is None:
                        errors.append(
                            f"{path}: new legacy-language baseline entry is forbidden"
                        )
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
