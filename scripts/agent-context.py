#!/usr/bin/env python3
"""Print the canonical repository context for one path, in reading order."""

from __future__ import annotations

import argparse
import json
import os
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


def projects_for_targets(
    relatives: list[Path], registry: dict[str, object]
) -> list[dict[str, object]]:
    """Select lexical owners first, then add resolved-target owners once."""
    result: list[dict[str, object]] = []
    seen: set[int] = set()
    for relative in relatives:
        for project in selected_projects(relative, registry):
            identity = id(project)
            if identity not in seen:
                seen.add(identity)
                result.append(project)
    return result


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
        errors.append(f"{label}: missing path in docs/projects.toml")
        return
    candidate = (root / raw).resolve(strict=False)
    if not inside_root(root, candidate):
        errors.append(f"{label}: path leaves the repository: {raw}")
        return
    if not candidate.is_file():
        errors.append(f"{label}: file does not exist: {raw}")
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


def shared_rule_documents(
    root: Path, registry: dict[str, object], errors: list[str]
) -> list[Path]:
    """Return the cross-cutting rules every local AGENTS.md already requires.

    Local contracts point at the workflow, governance and engineering standards
    but cannot enumerate them for a path, so the registry owns that list and
    this helper keeps the printed context complete instead of only sufficient.
    """
    suite = registry.get("suite")
    assert isinstance(suite, dict)
    if "shared_rules" not in suite:
        errors.append("suite.shared_rules: required in docs/projects.toml")
        return []
    raw_rules = suite.get("shared_rules")
    if not isinstance(raw_rules, list):
        errors.append("suite.shared_rules: must be a list in docs/projects.toml")
        return []
    if not raw_rules:
        errors.append("suite.shared_rules: must be a non-empty list in docs/projects.toml")
        return []
    result: list[Path] = []
    seen: set[Path] = set()
    for index, raw in enumerate(raw_rules):
        add_path(result, seen, errors, root, raw, f"suite.shared_rules[{index}]")
    return result


def owner_context_documents(
    root: Path, owner: dict[str, object], errors: list[str]
) -> list[Path]:
    """Return the explicit owner-local documents required for its paths."""
    owner_id = str(owner.get("id", "project"))
    if "context_documents" not in owner:
        errors.append(
            f"{owner_id}.context_documents: required list in docs/projects.toml"
        )
        return []
    raw_documents = owner.get("context_documents")
    if not isinstance(raw_documents, list):
        errors.append(
            f"{owner_id}.context_documents: must be a list in docs/projects.toml"
        )
        return []
    result: list[Path] = []
    seen: set[Path] = set()
    for index, raw in enumerate(raw_documents):
        add_path(
            result,
            seen,
            errors,
            root,
            raw,
            f"{owner_id}.context_documents[{index}]",
        )
    return result


def relevant_contracts(
    root: Path, projects: list[dict[str, object]], errors: list[str]
) -> list[Path]:
    directory = root / "docs/contracts"
    if not directory.is_dir():
        errors.append("docs/contracts: directory does not exist")
        return []
    result: list[Path] = []
    for path in sorted(directory.glob("*.md")):
        if path.name == "README.md":
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"{path.relative_to(root)}: cannot read: {error}")
            continue
        scope = markdown_metadata(text).get("scope", "")
        if scope_matches(scope, projects):
            result.append(path.resolve())
    return result


def relevant_active_plans(
    root: Path,
    registry: dict[str, object],
    projects: list[dict[str, object]],
    relative_targets: list[Path],
    errors: list[str],
) -> list[Path]:
    suite = registry.get("suite")
    assert isinstance(suite, dict)
    result: list[Path] = []
    target_labels = [target.as_posix() for target in relative_targets]
    owners = [("suite", suite), *[(str(project.get("id", "project")), project) for project in projects]]
    seen_directories: set[Path] = set()
    for owner_label, owner in owners:
        raw_directory = owner.get("active_plans")
        if raw_directory is None and owner_label != "suite":
            continue
        if not isinstance(raw_directory, str):
            errors.append(
                f"{owner_label}.active_plans: missing path in docs/projects.toml"
            )
            continue
        directory = (root / raw_directory).resolve(strict=False)
        if directory in seen_directories:
            continue
        seen_directories.add(directory)
        if not inside_root(root, directory) or not directory.is_dir():
            errors.append(
                f"{owner_label}.active_plans: directory does not exist: {raw_directory}"
            )
            continue
        for path in sorted(directory.glob("*.md")):
            if path.name == "README.md":
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError) as error:
                errors.append(f"{path.relative_to(root)}: cannot read: {error}")
                continue
            fields = markdown_metadata(text)
            if normalized_status(fields.get("status", "")) != "active":
                continue
            scope = fields.get("scope", "")
            ledger_mentions_target = any(
                target_label not in {"", "."} and target_label in text
                for target_label in target_labels
            )
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

    raw_path = Path(raw_target)
    if not raw_path.is_absolute():
        raw_path = root / raw_path
    lexical_target = Path(os.path.abspath(raw_path))
    if not inside_root(root, lexical_target):
        return [], [f"lexical path leaves the repository: {raw_target}"]
    resolved_target = lexical_target.resolve(strict=False)
    if not inside_root(root, resolved_target):
        return [], [f"resolved path leaves the repository: {raw_target}"]

    targets = [lexical_target]
    if resolved_target != lexical_target:
        targets.append(resolved_target)
    relative_targets = [target.relative_to(root) for target in targets]
    projects = projects_for_targets(relative_targets, registry)

    result: list[Path] = []
    seen: set[Path] = set()
    suite = registry.get("suite")
    assert isinstance(suite, dict)

    add_path(result, seen, errors, root, suite.get("agents"), "suite.agents")
    for target in targets:
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

    for rule in shared_rule_documents(root, registry, errors):
        if rule not in seen:
            seen.add(rule)
            result.append(rule)

    for project in projects:
        for document in owner_context_documents(root, project, errors):
            if document not in seen:
                seen.add(document)
                result.append(document)

    document_owners = projects if projects else [suite]
    for owner in document_owners:
        owner_id = str(owner.get("id", "suite"))
        for field in DOCUMENT_FIELDS:
            add_path(result, seen, errors, root, owner.get(field), f"{owner_id}.{field}")

    for contract in relevant_contracts(root, projects, errors):
        if contract not in seen:
            seen.add(contract)
            result.append(contract)
    for plan in relevant_active_plans(
        root, registry, projects, relative_targets, errors
    ):
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
