#!/usr/bin/env python3
"""Validate Celestina's vendor-neutral documentation contract."""

from __future__ import annotations

import argparse
from datetime import date
import hashlib
import os
from pathlib import Path
import posixpath
import re
import stat
import subprocess
import sys
import tomllib
from urllib.parse import unquote, urlsplit

from project_registry import (
    CommitScope,
    build_commit_scopes,
    parse_subject_prefix,
    path_allowed,
    pathspec_allowed,
)


DATE_RE = re.compile(r"\d{4}-\d{2}-\d{2}")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*#*\s*$")
META_RE = re.compile(
    r"\*\*([^*\n:]+):\*\*\s*([^*\n]*?)"
    r"(?=(?:\s*[·|]\s*)?\*\*[^*\n:]+:\*\*|$)"
)

PLAN_STATES = {"planned", "active", "blocked", "done"}
ROADMAP_STATES = {"planned", "active", "blocked", "done", "idle"}
DECISION_STATES = {"proposed", "accepted", "rejected", "superseded"}
DISCUSSION_STATES = {"open", "concluded", "applied"}
VALIDATION_STATES = {"pending", "passed", "failed", "obsolete", "deferred"}
CHECKPOINT_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
DONE_DIFFSTAT_RE = re.compile(
    r"(?P<files>[0-9]+) files, \+(?P<added>[0-9]+)/-(?P<deleted>[0-9]+)"
)
VALIDATION_ID_RE = re.compile(r"VAL-[A-Z0-9]+(?:-[A-Z0-9]+)*")
VALIDATION_TITLE_RE = re.compile(
    r"(?P<id>VAL-[A-Z0-9]+(?:-[A-Z0-9]+)*)(?:\s+(?:—|–|-|:)\s+.+)?"
)
NUMSTAT_HEADER = ("added", "deleted", "content", "path")
NUMSTAT_CONTENT_RE = re.compile(r"(?:[0-9a-f]{64}|deleted|self)")
NUMSTAT_VALUE_RE = re.compile(r"(?:[0-9]+|-)")
BASE_REVISION_RE = re.compile(r"Base revision\t[0-9a-f]{40}")
REMEDIATION_UNIT_RE = re.compile(r"`([A-Za-z0-9][A-Za-z0-9._-]*)`")
DATED_DOCUMENT_RE = re.compile(
    r"(?P<date>\d{4}-\d{2}-\d{2})-[a-z0-9]+(?:-[a-z0-9]+)*\.md"
)
ADR_DOCUMENT_RE = re.compile(r"[0-9]{4}-[a-z0-9]+(?:-[a-z0-9]+)*\.md")

LEDGER_COLUMNS = (
    "unit",
    "commit prefix",
    "status",
    "files / areas",
    "intended change",
    "diffstat",
    "automated evidence",
    "author validation",
)

VENDOR_FILENAMES = {
    "claude.md",
    "gemini.md",
    ".cursorrules",
    "copilot-instructions.md",
}


def valid_date(value: str) -> bool:
    try:
        date.fromisoformat(value)
    except ValueError:
        return False
    return bool(DATE_RE.fullmatch(value))


class RegistryError(ValueError):
    """The project registry cannot be used safely."""


def repository_root() -> Path:
    return Path(__file__).resolve().parent.parent


def inside_root(root: Path, candidate: Path) -> bool:
    try:
        candidate.relative_to(root)
    except ValueError:
        return False
    return True


def load_registry(root: Path) -> dict[str, object]:
    registry_path = root / "docs/projects.toml"
    try:
        with registry_path.open("rb") as handle:
            registry = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise RegistryError(f"no se puede leer docs/projects.toml: {error}") from error

    if registry.get("schema_version") != 1:
        raise RegistryError("docs/projects.toml requiere schema_version = 1")
    if not isinstance(registry.get("suite"), dict):
        raise RegistryError("docs/projects.toml no contiene [suite]")
    if not isinstance(registry.get("projects"), list):
        raise RegistryError("docs/projects.toml no contiene [[projects]]")
    return registry


def markdown_metadata(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in text.splitlines():
        for match in META_RE.finditer(line):
            key = " ".join(match.group(1).lower().split())
            fields.setdefault(key, match.group(2).strip().strip("·|").strip())
    return fields


def markdown_headings(text: str) -> list[tuple[int, str, int]]:
    headings: list[tuple[int, str, int]] = []
    fence: str | None = None
    for line_number, line in enumerate(text.splitlines(), start=1):
        stripped = line.lstrip()
        fence_match = re.match(r"(```+|~~~+)", stripped)
        if fence_match:
            marker = fence_match.group(1)[0]
            if fence is None:
                fence = marker
            elif fence == marker:
                fence = None
            continue
        if fence is not None:
            continue
        match = HEADING_RE.match(line)
        if match:
            headings.append((len(match.group(1)), match.group(2).strip(), line_number))
    return headings


def normalized_heading(value: str) -> str:
    value = re.sub(r"\[([^]]+)\]\([^)]+\)", r"\1", value)
    value = value.replace("`", "").replace("*", "")
    return " ".join(value.casefold().split())


def normalized_status(value: str) -> str:
    value = re.sub(r"\[([^]]+)\]\([^)]+\)", r"\1", value)
    return value.strip().strip("`*").casefold()


def checkpoint_value(value: str) -> str:
    """Return a metadata checkpoint as a comparable literal identifier."""
    return value.strip().strip("`*").strip()


def no_active_checkpoint(value: str) -> bool:
    """Accept `none` plus an explanatory suffix in inactive roadmaps."""
    return normalized_status(value).split(";", maxsplit=1)[0].strip() == "none"


def scope_matches_owner(scope: str, owner_id: str, owner_name: str) -> bool:
    normalized = " ".join(scope.casefold().replace("`", "").split())
    tokens = set(re.findall(r"[a-z0-9][a-z0-9-]*", normalized))
    if owner_id.casefold() == "suite":
        return bool(tokens.intersection({"suite", "all", "global"}))
    return owner_id.casefold() in normalized or (
        bool(owner_name) and owner_name.casefold() in normalized
    )


def split_table_row(line: str) -> list[str]:
    content = line.strip()
    if content.startswith("|"):
        content = content[1:]
    if content.endswith("|"):
        content = content[:-1]
    return [cell.replace(r"\|", "|").strip() for cell in re.split(r"(?<!\\)\|", content)]


def is_separator_row(cells: list[str]) -> bool:
    return bool(cells) and all(re.fullmatch(r":?-{3,}:?", cell.replace(" ", "")) for cell in cells)


def excluded_path(relative: Path) -> bool:
    parts = relative.parts
    if not parts:
        return False
    if parts[0] == ".git":
        return True
    if len(parts) >= 2 and parts[0] == ".claude" and parts[1] == "worktrees":
        return True
    if len(parts) >= 3 and parts[:3] == ("scripts", "fixtures", "documentation"):
        return True
    return any(
        part in {"build", "target", "__pycache__", ".venv", "node_modules"}
        or part.startswith("cmake-build-")
        for part in parts
    )


def iter_repository_files(root: Path) -> list[Path]:
    found: list[Path] = []
    for base, directories, files in os.walk(root, followlinks=False):
        base_path = Path(base)
        directories[:] = [
            directory
            for directory in directories
            if not excluded_path((base_path / directory).relative_to(root))
        ]
        for filename in files:
            path = base_path / filename
            if not excluded_path(path.relative_to(root)):
                found.append(path)
    return sorted(found)


def extract_inline_links(text: str) -> list[tuple[int, str]]:
    links: list[tuple[int, str]] = []
    fence: str | None = None
    for line_number, original_line in enumerate(text.splitlines(), start=1):
        stripped = original_line.lstrip()
        fence_match = re.match(r"(```+|~~~+)", stripped)
        if fence_match:
            marker = fence_match.group(1)[0]
            if fence is None:
                fence = marker
            elif fence == marker:
                fence = None
            continue
        if fence is not None:
            continue

        line = re.sub(r"`[^`]*`", "", original_line)
        position = 0
        while True:
            opener = line.find("](", position)
            if opener < 0:
                break
            start = opener + 2
            if start >= len(line):
                break

            if line[start] == "<":
                end = line.find(">", start + 1)
                if end < 0:
                    break
                target = line[start + 1 : end]
                close = line.find(")", end + 1)
                position = close + 1 if close >= 0 else end + 1
            else:
                depth = 1
                escaped = False
                cursor = start
                while cursor < len(line):
                    character = line[cursor]
                    if escaped:
                        escaped = False
                    elif character == "\\":
                        escaped = True
                    elif character == "(":
                        depth += 1
                    elif character == ")":
                        depth -= 1
                        if depth == 0:
                            break
                    cursor += 1
                if depth != 0:
                    break
                raw_target = line[start:cursor].strip()
                target = raw_target.split(maxsplit=1)[0] if raw_target else ""
                position = cursor + 1
            if target:
                links.append((line_number, target.replace(r"\ ", " ")))
    return links


def github_anchors(text: str) -> set[str]:
    anchors: set[str] = set()
    duplicates: dict[str, int] = {}
    for _level, heading, _line in markdown_headings(text):
        explicit = re.search(r"\{#([^}]+)\}\s*$", heading)
        if explicit:
            anchors.add(explicit.group(1))
            heading = heading[: explicit.start()].rstrip()
        heading = re.sub(r"<[^>]+>", "", heading)
        heading = re.sub(r"\[([^]]+)\]\([^)]+\)", r"\1", heading)
        heading = heading.replace("`", "").replace("*", "").replace("_", "")
        slug = "".join(character for character in heading.casefold() if character.isalnum() or character in " -")
        slug = slug.replace(" ", "-")
        count = duplicates.get(slug, 0)
        duplicates[slug] = count + 1
        anchors.add(slug if count == 0 else f"{slug}-{count}")
    for explicit in re.findall(r"<a\s+(?:name|id)=[\"']([^\"']+)[\"']", text, re.IGNORECASE):
        anchors.add(explicit)
    return anchors


class DocumentationContract:
    def __init__(self, root: Path) -> None:
        self.root = root.resolve()
        self.errors: list[str] = []
        self.registry: dict[str, object] = {}
        self.prefixes: set[str] = set()
        self.owner_prefixes: dict[str, set[str]] = {}
        self.owner_evidence_directories: dict[str, Path] = {}
        self.roadmaps: dict[str, tuple[Path, str, str]] = {}
        self.active_plan_records: list[tuple[str, Path, str]] = []
        self.checked_documents: set[Path] = set()
        self.validation_ids: dict[str, tuple[Path, int]] = {}
        self.inventory_units: dict[Path, tuple[Path, str, int]] = {}
        self.commit_scopes: dict[str, CommitScope] = {}
        self.inventory_claims: list[
            tuple[str, Path, str, str, str, set[str]]
        ] = []
        self.inventory_group_paths: dict[str, set[str]] = {}
        self.owner_plan_directories: dict[str, tuple[str, str]] = {}
        self.owner_inventory_roots: dict[str, str] = {}
        self.owner_plan_ids: dict[tuple[str, str], Path] = {}
        self.historical_plan_inventories: dict[
            str, list[tuple[str, Path, str, dict[str, str]]]
        ] = {}
        self.anchor_cache: dict[Path, set[str]] = {}
        self._is_git_root: bool | None = None

    def relative(self, path: Path) -> str:
        try:
            return path.relative_to(self.root).as_posix()
        except ValueError:
            return str(path)

    def error(self, path: Path | str, message: str) -> None:
        label = self.relative(path) if isinstance(path, Path) else path
        self.errors.append(f"{label}: {message}")

    def register_owner_delivery_roots(self, owner_id: str, active: Path) -> None:
        active_relative = self.relative(active)
        archive_relative = self.relative(active.parent / "archive")
        inventory_relative = self.relative(active.parent.parent / "inventories")
        self.owner_plan_directories[owner_id] = (
            active_relative.rstrip("/"),
            archive_relative.rstrip("/"),
        )
        self.owner_inventory_roots[owner_id] = inventory_relative.rstrip("/")

    def registry_path(self, raw: object, field: str) -> Path | None:
        if not isinstance(raw, str) or not raw.strip():
            self.error("docs/projects.toml", f"{field} debe ser una ruta no vacía")
            return None
        if Path(raw).is_absolute():
            self.error("docs/projects.toml", f"{field} debe ser relativo: {raw}")
            return None
        candidate = (self.root / raw).resolve(strict=False)
        if not inside_root(self.root, candidate):
            self.error("docs/projects.toml", f"{field} sale del repositorio: {raw}")
            return None
        return candidate

    def read_document(self, path: Path) -> str | None:
        try:
            return path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as error:
            self.error(path, f"no se puede leer como UTF-8: {error}")
            return None

    def require_document(self, raw: object, role: str, owner_id: str = "") -> None:
        path = self.registry_path(raw, role)
        if path is None:
            return
        if not path.is_file():
            self.error(path, f"documento registrado no existe ({role})")
            return
        if path in self.checked_documents:
            return
        self.checked_documents.add(path)
        text = self.read_document(path)
        if text is None:
            return
        headings = markdown_headings(text)
        if not any(level == 1 for level, _heading, _line in headings):
            self.error(path, "falta título H1")
        if role == "status":
            updated = markdown_metadata(text).get("updated", "")
            if not valid_date(updated):
                self.error(path, "STATUS requiere `- **Updated:** YYYY-MM-DD`")
        elif role == "roadmap":
            normalized = [normalized_heading(heading) for _level, heading, _line in headings]
            if not any("implementation exit" in heading for heading in normalized):
                self.error(path, "ROADMAP requiere un heading `Implementation exit`")
            fields = markdown_metadata(text)
            status_value = fields.get("status")
            roadmap_status = normalized_status(status_value or "")
            if not status_value:
                self.error(path, "ROADMAP requiere metadata `Status`")
            elif roadmap_status not in ROADMAP_STATES:
                self.error(path, f"estado de roadmap inválido: {status_value}")

            checkpoint_raw = fields.get("active implementation checkpoint", "")
            checkpoint = checkpoint_value(checkpoint_raw)
            if not checkpoint:
                self.error(
                    path,
                    "ROADMAP requiere metadata `Active implementation checkpoint`",
                )
            elif roadmap_status in {"active", "blocked"}:
                if no_active_checkpoint(checkpoint):
                    self.error(
                        path,
                        f"ROADMAP {roadmap_status} requiere un checkpoint activo",
                    )
                elif not CHECKPOINT_RE.fullmatch(checkpoint):
                    self.error(path, f"checkpoint de roadmap inválido: {checkpoint}")
            elif roadmap_status in {"planned", "idle", "done"} and not no_active_checkpoint(checkpoint):
                self.error(
                    path,
                    f"ROADMAP {roadmap_status} requiere `Active implementation checkpoint: none`",
                )

            if owner_id and roadmap_status in ROADMAP_STATES and checkpoint:
                self.roadmaps[owner_id] = (path, roadmap_status, checkpoint)
        elif role == "validation":
            self.check_validation_sections(path, text)

    def check_plan_id(
        self,
        path: Path,
        fields: dict[str, str],
        label: str,
        owner_id: str,
    ) -> None:
        plan_id = checkpoint_value(fields.get("plan id", ""))
        if not plan_id:
            self.error(path, f"{label} requiere metadata `Plan ID`")
            return
        if not CHECKPOINT_RE.fullmatch(plan_id):
            self.error(path, f"Plan ID inválido: {plan_id}")
            return
        key = owner_id, plan_id
        previous = self.owner_plan_ids.get(key)
        if previous is not None and previous != path:
            self.error(
                path,
                f"Plan ID `{plan_id}` duplicado para owner `{owner_id}`; "
                f"ya pertenece a {self.relative(previous)}",
            )
            return
        self.owner_plan_ids[key] = path

    def require_script(self, raw: object, field: str) -> None:
        path = self.registry_path(raw, field)
        if path is None:
            return
        if not path.is_file():
            self.error(path, f"script registrado no existe ({field})")
            return
        try:
            mode = path.stat().st_mode
        except OSError as error:
            self.error(path, f"no se puede inspeccionar el script: {error}")
            return
        if not mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH):
            self.error(path, f"script registrado no es ejecutable ({field})")

    def check_registry(self) -> None:
        try:
            self.registry = load_registry(self.root)
        except RegistryError as error:
            self.error("docs/projects.toml", str(error))
            return
        try:
            self.commit_scopes = build_commit_scopes(self.registry)
        except (KeyError, TypeError, ValueError) as error:
            self.error("docs/projects.toml", f"alcance de commits inválido: {error}")

        suite = self.registry["suite"]
        assert isinstance(suite, dict)
        self.owner_evidence_directories["suite"] = (self.root / "docs" / "evidence").resolve()
        for field in ("agents", "readme", "status", "roadmap", "validation"):
            self.require_document(suite.get(field), field, "suite")
        active_plans = self.registry_path(suite.get("active_plans"), "suite.active_plans")
        if active_plans is not None and not active_plans.is_dir():
            self.error(active_plans, "directorio de planes activos registrado no existe")
        elif active_plans is not None:
            self.register_owner_delivery_roots("suite", active_plans)

        suite_prefix = suite.get("commit_prefix")
        if isinstance(suite_prefix, str) and suite_prefix:
            self.prefixes.add(suite_prefix)
            self.owner_prefixes["suite"] = {suite_prefix}
        else:
            self.error("docs/projects.toml", "suite.commit_prefix es obligatorio")

        seen_ids: set[str] = set()
        seen_prefixes: set[str] = set(self.prefixes)
        projects = self.registry["projects"]
        assert isinstance(projects, list)
        if not projects:
            self.error("docs/projects.toml", "debe registrar al menos un proyecto")
        for index, project in enumerate(projects):
            label = f"projects[{index}]"
            if not isinstance(project, dict):
                self.error("docs/projects.toml", f"{label} no es una tabla")
                continue
            project_id = project.get("id")
            if not isinstance(project_id, str) or not project_id:
                self.error("docs/projects.toml", f"{label}.id es obligatorio")
            elif project_id in seen_ids:
                self.error("docs/projects.toml", f"id duplicado: {project_id}")
            else:
                seen_ids.add(project_id)

            prefix = project.get("commit_prefix")
            if not isinstance(prefix, str) or not prefix:
                self.error("docs/projects.toml", f"{label}.commit_prefix es obligatorio")
            elif prefix in seen_prefixes:
                self.error("docs/projects.toml", f"prefijo duplicado: {prefix}")
            else:
                seen_prefixes.add(prefix)
                self.prefixes.add(prefix)

            project_root = self.registry_path(project.get("path"), f"{label}.path")
            if project_root is not None and not project_root.is_dir():
                self.error(project_root, "directorio de proyecto registrado no existe")
            owner_id = project_id if isinstance(project_id, str) and project_id else label
            if project_root is not None:
                self.owner_evidence_directories[owner_id] = (
                    project_root / "docs" / "evidence"
                ).resolve()
            for field in ("agents", "readme", "status", "roadmap", "validation"):
                self.require_document(project.get(field), field, owner_id)
            if "active_plans" in project:
                project_plans = self.registry_path(project.get("active_plans"), f"{label}.active_plans")
                if project_plans is not None and not project_plans.is_dir():
                    self.error(project_plans, "directorio de planes activos registrado no existe")
                elif project_plans is not None:
                    self.register_owner_delivery_roots(owner_id, project_plans)

            source_roots = project.get("source_roots")
            if not isinstance(source_roots, list) or not source_roots:
                self.error("docs/projects.toml", f"{label}.source_roots debe ser una lista no vacía")
            else:
                for source_index, source_root in enumerate(source_roots):
                    path = self.registry_path(source_root, f"{label}.source_roots[{source_index}]")
                    if path is not None and not path.exists():
                        self.error(path, "source root registrado no existe")

            commit_roots = project.get("commit_roots")
            if not isinstance(commit_roots, list) or not commit_roots:
                self.error("docs/projects.toml", f"{label}.commit_roots debe ser una lista no vacía")

            for field in ("production_role", "artifact_manifest", "artifact_paths"):
                if field not in project:
                    self.error("docs/projects.toml", f"{label}.{field} es obligatorio")
            self.require_script(project.get("build_script"), f"{label}.build_script")
            self.require_script(project.get("verify_script"), f"{label}.verify_script")
            self.require_script(project.get("status_script"), f"{label}.status_script")
            deployable = project.get("deployable")
            if not isinstance(deployable, bool):
                self.error("docs/projects.toml", f"{label}.deployable debe ser booleano")
            elif deployable:
                self.require_script(project.get("deploy_script"), f"{label}.deploy_script")
                self.require_script(project.get("complete_script"), f"{label}.complete_script")
            if "activate_script" in project:
                self.require_script(project.get("activate_script"), f"{label}.activate_script")

            component_scopes = project.get("component_commit_scopes", [])
            allowed_prefixes: set[str] = set()
            if isinstance(prefix, str) and prefix:
                allowed_prefixes.add(prefix)
            if not isinstance(component_scopes, list):
                self.error("docs/projects.toml", f"{label}.component_commit_scopes debe ser una lista")
                self.owner_prefixes[owner_id] = allowed_prefixes
                continue
            for component_index, component in enumerate(component_scopes):
                component_label = f"{label}.component_commit_scopes[{component_index}]"
                if not isinstance(component, dict):
                    self.error("docs/projects.toml", f"{component_label} no es una tabla")
                    continue
                component_prefix = component.get("prefix")
                if not isinstance(component_prefix, str) or not component_prefix:
                    self.error("docs/projects.toml", f"{component_label}.prefix es obligatorio")
                elif component_prefix in seen_prefixes:
                    self.error("docs/projects.toml", f"prefijo duplicado: {component_prefix}")
                else:
                    seen_prefixes.add(component_prefix)
                    self.prefixes.add(component_prefix)
            self.owner_prefixes[owner_id] = allowed_prefixes

    def resolve_local_link(self, document: Path, target: str) -> Path | None:
        if target.startswith("#"):
            target_path = document
            fragment = unquote(target[1:])
        else:
            parsed = urlsplit(target)
            if parsed.scheme or target.startswith("//"):
                return None
            raw_path = unquote(parsed.path)
            fragment = unquote(parsed.fragment)
            if not raw_path:
                target_path = document
            elif Path(raw_path).is_absolute():
                return None
            else:
                target_path = (document.parent / raw_path).resolve(strict=False)

        if not inside_root(self.root, target_path) or not target_path.is_file():
            return None
        if fragment:
            if target_path.suffix.casefold() != ".md":
                return None
            anchors = self.anchor_cache.get(target_path)
            if anchors is None:
                linked_text = self.read_document(target_path)
                if linked_text is None:
                    return None
                anchors = github_anchors(linked_text)
                self.anchor_cache[target_path] = anchors
            if fragment not in anchors:
                return None
        return target_path

    def resolvable_links(self, document: Path, value: str) -> list[Path]:
        resolved: list[Path] = []
        for _line, target in extract_inline_links(value):
            candidate = self.resolve_local_link(document, target)
            if candidate is not None:
                resolved.append(candidate)
        return resolved

    def record_validation_id(self, path: Path, validation_id: str, line_number: int) -> None:
        previous = self.validation_ids.get(validation_id)
        if previous is not None:
            previous_path, previous_line = previous
            self.error(
                path,
                f"{validation_id} duplicado; primero en "
                f"{self.relative(previous_path)}:{previous_line}",
            )
        else:
            self.validation_ids[validation_id] = (path, line_number)

    def check_validation_case(
        self,
        path: Path,
        validation_id: str,
        line_number: int,
        fields: dict[str, str],
    ) -> None:
        required_fields = (
            "status",
            "related implementation",
            "requires",
            "procedure",
            "pass condition",
            "result",
            "evidence",
        )
        self.record_validation_id(path, validation_id, line_number)
        for field in required_fields:
            if not fields.get(field, "").strip():
                self.error(path, f"{validation_id} requiere metadata `{field.title()}`")

        status_value = fields.get("status", "")
        status = normalized_status(status_value)
        if status not in VALIDATION_STATES:
            self.error(path, f"{validation_id} requiere Status en {sorted(VALIDATION_STATES)}")
            return

        result = normalized_status(fields.get("result", ""))
        evidence = normalized_status(fields.get("evidence", ""))
        if status in {"passed", "failed", "obsolete"}:
            if evidence in {"", "none", "not available", "n/a"}:
                self.error(path, f"{validation_id} cerrado requiere evidencia")
            if result in {"", "not run", "pending", "none", "n/a"}:
                self.error(path, f"{validation_id} cerrado requiere resultado observado")

        if status in {"passed", "failed"}:
            evidence_targets = [
                candidate
                for candidate in self.resolvable_links(path, fields.get("evidence", ""))
                if candidate.suffix.casefold() == ".md"
                and candidate.name.casefold() != "readme.md"
                and self.is_canonical_record_target(candidate, "evidence")
            ]
            if not evidence_targets:
                self.error(
                    path,
                    f"{validation_id} {status} requiere expediente `.md` resoluble "
                    "bajo `evidence/`",
                )

        if status == "failed":
            remediation = fields.get("remediation", "")
            if not remediation.strip():
                self.error(path, f"{validation_id} failed requiere metadata `Remediation`")
            else:
                remediation_targets = [
                    candidate
                    for candidate in self.resolvable_links(path, remediation)
                    if candidate.suffix.casefold() == ".md"
                    and candidate.name.casefold() != "readme.md"
                    and self.is_canonical_plan_target(candidate)
                ]
                if not remediation_targets:
                    self.error(
                        path,
                        f"{validation_id} failed requiere remediación enlazada a un plan "
                        "active/archive resoluble",
                    )
                remediation_units = set(REMEDIATION_UNIT_RE.findall(remediation))
                if len(remediation_units) != 1:
                    self.error(
                        path,
                        f"{validation_id} failed requiere una única unidad de remediación "
                        "entre backticks",
                    )
                elif remediation_targets:
                    remediation_unit = next(iter(remediation_units))
                    if not any(
                        remediation_unit in self.plan_ledger_units(candidate)
                        for candidate in remediation_targets
                    ):
                        self.error(
                            path,
                            f"{validation_id} failed enlaza un plan cuyo ledger no contiene "
                            f"la unidad `{remediation_unit}`",
                        )

        if status == "obsolete":
            targets = self.resolvable_links(path, fields.get("evidence", ""))
            accepted_targets = [
                target
                for target in targets
                if self.is_canonical_record_target(target, "decisions")
                or self.is_canonical_record_target(target, "evidence")
            ]
            if not accepted_targets:
                self.error(
                    path,
                    f"{validation_id} obsolete requiere enlace resoluble a decisión o evidencia",
                )

    def check_validation_sections(self, path: Path, text: str) -> None:
        required_fields = (
            "status",
            "related implementation",
            "requires",
            "procedure",
            "pass condition",
            "result",
            "evidence",
        )
        case_field_names = set(required_fields) | {"remediation"}

        heading_matches = list(
            re.finditer(r"^(#{2,6})\s+(.+?)\s*#*\s*$", text, re.MULTILINE)
        )
        for index, match in enumerate(heading_matches):
            direct_end = (
                heading_matches[index + 1].start()
                if index + 1 < len(heading_matches)
                else len(text)
            )
            direct_section = text[match.end() : direct_end]
            fields = markdown_metadata(direct_section)
            title = match.group(2).strip()
            title_match = VALIDATION_TITLE_RE.fullmatch(title)
            looks_like_case = title.casefold().startswith("val-") or bool(
                case_field_names.intersection(fields)
            )
            line_number = text.count("\n", 0, match.start()) + 1
            if title_match is None:
                if looks_like_case:
                    self.error(
                        path,
                        f"línea {line_number}: caso manual debe comenzar con ID `VAL-*` válido",
                    )
                continue
            validation_id = title_match.group("id")
            self.check_validation_case(path, validation_id, line_number, fields)

        lines = text.splitlines()
        cursor = 0
        while cursor + 1 < len(lines):
            if not lines[cursor].lstrip().startswith("|"):
                cursor += 1
                continue
            header = [" ".join(cell.casefold().split()) for cell in split_table_row(lines[cursor])]
            separator = split_table_row(lines[cursor + 1])
            if not is_separator_row(separator):
                cursor += 1
                continue
            marker_columns = set(header).intersection(required_fields)
            is_validation_table = "id" in header or (
                "status" in marker_columns and len(marker_columns) >= 2
            )
            if not is_validation_table:
                cursor += 1
                continue

            mandatory_columns = ("id",) + required_fields
            missing_columns = [field for field in mandatory_columns if field not in header]
            if missing_columns:
                self.error(
                    path,
                    "tabla de validación carece de columnas: " + ", ".join(missing_columns),
                )
            cursor += 2
            while cursor < len(lines) and lines[cursor].lstrip().startswith("|"):
                cells = split_table_row(lines[cursor])
                line_number = cursor + 1
                cursor += 1
                if len(cells) != len(header):
                    self.error(path, f"validación línea {line_number}: número de celdas incorrecto")
                    continue

                for field in mandatory_columns:
                    if field in header and not cells[header.index(field)].strip():
                        self.error(path, f"validación línea {line_number}: `{field}` vacío")

                if "id" not in header:
                    continue
                validation_id = cells[header.index("id")].strip("` ")
                if not VALIDATION_ID_RE.fullmatch(validation_id):
                    self.error(path, f"validación línea {line_number}: ID `VAL-*` inválido: {validation_id}")
                    continue
                fields = {
                    field: cells[header.index(field)] if field in header else ""
                    for field in required_fields
                }
                if "remediation" in header:
                    fields["remediation"] = cells[header.index("remediation")]
                self.check_validation_case(path, validation_id, line_number, fields)

    def check_vendor_files(self, files: list[Path]) -> None:
        for path in files:
            relative = path.relative_to(self.root)
            lowered = tuple(part.casefold() for part in relative.parts)
            filename = relative.name.casefold()
            is_copilot_instruction = (
                len(lowered) >= 3
                and lowered[0] == ".github"
                and lowered[1] == "instructions"
                and filename.endswith(".instructions.md")
            )
            if filename in VENDOR_FILENAMES or is_copilot_instruction:
                self.error(path, "archivo normativo específico de proveedor prohibido")

    def check_markdown_links(self, files: list[Path]) -> None:
        markdown_files = [path for path in files if path.suffix.casefold() == ".md"]
        anchor_cache: dict[Path, set[str]] = {}
        for document in markdown_files:
            text = self.read_document(document)
            if text is None:
                continue
            for line_number, target in extract_inline_links(text):
                if target.startswith("#"):
                    target_path = document
                    fragment = unquote(target[1:])
                else:
                    parsed = urlsplit(target)
                    if parsed.scheme or target.startswith("//"):
                        continue
                    raw_path = unquote(parsed.path)
                    fragment = unquote(parsed.fragment)
                    if not raw_path:
                        target_path = document
                    elif Path(raw_path).is_absolute():
                        self.error(document, f"línea {line_number}: enlace local debe ser relativo: {target}")
                        continue
                    else:
                        target_path = (document.parent / raw_path).resolve(strict=False)

                if not inside_root(self.root, target_path):
                    self.error(document, f"línea {line_number}: enlace sale del repositorio: {target}")
                    continue
                if not target_path.exists():
                    self.error(document, f"línea {line_number}: enlace local roto: {target}")
                    continue
                if fragment and target_path.is_file() and target_path.suffix.casefold() == ".md":
                    anchors = anchor_cache.get(target_path)
                    if anchors is None:
                        linked_text = self.read_document(target_path)
                        if linked_text is None:
                            continue
                        anchors = github_anchors(linked_text)
                        anchor_cache[target_path] = anchors
                    if fragment not in anchors:
                        self.error(document, f"línea {line_number}: anchor local no existe: {target}")

    def check_dated_document_name(self, path: Path, role: str) -> None:
        match = DATED_DOCUMENT_RE.fullmatch(path.name)
        if match is None or not valid_date(match.group("date") if match else ""):
            self.error(path, f"{role} requiere nombre `YYYY-MM-DD-short-topic.md`")

    def required_section_bodies(
        self,
        path: Path,
        text: str,
        required: tuple[str, ...],
        role: str,
    ) -> dict[str, list[str]]:
        lines = text.splitlines()
        headings = markdown_headings(text)
        bodies: dict[str, list[str]] = {name: [] for name in required}
        for index, (level, heading, line_number) in enumerate(headings):
            normalized = normalized_heading(heading)
            if normalized not in bodies:
                continue
            end_line = len(lines) + 1
            for next_level, _next_heading, next_line in headings[index + 1 :]:
                if next_level <= level:
                    end_line = next_line
                    break
            body = "\n".join(lines[line_number : end_line - 1])
            body_without_comments = re.sub(r"<!--.*?-->", "", body, flags=re.DOTALL)
            bodies[normalized].append(body_without_comments)

        for section in required:
            if not bodies[section]:
                self.error(path, f"{role} requiere heading `{section.title()}`")
                continue
            for body in bodies[section]:
                if not body.strip():
                    self.error(path, f"sección obligatoria `{section.title()}` está vacía")
        return bodies

    def canonical_applied_target(self, target: Path) -> bool:
        relative = target.relative_to(self.root)
        parts = set(relative.parts)
        if target.suffix.casefold() != ".md" or target.name.casefold() == "readme.md":
            return False
        if parts.intersection({"discussions", "history", "evidence", "templates"}):
            return False
        return bool(
            parts.intersection({"decisions", "contracts"})
            or target.name in {"ROADMAP.md", "DESIGN.md", "VISION.md", "STATUS.md"}
            or ({"plans", "active"} <= parts)
        )

    def check_canonical_index(
        self,
        directory: Path,
        records: list[Path],
        role: str,
    ) -> None:
        readme = directory / "README.md"
        if not readme.is_file():
            self.error(readme, f"índice de {role} no existe")
            return
        text = self.read_document(readme)
        if text is None:
            return

        record_set = set(records)
        link_counts = {record: 0 for record in records}
        for _line, target in extract_inline_links(text):
            resolved = self.resolve_local_link(readme, target)
            if resolved in record_set:
                link_counts[resolved] += 1
        for record, count in link_counts.items():
            if count == 0:
                self.error(readme, f"registro huérfano no enlazado: {self.relative(record)}")
            elif count != 1:
                self.error(
                    readme,
                    f"registro debe enlazarse exactamente una vez: {self.relative(record)} "
                    f"({count} enlaces)",
                )

        metadata_status: dict[Path, str] = {}
        for record in records:
            record_text = self.read_document(record)
            if record_text is not None:
                metadata_status[record] = normalized_status(
                    markdown_metadata(record_text).get("status", "")
                )

        lines = text.splitlines()
        cursor = 0
        while cursor + 1 < len(lines):
            if not lines[cursor].lstrip().startswith("|"):
                cursor += 1
                continue
            header = [" ".join(cell.casefold().split()) for cell in split_table_row(lines[cursor])]
            separator = split_table_row(lines[cursor + 1])
            if "status" not in header or not is_separator_row(separator):
                cursor += 1
                continue
            status_index = header.index("status")
            cursor += 2
            while cursor < len(lines) and lines[cursor].lstrip().startswith("|"):
                cells = split_table_row(lines[cursor])
                line_number = cursor + 1
                cursor += 1
                if len(cells) != len(header):
                    self.error(readme, f"índice línea {line_number}: número de celdas incorrecto")
                    continue
                targets = [
                    self.resolve_local_link(readme, target)
                    for _line, target in extract_inline_links(" | ".join(cells))
                ]
                indexed_records = [target for target in targets if target in record_set]
                if len(indexed_records) != 1:
                    self.error(
                        readme,
                        f"índice línea {line_number}: entrada stale requiere un registro "
                        f"canónico de {role}",
                    )
                    continue
                record = indexed_records[0]
                listed_status = normalized_status(cells[status_index])
                actual_status = metadata_status.get(record, "")
                if listed_status != actual_status:
                    self.error(
                        readme,
                        f"índice línea {line_number}: Status `{listed_status}` no coincide "
                        f"con metadata `{actual_status}` de {self.relative(record)}",
                    )

    def canonical_document_directories(self, name: str) -> list[Path]:
        directories = [self.root / "docs" / name]
        projects = self.registry.get("projects", [])
        if isinstance(projects, list):
            for project in projects:
                if not isinstance(project, dict):
                    continue
                raw_path = project.get("path")
                if not isinstance(raw_path, str):
                    continue
                candidate = self.root / raw_path / "docs" / name
                if candidate.is_dir():
                    directories.append(candidate)
        return sorted(set(directories))

    def is_canonical_record_target(self, target: Path, role: str) -> bool:
        if target.suffix.casefold() != ".md" or target.name.casefold() == "readme.md":
            return False
        if role == "evidence":
            match = DATED_DOCUMENT_RE.fullmatch(target.name)
            if match is None or not valid_date(match.group("date")):
                return False
        elif role == "decisions" and ADR_DOCUMENT_RE.fullmatch(target.name) is None:
            return False
        for directory in self.canonical_document_directories(role):
            try:
                target.relative_to(directory)
            except ValueError:
                continue
            return True
        return False

    def is_owner_evidence_target(self, target: Path, owner_id: str) -> bool:
        if not self.is_canonical_record_target(target, "evidence"):
            return False
        directory = self.owner_evidence_directories.get(owner_id)
        if directory is None:
            return False
        try:
            target.relative_to(directory)
        except ValueError:
            return False
        return True

    def canonical_plan_directories(self) -> set[Path]:
        owners: list[dict[str, object]] = []
        suite = self.registry.get("suite")
        if isinstance(suite, dict):
            owners.append(suite)
        projects = self.registry.get("projects", [])
        if isinstance(projects, list):
            owners.extend(project for project in projects if isinstance(project, dict))

        directories: set[Path] = set()
        for owner in owners:
            raw_path = owner.get("active_plans")
            if not isinstance(raw_path, str) or not raw_path or Path(raw_path).is_absolute():
                continue
            active = (self.root / raw_path).resolve(strict=False)
            if not inside_root(self.root, active):
                continue
            directories.update({active, active.parent / "archive"})
        return directories

    def is_canonical_plan_target(self, target: Path) -> bool:
        return target.parent in self.canonical_plan_directories()

    def plan_ledger_units(self, path: Path) -> set[str]:
        text = self.read_document(path)
        if text is None:
            return set()
        lines = text.splitlines()
        units: set[str] = set()
        cursor = 0
        while cursor + 1 < len(lines):
            if not lines[cursor].lstrip().startswith("|"):
                cursor += 1
                continue
            header = [" ".join(cell.casefold().split()) for cell in split_table_row(lines[cursor])]
            separator = split_table_row(lines[cursor + 1])
            if "unit" not in header or not is_separator_row(separator):
                cursor += 1
                continue
            unit_index = header.index("unit")
            cursor += 2
            while cursor < len(lines) and lines[cursor].lstrip().startswith("|"):
                cells = split_table_row(lines[cursor])
                cursor += 1
                if len(cells) == len(header):
                    unit = cells[unit_index].strip("` ")
                    if unit:
                        units.add(unit)
        return units

    def check_decisions(self) -> None:
        directories = self.canonical_document_directories("decisions")
        if not directories[0].is_dir():
            self.error(directories[0], "directorio de decisiones no existe")
            return
        for directory in directories:
            records = sorted(
                candidate
                for candidate in directory.rglob("*.md")
                if candidate.name != "README.md"
            )
            self.check_canonical_index(directory, records, "decisions")
            for path in records:
                if not ADR_DOCUMENT_RE.fullmatch(path.name):
                    self.error(path, "decisión requiere nombre `NNNN-short-topic.md`")
                text = self.read_document(path)
                if text is None:
                    continue
                fields = markdown_metadata(text)
                if not valid_date(fields.get("date", "")):
                    self.error(path, "decisión requiere `Date: YYYY-MM-DD`")
                status_value = normalized_status(fields.get("status", ""))
                if status_value not in DECISION_STATES:
                    self.error(path, f"estado de decisión inválido: {fields.get('status', '')}")
                self.required_section_bodies(
                    path,
                    text,
                    ("context", "decision", "consequences", "revisit when"),
                    "decisión",
                )

    def check_discussions(self) -> None:
        directories = self.canonical_document_directories("discussions")
        if not directories[0].is_dir():
            self.error(directories[0], "directorio de discusiones no existe")
            return
        for directory in directories:
            records = sorted(
                candidate
                for candidate in directory.rglob("*.md")
                if candidate.name != "README.md"
            )
            self.check_canonical_index(directory, records, "discussions")
            for path in records:
                self.check_dated_document_name(path, "discusión")
                text = self.read_document(path)
                if text is None:
                    continue
                fields = markdown_metadata(text)
                if not valid_date(fields.get("opened", "")):
                    self.error(path, "discusión requiere `Opened: YYYY-MM-DD`")
                discussion_status = normalized_status(fields.get("status", ""))
                if discussion_status not in DISCUSSION_STATES:
                    self.error(path, f"estado de discusión inválido: {fields.get('status', '')}")
                if not fields.get("question"):
                    self.error(path, "discusión requiere metadata `Question`")
                bodies = self.required_section_bodies(
                    path,
                    text,
                    (
                        "context",
                        "strongest case",
                        "counter-case",
                        "alternatives",
                        "falsifiers and evidence needed",
                        "conclusion",
                    ),
                    "discusión",
                )
                if discussion_status in {"concluded", "applied"}:
                    conclusion = "\n".join(bodies.get("conclusion", []))
                    first_content = next(
                        (line.strip(" -*_`.") for line in conclusion.splitlines() if line.strip()),
                        "",
                    )
                    if re.search(r"\bpending\b", first_content, re.IGNORECASE):
                        self.error(path, f"discusión {discussion_status} conserva Conclusion Pending")
                if discussion_status == "applied":
                    canonical_targets = [
                        candidate
                        for _line, target in extract_inline_links(text)
                        if (candidate := self.resolve_local_link(path, target)) is not None
                        and self.canonical_applied_target(candidate)
                    ]
                    if not canonical_targets:
                        self.error(path, "discusión applied requiere enlace al hogar canónico actualizado")

    def check_evidence(self) -> None:
        directories = self.canonical_document_directories("evidence")
        if not directories[0].is_dir():
            self.error(directories[0], "directorio de evidencia no existe")
            return
        for path in sorted(
            candidate
            for directory in directories
            for candidate in directory.rglob("*.md")
            if candidate.name != "README.md"
        ):
            self.check_dated_document_name(path, "evidencia")
            text = self.read_document(path)
            if text is None:
                continue
            fields = markdown_metadata(text)
            if not valid_date(fields.get("date", "")):
                self.error(path, "evidencia requiere `Date: YYYY-MM-DD`")
            for field in ("scope", "environment", "artifact"):
                if not fields.get(field):
                    self.error(path, f"evidencia requiere metadata `{field.title()}`")
            self.required_section_bodies(
                path,
                text,
                ("procedure", "result", "limits"),
                "evidencia",
            )

    def check_active_plan_directory(
        self,
        owner: dict[str, object],
        owner_label: str,
        owner_id: str,
    ) -> None:
        directory = self.registry_path(owner.get("active_plans"), f"{owner_label}.active_plans")
        if directory is None or not directory.is_dir():
            return
        for path in sorted(directory.glob("*.md")):
            if path.name == "README.md":
                continue
            self.check_dated_document_name(path, "plan activo")
            text = self.read_document(path)
            if text is None:
                continue
            fields = markdown_metadata(text)
            self.check_plan_id(path, fields, "plan activo", owner_id)
            if not valid_date(fields.get("opened", "")):
                self.error(path, "plan activo requiere `Opened: YYYY-MM-DD`")
            plan_status = normalized_status(fields.get("status", ""))
            if plan_status != "active":
                self.error(path, "un archivo bajo plans/active requiere `Status: active`")
            for field in ("scope", "implementation checkpoint", "author-validation checkpoint"):
                if not fields.get(field):
                    self.error(path, f"plan activo requiere metadata `{field.title()}`")
            scope = fields.get("scope", "")
            expected_scope = owner_id
            if not scope_matches_owner(scope, expected_scope, str(owner.get("name", ""))):
                self.error(path, f"Scope no corresponde al directorio activo de `{expected_scope}`")

            checkpoint = checkpoint_value(fields.get("implementation checkpoint", ""))
            if checkpoint and (
                no_active_checkpoint(checkpoint)
                or not CHECKPOINT_RE.fullmatch(checkpoint)
            ):
                self.error(path, f"checkpoint de plan activo inválido: {checkpoint}")
            if plan_status == "active" and checkpoint:
                self.active_plan_records.append((owner_id, path, checkpoint))

            headings = markdown_headings(text)
            normalized = {normalized_heading(heading): line for _level, heading, line in headings}
            for required in (
                "hypothesis",
                "tangible outcome",
                "scope",
                "exclusions",
                "build order",
                "implementation exit",
                "change and commit ledger",
            ):
                if required not in normalized:
                    self.error(path, f"plan activo requiere heading `{required.title()}`")
            ledger_line = normalized.get("change and commit ledger")
            if ledger_line is not None:
                self.check_ledger(path, text, ledger_line, owner_id)

    def check_archived_plan_directory(
        self,
        owner: dict[str, object],
        owner_label: str,
        owner_id: str,
    ) -> None:
        active = self.registry_path(owner.get("active_plans"), f"{owner_label}.active_plans")
        if active is None:
            return
        directory = active.parent / "archive"
        if not directory.is_dir():
            self.error(directory, "directorio de planes archivados no existe")
            return
        for path in sorted(directory.glob("*.md")):
            if path.name == "README.md":
                continue
            self.check_dated_document_name(path, "plan archivado")
            text = self.read_document(path)
            if text is None:
                continue
            fields = markdown_metadata(text)
            self.check_plan_id(path, fields, "plan archivado", owner_id)
            if not valid_date(fields.get("opened", "")):
                self.error(path, "plan archivado requiere `Opened: YYYY-MM-DD`")
            if not valid_date(fields.get("closed", "")):
                self.error(path, "plan archivado requiere `Closed: YYYY-MM-DD`")
            if normalized_status(fields.get("status", "")) != "done":
                self.error(path, "un archivo bajo plans/archive requiere `Status: done`")
            for field in (
                "scope",
                "implementation checkpoint",
                "author-validation checkpoint",
                "successor",
            ):
                if not fields.get(field):
                    self.error(path, f"plan archivado requiere metadata `{field.title()}`")
            if not scope_matches_owner(
                fields.get("scope", ""), owner_id, str(owner.get("name", ""))
            ):
                self.error(path, f"Scope no corresponde al archivo de `{owner_id}`")
            headings = markdown_headings(text)
            normalized = {normalized_heading(heading): line for _level, heading, line in headings}
            for required in (
                "hypothesis",
                "tangible outcome",
                "scope",
                "exclusions",
                "build order",
                "implementation exit",
                "change and commit ledger",
            ):
                if required not in normalized:
                    self.error(path, f"plan archivado requiere heading `{required.title()}`")
            ledger_line = normalized.get("change and commit ledger")
            if ledger_line is not None:
                self.check_ledger(path, text, ledger_line, owner_id, require_done=True)
                self.check_archived_plan_transition(path, owner_id)
            if re.search(r"PENDING FINAL|pending final split|0 files, \+0/-0", text, re.IGNORECASE):
                self.error(path, "plan archivado conserva placeholders de cierre")

    def check_archived_plan_transition(self, path: Path, owner_id: str) -> None:
        if not self.is_real_git_root():
            return
        plan_relative = self.relative(path)
        plan_directories = self.owner_plan_directories.get(owner_id)
        if plan_directories is None:
            return
        active_directory, archive_directory = plan_directories
        if posixpath.dirname(plan_relative) != archive_directory:
            return

        status = self.git_command(
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            plan_relative,
        )
        if status is None or status.returncode != 0 or status.stdout.strip():
            return
        addition = self.git_command(
            "log",
            "-1",
            "--format=%H",
            "--diff-filter=A",
            "--no-renames",
            "--",
            plan_relative,
        )
        if addition is None or addition.returncode != 0 or not addition.stdout.strip():
            return
        archive_commit = addition.stdout.decode("ascii").strip()
        parent = self.git_command("rev-parse", f"{archive_commit}^")
        if parent is None or parent.returncode != 0:
            return
        parent_commit = parent.stdout.decode("ascii").strip()
        active_relative = posixpath.join(active_directory, posixpath.basename(plan_relative))
        if not self.git_object_exists(f"{parent_commit}:{active_relative}"):
            return

        transition_found = False
        for _unit, _inventory, endpoint, row_contents in self.historical_plan_inventories.get(
            plan_relative, []
        ):
            if endpoint != archive_commit:
                continue
            if row_contents.get(active_relative) != "deleted":
                continue
            archive_content = row_contents.get(plan_relative)
            if archive_content is None or archive_content in {"deleted", "self"}:
                continue
            transition_found = True
            break
        if not transition_found:
            self.error(
                path,
                f"movimiento `{active_relative}` -> `{plan_relative}` en {archive_commit} "
                "requiere una unidad done con inventario del mismo endpoint que reclame "
                "la eliminación active y la adición archive",
            )

    def check_active_plans(self) -> None:
        suite = self.registry.get("suite")
        if not isinstance(suite, dict):
            return
        self.check_active_plan_directory(suite, "suite", "suite")
        self.check_archived_plan_directory(suite, "suite", "suite")
        projects = self.registry.get("projects", [])
        if not isinstance(projects, list):
            return
        seen_directories = {str(suite.get("active_plans", ""))}
        for project in projects:
            if not isinstance(project, dict) or "active_plans" not in project:
                continue
            raw_directory = str(project.get("active_plans", ""))
            if raw_directory in seen_directories:
                continue
            seen_directories.add(raw_directory)
            owner_id = str(project.get("id", "project"))
            self.check_active_plan_directory(project, owner_id, owner_id)
            self.check_archived_plan_directory(project, owner_id, owner_id)

    def git_command(self, *arguments: str) -> subprocess.CompletedProcess[bytes] | None:
        try:
            return subprocess.run(
                ["git", "-C", str(self.root), *arguments],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
        except OSError:
            return None

    def is_real_git_root(self) -> bool:
        if self._is_git_root is not None:
            return self._is_git_root
        result = self.git_command("rev-parse", "--show-toplevel")
        if result is None or result.returncode != 0:
            self._is_git_root = False
            return False
        try:
            top_level = Path(result.stdout.decode("utf-8").strip()).resolve()
        except UnicodeError:
            self._is_git_root = False
            return False
        self._is_git_root = top_level == self.root
        return self._is_git_root

    def git_object_exists(self, object_name: str) -> bool:
        result = self.git_command("cat-file", "-e", object_name)
        return result is not None and result.returncode == 0

    def parse_git_numstat(
        self,
        inventory_path: Path,
        unit: str,
        raw_path: str,
        output: bytes,
    ) -> tuple[str, str] | None:
        lines = [line for line in output.splitlines() if line]
        if not lines:
            return "0", "0"
        if len(lines) != 1:
            self.error(
                inventory_path,
                f"inventario de {unit}: Git no devuelve una única fila numstat para {raw_path}",
            )
            return None
        cells = lines[0].split(b"\t", maxsplit=2)
        values_are_lines = len(cells) == 3 and cells[0].isdigit() and cells[1].isdigit()
        values_are_binary = len(cells) == 3 and cells[0] == b"-" and cells[1] == b"-"
        if not values_are_lines and not values_are_binary:
            self.error(
                inventory_path,
                f"inventario de {unit}: Git devuelve numstat inválido para {raw_path}",
            )
            return None
        return cells[0].decode("ascii"), cells[1].decode("ascii")

    @staticmethod
    def path_matches_pathspec(raw_path: str, pathspec: str) -> bool:
        if pathspec == ".":
            return True
        if pathspec.endswith("/"):
            return raw_path.startswith(pathspec)
        return raw_path == pathspec

    @staticmethod
    def plan_identity(text: str) -> tuple[str, bool]:
        fields = markdown_metadata(text)
        explicit = checkpoint_value(fields.get("plan id", ""))
        return explicit, bool(explicit)

    @staticmethod
    def repository_link_path(document_path: str, target: str) -> str | None:
        parsed = urlsplit(target)
        if parsed.scheme or target.startswith("//"):
            return None
        raw_path = unquote(parsed.path)
        if not raw_path or raw_path.startswith("/"):
            return None
        normalized = posixpath.normpath(
            posixpath.join(posixpath.dirname(document_path), raw_path)
        )
        if normalized in {"", ".", ".."} or normalized.startswith("../"):
            return None
        return normalized

    def endpoint_plan_has_unit(
        self,
        plan_path: str,
        text: str,
        unit: str,
        inventory_path: str,
    ) -> bool:
        lines = text.splitlines()
        ledger_lines = [
            line_number
            for _level, heading, line_number in markdown_headings(text)
            if normalized_heading(heading) == "change and commit ledger"
        ]
        if len(ledger_lines) != 1:
            return False
        cursor = ledger_lines[0]
        while cursor < len(lines) and not lines[cursor].strip():
            cursor += 1
        while cursor < len(lines) and not lines[cursor].lstrip().startswith("|"):
            if HEADING_RE.match(lines[cursor]):
                return False
            cursor += 1
        if cursor + 1 >= len(lines) or not lines[cursor].lstrip().startswith("|"):
            return False
        header = [" ".join(cell.casefold().split()) for cell in split_table_row(lines[cursor])]
        if not is_separator_row(split_table_row(lines[cursor + 1])):
            return False
        required = {"unit", "status", "files / areas"}
        if not required.issubset(header):
            return False
        indexes = {column: header.index(column) for column in required}
        cursor += 2
        matches = 0
        while cursor < len(lines) and lines[cursor].lstrip().startswith("|"):
            cells = split_table_row(lines[cursor])
            cursor += 1
            if len(cells) != len(header):
                continue
            if cells[indexes["unit"]].strip("` ") != unit:
                continue
            if normalized_status(cells[indexes["status"]]) != "done":
                continue
            inventory_links = {
                resolved
                for _line, target in extract_inline_links(cells[indexes["files / areas"]])
                if (resolved := self.repository_link_path(plan_path, target)) is not None
                and resolved.endswith(".numstat.tsv")
            }
            if inventory_links == {inventory_path}:
                matches += 1
        return matches == 1

    def check_historical_plan_host(
        self,
        inventory_path: Path,
        current_plan_path: Path,
        unit: str,
        owner_id: str,
        endpoint: str,
        row_paths: set[str],
        current_plan_id: str,
        current_plan_id_explicit: bool,
    ) -> str | None:
        inventory_relative = self.relative(inventory_path)
        current_plan_relative = self.relative(current_plan_path)
        plan_moved = current_plan_relative not in row_paths
        stable_root = self.owner_inventory_roots.get(owner_id, "")
        inventory_is_stable = bool(stable_root) and inventory_relative.startswith(
            f"{stable_root}/"
        )

        if plan_moved and not inventory_is_stable:
            self.error(
                inventory_path,
                f"inventario histórico de {unit} sólo puede sobrevivir al movimiento del plan "
                f"desde el root estable `{stable_root}/`",
            )
            return None
        if plan_moved and (not current_plan_id or not current_plan_id_explicit):
            self.error(
                current_plan_path,
                f"plan movido con inventario histórico requiere metadata `Plan ID` estable "
                f"para {unit}",
            )
            return None

        plan_directories = self.owner_plan_directories.get(owner_id, ())
        candidate_paths = [
            raw_path
            for raw_path in sorted(row_paths)
            if raw_path.endswith(".md")
            and posixpath.basename(raw_path).casefold() != "readme.md"
            and posixpath.dirname(raw_path) in plan_directories
        ]
        matching_hosts: list[str] = []
        for candidate_path in candidate_paths:
            final_bytes = self.commit_path_bytes(endpoint, candidate_path)
            if final_bytes is None:
                continue
            try:
                endpoint_text = final_bytes.decode("utf-8")
            except UnicodeDecodeError:
                continue
            endpoint_plan_id, endpoint_plan_id_explicit = self.plan_identity(endpoint_text)
            if endpoint_plan_id != current_plan_id:
                continue
            if plan_moved and not endpoint_plan_id_explicit:
                continue
            if self.endpoint_plan_has_unit(
                candidate_path,
                endpoint_text,
                unit,
                inventory_relative,
            ):
                matching_hosts.append(candidate_path)

        if len(matching_hosts) != 1:
            rendered = current_plan_id or "<missing>"
            self.error(
                inventory_path,
                f"endpoint histórico de {unit} requiere un único plan host con Plan ID "
                f"`{rendered}`, unidad done y enlace al mismo inventario; "
                f"encontrados: {len(matching_hosts)}",
            )
            return None
        return matching_hosts[0]

    def check_inventory_claims(self) -> None:
        grouped: dict[str, list[tuple[Path, str, str, str, set[str]]]] = {}
        for group, inventory, unit, prefix, plan, paths in self.inventory_claims:
            grouped.setdefault(group, []).append((inventory, unit, prefix, plan, paths))

        for group, claims in grouped.items():
            prefixes = {prefix for _path, _unit, prefix, _plan, _paths in claims}
            if group.startswith("commit:") and len(prefixes) > 1:
                rendered = ", ".join(f"`{prefix}:`" for prefix in sorted(prefixes))
                self.error(
                    claims[-1][0],
                    f"unidades del mismo lote {group} usan prefijos incompatibles: {rendered}",
                )

            for index, (left_path, left_unit, _left_prefix, left_plan, left_paths) in enumerate(claims):
                for right_path, right_unit, _right_prefix, right_plan, right_paths in claims[index + 1 :]:
                    shared = left_paths.intersection(right_paths)
                    if left_plan == right_plan:
                        shared.discard(left_plan)
                    if not shared:
                        continue
                    rendered = ", ".join(f"`{path}`" for path in sorted(shared)[:10])
                    suffix = f" y {len(shared) - 10} más" if len(shared) > 10 else ""
                    self.error(
                        right_path,
                        f"inventarios {left_unit} ({self.relative(left_path)}) y {right_unit} "
                        f"reclaman las mismas rutas: {rendered}{suffix}",
                    )

            expected_group_paths = self.inventory_group_paths.get(group)
            if expected_group_paths is not None:
                claimed_paths = set().union(*(paths for _p, _u, _x, _l, paths in claims))
                missing = sorted(expected_group_paths - claimed_paths)
                if missing:
                    rendered = ", ".join(f"`{path}`" for path in missing[:10])
                    suffix = f" y {len(missing) - 10} más" if len(missing) > 10 else ""
                    self.error(
                        claims[-1][0],
                        f"lote histórico {group} contiene rutas sin inventario: "
                        f"{rendered}{suffix}",
                    )

    def current_path_bytes(self, raw_path: str) -> bytes | None:
        candidate = self.root / raw_path
        try:
            if candidate.is_symlink():
                return os.readlink(candidate).encode("utf-8")
            if candidate.is_file():
                return candidate.read_bytes()
        except (OSError, UnicodeError):
            return None
        return None

    def commit_path_bytes(self, commit: str, raw_path: str) -> bytes | None:
        if not self.git_object_exists(f"{commit}:{raw_path}"):
            return None
        result = self.git_command("show", f"{commit}:{raw_path}")
        if result is None or result.returncode != 0:
            return None
        return result.stdout

    def check_inventory_against_git(
        self,
        inventory_path: Path,
        plan_path: Path,
        unit: str,
        owner_id: str,
        commit_prefix: str,
        base_revision: str,
        pathspecs: set[str],
        rows: list[tuple[int, str, str, str, str]],
        current_plan_id: str,
        current_plan_id_explicit: bool,
    ) -> None:
        row_paths = {raw_path for _line, _add, _delete, _content, raw_path in rows}
        plan_relative = self.relative(plan_path)
        claim_plan_relative = plan_relative
        if not self.is_real_git_root():
            if plan_relative not in row_paths:
                self.error(
                    inventory_path,
                    f"inventario de {unit} no contiene el plan que aloja el ledger: "
                    f"{plan_relative}",
                )
            if pathspecs and plan_path.parent.name == "active":
                self.inventory_claims.append(
                    ("fixture", inventory_path, unit, commit_prefix, plan_relative, row_paths)
                )
            return
        if not self.git_object_exists(f"{base_revision}^{{commit}}"):
            self.error(inventory_path, f"Base revision no existe en Git: {base_revision}")
            return

        inventory_relative = self.relative(inventory_path)
        tracked = self.git_command("ls-files", "--error-unmatch", "--", inventory_relative)
        status = self.git_command(
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            inventory_relative,
        )
        if tracked is None or status is None:
            self.error(inventory_path, "no se pudo consultar el estado Git del inventario")
            return
        inventory_is_tracked = tracked.returncode == 0
        inventory_is_dirty = not inventory_is_tracked or bool(status.stdout.strip())

        if inventory_is_dirty:
            if plan_relative not in row_paths:
                self.error(
                    inventory_path,
                    f"inventario de {unit} no contiene el plan que aloja el ledger: "
                    f"{plan_relative}",
                )
            head = self.git_command("rev-parse", "HEAD")
            if head is None or head.returncode != 0:
                self.error(inventory_path, "no se pudo resolver HEAD para verificar el inventario")
                return
            endpoint = None
            ancestor_target = head.stdout.decode("ascii").strip()
            claim_group = f"worktree:{ancestor_target}"
            if base_revision != ancestor_target:
                self.error(
                    inventory_path,
                    f"Base revision de {unit} debe ser el HEAD previo al commit: "
                    f"{ancestor_target}",
                )
                return
        else:
            last_change = self.git_command("log", "-1", "--format=%H", "--", inventory_relative)
            if last_change is None or last_change.returncode != 0 or not last_change.stdout.strip():
                self.error(inventory_path, "no se pudo resolver el commit histórico del inventario")
                return
            endpoint = last_change.stdout.decode("ascii").strip()
            ancestor_target = endpoint
            claim_group = f"commit:{endpoint}"
            parent = self.git_command("rev-parse", f"{endpoint}^")
            if parent is None or parent.returncode != 0:
                self.error(inventory_path, "no se pudo resolver el padre del commit del inventario")
                return
            direct_parent = parent.stdout.decode("ascii").strip()
            if base_revision != direct_parent:
                self.error(
                    inventory_path,
                    f"Base revision de {unit} debe ser el padre directo del commit "
                    f"del inventario: {direct_parent}",
                )
                return

            subject = self.git_command("log", "-1", "--format=%s", endpoint)
            historical_prefix = ""
            if subject is not None and subject.returncode == 0:
                try:
                    historical_prefix, _action = parse_subject_prefix(
                        subject.stdout.decode("utf-8", "replace").strip()
                    )
                except ValueError:
                    historical_prefix = ""
            if historical_prefix != commit_prefix:
                self.error(
                    inventory_path,
                    f"commit histórico de {unit} requiere asunto con `{commit_prefix}: `",
                )
            historical_plan_host = self.check_historical_plan_host(
                inventory_path,
                plan_path,
                unit,
                owner_id,
                endpoint,
                row_paths,
                current_plan_id,
                current_plan_id_explicit,
            )
            if historical_plan_host is not None:
                claim_plan_relative = historical_plan_host
            self.historical_plan_inventories.setdefault(plan_relative, []).append(
                (
                    unit,
                    inventory_path,
                    endpoint,
                    {
                        raw_path: content
                        for _line, _added, _deleted, content, raw_path in rows
                    },
                )
            )

        ancestor = self.git_command("merge-base", "--is-ancestor", base_revision, ancestor_target)
        if ancestor is None or ancestor.returncode != 0:
            self.error(
                inventory_path,
                f"Base revision {base_revision} no es ancestro del endpoint {ancestor_target}",
            )
            return

        if endpoint is None:
            changed = self.git_command(
                "diff",
                "--no-ext-diff",
                "--name-only",
                "-z",
                "--no-renames",
                base_revision,
            )
            untracked = self.git_command(
                "ls-files",
                "--others",
                "--exclude-standard",
                "-z",
            )
            if (
                changed is None
                or changed.returncode != 0
                or untracked is None
                or untracked.returncode != 0
            ):
                self.error(inventory_path, "no se pudo enumerar el cambio Git actual")
                return
            worktree_paths = {
                os.fsdecode(raw_path)
                for raw_path in changed.stdout.split(b"\0") + untracked.stdout.split(b"\0")
                if raw_path
            }
        else:
            changed = self.git_command(
                "diff",
                "--no-ext-diff",
                "--name-only",
                "-z",
                "--no-renames",
                base_revision,
                endpoint,
            )
            if changed is None or changed.returncode != 0:
                self.error(inventory_path, "no se pudo enumerar el cambio Git histórico")
                return
            worktree_paths = {
                os.fsdecode(raw_path) for raw_path in changed.stdout.split(b"\0") if raw_path
            }
            commit_scope = self.commit_scopes.get(commit_prefix)
            if commit_scope is not None:
                outside_scope = sorted(
                    raw_path
                    for raw_path in worktree_paths
                    if not path_allowed(raw_path, commit_scope)
                )
                if outside_scope:
                    rendered = ", ".join(f"`{path}`" for path in outside_scope[:10])
                    suffix = (
                        f" y {len(outside_scope) - 10} más"
                        if len(outside_scope) > 10
                        else ""
                    )
                    self.error(
                        inventory_path,
                        f"commit histórico fuera de alcance de `{commit_prefix}:`: "
                        f"{rendered}{suffix}",
                    )
            self.inventory_group_paths.setdefault(claim_group, set()).update(worktree_paths)

        if pathspecs:
            self.inventory_claims.append(
                (
                    claim_group,
                    inventory_path,
                    unit,
                    commit_prefix,
                    claim_plan_relative,
                    row_paths,
                )
            )

        actual_paths = {
            raw_path
            for raw_path in worktree_paths
            if any(self.path_matches_pathspec(raw_path, pathspec) for pathspec in pathspecs)
        }

        inventory_paths = {raw_path for _line, _add, _delete, _content, raw_path in rows}
        missing_paths = sorted(actual_paths - inventory_paths)
        extra_paths = sorted(inventory_paths - actual_paths)
        if missing_paths:
            rendered = ", ".join(f"`{path}`" for path in missing_paths[:10])
            suffix = f" y {len(missing_paths) - 10} más" if len(missing_paths) > 10 else ""
            self.error(
                inventory_path,
                f"inventario de {unit} omite rutas cambiadas según Git: {rendered}{suffix}",
            )
        if extra_paths:
            rendered = ", ".join(f"`{path}`" for path in extra_paths[:10])
            suffix = f" y {len(extra_paths) - 10} más" if len(extra_paths) > 10 else ""
            self.error(
                inventory_path,
                f"inventario de {unit} contiene rutas sin cambio según Git: {rendered}{suffix}",
            )

        for line_number, expected_added, expected_deleted, content, raw_path in rows:
            if endpoint is None:
                base_has_path = self.git_object_exists(f"{base_revision}:{raw_path}")
                indexed = self.git_command("ls-files", "--error-unmatch", "--", raw_path)
                is_indexed = indexed is not None and indexed.returncode == 0
                if base_has_path or is_indexed:
                    diff = self.git_command(
                        "diff",
                        "--no-ext-diff",
                        "--numstat",
                        "--no-renames",
                        base_revision,
                        "--",
                        raw_path,
                    )
                    actual = (
                        self.parse_git_numstat(inventory_path, unit, raw_path, diff.stdout)
                        if diff is not None and diff.returncode == 0
                        else None
                    )
                else:
                    final_bytes = self.current_path_bytes(raw_path)
                    if final_bytes is None:
                        actual = None
                    else:
                        diff = self.git_command(
                            "diff",
                            "--no-index",
                            "--numstat",
                            "--no-renames",
                            "--",
                            "/dev/null",
                            str(self.root / raw_path),
                        )
                        if diff is not None and diff.returncode in {0, 1} and diff.stdout.strip():
                            actual = self.parse_git_numstat(
                                inventory_path, unit, raw_path, diff.stdout
                            )
                        elif diff is not None and diff.returncode in {0, 1}:
                            actual = ("0", "0")
                        else:
                            actual = None
                final_bytes = self.current_path_bytes(raw_path)
            else:
                diff = self.git_command(
                    "diff",
                    "--no-ext-diff",
                    "--numstat",
                    "--no-renames",
                    base_revision,
                    endpoint,
                    "--",
                    raw_path,
                )
                actual = (
                    self.parse_git_numstat(inventory_path, unit, raw_path, diff.stdout)
                    if diff is not None and diff.returncode == 0
                    else None
                )
                final_bytes = self.commit_path_bytes(endpoint, raw_path)

            if actual != (expected_added, expected_deleted):
                rendered = "sin cambio" if actual is None else f"{actual[0]}/{actual[1]}"
                self.error(
                    inventory_path,
                    f"línea {line_number}: numstat stale para {raw_path}; "
                    f"inventario {expected_added}/{expected_deleted}, Git {rendered}",
                )

            if content == "deleted":
                if final_bytes is not None:
                    self.error(
                        inventory_path,
                        f"línea {line_number}: {raw_path} usa `deleted` pero existe en el estado final",
                    )
            elif content == "self":
                if final_bytes is None:
                    self.error(
                        inventory_path,
                        f"línea {line_number}: el inventario `self` no existe en el estado final",
                    )
            elif final_bytes is None:
                self.error(
                    inventory_path,
                    f"línea {line_number}: falta el estado final de {raw_path} para verificar SHA-256",
                )
            else:
                actual_hash = hashlib.sha256(final_bytes).hexdigest()
                if content != actual_hash:
                    self.error(
                        inventory_path,
                        f"línea {line_number}: SHA-256 stale para {raw_path}; "
                        f"inventario {content}, estado final {actual_hash}",
                    )

    def check_numstat_inventory(
        self,
        inventory_path: Path,
        plan_path: Path,
        evidence_paths: list[Path],
        unit: str,
        expected_files: int,
        expected_added: int,
        expected_deleted: int,
        owner_id: str,
        commit_prefix: str,
    ) -> None:
        text = self.read_document(inventory_path)
        if text is None:
            return
        inventory_relative = self.relative(inventory_path)
        stable_root = self.owner_inventory_roots.get(owner_id, "")
        expected_inventory = posixpath.join(
            stable_root,
            plan_path.stem,
            f"{unit}.numstat.tsv",
        )
        if not stable_root or inventory_relative != expected_inventory:
            self.error(
                inventory_path,
                f"inventario de {unit} debe vivir exactamente en "
                f"`{expected_inventory}`",
            )
        plan_text = self.read_document(plan_path)
        current_plan_id, current_plan_id_explicit = self.plan_identity(plan_text or "")
        lines = text.splitlines()
        base_revision_lines = [line for line in lines if line.startswith("Base revision")]
        base_revision_valid = len(base_revision_lines) == 1 and BASE_REVISION_RE.fullmatch(
            base_revision_lines[0] if base_revision_lines else ""
        )
        if not base_revision_valid:
            self.error(
                inventory_path,
                f"inventario de {unit} requiere una única línea "
                "`Base revision<TAB><40 hex>`",
            )
        base_revision = base_revision_lines[0].split("\t", maxsplit=1)[1] if base_revision_valid else ""
        pathspecs: set[str] = set()
        for index, line in enumerate(lines, start=1):
            if not line.startswith("Pathspec"):
                continue
            cells = line.split("\t")
            if len(cells) != 2 or cells[0] != "Pathspec":
                self.error(
                    inventory_path,
                    f"línea {index}: Pathspec requiere `Pathspec<TAB>ruta`",
                )
                continue
            pathspec = cells[1]
            comparable = pathspec[:-1] if pathspec.endswith("/") else pathspec
            parts = comparable.split("/")
            pathspec_valid = not (
                not pathspec
                or pathspec.startswith("/")
                or pathspec.startswith("./")
                or pathspec == ".."
                or any(part in {"", ".", ".."} for part in parts)
            )
            if pathspec == ".":
                pathspec_valid = owner_id == "suite"
            if not pathspec_valid:
                self.error(
                    inventory_path,
                    f"línea {index}: Pathspec no es una frontera normalizada permitida "
                    f"para `{owner_id}`: {pathspec}",
                )
            elif pathspec in pathspecs:
                self.error(inventory_path, f"línea {index}: Pathspec duplicado: {pathspec}")
            else:
                pathspecs.add(pathspec)
                commit_scope = self.commit_scopes.get(commit_prefix)
                if commit_scope is not None and not pathspec_allowed(pathspec, commit_scope):
                    self.error(
                        inventory_path,
                        f"línea {index}: Pathspec fuera del alcance de `{commit_prefix}:`: "
                        f"{pathspec}",
                    )
        if not pathspecs:
            self.error(inventory_path, f"inventario de {unit} requiere al menos un `Pathspec`")
        header_lines = [
            index
            for index, line in enumerate(lines)
            if tuple(line.split("\t")) == NUMSTAT_HEADER
        ]
        if len(header_lines) != 1:
            self.error(
                inventory_path,
                f"inventario de {unit} requiere una única cabecera "
                "`added\\tdeleted\\tcontent\\tpath`",
            )
            return

        paths: set[str] = set()
        evidence_relatives = {self.relative(candidate) for candidate in evidence_paths}
        self_rows = 0
        git_rows: list[tuple[int, str, str, str, str]] = []
        added_total = 0
        deleted_total = 0
        for index, line in enumerate(lines[header_lines[0] + 1 :], start=header_lines[0] + 2):
            if not line.strip():
                continue
            cells = line.split("\t")
            if len(cells) != len(NUMSTAT_HEADER):
                self.error(
                    inventory_path,
                    f"línea {index}: fila numstat requiere cuatro columnas separadas por tabulador",
                )
                continue
            added, deleted, content, raw_path = cells
            values_are_lines = added.isdecimal() and deleted.isdecimal()
            values_are_binary = added == "-" and deleted == "-"
            if (
                not NUMSTAT_VALUE_RE.fullmatch(added)
                or not NUMSTAT_VALUE_RE.fullmatch(deleted)
                or not (values_are_lines or values_are_binary)
            ):
                self.error(
                    inventory_path,
                    f"línea {index}: added/deleted deben ser enteros no negativos o `-/-`",
                )
                continue
            content_valid = NUMSTAT_CONTENT_RE.fullmatch(content) is not None
            if not content_valid:
                self.error(
                    inventory_path,
                    f"línea {index}: content debe ser SHA-256 o marcador de cierre permitido",
                )
            if content == "self":
                self_rows += 1
                if raw_path != inventory_relative:
                    self.error(
                        inventory_path,
                        f"línea {index}: marcador `self` sólo pertenece al propio inventario",
                    )
            elif raw_path == inventory_relative:
                self.error(
                    inventory_path,
                    f"línea {index}: la fila del propio inventario requiere marcador `self`",
                )
            path_parts = raw_path.split("/")
            path_valid = not (
                not raw_path
                or raw_path.startswith("/")
                or raw_path.startswith("./")
                or raw_path.endswith("/")
                or any(part in {"", ".", ".."} for part in path_parts)
            )
            if not path_valid:
                self.error(
                    inventory_path,
                    f"línea {index}: ruta numstat no es relativa y normalizada: {raw_path}",
                )
            elif raw_path in paths:
                self.error(inventory_path, f"línea {index}: ruta numstat duplicada: {raw_path}")
            else:
                paths.add(raw_path)
                if pathspecs and not any(
                    self.path_matches_pathspec(raw_path, pathspec) for pathspec in pathspecs
                ):
                    self.error(
                        inventory_path,
                        f"línea {index}: ruta fuera de Pathspec para {unit}: {raw_path}",
                    )
            if values_are_lines:
                added_total += int(added)
                deleted_total += int(deleted)
            if content_valid and path_valid:
                git_rows.append((index, added, deleted, content, raw_path))

        if self_rows != 1:
            self.error(
                inventory_path,
                f"inventario de {unit} requiere exactamente una fila `self` propia",
            )
        if evidence_relatives and not paths.intersection(evidence_relatives):
            self.error(
                inventory_path,
                f"inventario de {unit} no contiene ningún expediente enlazado "
                "desde `Automated evidence`",
            )
        if len(paths) != expected_files:
            self.error(
                inventory_path,
                f"inventario de {unit} declara {len(paths)} rutas únicas; "
                f"Diffstat declara {expected_files} archivos",
            )
        if added_total != expected_added or deleted_total != expected_deleted:
            self.error(
                inventory_path,
                f"inventario de {unit} suma +{added_total}/-{deleted_total}; "
                f"Diffstat declara +{expected_added}/-{expected_deleted}",
            )
        if base_revision:
            self.check_inventory_against_git(
                inventory_path,
                plan_path,
                unit,
                owner_id,
                commit_prefix,
                base_revision,
                pathspecs,
                git_rows,
                current_plan_id,
                current_plan_id_explicit,
            )

    def check_done_ledger_row(
        self,
        path: Path,
        unit: str,
        line_number: int,
        files_cell: str,
        evidence_cell: str,
        diffstat_match: re.Match[str],
        owner_id: str,
        commit_prefix: str,
    ) -> None:
        canonical_evidence_paths = [
            candidate
            for candidate in self.resolvable_links(path, evidence_cell)
            if candidate.suffix.casefold() == ".md"
            and candidate.name.casefold() != "readme.md"
            and self.is_canonical_record_target(candidate, "evidence")
        ]
        evidence_paths = [
            candidate
            for candidate in canonical_evidence_paths
            if self.is_owner_evidence_target(candidate, owner_id)
        ]
        foreign_evidence_paths = set(canonical_evidence_paths) - set(evidence_paths)
        if foreign_evidence_paths:
            self.error(
                path,
                f"ledger línea {line_number}: Automated evidence fuera del owner "
                f"`{owner_id}`",
            )
        if not evidence_paths:
            self.error(
                path,
                f"ledger línea {line_number}: unidad done requiere expediente resoluble "
                f"bajo `evidence/` del owner `{owner_id}` en `Automated evidence`",
            )

        inventory_targets = [
            target
            for _line, target in extract_inline_links(files_cell)
            if Path(unquote(urlsplit(target).path)).suffix.casefold() == ".tsv"
            and unquote(urlsplit(target).path).casefold().endswith(".numstat.tsv")
        ]
        if len(inventory_targets) != 1:
            self.error(
                path,
                f"ledger línea {line_number}: unidad done requiere un único enlace "
                "a inventario `.numstat.tsv` en `Files / areas`",
            )
        else:
            inventory_path = self.resolve_local_link(path, inventory_targets[0])
            if inventory_path is None:
                self.error(
                    path,
                    f"ledger línea {line_number}: inventario `.numstat.tsv` no es resoluble",
                )
            else:
                previous = self.inventory_units.get(inventory_path)
                if previous is not None:
                    previous_plan, previous_unit, previous_line = previous
                    self.error(
                        path,
                        f"ledger línea {line_number}: inventario ya pertenece a {previous_unit} "
                        f"en {self.relative(previous_plan)}:{previous_line}",
                    )
                else:
                    self.inventory_units[inventory_path] = (path, unit, line_number)
                    self.check_numstat_inventory(
                        inventory_path,
                        path,
                        evidence_paths,
                        unit,
                        int(diffstat_match.group("files")),
                        int(diffstat_match.group("added")),
                        int(diffstat_match.group("deleted")),
                        owner_id,
                        commit_prefix,
                    )

    def check_ledger(
        self,
        path: Path,
        text: str,
        heading_line: int,
        owner_id: str,
        *,
        require_done: bool = False,
    ) -> None:
        lines = text.splitlines()
        cursor = heading_line
        while cursor < len(lines) and not lines[cursor].strip():
            cursor += 1
        while cursor < len(lines) and not lines[cursor].lstrip().startswith("|"):
            if HEADING_RE.match(lines[cursor]):
                break
            cursor += 1
        if cursor + 1 >= len(lines) or not lines[cursor].lstrip().startswith("|"):
            self.error(path, "ledger no contiene tabla Markdown")
            return
        header = [" ".join(cell.casefold().split()) for cell in split_table_row(lines[cursor])]
        separator = split_table_row(lines[cursor + 1])
        if not is_separator_row(separator):
            self.error(path, "ledger no contiene separador de tabla válido")
            return
        missing = [column for column in LEDGER_COLUMNS if column not in header]
        if missing:
            self.error(path, f"ledger carece de columnas: {', '.join(missing)}")
            return
        indexes = {column: header.index(column) for column in LEDGER_COLUMNS}
        cursor += 2
        row_count = 0
        units: set[str] = set()
        while cursor < len(lines) and lines[cursor].lstrip().startswith("|"):
            cells = split_table_row(lines[cursor])
            cursor += 1
            if len(cells) != len(header):
                self.error(path, f"ledger línea {cursor}: número de celdas incorrecto")
                continue
            row_count += 1
            unit = cells[indexes["unit"]].strip("` ")
            if not unit:
                self.error(path, f"ledger línea {cursor}: Unit vacío")
            elif unit in units:
                self.error(path, f"ledger línea {cursor}: Unit duplicado: {unit}")
            units.add(unit)

            prefix = cells[indexes["commit prefix"]].strip("` ")
            allowed_prefixes = self.owner_prefixes.get(owner_id, set())
            if not prefix.endswith(":") or prefix[:-1] not in allowed_prefixes:
                allowed = ", ".join(f"{item}:" for item in sorted(allowed_prefixes)) or "ninguno"
                self.error(
                    path,
                    f"ledger línea {cursor}: prefijo fuera del owner `{owner_id}`: "
                    f"{prefix} (permitidos: {allowed})",
                )
            status_value = normalized_status(cells[indexes["status"]])
            if status_value not in PLAN_STATES:
                self.error(path, f"ledger línea {cursor}: estado inválido: {status_value}")
            elif require_done and status_value != "done":
                self.error(path, f"ledger línea {cursor}: plan archivado requiere unidades done")
            for column in ("files / areas", "intended change", "automated evidence", "author validation"):
                if not cells[indexes[column]].strip():
                    self.error(path, f"ledger línea {cursor}: `{column}` vacío")
            diffstat = cells[indexes["diffstat"]].strip()
            if not diffstat:
                self.error(path, f"ledger línea {cursor}: `Diffstat` vacío")
            elif status_value == "done":
                final_diffstat = diffstat.strip("` ")
                diffstat_match = DONE_DIFFSTAT_RE.fullmatch(final_diffstat)
                if diffstat_match is None:
                    self.error(
                        path,
                        f"ledger línea {cursor}: unidad done requiere diffstat "
                        "`N files, +X/-Y`",
                    )
                else:
                    self.check_done_ledger_row(
                        path,
                        unit,
                        cursor,
                        cells[indexes["files / areas"]],
                        cells[indexes["automated evidence"]],
                        diffstat_match,
                        owner_id,
                        prefix[:-1] if prefix.endswith(":") else prefix,
                    )
        if row_count == 0:
            self.error(path, "ledger no contiene unidades")

    def check_roadmap_plan_links(self) -> None:
        plans_by_owner: dict[str, list[tuple[Path, str]]] = {}
        for owner_id, path, checkpoint in self.active_plan_records:
            plans_by_owner.setdefault(owner_id, []).append((path, checkpoint))

        for owner_id, (roadmap_path, status_value, checkpoint) in self.roadmaps.items():
            if status_value not in {"active", "blocked"}:
                continue
            matching = [
                path
                for path, plan_checkpoint in plans_by_owner.get(owner_id, [])
                if plan_checkpoint == checkpoint
            ]
            if not matching:
                self.error(
                    roadmap_path,
                    f"ROADMAP {status_value} requiere exactamente un plan activo del owner "
                    f"`{owner_id}` para `{checkpoint}`",
                )
            elif len(matching) > 1:
                self.error(
                    roadmap_path,
                    f"ROADMAP `{owner_id}` tiene varios planes activos para `{checkpoint}`",
                )

        for owner_id, plan_path, checkpoint in self.active_plan_records:
            roadmap = self.roadmaps.get(owner_id)
            if roadmap is None:
                self.error(plan_path, f"plan activo huérfano: owner `{owner_id}` sin ROADMAP")
                continue
            _roadmap_path, status_value, roadmap_checkpoint = roadmap
            if status_value not in {"active", "blocked"}:
                self.error(
                    plan_path,
                    f"plan activo huérfano: ROADMAP `{owner_id}` está {status_value}",
                )
            elif checkpoint != roadmap_checkpoint:
                self.error(
                    plan_path,
                    f"checkpoint de plan `{checkpoint}` no coincide con ROADMAP "
                    f"`{roadmap_checkpoint}` del owner `{owner_id}`",
                )

    def run(self) -> list[str]:
        self.check_registry()
        files = iter_repository_files(self.root)
        self.check_vendor_files(files)
        self.check_markdown_links(files)
        if self.registry:
            self.check_decisions()
            self.check_discussions()
            self.check_evidence()
            self.check_active_plans()
            self.check_inventory_claims()
            self.check_roadmap_plan_links()
        return sorted(set(self.errors))


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=repository_root(),
        help="repository root (defaults to the checkout containing this script)",
    )
    parser.add_argument("--quiet", action="store_true", help="print only errors")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    arguments = parse_args(sys.argv[1:] if argv is None else argv)
    root = arguments.root.resolve()
    errors = DocumentationContract(root).run()
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        print(f"Contrato de documentación: {len(errors)} error(es).", file=sys.stderr)
        return 1
    if not arguments.quiet:
        print("Contrato de documentación: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
