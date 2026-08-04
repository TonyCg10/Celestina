#!/usr/bin/env python3
"""Inspect and update versions governed by the Celestina version contract."""

from __future__ import annotations

import argparse
from collections.abc import Mapping, Sequence
import os
from pathlib import Path
import re
import sys
import tempfile
import tomllib

if __package__:
    from .version_contract import (
        DELIVERY_KINDS,
        HistoryRow,
        SemVer,
        SourceSpec,
        VersionContractError,
        VersionHistoryError,
        VersionRegistryError,
        VersionSourceError,
        VersionTransitionError,
        appended_history_rows,
        parse_registry,
        read_source_version,
        validate_snapshot,
    )
else:
    from version_contract import (
        DELIVERY_KINDS,
        HistoryRow,
        SemVer,
        SourceSpec,
        VersionContractError,
        VersionHistoryError,
        VersionRegistryError,
        VersionSourceError,
        VersionTransitionError,
        appended_history_rows,
        parse_registry,
        read_source_version,
        validate_snapshot,
    )


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_REGISTRY = "docs/projects.toml"


def load_registry(path: Path) -> dict[str, object]:
    try:
        with path.open("rb") as handle:
            data = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise VersionRegistryError(f"cannot read {path}: {error}") from error
    if not isinstance(data, dict):
        raise VersionRegistryError(f"{path} must contain a TOML table")
    return data


def worktree_snapshot(
    root: Path, registry: Mapping[str, object], registry_label: str
):
    model = parse_registry(registry, registry_label)

    def read_worktree(_revision: str, path: str) -> bytes | None:
        try:
            return (root / path).read_bytes()
        except OSError:
            return None

    return model, validate_snapshot(model, "WORKTREE", read_worktree)


def replace_source_version(
    spec: SourceSpec, raw: bytes, old: SemVer, new: SemVer, label: str
) -> bytes:
    current = read_source_version(spec, raw, label)
    if current != old:
        raise VersionSourceError(f"{label}: expected {old}, found {current}")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise VersionSourceError(f"{label}: source is not UTF-8: {error}") from error
    replacement = str(new)

    if spec.kind == "cargo-package":
        section = re.search(
            r"(?ms)^\[package\][ \t]*\r?$.*?(?=^\[|\Z)", text
        )
        if section is None:
            raise VersionSourceError(f"{label}: missing [package] section")
        matches = list(
            re.finditer(
                r'(?m)^[ \t]*version[ \t]*=[ \t]*"(?P<version>[^"]+)"'
                r"[ \t]*(?:#.*)?$",
                section.group(0),
            )
        )
        if len(matches) != 1:
            raise VersionSourceError(
                f"{label}: expected one [package].version assignment"
            )
        match = matches[0]
        start = section.start() + match.start("version")
        end = section.start() + match.end("version")
        return (text[:start] + replacement + text[end:]).encode("utf-8")

    if spec.kind == "cargo-lock":
        block_starts = [
            match.start()
            for match in re.finditer(r"(?m)^\[\[package\]\][ \t]*\r?$", text)
        ]
        block_starts.append(len(text))
        candidates: list[tuple[int, int]] = []
        for start, end in zip(block_starts, block_starts[1:]):
            block = text[start:end]
            name_match = re.search(
                r'(?m)^name[ \t]*=[ \t]*"(?P<name>[^"]+)"[ \t]*$', block
            )
            if name_match is None or name_match.group("name") != spec.name:
                continue
            version_match = re.search(
                r'(?m)^version[ \t]*=[ \t]*"(?P<version>[^"]+)"[ \t]*$',
                block,
            )
            if version_match is None:
                raise VersionSourceError(
                    f'{label}: package "{spec.name}" has no version assignment'
                )
            candidates.append(
                (
                    start + version_match.start("version"),
                    start + version_match.end("version"),
                )
            )
        if len(candidates) != 1:
            raise VersionSourceError(
                f'{label}: expected one lock package "{spec.name}", '
                f"found {len(candidates)}"
            )
        start, end = candidates[0]
        return (text[:start] + replacement + text[end:]).encode("utf-8")

    project_call_re = re.compile(
        r"^[ \t]*project[ \t]*\([ \t]*"
        r"(?P<name>[A-Za-z0-9_.+\-]+)(?P<body>.*?)\)",
        re.IGNORECASE | re.MULTILINE | re.DOTALL,
    )
    project_version_re = re.compile(
        r"\bVERSION[ \t\r\n]+(?P<version>[^ \t\r\n\)]+)",
        re.IGNORECASE,
    )
    candidates = []
    for call in project_call_re.finditer(text):
        if spec.name is not None and call.group("name") != spec.name:
            continue
        version_match = project_version_re.search(call.group("body"))
        if version_match is None:
            continue
        body_start = call.start("body")
        candidates.append(
            (
                body_start + version_match.start("version"),
                body_start + version_match.end("version"),
            )
        )
    if len(candidates) != 1:
        raise VersionSourceError(f"{label}: expected one CMake project VERSION")
    start, end = candidates[0]
    return (text[:start] + replacement + text[end:]).encode("utf-8")


def atomic_write(path: Path, raw: bytes) -> None:
    try:
        mode = path.stat().st_mode & 0o7777
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{path.name}.", dir=path.parent
        )
        try:
            with os.fdopen(descriptor, "wb") as handle:
                handle.write(raw)
                handle.flush()
                os.fsync(handle.fileno())
            os.chmod(temporary_name, mode)
            os.replace(temporary_name, path)
        except Exception:
            try:
                os.unlink(temporary_name)
            except OSError:
                pass
            raise
    except OSError as error:
        raise VersionSourceError(f"cannot update {path}: {error}") from error


def bump_repository(
    root: Path,
    registry: Mapping[str, object],
    registry_label: str,
    owner_name: str,
    kind: str,
    unit: str,
    summary: str,
) -> tuple[SemVer, SemVer]:
    if not isinstance(kind, str) or kind not in DELIVERY_KINDS:
        raise VersionTransitionError(
            "bump kind must be bug, milestone, or release"
        )
    for value, label in ((unit, "unit"), (summary, "summary")):
        if (
            not value
            or value != value.strip()
            or "\t" in value
            or "\n" in value
            or "\r" in value
        ):
            raise VersionTransitionError(
                f"{label} must be one trimmed TSV-safe line"
            )
    model, snapshot = worktree_snapshot(root, registry, registry_label)
    if model.policy is None:
        raise VersionRegistryError("registry has no [version_policy]")
    owner = model.owner_map().get(owner_name)
    if owner is None:
        raise VersionTransitionError(f'unknown version owner "{owner_name}"')
    if not owner.versioned:
        raise VersionTransitionError(f'owner "{owner_name}" is unversioned')
    old = snapshot.versions[owner_name]
    new = old.bumped(kind)

    updates: dict[str, bytes] = {}
    for spec in owner.sources:
        path = root / spec.path
        raw = updates.get(spec.path)
        if raw is None:
            try:
                raw = path.read_bytes()
            except OSError as error:
                raise VersionSourceError(f"cannot read {path}: {error}") from error
        updates[spec.path] = replace_source_version(
            spec, raw, old, new, str(path)
        )

    history_path = root / model.policy.history_file
    try:
        history_raw = history_path.read_bytes()
    except OSError as error:
        raise VersionHistoryError(f"cannot read {history_path}: {error}") from error
    separator = (
        b""
        if not history_raw or history_raw.endswith((b"\n", b"\r"))
        else b"\n"
    )
    row = (
        f"{owner_name}\t{new}\t{kind}\t{unit}\t{summary}\n".encode("utf-8")
    )
    updates[model.policy.history_file] = history_raw + separator + row

    def read_updated(_revision: str, path: str) -> bytes | None:
        if path in updates:
            return updates[path]
        try:
            return (root / path).read_bytes()
        except OSError:
            return None

    updated_snapshot = validate_snapshot(model, "UPDATED", read_updated)
    new_rows = appended_history_rows(
        snapshot, updated_snapshot, model.policy.history_file
    )
    expected_row = HistoryRow(owner_name, new, kind, unit, summary)
    if len(new_rows) != 1 or new_rows[0] != expected_row:
        raise VersionHistoryError("generated history row failed self-validation")

    for relative, raw in updates.items():
        atomic_write(root / relative, raw)
    return old, new


def command_root(args: argparse.Namespace) -> tuple[Path, Path]:
    root = args.root.resolve()
    registry = Path(args.registry)
    if not registry.is_absolute():
        registry = root / registry
    return root, registry


def add_subcommand_paths(parser: argparse.ArgumentParser) -> None:
    # Suppressed defaults preserve values supplied before the subcommand while
    # still accepting the more natural ``check --root ...`` spelling.
    parser.add_argument("--root", type=Path, default=argparse.SUPPRESS)
    parser.add_argument("--registry", default=argparse.SUPPRESS)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--registry", default=DEFAULT_REGISTRY)
    subparsers = parser.add_subparsers(dest="command", required=True)
    check_parser = subparsers.add_parser(
        "check", help="validate sources and history"
    )
    add_subcommand_paths(check_parser)
    show_parser = subparsers.add_parser(
        "show", help="print current owner versions"
    )
    add_subcommand_paths(show_parser)
    bump_parser = subparsers.add_parser(
        "bump", help="bump one owner and append history"
    )
    bump_parser.add_argument("owner")
    bump_parser.add_argument("kind", choices=sorted(DELIVERY_KINDS))
    bump_parser.add_argument("--unit", required=True)
    bump_parser.add_argument("--summary", required=True)
    add_subcommand_paths(bump_parser)
    args = parser.parse_args(argv)

    try:
        root, registry_path = command_root(args)
        registry = load_registry(registry_path)
        model, snapshot = worktree_snapshot(
            root, registry, str(registry_path)
        )
        if args.command == "check":
            print(f"version-contract: OK ({len(snapshot.versions)} owners)")
            return 0
        if args.command == "show":
            for owner in model.owners:
                if owner.versioned:
                    print(f"{owner.owner}\t{snapshot.versions[owner.owner]}")
            return 0
        old, new = bump_repository(
            root,
            registry,
            str(registry_path),
            args.owner,
            args.kind,
            args.unit,
            args.summary,
        )
        print(f"{args.owner}: {old} -> {new} ({args.kind})")
        return 0
    except VersionContractError as error:
        print(f"version-contract: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
