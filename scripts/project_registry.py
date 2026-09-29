#!/usr/bin/env python3
"""Shared project-registry rules for commit and documentation guards."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from pathlib import Path, PurePosixPath
import re
import tomllib
from typing import Any


COMMIT_KINDS = ("bug", "milestone", "release", "maintenance")
REGISTRY_PATH = "docs/projects.toml"


def parse_registry(raw: bytes, source: str = REGISTRY_PATH) -> dict[str, Any]:
    """The one reader of `docs/projects.toml`'s shape (TOOL-18).

    UTF-8 TOML with `schema_version = 1`, a `[[projects]]` list of tables
    whose `id`s are distinct strings and, when present, a `[suite]` table.
    Every other rule, such as requiring `[suite]`, belongs to the tool that
    applies it. Raises ValueError naming `source`.
    """
    try:
        registry = tomllib.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ValueError(f"{source}: invalid registry: {error}") from error
    if registry.get("schema_version") != 1:
        raise ValueError(f"{source} requires schema_version = 1")
    if "suite" in registry and not isinstance(registry["suite"], dict):
        raise ValueError(f"{source}: [suite] must be a table")
    projects = registry.get("projects")
    if not isinstance(projects, list):
        raise ValueError(f"{source} does not contain [[projects]]")
    seen: set[str] = set()
    for index, project in enumerate(projects):
        if not isinstance(project, dict):
            raise ValueError(f"{source}: projects[{index}] is not a table")
        project_id = project.get("id")
        if not isinstance(project_id, str) or not project_id:
            raise ValueError(f"{source}: projects[{index}].id is required")
        if project_id in seen:
            raise ValueError(f"{source}: duplicate id: {project_id}")
        seen.add(project_id)
    return registry


def load_registry(path: Path) -> dict[str, Any]:
    """`parse_registry` over the file at `path`."""
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise ValueError(f"cannot read {path}: {error}") from error
    return parse_registry(raw, str(path))


@dataclass(frozen=True)
class CommitScope:
    roots: tuple[str, ...]
    files: tuple[str, ...] = ()
    allow_all: bool = False


@dataclass(frozen=True)
class HaltedProject:
    """A project the author halted; AGENTS.md "Halted projects" is the rule."""

    id: str
    name: str
    since: str
    commit_prefixes: tuple[str, ...]
    commit_roots: tuple[str, ...]


HALTED_DATE = re.compile(r"\d{4}-\d{2}-\d{2}")


def halted_projects(registry: dict[str, Any]) -> tuple[HaltedProject, ...]:
    """Return every project whose registry entry carries `halted = "YYYY-MM-DD"`.

    A halted project keeps its registration, so history, scopes and inventories
    still resolve; this is the one reader of the marker that every tool uses to
    announce the halt, refuse its commits and leave it out of production runs.
    """
    halted: list[HaltedProject] = []
    projects = registry.get("projects", ())
    if not isinstance(projects, (list, tuple)):
        raise ValueError("projects must be a list of tables")
    for index, project in enumerate(projects):
        if not isinstance(project, dict) or "halted" not in project:
            continue
        label = f"projects[{index}].halted"
        since = project["halted"]
        if not isinstance(since, str) or HALTED_DATE.fullmatch(since) is None:
            raise ValueError(f'{label} must be a date string "YYYY-MM-DD"')
        try:
            date.fromisoformat(since)
        except ValueError as error:
            raise ValueError(f"{label} is not a calendar date: {since}") from error
        project_id = project.get("id")
        prefix = project.get("commit_prefix")
        if not isinstance(project_id, str) or not isinstance(prefix, str):
            raise ValueError(f"projects[{index}] needs an id and a commit_prefix to be halted")
        name = project.get("name")
        prefixes = [prefix]
        roots = list(
            normalized_roots(project.get("commit_roots", ()), f"projects[{index}].commit_roots")
        )
        for component_index, component in enumerate(project.get("component_commit_scopes", ())):
            component_label = f"projects[{index}].component_commit_scopes[{component_index}]"
            component_prefix = component.get("prefix") if isinstance(component, dict) else None
            if not isinstance(component_prefix, str):
                raise ValueError(f"{component_label}.prefix must be a string")
            prefixes.append(component_prefix)
            roots.extend(normalized_roots(component.get("roots", ()), f"{component_label}.roots"))
        halted.append(
            HaltedProject(
                id=project_id,
                name=name if isinstance(name, str) and name else project_id,
                since=since,
                commit_prefixes=unique(prefixes),
                commit_roots=unique(roots),
            )
        )
    return tuple(halted)


def halted_owner(path: str, halted: tuple[HaltedProject, ...]) -> HaltedProject | None:
    """The halted project whose commit roots hold `path`, if any."""
    for project in halted:
        if path_allowed(path, CommitScope(project.commit_roots)):
            return project
    return None


def optional_bool(table: dict[str, Any], key: str, label: str) -> bool:
    value = table.get(key, False)
    if not isinstance(value, bool):
        raise ValueError(f"{label}.{key} must be boolean")
    return value


def parse_subject_change(
    subject: str, *, allow_legacy: bool = False
) -> tuple[str, str | None, str, str | None]:
    wrapper: str | None = None
    for marker in ("fixup! ", "squash! ", "amend! "):
        if subject.startswith(marker):
            subject = subject.removeprefix(marker)
            wrapper = marker.removesuffix("! ")
            break
    if subject.startswith('Revert "') and subject.endswith('"'):
        subject = subject[len('Revert "') : -1]
        wrapper = "revert"

    prefix, separator, action = subject.partition(": ")
    if not separator or not prefix or not action.strip() or action != action.strip():
        raise ValueError(
            "the subject must use '<prefix>-<kind>: <English imperative>'"
        )
    normalized = prefix.lower()
    if prefix != normalized:
        raise ValueError(f'the prefix must be lowercase: "{normalized}:"')

    base_prefix = normalized
    kind: str | None = None
    for candidate in COMMIT_KINDS:
        suffix = f"-{candidate}"
        if normalized.endswith(suffix) and len(normalized) > len(suffix):
            base_prefix = normalized.removesuffix(suffix)
            kind = candidate
            break
    if kind is None and not allow_legacy:
        kinds = "|".join(COMMIT_KINDS)
        raise ValueError(
            f"the subject must declare a change kind: '<prefix>-<{kinds}>: "
            "<English imperative>'"
        )
    return base_prefix, kind, action, wrapper


def parse_subject_prefix(subject: str) -> tuple[str, str]:
    prefix, _kind, action, _wrapper = parse_subject_change(
        subject, allow_legacy=True
    )
    return prefix, action


def unique(values: list[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(value for value in values if value))


def normalized_roots(values: object, label: str) -> tuple[str, ...]:
    if not isinstance(values, (list, tuple)) or not values:
        raise ValueError(f"{label} must contain directory roots")
    roots: list[str] = []
    for value in values:
        if not isinstance(value, str) or not value.endswith("/"):
            raise ValueError(f"{label} requires roots ending in `/`: {value}")
        path = PurePosixPath(value)
        if (
            path.is_absolute()
            or str(path) + "/" != value
            or any(part in {"", ".", ".."} for part in path.parts)
        ):
            raise ValueError(f"{label} contains a non-normalized root: {value}")
        roots.append(value)
    return unique(roots)


def normalized_files(values: object, label: str) -> tuple[str, ...]:
    if not isinstance(values, (list, tuple)):
        raise ValueError(f"{label} must be a list")
    files: list[str] = []
    for value in values:
        if not isinstance(value, str) or value.endswith("/"):
            raise ValueError(f"{label} requires exact file paths: {value}")
        path = PurePosixPath(value)
        if (
            path.is_absolute()
            or str(path) != value
            or any(part in {"", ".", ".."} for part in path.parts)
        ):
            raise ValueError(f"{label} contains a non-normalized file path: {value}")
        files.append(value)
    return unique(files)


def build_commit_scopes(registry: dict[str, Any]) -> dict[str, CommitScope]:
    policy = registry.get("commit_policy", {})
    manifests = normalized_files(
        policy.get("workspace_manifests", ()),
        "commit_policy.workspace_manifests",
    )
    # A ratchet row is the same commit as the change that moved it. Leaving it
    # to a follow-up commit would publish one revision whose guard is already
    # red, so every prefix that can shrink a guarded file may also lower it.
    ratchets = normalized_files(
        policy.get("shared_ratchet_files", ()),
        "commit_policy.shared_ratchet_files",
    )
    version_policy = registry.get("version_policy")
    version_history: tuple[str, ...] = ()
    if version_policy is not None:
        if not isinstance(version_policy, dict):
            raise ValueError("version_policy must be a table")
        history_file = version_policy.get("history_file")
        version_history = normalized_files(
            [history_file],
            "version_policy.history_file",
        )
    scopes: dict[str, CommitScope] = {}

    def add(prefix: str, scope: CommitScope) -> None:
        previous = scopes.get(prefix)
        if previous is not None and previous != scope:
            raise ValueError(f'the registered prefix "{prefix}:" has conflicting scopes')
        scopes[prefix] = scope

    suite = registry["suite"]
    add(
        suite["commit_prefix"],
        CommitScope(
            roots=(),
            files=(),
            allow_all=optional_bool(suite, "allow_all_commit_paths", "suite"),
        ),
    )

    for index, project in enumerate(registry.get("projects", ())):
        project_label = f"projects[{index}]"
        project_roots = normalized_roots(
            project.get("commit_roots", ()), f"{project_label}.commit_roots"
        )
        project_files: list[str] = list(ratchets)
        if optional_bool(project, "include_workspace_manifests", project_label):
            project_files.extend(manifests)
        if version_policy is not None:
            has_version_source = isinstance(project.get("version_source"), dict)
            explicitly_unversioned = project.get("versioned") is False
            if has_version_source == explicitly_unversioned:
                raise ValueError(
                    f"{project_label} must declare either version_source or "
                    "versioned = false"
                )
            if has_version_source:
                project_files.extend(version_history)
        add(
            project["commit_prefix"],
            CommitScope(unique(project_roots), unique(project_files)),
        )

        for component_index, component in enumerate(project.get("component_commit_scopes", ())):
            component_label = f"{project_label}.component_commit_scopes[{component_index}]"
            component_roots = normalized_roots(
                component.get("roots", ()), f"{component_label}.roots"
            )
            component_files: list[str] = list(ratchets)
            if optional_bool(
                component,
                "include_workspace_manifests",
                component_label,
            ):
                component_files.extend(manifests)
            add(
                component["prefix"],
                CommitScope(unique(component_roots), unique(component_files)),
            )
    return scopes


def path_allowed(path: str, scope: CommitScope) -> bool:
    return scope.allow_all or path in scope.files or any(
        path == root.rstrip("/") or path.startswith(root) for root in scope.roots
    )


def pathspec_allowed(pathspec: str, scope: CommitScope) -> bool:
    if scope.allow_all:
        return True
    if pathspec.endswith("/"):
        return any(pathspec.startswith(root) for root in scope.roots)
    return path_allowed(pathspec, scope)
