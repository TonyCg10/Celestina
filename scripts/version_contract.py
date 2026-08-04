#!/usr/bin/env python3
"""Interpret and validate project versions declared by ``docs/projects.toml``.

The registry identifies version sources; it never duplicates their current
values.  This module is deliberately stdlib-only because the committed copy is
loaded by the commit hook to interpret HEAD and INDEX data.
"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Callable, Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import PurePosixPath
import re
import tomllib


VERSION_KINDS = frozenset({"baseline", "bug", "milestone", "release"})
DELIVERY_KINDS = frozenset({"bug", "milestone", "release"})
COMMIT_KINDS = DELIVERY_KINDS | {"maintenance"}
NON_REPLAY_WRAPPERS = frozenset({"fixup", "squash", "amend"})
SOURCE_KINDS = frozenset({"cargo-package", "cargo-lock", "cmake-project"})
SEMVER_RE = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)")
PROJECT_CALL_RE = re.compile(
    r"^[ \t]*project[ \t]*\([ \t]*(?P<name>[A-Za-z0-9_.+\-]+)(?P<body>.*?)\)",
    re.IGNORECASE | re.MULTILINE | re.DOTALL,
)
PROJECT_VERSION_RE = re.compile(
    r"\bVERSION[ \t\r\n]+(?P<version>[^ \t\r\n\)]+)", re.IGNORECASE
)


class VersionContractError(ValueError):
    """Base class for all fail-closed version contract errors."""


class VersionRegistryError(VersionContractError):
    """The registry's version schema is invalid or changed illegally."""


class VersionSourceError(VersionContractError):
    """A configured version source cannot be read unambiguously."""


class VersionHistoryError(VersionContractError):
    """The append-only version history is malformed or inconsistent."""


class VersionTransitionError(VersionContractError):
    """A staged or requested version transition violates the policy."""


@dataclass(frozen=True, order=True)
class SemVer:
    major: int
    minor: int
    patch: int

    @classmethod
    def parse(cls, value: object, label: str) -> "SemVer":
        if not isinstance(value, str) or SEMVER_RE.fullmatch(value) is None:
            raise VersionSourceError(
                f"{label}: expected strict SemVer X.Y.Z without prerelease or build metadata"
            )
        major, minor, patch = (int(part) for part in value.split("."))
        return cls(major, minor, patch)

    def bumped(self, kind: str) -> "SemVer":
        if kind == "bug":
            return SemVer(self.major, self.minor, self.patch + 1)
        if kind == "milestone":
            return SemVer(self.major, self.minor + 1, 0)
        if kind == "release":
            return SemVer(self.major + 1, 0, 0)
        raise VersionTransitionError(f'cannot bump a version with kind "{kind}"')

    def __str__(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"


@dataclass(frozen=True)
class SourceSpec:
    kind: str
    path: str
    name: str | None = None


@dataclass(frozen=True)
class OwnerConfig:
    owner: str
    prefix: str
    versioned: bool
    source: SourceSpec | None = None
    mirrors: tuple[SourceSpec, ...] = ()

    @property
    def sources(self) -> tuple[SourceSpec, ...]:
        if self.source is None:
            return ()
        return (self.source, *self.mirrors)


@dataclass(frozen=True)
class VersionPolicy:
    history_file: str
    tag_format: str
    bug_increment: str
    milestone_increment: str
    release_increment: str
    maintenance_increment: str


@dataclass(frozen=True)
class RegistryModel:
    policy: VersionPolicy | None
    suite_prefix: str
    owners: tuple[OwnerConfig, ...]
    component_prefixes: frozenset[str]

    def owner_map(self) -> dict[str, OwnerConfig]:
        return {owner.owner: owner for owner in self.owners}

    def prefix_map(self) -> dict[str, OwnerConfig]:
        return {owner.prefix: owner for owner in self.owners}


@dataclass(frozen=True)
class HistoryRow:
    owner: str
    version: SemVer
    kind: str
    unit: str
    summary: str


@dataclass(frozen=True)
class Snapshot:
    versions: Mapping[str, SemVer]
    history: tuple[HistoryRow, ...]
    history_raw: bytes


BlobReader = Callable[[str, str], bytes | str | None]


def _normalized_path(value: object, label: str) -> str:
    if not isinstance(value, str) or not value or value.endswith("/"):
        raise VersionRegistryError(f"{label} must be an exact relative file path")
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or str(path) != value
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise VersionRegistryError(f"{label} is not a normalized relative path: {value}")
    return value


def _required_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip() or value != value.strip():
        raise VersionRegistryError(f"{label} must be a non-empty trimmed string")
    return value


def parse_source_spec(raw: object, label: str) -> SourceSpec:
    if not isinstance(raw, Mapping):
        raise VersionRegistryError(f"{label} must be an inline table")
    kind = raw.get("kind")
    if not isinstance(kind, str) or kind not in SOURCE_KINDS:
        known = ", ".join(sorted(SOURCE_KINDS))
        raise VersionRegistryError(f"{label}.kind must be one of: {known}")
    path = _normalized_path(raw.get("path"), f"{label}.path")
    selector = "project" if kind == "cmake-project" else "package"
    name = _required_string(raw.get(selector), f"{label}.{selector}")
    return SourceSpec(str(kind), path, name)


def parse_registry(registry: Mapping[str, object], label: str = "registry") -> RegistryModel:
    if not isinstance(registry, Mapping):
        raise VersionRegistryError(f"{label} must be a TOML table")
    suite = registry.get("suite")
    if not isinstance(suite, Mapping):
        raise VersionRegistryError(f"{label}.suite must be a table")
    suite_prefix = _required_string(
        suite.get("commit_prefix"), f"{label}.suite.commit_prefix"
    )

    raw_policy = registry.get("version_policy")
    if raw_policy is None:
        policy = None
    else:
        if not isinstance(raw_policy, Mapping):
            raise VersionRegistryError(f"{label}.version_policy must be a table")
        expected_policy = {
            "tag_format": "<project>-v<version>",
            "bug_increment": "patch",
            "milestone_increment": "minor",
            "release_increment": "major",
            "maintenance_increment": "none",
        }
        for field, expected in expected_policy.items():
            if raw_policy.get(field) != expected:
                raise VersionRegistryError(
                    f"{label}.version_policy.{field} must be {expected!r}"
                )
        policy = VersionPolicy(
            history_file=_normalized_path(
                raw_policy.get("history_file"),
                f"{label}.version_policy.history_file",
            ),
            tag_format=str(raw_policy["tag_format"]),
            bug_increment=str(raw_policy["bug_increment"]),
            milestone_increment=str(raw_policy["milestone_increment"]),
            release_increment=str(raw_policy["release_increment"]),
            maintenance_increment=str(raw_policy["maintenance_increment"]),
        )

    raw_projects = registry.get("projects", [])
    if not isinstance(raw_projects, Sequence) or isinstance(raw_projects, (str, bytes)):
        raise VersionRegistryError(f"{label}.projects must be an array of tables")
    owners: list[OwnerConfig] = []
    component_prefixes: set[str] = set()
    seen_ids: set[str] = set()
    seen_prefixes: set[str] = {suite_prefix}
    for index, raw_project in enumerate(raw_projects):
        project_label = f"{label}.projects[{index}]"
        if not isinstance(raw_project, Mapping):
            raise VersionRegistryError(f"{project_label} must be a table")
        owner = _required_string(raw_project.get("id"), f"{project_label}.id")
        prefix = _required_string(
            raw_project.get("commit_prefix"), f"{project_label}.commit_prefix"
        )
        if owner in seen_ids:
            raise VersionRegistryError(f'{label} repeats project id "{owner}"')
        if prefix in seen_prefixes:
            raise VersionRegistryError(f'{label} repeats commit prefix "{prefix}"')
        seen_ids.add(owner)
        seen_prefixes.add(prefix)

        raw_versioned = raw_project.get("versioned")
        if raw_versioned is not None and not isinstance(raw_versioned, bool):
            raise VersionRegistryError(f"{project_label}.versioned must be boolean")
        has_source = "version_source" in raw_project
        versioned = bool(raw_versioned) if raw_versioned is not None else has_source
        if policy is not None and raw_versioned is None and not has_source:
            raise VersionRegistryError(
                f"{project_label} must declare version_source or versioned = false"
            )
        if not versioned:
            if has_source or raw_project.get("version_mirrors"):
                raise VersionRegistryError(
                    f"{project_label} is unversioned and cannot declare version sources"
                )
            source = None
            mirrors: tuple[SourceSpec, ...] = ()
        else:
            if not has_source:
                raise VersionRegistryError(f"{project_label} requires one version_source")
            source = parse_source_spec(
                raw_project.get("version_source"), f"{project_label}.version_source"
            )
            raw_mirrors = raw_project.get("version_mirrors", [])
            if not isinstance(raw_mirrors, Sequence) or isinstance(
                raw_mirrors, (str, bytes)
            ):
                raise VersionRegistryError(f"{project_label}.version_mirrors must be a list")
            mirrors = tuple(
                parse_source_spec(value, f"{project_label}.version_mirrors[{mirror_index}]")
                for mirror_index, value in enumerate(raw_mirrors)
            )
            identities = {
                (item.kind, item.path, item.name) for item in (source, *mirrors)
            }
            if len(identities) != 1 + len(mirrors):
                raise VersionRegistryError(f"{project_label} repeats a version source")
        owners.append(OwnerConfig(owner, prefix, versioned, source, mirrors))

        raw_components = raw_project.get("component_commit_scopes", [])
        if not isinstance(raw_components, Sequence) or isinstance(
            raw_components, (str, bytes)
        ):
            raise VersionRegistryError(
                f"{project_label}.component_commit_scopes must be a list"
            )
        for component_index, component in enumerate(raw_components):
            component_label = f"{project_label}.component_commit_scopes[{component_index}]"
            if not isinstance(component, Mapping):
                raise VersionRegistryError(f"{component_label} must be a table")
            component_prefix = _required_string(
                component.get("prefix"), f"{component_label}.prefix"
            )
            if component_prefix in seen_prefixes or component_prefix in component_prefixes:
                raise VersionRegistryError(
                    f'{label} repeats commit prefix "{component_prefix}"'
                )
            component_prefixes.add(component_prefix)
            seen_prefixes.add(component_prefix)

    return RegistryModel(policy, suite_prefix, tuple(owners), frozenset(component_prefixes))


def _decode(raw: bytes | str, label: str) -> str:
    if isinstance(raw, str):
        return raw
    if not isinstance(raw, bytes):
        raise VersionSourceError(f"{label}: expected bytes")
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise VersionSourceError(f"{label}: source is not UTF-8: {error}") from error


def _parse_toml(text: str, label: str) -> dict[str, object]:
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise VersionSourceError(f"{label}: invalid TOML: {error}") from error
    if not isinstance(data, dict):
        raise VersionSourceError(f"{label}: expected a TOML table")
    return data


def read_source_version(spec: SourceSpec, raw: bytes | str, label: str) -> SemVer:
    text = _decode(raw, label)
    if spec.kind == "cargo-package":
        package = _parse_toml(text, label).get("package")
        if not isinstance(package, Mapping):
            raise VersionSourceError(f"{label}: missing [package] table")
        name = package.get("name")
        if not isinstance(name, str) or not name:
            raise VersionSourceError(f"{label}: [package].name must be a string")
        if spec.name is not None and name != spec.name:
            raise VersionSourceError(
                f'{label}: expected package "{spec.name}", found "{name}"'
            )
        return SemVer.parse(package.get("version"), f"{label} [package].version")

    if spec.kind == "cargo-lock":
        packages = _parse_toml(text, label).get("package")
        if not isinstance(packages, list):
            raise VersionSourceError(f"{label}: missing [[package]] entries")
        matches = [
            package
            for package in packages
            if isinstance(package, Mapping) and package.get("name") == spec.name
        ]
        if len(matches) != 1:
            raise VersionSourceError(
                f'{label}: expected exactly one [[package]] named "{spec.name}", '
                f"found {len(matches)}"
            )
        return SemVer.parse(
            matches[0].get("version"), f'{label} package "{spec.name}" version'
        )

    calls: list[tuple[re.Match[str], re.Match[str]]] = []
    for call in PROJECT_CALL_RE.finditer(text):
        if spec.name is not None and call.group("name") != spec.name:
            continue
        version_match = PROJECT_VERSION_RE.search(call.group("body"))
        if version_match is not None:
            calls.append((call, version_match))
    if len(calls) != 1:
        selector = f' named "{spec.name}"' if spec.name is not None else ""
        raise VersionSourceError(
            f"{label}: expected exactly one CMake project{selector} with VERSION, "
            f"found {len(calls)}"
        )
    return SemVer.parse(
        calls[0][1].group("version"), f"{label} project VERSION"
    )


def parse_history(raw: bytes | str, label: str) -> tuple[HistoryRow, ...]:
    text = _decode(raw, label)
    rows: list[HistoryRow] = []
    seen_units: set[tuple[str, str]] = set()
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 5:
            raise VersionHistoryError(
                f"{label}:{number}: expected owner, version, kind, unit, and summary TSV fields"
            )
        owner, version_text, kind, unit, summary = fields
        if not owner or owner != owner.strip():
            raise VersionHistoryError(f"{label}:{number}: owner must be non-empty and trimmed")
        if kind not in VERSION_KINDS:
            raise VersionHistoryError(f"{label}:{number}: unknown version kind: {kind}")
        if not unit or unit != unit.strip():
            raise VersionHistoryError(f"{label}:{number}: unit must be non-empty and trimmed")
        if not summary or summary != summary.strip():
            raise VersionHistoryError(f"{label}:{number}: summary must be non-empty and trimmed")
        key = (owner, unit)
        if key in seen_units:
            raise VersionHistoryError(
                f'{label}:{number}: owner "{owner}" repeats unit "{unit}"'
            )
        seen_units.add(key)
        rows.append(
            HistoryRow(
                owner,
                SemVer.parse(version_text, f"{label}:{number} version"),
                kind,
                unit,
                summary,
            )
        )
    return tuple(rows)


def _validate_history(
    model: RegistryModel,
    versions: Mapping[str, SemVer],
    rows: Sequence[HistoryRow],
    label: str,
) -> None:
    owners = model.owner_map()
    grouped: dict[str, list[HistoryRow]] = defaultdict(list)
    for row in rows:
        config = owners.get(row.owner)
        if config is None:
            raise VersionHistoryError(f'{label}: history names unknown owner "{row.owner}"')
        if not config.versioned:
            raise VersionHistoryError(f'{label}: unversioned owner "{row.owner}" has history')
        grouped[row.owner].append(row)

    for owner in model.owners:
        if not owner.versioned:
            continue
        owner_rows = grouped.get(owner.owner, [])
        if not owner_rows:
            raise VersionHistoryError(f'{label}: versioned owner "{owner.owner}" has no baseline')
        if owner_rows[0].kind != "baseline":
            raise VersionHistoryError(
                f'{label}: first row for "{owner.owner}" must be baseline'
            )
        for previous, current in zip(owner_rows, owner_rows[1:]):
            if current.kind not in DELIVERY_KINDS:
                raise VersionHistoryError(
                    f'{label}: later row for "{owner.owner}" cannot be {current.kind}'
                )
            expected = previous.version.bumped(current.kind)
            if current.version != expected:
                raise VersionHistoryError(
                    f'{label}: {owner.owner} {current.kind} must move '
                    f"{previous.version} -> {expected}, found {current.version}"
                )
        current_version = versions.get(owner.owner)
        if current_version is None:
            raise VersionSourceError(f'{label}: no source version for "{owner.owner}"')
        if owner_rows[-1].version != current_version:
            raise VersionHistoryError(
                f'{label}: last history version for "{owner.owner}" is '
                f"{owner_rows[-1].version}, source is {current_version}"
            )


def _blob(read_blob: BlobReader, revision: str, path: str) -> bytes:
    try:
        raw = read_blob(revision, path)
    except Exception as error:
        if isinstance(error, VersionContractError):
            raise
        raise VersionSourceError(
            f"{revision}:{path}: blob reader failed: {error}"
        ) from error
    if raw is None:
        raise VersionSourceError(f"{revision}:{path}: file is missing")
    if isinstance(raw, str):
        return raw.encode("utf-8")
    if not isinstance(raw, bytes):
        raise VersionSourceError(f"{revision}:{path}: blob reader returned invalid data")
    return raw


def validate_snapshot(
    model: RegistryModel, revision: str, read_blob: BlobReader
) -> Snapshot:
    """Validate all versions and history visible through one blob snapshot."""

    if model.policy is None:
        return Snapshot({}, (), b"")
    versions: dict[str, SemVer] = {}
    for owner in model.owners:
        if not owner.versioned:
            continue
        found = [
            read_source_version(
                spec,
                _blob(read_blob, revision, spec.path),
                f"{revision}:{spec.path}",
            )
            for spec in owner.sources
        ]
        if len(set(found)) != 1:
            detail = ", ".join(
                f"{spec.path}={version}" for spec, version in zip(owner.sources, found)
            )
            raise VersionSourceError(
                f'{revision}: version sources for "{owner.owner}" disagree: {detail}'
            )
        versions[owner.owner] = found[0]
    history_raw = _blob(read_blob, revision, model.policy.history_file)
    history = parse_history(history_raw, f"{revision}:{model.policy.history_file}")
    _validate_history(model, versions, history, revision)
    return Snapshot(versions, history, history_raw)


def appended_history_rows(
    before: Snapshot, after: Snapshot, history_path: str
) -> tuple[HistoryRow, ...]:
    """Return newly appended rows after proving old history is byte-stable."""

    if not after.history_raw.startswith(before.history_raw):
        raise VersionHistoryError(
            f"{history_path} must be append-only; existing bytes changed"
        )
    if tuple(after.history[: len(before.history)]) != tuple(before.history):
        raise VersionHistoryError(
            f"{history_path} must preserve all existing history rows"
        )
    return tuple(after.history[len(before.history) :])


def _normalized_wrapper(wrapper: object) -> str:
    if wrapper is None or wrapper == "" or wrapper == "normal":
        return "normal"
    if not isinstance(wrapper, str):
        raise VersionTransitionError("commit wrapper must be a string")
    normalized = wrapper.casefold().removesuffix("!")
    if normalized not in NON_REPLAY_WRAPPERS | {"revert"}:
        raise VersionTransitionError(f'unknown commit wrapper "{wrapper}"')
    return normalized


def _base_prefix(prefix: object, kind: str) -> str:
    if not isinstance(prefix, str) or not prefix:
        raise VersionTransitionError("commit prefix must be a non-empty string")
    suffix = f"-{kind}"
    return prefix[: -len(suffix)] if prefix.endswith(suffix) else prefix


def _assert_normal_config(head: RegistryModel, index: RegistryModel) -> None:
    if (
        head.policy != index.policy
        or head.suite_prefix != index.suite_prefix
        or head.owners != index.owners
    ):
        raise VersionRegistryError(
            "normal delivery commits cannot change version policy or owner configuration; "
            "land governance changes under suite-maintenance"
        )


def _changed_versions(before: Snapshot, after: Snapshot) -> set[str]:
    return {
        owner
        for owner in set(before.versions) | set(after.versions)
        if before.versions.get(owner) != after.versions.get(owner)
    }


def _require_changed_path(
    changed_paths: set[str], path: str, before: bytes, after: bytes
) -> None:
    if before != after and path not in changed_paths:
        raise VersionTransitionError(
            f"{path} changed between HEAD and INDEX but is absent from staged paths"
        )


def _validate_no_delta(
    head: RegistryModel,
    index: RegistryModel,
    before: Snapshot,
    after: Snapshot,
) -> None:
    _assert_normal_config(head, index)
    if before.versions != after.versions:
        raise VersionTransitionError("this commit kind must not change project versions")
    if before.history_raw != after.history_raw:
        raise VersionTransitionError("this commit kind must not change version history")


def _validate_policy_addition(
    head: RegistryModel,
    index: RegistryModel,
    before: Snapshot,
    after: Snapshot,
    changed_paths: set[str],
) -> None:
    if head.policy != index.policy or head.suite_prefix != index.suite_prefix:
        raise VersionRegistryError(
            "suite-maintenance cannot change or remove the existing version policy"
        )
    head_owners = head.owner_map()
    index_owners = index.owner_map()
    for owner, config in head_owners.items():
        if index_owners.get(owner) != config:
            raise VersionRegistryError(
                f'suite-maintenance cannot change or remove existing owner config "{owner}"'
            )
    added = [owner for owner in index.owners if owner.owner not in head_owners]
    if not added:
        _validate_no_delta(head, index, before, after)
        return

    history_path = head.policy.history_file if head.policy is not None else ""
    new_rows = appended_history_rows(before, after, history_path)
    expected = {owner.owner for owner in added if owner.versioned}
    if len(new_rows) != len(expected) or {row.owner for row in new_rows} != expected:
        raise VersionHistoryError(
            "each newly versioned owner requires exactly one appended baseline row"
        )
    for row in new_rows:
        if row.kind != "baseline" or row.version != after.versions[row.owner]:
            raise VersionHistoryError(
                f'new owner "{row.owner}" requires a baseline at {after.versions[row.owner]}'
            )
    if any(before.versions[owner] != after.versions[owner] for owner in before.versions):
        raise VersionTransitionError(
            "suite-maintenance owner additions cannot bump existing versions"
        )
    if new_rows and history_path not in changed_paths:
        raise VersionTransitionError(f"{history_path} must be staged with new baselines")


def validate_staged_transition(
    head_registry: Mapping[str, object],
    index_registry: Mapping[str, object],
    prefix: str,
    kind: str,
    action: str,
    wrapper: str | None,
    read_blob: BlobReader,
    changed_paths: Iterable[str],
) -> None:
    """Validate one staged version transition using committed interpretation.

    ``read_blob`` is called as ``read_blob("HEAD" | "INDEX", path)``.  The
    caller must pass the base project/suite prefix or its ``-kind`` spelling;
    component prefixes remain identifiable through the registry.
    """

    if not isinstance(head_registry.get("version_policy"), Mapping):
        return
    if not isinstance(kind, str) or kind not in COMMIT_KINDS:
        raise VersionTransitionError(f'unknown version commit kind "{kind}"')
    if not isinstance(action, str) or not action.strip() or action != action.strip():
        raise VersionTransitionError("commit action must be a non-empty trimmed string")
    normalized_wrapper = _normalized_wrapper(wrapper)
    base_prefix = _base_prefix(prefix, kind)
    staged = {str(path) for path in changed_paths}
    head = parse_registry(head_registry, "HEAD:docs/projects.toml")
    index = parse_registry(index_registry, "INDEX:docs/projects.toml")
    if index.policy is None:
        raise VersionRegistryError("INDEX removed the committed version policy")
    before = validate_snapshot(head, "HEAD", read_blob)
    after = validate_snapshot(index, "INDEX", read_blob)

    if normalized_wrapper == "revert" and kind in DELIVERY_KINDS:
        raise VersionTransitionError(
            "reverting a versioned delivery is ambiguous; use a new bug delivery "
            "with its own forward version and history row"
        )
    if normalized_wrapper in NON_REPLAY_WRAPPERS or normalized_wrapper == "revert":
        _validate_no_delta(head, index, before, after)
        return

    if kind == "maintenance":
        if base_prefix == head.suite_prefix and head.owners != index.owners:
            _validate_policy_addition(head, index, before, after, staged)
        else:
            _validate_no_delta(head, index, before, after)
        return

    _assert_normal_config(head, index)
    if base_prefix in head.component_prefixes:
        raise VersionTransitionError(
            f'component prefix "{base_prefix}" cannot own a {kind} version bump; '
            "use the versioned product prefix"
        )
    owner_by_prefix = head.prefix_map()
    selected = owner_by_prefix.get(base_prefix)
    if base_prefix != head.suite_prefix:
        if selected is None:
            raise VersionTransitionError(f'prefix "{base_prefix}" has no version owner')
        if not selected.versioned:
            raise VersionTransitionError(
                f'owner "{selected.owner}" is unversioned and cannot use {kind}'
            )

    changed = _changed_versions(before, after)
    if base_prefix == head.suite_prefix:
        if not changed:
            raise VersionTransitionError(
                f"suite-{kind} must bump at least one versioned owner"
            )
        expected_owners = changed
    else:
        expected_owners = {selected.owner}
        if changed != expected_owners:
            detail = ", ".join(sorted(changed)) or "none"
            raise VersionTransitionError(
                f'{selected.owner}-{kind} must bump only "{selected.owner}", changed: {detail}'
            )

    for owner in expected_owners:
        expected = before.versions[owner].bumped(kind)
        if after.versions.get(owner) != expected:
            raise VersionTransitionError(
                f"{owner} {kind} must move {before.versions[owner]} -> {expected}, "
                f"found {after.versions.get(owner)}"
            )

    history_path = head.policy.history_file if head.policy is not None else ""
    new_rows = appended_history_rows(before, after, history_path)
    if len(new_rows) != len(expected_owners) or {row.owner for row in new_rows} != expected_owners:
        raise VersionHistoryError(
            f"{kind} delivery requires exactly one appended history row per bumped owner"
        )
    for row in new_rows:
        if (
            row.kind != kind
            or row.version != after.versions[row.owner]
            or row.summary != action
        ):
            raise VersionHistoryError(
                f'{row.owner} history row must record {after.versions[row.owner]}, '
                f'{kind}, and summary "{action}"'
            )
    if history_path not in staged:
        raise VersionTransitionError(f"{history_path} must be staged with the version bump")

    owners = head.owner_map()
    for owner in expected_owners:
        for spec in owners[owner].sources:
            head_raw = _blob(read_blob, "HEAD", spec.path)
            index_raw = _blob(read_blob, "INDEX", spec.path)
            _require_changed_path(staged, spec.path, head_raw, index_raw)
