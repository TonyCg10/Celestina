#!/usr/bin/env python3
"""Shared project-registry rules for commit and documentation guards."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import PurePosixPath
from typing import Any


@dataclass(frozen=True)
class CommitScope:
    roots: tuple[str, ...]
    files: tuple[str, ...] = ()
    allow_all: bool = False


def optional_bool(table: dict[str, Any], key: str, label: str) -> bool:
    value = table.get(key, False)
    if not isinstance(value, bool):
        raise ValueError(f"{label}.{key} debe ser booleano")
    return value


def parse_subject_prefix(subject: str) -> tuple[str, str]:
    for marker in ("fixup! ", "squash! ", "amend! "):
        if subject.startswith(marker):
            subject = subject.removeprefix(marker)
            break
    if subject.startswith('Revert "') and subject.endswith('"'):
        subject = subject[len('Revert "') : -1]

    prefix, separator, action = subject.partition(": ")
    if not separator or not prefix or not action.strip() or action != action.strip():
        raise ValueError("el asunto debe usar '<prefix>: <English imperative>'")
    normalized = prefix.lower()
    if prefix != normalized:
        raise ValueError(f'el prefijo debe ir en minúsculas: "{normalized}:"')
    return normalized, action


def unique(values: list[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(value for value in values if value))


def normalized_roots(values: object, label: str) -> tuple[str, ...]:
    if not isinstance(values, (list, tuple)) or not values:
        raise ValueError(f"{label} debe contener roots de directorio")
    roots: list[str] = []
    for value in values:
        if not isinstance(value, str) or not value.endswith("/"):
            raise ValueError(f"{label} requiere roots terminados en `/`: {value}")
        path = PurePosixPath(value)
        if (
            path.is_absolute()
            or str(path) + "/" != value
            or any(part in {"", ".", ".."} for part in path.parts)
        ):
            raise ValueError(f"{label} contiene un root no normalizado: {value}")
        roots.append(value)
    return unique(roots)


def normalized_files(values: object, label: str) -> tuple[str, ...]:
    if not isinstance(values, (list, tuple)):
        raise ValueError(f"{label} debe ser una lista")
    files: list[str] = []
    for value in values:
        if not isinstance(value, str) or value.endswith("/"):
            raise ValueError(f"{label} requiere archivos exactos: {value}")
        path = PurePosixPath(value)
        if (
            path.is_absolute()
            or str(path) != value
            or any(part in {"", ".", ".."} for part in path.parts)
        ):
            raise ValueError(f"{label} contiene un archivo no normalizado: {value}")
        files.append(value)
    return unique(files)


def build_commit_scopes(registry: dict[str, Any]) -> dict[str, CommitScope]:
    policy = registry.get("commit_policy", {})
    manifests = normalized_files(
        policy.get("workspace_manifests", ()),
        "commit_policy.workspace_manifests",
    )
    scopes: dict[str, CommitScope] = {}

    def add(prefix: str, scope: CommitScope) -> None:
        previous = scopes.get(prefix)
        if previous is not None and previous != scope:
            raise ValueError(f'el prefijo registrado "{prefix}:" tiene alcances contradictorios')
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
        project_files: list[str] = []
        if optional_bool(project, "include_workspace_manifests", project_label):
            project_files.extend(manifests)
        add(
            project["commit_prefix"],
            CommitScope(unique(project_roots), unique(project_files)),
        )

        for component_index, component in enumerate(project.get("component_commit_scopes", ())):
            component_label = f"{project_label}.component_commit_scopes[{component_index}]"
            component_roots = normalized_roots(
                component.get("roots", ()), f"{component_label}.roots"
            )
            component_files: list[str] = []
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
