#!/usr/bin/env python3
"""Print the canonical repository context for one path, in reading order."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys

from documentation_contract import (
    RegistryError,
    inside_root,
    load_registry,
    markdown_metadata,
    normalized_status,
    repository_root,
)


DOCUMENT_FIELDS = ("readme", "status", "roadmap", "validation")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", help="file or directory inside the repository")
    parser.add_argument(
        "--root",
        type=Path,
        default=repository_root(),
        help="repository root (defaults to the checkout containing this script)",
    )
    parser.add_argument("--json", action="store_true", help="emit a JSON array")
    return parser.parse_args(argv)


def is_within(relative: Path, registered: str) -> bool:
    base = Path(registered)
    return relative == base or base in relative.parents


def project_matches(relative: Path, project: dict[str, object]) -> int | None:
    roots: list[str] = []
    path = project.get("path")
    if isinstance(path, str):
        roots.append(path)
    source_roots = project.get("source_roots")
    if isinstance(source_roots, list):
        roots.extend(root for root in source_roots if isinstance(root, str))
    scores = [len(Path(root).parts) for root in roots if is_within(relative, root)]
    return max(scores) if scores else None


def selected_projects(relative: Path, registry: dict[str, object]) -> list[dict[str, object]]:
    candidates: list[tuple[int, dict[str, object]]] = []
    projects = registry.get("projects", [])
    assert isinstance(projects, list)
    for project in projects:
        if not isinstance(project, dict):
            continue
        score = project_matches(relative, project)
        if score is not None:
            candidates.append((score, project))
    if not candidates:
        return []
    return sorted(
        (project for _score, project in candidates),
        key=lambda project: (
            project_matches(relative, project) or 0,
            str(project.get("id", "")),
        ),
    )


def scope_matches(scope: str, projects: list[dict[str, object]]) -> bool:
    normalized = " ".join(scope.casefold().replace("`", "").split())
    tokens = set(re.findall(r"[a-z0-9][a-z0-9-]*", normalized))
    if not normalized or tokens.intersection({"suite", "all", "global"}):
        return True
    for project in projects:
        for field in ("id", "name"):
            value = project.get(field)
            if isinstance(value, str) and value.casefold() in normalized:
                return True
    return False


def add_path(
    result: list[Path],
    seen: set[Path],
    errors: list[str],
    root: Path,
    raw: object,
    label: str,
) -> None:
    if not isinstance(raw, str) or not raw:
        errors.append(f"{label}: ruta ausente en docs/projects.toml")
        return
    candidate = (root / raw).resolve(strict=False)
    if not inside_root(root, candidate):
        errors.append(f"{label}: ruta sale del repositorio: {raw}")
        return
    if not candidate.is_file():
        errors.append(f"{label}: archivo no existe: {raw}")
        return
    if candidate not in seen:
        seen.add(candidate)
        result.append(candidate)


def physical_agent_files(root: Path, target: Path) -> list[Path]:
    start = target if target.is_dir() else target.parent
    if not inside_root(root, start):
        return []
    relative = start.relative_to(root)
    agents: list[Path] = []
    for depth in range(0, len(relative.parts) + 1):
        directory = root.joinpath(*relative.parts[:depth])
        candidate = directory / "AGENTS.md"
        if candidate.is_file():
            agents.append(candidate.resolve())
    return agents


def relevant_contracts(
    root: Path, projects: list[dict[str, object]], errors: list[str]
) -> list[Path]:
    directory = root / "docs/contracts"
    if not directory.is_dir():
        errors.append("docs/contracts: directorio no existe")
        return []
    result: list[Path] = []
    for path in sorted(directory.glob("*.md")):
        if path.name == "README.md":
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"{path.relative_to(root)}: no se puede leer: {error}")
            continue
        scope = markdown_metadata(text).get("scope", "")
        if scope_matches(scope, projects):
            result.append(path.resolve())
    return result


def relevant_active_plans(
    root: Path,
    registry: dict[str, object],
    projects: list[dict[str, object]],
    relative_target: Path,
    errors: list[str],
) -> list[Path]:
    suite = registry.get("suite")
    assert isinstance(suite, dict)
    result: list[Path] = []
    target_label = relative_target.as_posix()
    owners = [("suite", suite), *[(str(project.get("id", "project")), project) for project in projects]]
    seen_directories: set[Path] = set()
    for owner_label, owner in owners:
        raw_directory = owner.get("active_plans")
        if raw_directory is None and owner_label != "suite":
            continue
        if not isinstance(raw_directory, str):
            errors.append(f"{owner_label}.active_plans: ruta ausente en docs/projects.toml")
            continue
        directory = (root / raw_directory).resolve(strict=False)
        if directory in seen_directories:
            continue
        seen_directories.add(directory)
        if not inside_root(root, directory) or not directory.is_dir():
            errors.append(f"{owner_label}.active_plans: directorio no existe: {raw_directory}")
            continue
        for path in sorted(directory.glob("*.md")):
            if path.name == "README.md":
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError) as error:
                errors.append(f"{path.relative_to(root)}: no se puede leer: {error}")
                continue
            fields = markdown_metadata(text)
            if normalized_status(fields.get("status", "")) != "active":
                continue
            scope = fields.get("scope", "")
            ledger_mentions_target = target_label not in {"", "."} and target_label in text
            project_path_mentioned = any(
                isinstance(project.get("path"), str) and str(project["path"]) in text
                for project in projects
            )
            if scope_matches(scope, projects) or ledger_mentions_target or project_path_mentioned:
                result.append(path.resolve())
    return result


def resolve_context(root: Path, raw_target: str) -> tuple[list[Path], list[str]]:
    errors: list[str] = []
    try:
        registry = load_registry(root)
    except RegistryError as error:
        return [], [str(error)]

    target = Path(raw_target)
    if not target.is_absolute():
        target = root / target
    target = target.resolve(strict=False)
    if not inside_root(root, target):
        return [], [f"path sale del repositorio: {raw_target}"]
    relative = target.relative_to(root)
    projects = selected_projects(relative, registry)

    result: list[Path] = []
    seen: set[Path] = set()
    suite = registry.get("suite")
    assert isinstance(suite, dict)

    add_path(result, seen, errors, root, suite.get("agents"), "suite.agents")
    for agent_file in physical_agent_files(root, target):
        if agent_file not in seen:
            seen.add(agent_file)
            result.append(agent_file)
    for project in projects:
        add_path(
            result,
            seen,
            errors,
            root,
            project.get("agents"),
            f"{project.get('id', 'project')}.agents",
        )

    document_owners = projects if projects else [suite]
    for owner in document_owners:
        owner_id = str(owner.get("id", "suite"))
        for field in DOCUMENT_FIELDS:
            add_path(result, seen, errors, root, owner.get(field), f"{owner_id}.{field}")

    for contract in relevant_contracts(root, projects, errors):
        if contract not in seen:
            seen.add(contract)
            result.append(contract)
    for plan in relevant_active_plans(root, registry, projects, relative, errors):
        if plan not in seen:
            seen.add(plan)
            result.append(plan)
    return result, errors


def main(argv: list[str] | None = None) -> int:
    arguments = parse_args(sys.argv[1:] if argv is None else argv)
    root = arguments.root.resolve()
    context, errors = resolve_context(root, arguments.path)
    if errors:
        for error in errors:
            print(f"agent-context: {error}", file=sys.stderr)
        return 1
    relative_context = [path.relative_to(root).as_posix() for path in context]
    if arguments.json:
        print(json.dumps(relative_context, ensure_ascii=False, indent=2))
    else:
        print("\n".join(relative_context))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
