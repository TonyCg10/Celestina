#!/usr/bin/env python3
"""Prune the debug builds from the production build trees once the landing deployed.

    build_trees.py [--root DIR] [--registry PATH] prune [--dry-run]

A registered project builds its release artifact into a Cargo `target/` or a
CMake or Gradle `build/` directory. Most of a Cargo target is the debug
profile that tests, clippy and quick runs build: tens of gigabytes the
installed copy never uses. The release profile is the incremental cache the
next production build reuses, so a small change such as a colour recompiles
one crate and relinks instead of rebuilding every dependency. Pruning
therefore removes each Cargo `debug` profile directory directly under a build
root or one level below it (a target triple, or a nested target such as
`celestina-rs/target/workspace`), and keeps everything else. A registered
artifact or manifest inside a debug directory is kept too, so `check`,
`deploy-production.sh` and `status-production.sh` always judge the same bytes.

The build roots are derived from the registry: the first `target` or `build`
component of each artifact path or manifest of a project that is not halted.
A halted project's own tree is never touched (AGENTS.md "Halted projects").

Exit 0 on success, 1 when the tool refuses (a session worktree, a registry it
cannot read, a path outside the checkout), 2 on usage errors.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import sys
from typing import Any

from project_registry import load_registry


BUILD_DIRECTORY_NAMES = ("target", "build")
PRUNED_PROFILE = "debug"
# Written by scripts/worktree.sh at the root of every session worktree.
WORKTREE_MARKER = ".celestina-worktree"


class PruneError(RuntimeError):
    """The registry or the checkout does not allow a prune."""


@dataclass
class PruneReport:
    removed: list[str] = field(default_factory=list)
    freed_bytes: int = 0


def relative_path(text: object, label: str) -> PurePosixPath:
    if not isinstance(text, str) or not text:
        raise PruneError(f"{label} is not a path")
    path = PurePosixPath(text)
    if path.is_absolute() or ".." in path.parts:
        raise PruneError(f"{label} leaves the checkout: {text}")
    return path


def build_root(path: PurePosixPath) -> str | None:
    """The first `target` or `build` directory above `path`, or None."""
    for index, part in enumerate(path.parts[:-1]):
        if part in BUILD_DIRECTORY_NAMES:
            return PurePosixPath(*path.parts[: index + 1]).as_posix()
    return None


def registered_paths(project: dict[str, Any]) -> list[PurePosixPath]:
    identifier = project.get("id")
    paths = [
        relative_path(item, f"{identifier}: artifact_paths")
        for item in project.get("artifact_paths", [])
    ]
    if "artifact_manifest" in project:
        paths.append(relative_path(project["artifact_manifest"], f"{identifier}: artifact_manifest"))
    return paths


def projects(registry: dict[str, Any]) -> list[dict[str, Any]]:
    return [item for item in registry.get("projects", []) if isinstance(item, dict)]


def build_roots(registry: dict[str, Any]) -> tuple[str, ...]:
    """The build directories of every project that is not halted, outermost only."""
    roots: set[str] = set()
    for project in projects(registry):
        paths = registered_paths(project)
        if "halted" not in project:
            roots |= {root for path in paths if (root := build_root(path))}
    ordered = sorted(roots)
    return tuple(
        root
        for root in ordered
        if not any(root != other and root.startswith(other + "/") for other in ordered)
    )


def kept_paths(registry: dict[str, Any]) -> set[str]:
    return {path.as_posix() for project in projects(registry) for path in registered_paths(project)}


def real_directory(path: Path) -> bool:
    return path.is_dir() and not path.is_symlink()


def debug_directories(root: Path, build: str) -> list[str]:
    """The debug profile directories directly under `build` or one level below."""
    base = root / build
    if not real_directory(base):
        return []
    found = []
    if real_directory(base / PRUNED_PROFILE):
        found.append(f"{build}/{PRUNED_PROFILE}")
    for entry in sorted(os.listdir(base)):
        # A symlinked entry would lead the prune out of the build tree.
        if (
            entry != PRUNED_PROFILE
            and real_directory(base / entry)
            and real_directory(base / entry / PRUNED_PROFILE)
        ):
            found.append(f"{build}/{entry}/{PRUNED_PROFILE}")
    return found


def tree_size(path: Path) -> int:
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode):
        return info.st_size
    total = 0
    for directory, subdirectories, files in os.walk(path, followlinks=False):
        for name in files + subdirectories:
            try:
                entry = os.lstat(os.path.join(directory, name))
            except FileNotFoundError:
                continue
            if not stat.S_ISDIR(entry.st_mode):
                total += entry.st_size
    return total


def remove(path: Path) -> None:
    if real_directory(path):
        shutil.rmtree(path)
    else:
        path.unlink()


def prune_directory(
    root: Path, relative: str, kept: set[str], report: PruneReport, dry_run: bool
) -> None:
    """Remove `relative` whole, or, when it holds a kept path, everything else in it."""
    path = root / relative
    if real_directory(path) and any(item.startswith(relative + "/") for item in kept):
        for entry in sorted(os.listdir(path)):
            child = f"{relative}/{entry}"
            if child not in kept:
                prune_directory(root, child, kept, report, dry_run)
        return
    report.freed_bytes += tree_size(path)
    report.removed.append(relative)
    if not dry_run:
        remove(path)


def prune(root: Path, registry: dict[str, Any], *, dry_run: bool = False) -> PruneReport:
    """Remove every Cargo debug profile under the build roots, keeping registered paths."""
    report = PruneReport()
    kept = kept_paths(registry)
    for build in build_roots(registry):
        for directory in debug_directories(root, build):
            prune_directory(root, directory, kept, report, dry_run)
    return report


def human_size(size: int) -> str:
    value = float(size)
    for unit in ("B", "KiB", "MiB", "GiB"):
        if value < 1024 or unit == "GiB":
            return f"{value:.1f} {unit}" if unit != "B" else f"{int(value)} B"
        value /= 1024
    raise AssertionError(size)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--registry", type=Path)
    subparsers = parser.add_subparsers(dest="command", required=True)
    command = subparsers.add_parser("prune")
    command.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    root = args.root.resolve()
    try:
        if (root / WORKTREE_MARKER).exists():
            raise PruneError(
                "this is a session worktree; its builds share the session Cargo target"
            )
        try:
            registry = load_registry(args.registry or root / "docs/projects.toml")
        except ValueError as error:
            raise PruneError(str(error)) from error
        report = prune(root, registry, dry_run=args.dry_run)
    except (PruneError, OSError) as error:
        print(f"build-trees: {error}", file=sys.stderr)
        return 1
    for path in report.removed:
        print(f"{'would remove' if args.dry_run else 'removed'} {path}")
    verb = "would free" if args.dry_run else "freed"
    print(f"build-trees: {verb} {human_size(report.freed_bytes)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
