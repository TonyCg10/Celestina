#!/usr/bin/env python3
"""Validate a commit subject and its paths against docs/projects.toml."""

from __future__ import annotations

import argparse
import subprocess
import sys
import tomllib
from pathlib import Path

from project_registry import (
    CommitScope,
    build_commit_scopes,
    parse_subject_prefix,
    path_allowed,
)


ROOT = Path(__file__).resolve().parent.parent


def fail(message: str) -> "None":
    print(f"commit-msg: {message}", file=sys.stderr)
    raise SystemExit(1)


def load_scopes(root: Path) -> dict[str, CommitScope]:
    with (root / "docs" / "projects.toml").open("rb") as handle:
        data = tomllib.load(handle)
    try:
        return build_commit_scopes(data)
    except (KeyError, TypeError, ValueError) as error:
        fail(str(error))


def git_output(root: Path, *args: str, check: bool = True) -> bytes:
    process = subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if check and process.returncode != 0:
        fail(f"git {' '.join(args)} no pudo ejecutarse")
    return process.stdout


def is_merge(root: Path) -> bool:
    return subprocess.run(
        ["git", "-C", str(root), "rev-parse", "-q", "--verify", "MERGE_HEAD"],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode == 0


def staged_paths(root: Path) -> list[str]:
    raw = git_output(root, "diff", "--cached", "--name-only", "--no-renames", "-z")
    return [part.decode("utf-8", "surrogateescape") for part in raw.split(b"\0") if part]


def stdin_paths() -> list[str]:
    raw = sys.stdin.buffer.read()
    if b"\0" in raw:
        parts = raw.split(b"\0")
    else:
        parts = raw.splitlines()
    return [part.decode("utf-8", "surrogateescape") for part in parts if part]


def read_subject(message_file: str) -> str:
    try:
        return Path(message_file).read_text(encoding="utf-8").splitlines()[0]
    except (OSError, IndexError, UnicodeError) as error:
        fail(f"no se pudo leer el asunto del commit: {error}")


def parse_subject(
    subject: str, scopes: dict[str, CommitScope]
) -> tuple[str, CommitScope]:
    try:
        normalized, _action = parse_subject_prefix(subject)
    except ValueError as error:
        expected = next(iter(scopes))
        fail(
            f"{error}; por ejemplo '{expected}: update repository contracts'"
        )

    scope = scopes.get(normalized)
    if scope is None:
        known = ", ".join(f"{name}:" for name in sorted(scopes))
        fail(f'prefijo desconocido "{normalized}:"; registrados: {known}')
    return normalized, scope


def validate(subject: str, paths: list[str], root: Path) -> str:
    scopes = load_scopes(root)
    prefix, scope = parse_subject(subject, scopes)
    if scope.allow_all:
        return prefix

    outside = [path for path in paths if not path_allowed(path, scope)]
    if not outside:
        return prefix

    print(
        f'commit-msg: el prefijo "{prefix}:" no cubre lo que este commit toca.\n',
        file=sys.stderr,
    )
    print("Fuera de alcance:", file=sys.stderr)
    for path in outside:
        print(f"  {path}", file=sys.stderr)
    print(f'\nAlcance de "{prefix}:":', file=sys.stderr)
    for root in scope.roots:
        print(f"  {root}", file=sys.stderr)
    for path in scope.files:
        print(f"  {path}", file=sys.stderr)
    print('\nParte el commit o usa "suite:" para una unidad transversal real.', file=sys.stderr)
    raise SystemExit(1)


def validate_staged_unit_prefix(root: Path, prefix: str) -> None:
    checker = Path(__file__).resolve().parent / "check-staged-units.py"
    process = subprocess.run(
        [
            sys.executable,
            str(checker),
            "--root",
            str(root),
            "--quiet",
            "--commit-prefix",
            prefix,
        ],
        check=False,
    )
    if process.returncode != 0:
        fail("el asunto no coincide con el lote de inventarios staged")


def reject_merge_delivery(root: Path) -> None:
    checker = Path(__file__).resolve().parent / "check-staged-units.py"
    process = subprocess.run(
        [
            sys.executable,
            str(checker),
            "--root",
            str(root),
            "--quiet",
            "--forbid-delivery",
        ],
        check=False,
    )
    if process.returncode != 0:
        fail("un merge no puede incorporar un lote de entrega inventariado")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("message_file", nargs="?")
    parser.add_argument("--check", metavar="SUBJECT")
    parser.add_argument("--check-index", metavar="SUBJECT")
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    root = args.root.resolve()

    if args.check is not None:
        validate(args.check, stdin_paths(), root)
        return

    if args.check_index is not None:
        prefix = validate(args.check_index, staged_paths(root), root)
        validate_staged_unit_prefix(root, prefix)
        return

    if not args.message_file:
        fail("falta el archivo del mensaje de commit")
    if is_merge(root):
        reject_merge_delivery(root)
        return
    prefix = validate(read_subject(args.message_file), staged_paths(root), root)
    validate_staged_unit_prefix(root, prefix)


if __name__ == "__main__":
    main()
