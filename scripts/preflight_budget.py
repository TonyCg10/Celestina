#!/usr/bin/env python3
"""Byte budget for the documents a session reads before acting."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
from typing import Any

from project_registry import REGISTRY_PATH, halted_projects, load_registry


BASELINE = "scripts/preflight-baseline.tsv"
ROOT_ROW = "root-agents"
SUITE_ROW = "suite"


def _suite(registry: dict[str, Any]) -> dict[str, Any]:
    suite = registry.get("suite", {})
    return suite if isinstance(suite, dict) else {}


def _paths(table: dict[str, Any], key: str) -> list[str]:
    value = table.get(key)
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [item for item in value if isinstance(item, str)]
    return []


def now_documents(registry: dict[str, Any], project_id: str) -> list[str]:
    """The documents read before acting on `project_id`, deduplicated, in order."""
    suite = _suite(registry)
    if project_id == SUITE_ROW:
        keys = [(suite, "agents"), (suite, "status")]
    else:
        project = next(
            (
                item
                for item in registry.get("projects", ())
                if isinstance(item, dict) and item.get("id") == project_id
            ),
            {},
        )
        keys = [(suite, "agents"), (project, "agents"), (project, "status")]
    keys.append((suite, "now_rules"))
    ordered: list[str] = []
    for table, key in keys:
        for path in _paths(table, key):
            if path not in ordered:
                ordered.append(path)
    return ordered


def _size(root: Path, relative: str) -> int:
    try:
        return (root / relative).stat().st_size
    except OSError:
        return 0


def measure(root: Path, registry: dict[str, Any]) -> dict[str, int]:
    """Bytes read before acting, per row id; halted projects are left out."""
    halted = {project.id for project in halted_projects(registry)}
    rows = {ROOT_ROW: sum(_size(root, path) for path in _paths(_suite(registry), "agents"))}
    ids = [SUITE_ROW]
    for project in registry.get("projects", ()):
        if isinstance(project, dict) and isinstance(project.get("id"), str):
            if project["id"] not in halted:
                ids.append(project["id"])
    for project_id in ids:
        rows[project_id] = sum(_size(root, path) for path in now_documents(registry, project_id))
    return rows


def load_baseline(path: Path) -> dict[str, int]:
    rows: dict[str, int] = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 2 or not fields[0] or not (fields[1].isascii() and fields[1].isdigit()):
            raise ValueError(f"{path.name}:{number}: expected `id<TAB>bytes`")
        if fields[0] in rows:
            raise ValueError(f"{path.name}:{number}: duplicate row {fields[0]}")
        rows[fields[0]] = int(fields[1])
    return rows


def check(root: Path, registry: dict[str, Any]) -> list[str]:
    path = root / BASELINE
    if not path.exists():
        return []
    try:
        limits = load_baseline(path)
    except ValueError as error:
        return [f"preflight budget: {error}"]
    measured = measure(root, registry)
    messages: list[str] = []
    for row, size in measured.items():
        if row not in limits:
            messages.append(f"preflight budget: {row} has no row in {BASELINE}")
        elif size > limits[row]:
            messages.append(
                f"preflight budget: {row} reads {size} bytes before acting, over its limit of "
                f"{limits[row]}; move closed work out of its STATUS.md into docs/history, "
                "or shorten its AGENTS.md"
            )
    # A halted project is not measured and keeps its row for when the halt lifts.
    registered = {
        project.get("id") for project in registry.get("projects", ()) if isinstance(project, dict)
    }
    for row in limits:
        if row not in measured and row not in registered:
            messages.append(f"preflight budget: unknown row {row}")
    return messages


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("command", choices=["measure"])
    arguments = parser.parse_args(argv)
    root = arguments.root.resolve()
    registry = load_registry(root / REGISTRY_PATH)
    for row, size in sorted(measure(root, registry).items()):
        print(f"{row}\t{size}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
