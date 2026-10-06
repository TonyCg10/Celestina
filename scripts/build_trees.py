#!/usr/bin/env python3
"""Prune the production build trees once the landing deployed their artifacts.

    build_trees.py [--root DIR] [--registry PATH] prune [--dry-run]

A registered project builds its release artifact into a Cargo `target/` or a
CMake or Gradle `build/` directory. Once the landing has deployed and checked
the installed copy, everything in those directories except the registered
artifacts and their manifests is a cache the next build regenerates: debug and
test builds, dependency objects, incremental state. Pruning keeps the
registered artifact paths and the manifest of every project, halted ones
included, so `check`, `deploy-production.sh` and `status-production.sh` keep
judging the same bytes, and removes the rest.

The build roots are derived from the registry: the first `target` or `build`
component of each artifact path or manifest of a project that is not halted.
A halted project's own tree is never touched (AGENTS.md "Halted projects").

A pruned project gets a mark beside its manifest. Its artifact stays current,
but a verification alone would find the generated sources it lints, such as
the release QML module, gone; scripts/land-unit.py therefore builds a marked
project before verifying it and clears the mark after that build.

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
MARK_SUFFIX = ".pruned"
# Written by scripts/worktree.sh at the root of every session worktree.
WORKTREE_MARKER = ".celestina-worktree"


class PruneError(RuntimeError):
    """The registry or the checkout does not allow a prune."""


@dataclass
class PruneReport:
    removed: list[str] = field(default_factory=list)
    freed_bytes: int = 0
    marked: list[str] = field(default_factory=list)


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


def project_roots(project: dict[str, Any]) -> set[str]:
    return {root for path in registered_paths(project) if (root := build_root(path))}


def build_roots(registry: dict[str, Any]) -> tuple[str, ...]:
    """The build directories of every project that is not halted, outermost only."""
    roots: set[str] = set()
    for project in projects(registry):
        if "halted" not in project:
            roots |= project_roots(project)
        else:
            # A halted project's paths are still validated: they are kept.
            registered_paths(project)
    ordered = sorted(roots)
    return tuple(
        root
        for root in ordered
        if not any(root != other and root.startswith(other + "/") for other in ordered)
    )


def mark_path(root: Path, project: dict[str, Any]) -> Path | None:
    manifest = project.get("artifact_manifest")
    if not isinstance(manifest, str) or not manifest:
        return None
    return root / (manifest + MARK_SUFFIX)


def is_pruned(root: Path, project: dict[str, Any]) -> bool:
    """Whether `project`'s build tree was pruned since its last build."""
    path = mark_path(root, project)
    return path is not None and path.is_file()


def clear_mark(root: Path, project: dict[str, Any]) -> None:
    path = mark_path(root, project)
    if path is not None:
        try:
            path.unlink()
        except FileNotFoundError:
            pass


def kept_paths(registry: dict[str, Any]) -> set[str]:
    kept: set[str] = set()
    for project in projects(registry):
        for path in registered_paths(project):
            kept.add(path.as_posix())
        manifest = project.get("artifact_manifest")
        if isinstance(manifest, str) and manifest:
            kept.add(manifest + MARK_SUFFIX)
    return kept


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
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
    else:
        path.unlink()


def prune_directory(
    root: Path, relative: str, kept: set[str], report: PruneReport, dry_run: bool
) -> None:
    directory = root / relative
    for entry in sorted(os.listdir(directory)):
        child = f"{relative}/{entry}"
        if child in kept:
            continue
        path = directory / entry
        if path.is_dir() and not path.is_symlink() and any(
            item.startswith(child + "/") for item in kept
        ):
            prune_directory(root, child, kept, report, dry_run)
            continue
        report.freed_bytes += tree_size(path)
        report.removed.append(child)
        if not dry_run:
            remove(path)


def prune(root: Path, registry: dict[str, Any], *, dry_run: bool = False) -> PruneReport:
    """Remove every file under the build roots that no project registers."""
    report = PruneReport()
    roots = build_roots(registry)
    kept = kept_paths(registry)
    present = [item for item in roots if (root / item).is_dir() and not (root / item).is_symlink()]
    for item in present:
        prune_directory(root, item, kept, report, dry_run)
    if dry_run:
        return report
    for project in projects(registry):
        if "halted" in project:
            continue
        if not any(item in present for item in project_roots(project)):
            continue
        path = mark_path(root, project)
        if path is None:
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("pruned; the next landing builds before it verifies\n", encoding="utf-8")
        report.marked.append(str(project.get("id")))
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
