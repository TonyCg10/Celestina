#!/usr/bin/env python3
"""Create and validate reusable production-artifact manifests.

The project registry is the single source of paths.  This runner executes the
registered build or verification entrypoint and records success only after the
child exits zero and the phase-specific state is stable at seal time. Deployment
consumes that manifest instead of guessing from mtimes or silently rebuilding
another binary.
"""

from __future__ import annotations

import argparse
import datetime as dt
import glob
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import tomllib
from typing import Any, Iterable

from cargo_closure import CargoClosure, CargoGraphError, CargoMetadata, path_closure


SCHEMA_VERSION = 1
FINGERPRINT_SCHEMA = 1
INTERVAL_STATE_SCHEMA = 1
INTERNAL_ENTRY_ARGUMENT = "--production-runner-internal"
INTERNAL_PHASE_ENV = "CELESTINA_PRODUCTION_RUNNER_PHASE"
IGNORED_DIRECTORY_NAMES = {
    ".git",
    ".cache",
    "__pycache__",
    "build",
    "target",
}
IGNORED_FILE_SUFFIXES = {".pyc", ".pyo"}
# Written by scripts/worktree.sh at the root of every session worktree.
WORKTREE_MARKER = ".celestina-worktree"
# Every error goes to stderr on one line after this prefix, joined with "; ".
ERROR_PREFIX = "production-artifact: "
# The check errors that a verification alone clears; scripts/landing.py
# reads them to verify without rebuilding.
UNVERIFIED_ERROR = "artifact is not verified yet; run verify-production.sh"
VERIFICATION_CHANGED_ERROR = "tests or rules changed; run verify-production.sh again"
VERIFICATION_ERRORS = (UNVERIFIED_ERROR, VERIFICATION_CHANGED_ERROR)
# A version probe answers at once; one that hangs must not hang every check,
# status and deploy with it.
PROBE_TIMEOUT_SECONDS = 30
TOOLCHAIN_CHANGED_ERROR = "the toolchain changed since the build; run build-production.sh"
# A failed check names at most this many changed inputs, then counts the rest.
MAX_REPORTED_CHANGES = 20


class ContractError(RuntimeError):
    """A production artifact does not satisfy the repository contract.

    `details` are the facts behind the one-line message, such as the inputs
    that changed; they are printed on their own lines after it, so the first
    line keeps the format scripts/landing.py reads.
    """

    def __init__(self, message: str, details: Iterable[str] = ()) -> None:
        super().__init__(message)
        self.details = tuple(details)


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()


def run_text(command: list[str], cwd: Path) -> str:
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=PROBE_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        return "unavailable"
    return result.stdout.strip().splitlines()[0] if result.stdout.strip() else "unknown"


def git_state(root: Path) -> tuple[str, bool]:
    revision = run_text(["git", "rev-parse", "HEAD"], root)
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain", "--untracked-files=normal"],
            cwd=root,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return revision, True
    return revision, bool(result.stdout)


RUST_PROBES = {
    "cargo": ["cargo", "--version"],
    "rustc": ["rustc", "--version"],
}


def toolchain(root: Path, project: dict[str, Any]) -> dict[str, str]:
    """The versions of the tools the project's build runs.

    rustup chooses the compiler by the directory Cargo runs in, walking up to
    the nearest toolchain file, so the Rust probes run in the directory of
    each declared Cargo manifest: Magnetita's app on the default toolchain in
    `magnetita/`, its daemon on the one `celestina-rs/` pins. One directory
    records `cargo` and `rustc`; several record `cargo@<dir>` and
    `rustc@<dir>` for each. The C++ compiler, CMake and Qt do not depend on
    the directory and are probed once.
    """
    directories = rust_probe_directories(root, project)
    probes: dict[str, str] = {}
    for directory in directories:
        suffix = ""
        if len(directories) > 1:
            suffix = f"@{directory.relative_to(root).as_posix() or '.'}"
        for name, command in RUST_PROBES.items():
            probes[f"{name}{suffix}"] = run_text(command, directory)
    for name, command in (
        ("cmake", ["cmake", "--version"]),
        ("cxx", [os.environ.get("CXX", "c++"), "--version"]),
        ("qt", ["qtpaths6", "--qt-version"]),
    ):
        probes[name] = run_text(command, directories[0])
    return probes


def rust_probe_directories(root: Path, project: dict[str, Any]) -> list[Path]:
    """Where the project's Cargo builds run: each declared manifest's directory.

    A project without `cargo_manifests` builds in its registered directory,
    or at the root without one.
    """
    directories: list[Path] = []
    manifests = project.get("cargo_manifests")
    if isinstance(manifests, list):
        for manifest in manifests:
            if not isinstance(manifest, str) or not manifest:
                continue
            directory = lexical_repo_path(root, manifest).parent
            if directory.is_dir() and directory not in directories:
                directories.append(directory)
    if directories:
        return directories
    relative = project.get("path")
    if isinstance(relative, str) and relative:
        directory = lexical_repo_path(root, relative)
        if directory.is_dir():
            return [directory]
    return [root]


def load_registry(registry_path: Path) -> tuple[Path, dict[str, Any], dict[str, Any]]:
    registry_path = registry_path.resolve()
    try:
        data = tomllib.loads(registry_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ContractError(f"cannot read registry {registry_path}: {error}") from error

    root = registry_path.parent.parent
    projects = {project["id"]: project for project in data.get("projects", [])}
    return root, data, projects


def project_contract(
    registry_path: Path, project_id: str
) -> tuple[Path, dict[str, Any], dict[str, Any]]:
    root, registry, projects = load_registry(registry_path)
    try:
        project = projects[project_id]
    except KeyError as error:
        known = ", ".join(sorted(projects))
        raise ContractError(f"unknown project {project_id!r}; valid projects: {known}") from error
    return root, registry, project


def session_worktree_marker(root: Path) -> Path | None:
    """Return the session-worktree marker at `root`, or None in a canonical checkout."""
    marker = root / WORKTREE_MARKER
    return marker if marker.is_file() else None


def lexical_repo_path(root: Path, relative: str) -> Path:
    candidate = Path(os.path.abspath(root / relative))
    try:
        candidate.relative_to(root)
    except ValueError as error:
        raise ContractError(f"path outside repository: {relative}") from error
    return candidate


def expand_patterns(root: Path, patterns: Iterable[str]) -> list[tuple[Path, str]]:
    """Every declared input, expanded and required to exist.

    A pattern that matches nothing is an error whether or not it contains a
    glob. A verification glob used to be allowed to come back empty, so
    renaming a test directory quietly removed it from the fingerprint and a
    project could end up with no verification recorded and nothing said. An
    input that no longer exists is a change to the contract, not a nothing.
    """
    expanded: dict[str, Path] = {}
    for pattern in patterns:
        has_magic = glob.has_magic(pattern)
        absolute_pattern = str(lexical_repo_path(root, pattern))
        matches = sorted(glob.glob(absolute_pattern, recursive=True)) if has_magic else [absolute_pattern]
        existing = [Path(match) for match in matches if os.path.lexists(match)]
        if not existing:
            raise ContractError(f"declared input does not exist: {pattern}")
        for path in existing:
            logical = path.absolute().relative_to(root).as_posix()
            expanded[logical] = path
    return [(expanded[name], name) for name in sorted(expanded)]


def hash_bytes(hasher: Any, label: str, payload: bytes) -> None:
    encoded = label.encode("utf-8", errors="surrogateescape")
    hasher.update(len(encoded).to_bytes(8, "big"))
    hasher.update(encoded)
    hasher.update(len(payload).to_bytes(8, "big"))
    hasher.update(payload)


def should_ignore(path: Path) -> bool:
    return path.name in IGNORED_DIRECTORY_NAMES or path.suffix in IGNORED_FILE_SUFFIXES


def feed_path(
    hasher: Any,
    disk_path: Path,
    logical: str,
    *,
    ignore_build_outputs: bool,
    active_directories: set[Path] | None = None,
) -> None:
    active = set() if active_directories is None else active_directories
    try:
        info = disk_path.lstat()
    except OSError as error:
        raise ContractError(f"cannot read {logical}: {error}") from error

    if stat.S_ISLNK(info.st_mode):
        target = os.readlink(disk_path)
        hash_bytes(hasher, f"link:{logical}", target.encode("utf-8", errors="surrogateescape"))
        try:
            resolved = disk_path.resolve(strict=True)
        except OSError as error:
            raise ContractError(f"broken link in inputs: {logical}: {error}") from error
        feed_path(
            hasher,
            resolved,
            logical,
            ignore_build_outputs=ignore_build_outputs,
            active_directories=active,
        )
        return

    if stat.S_ISDIR(info.st_mode):
        real_directory = disk_path.resolve()
        if real_directory in active:
            hash_bytes(hasher, f"cycle:{logical}", b"")
            return
        hash_bytes(hasher, f"dir:{logical}", b"")
        active.add(real_directory)
        try:
            for child in sorted(disk_path.iterdir(), key=lambda item: item.name):
                if ignore_build_outputs and should_ignore(child):
                    continue
                feed_path(
                    hasher,
                    child,
                    f"{logical}/{child.name}",
                    ignore_build_outputs=ignore_build_outputs,
                    active_directories=active,
                )
        finally:
            active.remove(real_directory)
        return

    if not stat.S_ISREG(info.st_mode):
        hash_bytes(hasher, f"special:{logical}", str(info.st_mode).encode("ascii"))
        return

    hash_bytes(hasher, f"file:{logical}", disk_path.read_bytes())


def digest_paths(
    root: Path,
    paths: Iterable[str],
    *,
    contract_data: dict[str, Any],
) -> str:
    hasher = hashlib.sha256()
    hash_bytes(hasher, "fingerprint-schema", str(FINGERPRINT_SCHEMA).encode("ascii"))
    hash_bytes(
        hasher,
        "contract",
        json.dumps(contract_data, sort_keys=True, separators=(",", ":")).encode("utf-8"),
    )
    for disk_path, logical in expand_patterns(root, paths):
        feed_path(hasher, disk_path, logical, ignore_build_outputs=True)
    return f"sha256:{hasher.hexdigest()}"


def declared_production_inputs(project: dict[str, Any]) -> list[str]:
    """The project's own `production_inputs`, refused when a buildable project has none.

    A buildable project whose list is empty or absent would fingerprint only
    its build script, so its artifact would stay current whatever its sources
    became (TOOL-4).
    """
    inputs = project.get("production_inputs")
    if inputs is None:
        inputs = []
    if not isinstance(inputs, list) or not all(
        isinstance(item, str) and item for item in inputs
    ):
        raise ContractError(f"{project['id']} has invalid production_inputs")
    if project.get("build_script") and not inputs:
        raise ContractError(
            f"{project['id']} declares no production_inputs; a buildable project "
            "must name the inputs its artifact is made from"
        )
    return list(inputs)


def production_input_patterns(registry: dict[str, Any], project: dict[str, Any]) -> list[str]:
    """The declared inputs whose bytes decide whether `project`'s artifact is current."""
    inputs = list(project.get("production_inputs", []))
    build_script = project.get("build_script")
    if build_script:
        inputs.append(build_script)
    if project.get("include_workspace_manifests"):
        inputs.extend(registry.get("commit_policy", {}).get("workspace_manifests", []))
    return sorted(set(inputs))


def production_fingerprint(root: Path, registry: dict[str, Any], project: dict[str, Any]) -> str:
    declared_production_inputs(project)
    inputs = production_input_patterns(registry, project)
    contract = {
        "project": project["id"],
        "profile": "release",
        "artifacts": project.get("artifact_paths", []),
        "inputs": inputs,
    }
    return digest_paths(root, inputs, contract_data=contract)


def registered_script(project: dict[str, Any], key: str, *, required: bool) -> str | None:
    value = project.get(key)
    if value is None:
        if required:
            raise ContractError(f"{project['id']} does not declare {key}")
        return None
    if not isinstance(value, str) or not value:
        raise ContractError(f"{project['id']} has invalid {key}")
    return value


def verification_input_patterns(root: Path, project: dict[str, Any]) -> list[str]:
    """The declared inputs whose bytes decide whether `project`'s verification is current.

    They are the project's verification_inputs, its registered verify, status,
    activate, complete and deploy entries, and the shared paths below that
    exist at `root`; scripts/landing.py reads the same set.
    """
    inputs = list(project.get("verification_inputs", []))
    verify_script = registered_script(project, "verify_script", required=True)
    status_script = registered_script(project, "status_script", required=True)
    activate_script = registered_script(project, "activate_script", required=False)
    inputs.extend((verify_script, status_script))
    if activate_script is not None:
        inputs.append(activate_script)

    if project.get("deployable", False):
        complete_script = registered_script(project, "complete_script", required=True)
        deploy_script = registered_script(project, "deploy_script", required=True)
        inputs.extend((complete_script, deploy_script, "scripts/complete-production.py"))

    for path in (
        "scripts/production_artifact.py",
        "scripts/production-common.sh",
        "scripts/qmllint-cxxqt.sh",
        # The qmllint verdict now depends on the recorded warning ratchet, so a
        # change to it is a change to the verification.
        "scripts/qmllint-baseline.tsv",
        # The architecture guard now derives the set of projects it inspects
        # from the registry, so the registry decides what gets verified.
        "docs/projects.toml",
        "scripts/test-production-artifacts.py",
        "scripts/test-production-artifacts.sh",
        "scripts/test-production-common.sh",
        "scripts/check-architecture-contract.sh",
        "scripts/architecture_scanners.py",
        "scripts/architecture-baseline.tsv",
        "celestina-style/scripts/check-style-contract.sh",
        "celestina-style/scripts/check-contrast-contract.py",
    ):
        if path and os.path.lexists(root / path):
            inputs.append(path)
    return sorted(set(inputs))


def verification_fingerprint(root: Path, project: dict[str, Any]) -> str:
    inputs = verification_input_patterns(root, project)
    deployable = project.get("deployable", False)
    contract = {
        "project": project["id"],
        "verify_script": registered_script(project, "verify_script", required=True),
        "status_script": registered_script(project, "status_script", required=True),
        "complete_script": (
            registered_script(project, "complete_script", required=True) if deployable else None
        ),
        "deploy_script": (
            registered_script(project, "deploy_script", required=True) if deployable else None
        ),
        "activate_script": registered_script(project, "activate_script", required=False),
        "inputs": inputs,
    }
    return digest_paths(root, inputs, contract_data=contract)


def input_digests(root: Path, patterns: Iterable[str]) -> dict[str, str]:
    """One digest per expanded input path, so a failed check can name what changed.

    The fingerprints above stay the identity of an artifact; these digests are
    only the explanation recorded next to them.
    """
    digests = {}
    for disk_path, logical in expand_patterns(root, patterns):
        hasher = hashlib.sha256()
        feed_path(hasher, disk_path, logical, ignore_build_outputs=True)
        digests[logical] = f"sha256:{hasher.hexdigest()}"
    return digests


def changed_inputs(recorded: object, current: dict[str, str], label: str) -> list[str]:
    """The inputs whose recorded digest differs from `current`, one line each."""
    if not isinstance(recorded, dict):
        return [
            f"the manifest records no {label} input digests, "
            "so the changed inputs cannot be named"
        ]
    lines = [f"new {label} input: {path}" for path in sorted(current.keys() - recorded.keys())]
    lines.extend(
        f"removed {label} input: {path}" for path in sorted(recorded.keys() - current.keys())
    )
    lines.extend(
        f"changed {label} input: {path}"
        for path in sorted(current.keys() & recorded.keys())
        if current[path] != recorded[path]
    )
    lines.sort(key=lambda line: line.rsplit(": ", 1)[1])
    if not lines:
        return [f"every {label} input is unchanged; the declared input list or artifacts changed"]
    if len(lines) > MAX_REPORTED_CHANGES:
        hidden = len(lines) - MAX_REPORTED_CHANGES
        lines = lines[:MAX_REPORTED_CHANGES] + [f"and {hidden} more {label} inputs"]
    return lines


def changed_toolchain(recorded: object, current: dict[str, str]) -> list[str]:
    if not isinstance(recorded, dict):
        return ["the manifest records no toolchain"]
    return [
        f"toolchain {name}: {recorded.get(name, 'absent')!r} -> {current.get(name, 'absent')!r}"
        for name in sorted(set(recorded) | set(current))
        if recorded.get(name) != current.get(name)
    ]


def artifact_digest(path: Path, logical: str) -> tuple[str, int, str]:
    hasher = hashlib.sha256()
    feed_path(hasher, path, logical, ignore_build_outputs=False)
    if path.is_dir():
        size = sum(item.stat().st_size for item in path.rglob("*") if item.is_file())
        kind = "directory"
    else:
        size = path.stat().st_size
        kind = "file"
    return f"sha256:{hasher.hexdigest()}", size, kind


def collect_artifacts(root: Path, project: dict[str, Any]) -> list[dict[str, Any]]:
    artifacts = []
    for relative in project.get("artifact_paths", []):
        path = lexical_repo_path(root, relative)
        if not path.exists():
            raise ContractError(f"missing production artifact: {relative}")
        digest, size, kind = artifact_digest(path, relative)
        artifacts.append({"path": relative, "kind": kind, "size": size, "sha256": digest})
    if not artifacts:
        raise ContractError(f"{project['id']} does not declare artifact_paths")
    return artifacts


def toml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, list):
        return "[" + ", ".join(toml_value(item) for item in value) + "]"
    raise TypeError(f"unsupported TOML value: {type(value).__name__}")


def serialize_manifest(manifest: dict[str, Any]) -> str:
    scalar_order = (
        "schema_version",
        "project",
        "profile",
        "source_fingerprint",
        "verification_fingerprint",
        "git_revision",
        "worktree_dirty",
        "built_at",
        "verified",
        "verified_at",
        "build_commands",
        "verify_commands",
    )
    lines = [f"{key} = {toml_value(manifest[key])}" for key in scalar_order if key in manifest]
    lines.append("")
    lines.append("[toolchain]")
    for key, value in sorted(manifest.get("toolchain", {}).items()):
        lines.append(f"{toml_value(key)} = {toml_value(value)}")
    for table in ("production_input_digests", "verification_input_digests"):
        if table not in manifest:
            continue
        lines.extend(("", f"[{table}]"))
        for key, value in sorted(manifest[table].items()):
            lines.append(f"{toml_value(key)} = {toml_value(value)}")
    for artifact in manifest.get("artifacts", []):
        lines.extend(("", "[[artifacts]]"))
        for key in ("path", "kind", "size", "sha256"):
            lines.append(f"{key} = {toml_value(artifact[key])}")
    return "\n".join(lines) + "\n"


def manifest_path(root: Path, project: dict[str, Any]) -> Path:
    relative = project.get("artifact_manifest")
    if not relative:
        raise ContractError(f"{project['id']} does not declare artifact_manifest")
    return lexical_repo_path(root, relative)


def write_manifest(path: Path, manifest: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w", encoding="utf-8", dir=path.parent, prefix=f".{path.name}.", delete=False
        ) as temporary:
            temporary_name = temporary.name
            temporary.write(serialize_manifest(manifest))
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, path)
    finally:
        if temporary_name and os.path.exists(temporary_name):
            os.unlink(temporary_name)


def read_manifest(root: Path, project: dict[str, Any]) -> dict[str, Any]:
    path = manifest_path(root, project)
    if not path.exists():
        raise ContractError(
            f"missing {path.relative_to(root)}; run {project['build_script']} first"
        )
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ContractError(f"invalid manifest {path}: {error}") from error


def validate_manifest(
    root: Path,
    registry: dict[str, Any],
    project: dict[str, Any],
    *,
    require_verified: bool,
) -> dict[str, Any]:
    declared_production_inputs(project)
    manifest = read_manifest(root, project)
    errors: list[str] = []
    details: list[str] = []
    if manifest.get("schema_version") != SCHEMA_VERSION:
        errors.append("incompatible manifest version")
    if manifest.get("project") != project["id"]:
        errors.append("manifest belongs to another project")
    if manifest.get("profile") != "release":
        errors.append("manifest does not represent the release profile")

    current_source = production_fingerprint(root, registry, project)
    if manifest.get("source_fingerprint") != current_source:
        errors.append("production inputs changed; run build-production.sh")
        details.extend(
            changed_inputs(
                manifest.get("production_input_digests"),
                input_digests(root, production_input_patterns(registry, project)),
                "production",
            )
        )

    # An artifact another compiler or Qt produced is another artifact, even
    # from the same sources, so only a rebuild clears this (TOOL-22).
    toolchain_changes = changed_toolchain(
        manifest.get("toolchain"), toolchain(root, project)
    )
    if toolchain_changes:
        errors.append(TOOLCHAIN_CHANGED_ERROR)
        details.extend(toolchain_changes)

    try:
        current_artifacts = collect_artifacts(root, project)
    except ContractError as error:
        errors.append(str(error))
        current_artifacts = []
    recorded_artifacts = manifest.get("artifacts", [])
    if current_artifacts != recorded_artifacts:
        errors.append("artifact digest or set does not match the recorded build")

    if require_verified:
        if not manifest.get("verified", False):
            errors.append(UNVERIFIED_ERROR)
        current_verification = verification_fingerprint(root, project)
        if manifest.get("verification_fingerprint") != current_verification:
            errors.append(VERIFICATION_CHANGED_ERROR)
            details.extend(
                changed_inputs(
                    manifest.get("verification_input_digests"),
                    input_digests(root, verification_input_patterns(root, project)),
                    "verification",
                )
            )

    if errors:
        raise ContractError("; ".join(dict.fromkeys(errors)), details)
    return manifest


def hashed_by(path: str, inputs: Iterable[str]) -> bool:
    """Whether the fingerprint over the expanded `inputs` reads the bytes at `path`.

    An input covers itself and everything below it, except what the walk
    skips as build output or cache.
    """
    for item in inputs:
        if path == item:
            return True
        if path.startswith(f"{item}/"):
            below = path[len(item) + 1 :].split("/")
            if not any(part in IGNORED_DIRECTORY_NAMES for part in below):
                return True
    return False


def required_cargo_inputs(closure: CargoClosure, project_path: str) -> list[tuple[str, str]]:
    """Each path a release build of the closure reads, with why it is required.

    A path package outside the project is required whole: its build script,
    C++ sources and included files reach the artifact as much as its `src`.
    The project's own packages are required by manifest and compiled targets,
    because the rest of the project directory holds documents and scripts.
    """
    required: dict[str, str] = {}
    for package in closure.packages:
        own = package.directory == project_path or package.directory.startswith(
            f"{project_path}/"
        )
        if own:
            required.setdefault(package.manifest, f"the manifest of {package.name}")
            for source in package.sources:
                required.setdefault(source, f"a build target of {package.name}")
        else:
            required.setdefault(package.directory, f"{package.name}, a linked path package")
        if package.workspace_manifest != package.manifest:
            required.setdefault(
                package.workspace_manifest,
                f"the workspace manifest {package.name} inherits from",
            )
    for lockfile in closure.lockfiles:
        required.setdefault(lockfile, "the lockfile the build resolves against")
    for toolchain_file in closure.toolchain_files:
        required.setdefault(toolchain_file, "the toolchain file the build selects")
    return sorted(required.items())


def input_contract_errors(
    root: Path, registry: dict[str, Any], metadata: CargoMetadata | None = None
) -> list[str]:
    """Every registered project whose production inputs miss what its artifact is made from.

    A buildable project must declare production inputs. A project whose
    directory holds a `Cargo.toml` must name it in `cargo_manifests`, and the
    inputs of every project with `cargo_manifests` must hash the whole
    path-package closure Cargo reports for them (TOOL-3). The fingerprint
    itself stays a pure function of the declared inputs and never asks Cargo
    for the graph, so `check`, deploy, the landing's affected-project matching
    and agent-context.py read the same list this guard proves complete.
    """
    reader = metadata if metadata is not None else CargoMetadata(root)
    errors: list[str] = []
    for project in registry.get("projects", []):
        project_id = project.get("id", "project")
        try:
            declared_production_inputs(project)
        except ContractError as error:
            errors.append(str(error))
            continue
        try:
            inputs = [
                logical
                for _disk, logical in expand_patterns(
                    root, production_input_patterns(registry, project)
                )
            ]
        except ContractError as error:
            errors.append(f"{project_id}: {error}")
            continue

        manifests = project.get("cargo_manifests", [])
        if not isinstance(manifests, list) or not all(
            isinstance(item, str) and item for item in manifests
        ):
            errors.append(f"{project_id}: cargo_manifests must be a list of paths")
            continue
        project_path = project.get("path")
        if isinstance(project_path, str) and project_path:
            own_manifest = f"{project_path}/Cargo.toml"
            if (root / own_manifest).is_file() and own_manifest not in manifests:
                errors.append(
                    f"{project_id}: {own_manifest} exists, but cargo_manifests does not name it"
                )
        else:
            project_path = ""
        if not manifests:
            continue
        try:
            closure = path_closure(root, manifests, reader)
        except CargoGraphError as error:
            errors.append(f"{project_id}: {error}")
            continue
        for path, reason in required_cargo_inputs(closure, project_path):
            if not hashed_by(path, inputs):
                errors.append(f"{project_id}: production_inputs miss {path} ({reason})")
    return errors


def run_registered_entry(
    root: Path,
    project: dict[str, Any],
    key: str,
    phase: str,
) -> str:
    relative = registered_script(project, key, required=True)
    if relative is None:
        raise ContractError(f"{project['id']} does not declare {key}")
    script = lexical_repo_path(root, relative)
    if not script.is_file():
        raise ContractError(f"registered {key} does not exist: {relative}")

    environment = os.environ.copy()
    environment[INTERNAL_PHASE_ENV] = phase
    try:
        result = subprocess.run(
            [str(script), INTERNAL_ENTRY_ARGUMENT],
            cwd=root,
            env=environment,
            check=False,
        )
    except OSError as error:
        raise ContractError(f"cannot execute registered {key} {relative}: {error}") from error
    if result.returncode != 0:
        raise ContractError(
            f"registered {key} failed with exit {result.returncode}: {relative}"
        )
    return f"{relative} {INTERNAL_ENTRY_ARGUMENT}"


def run_build(
    root: Path,
    registry: dict[str, Any],
    project: dict[str, Any],
) -> Path:
    started_from = production_fingerprint(root, registry, project)
    command = run_registered_entry(root, project, "build_script", "build")
    current_source = production_fingerprint(root, registry, project)
    if current_source != started_from:
        raise ContractError(
            "production inputs changed during the build; rerun build-production.sh"
        )

    revision, dirty = git_state(root)
    artifacts = collect_artifacts(root, project)
    current_verification = verification_fingerprint(root, project)
    production_digests = input_digests(root, production_input_patterns(registry, project))
    verification_digests = input_digests(root, verification_input_patterns(root, project))
    current_toolchain = toolchain(root, project)
    if production_fingerprint(root, registry, project) != started_from:
        raise ContractError(
            "production inputs changed while recording the build; "
            "rerun build-production.sh"
        )
    if collect_artifacts(root, project) != artifacts:
        raise ContractError(
            "production artifacts changed while recording the build; "
            "rerun build-production.sh"
        )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "project": project["id"],
        "profile": "release",
        "source_fingerprint": current_source,
        "verification_fingerprint": current_verification,
        "git_revision": revision,
        "worktree_dirty": dirty,
        "built_at": utc_now(),
        "verified": False,
        "build_commands": [command],
        "verify_commands": [],
        "toolchain": current_toolchain,
        "production_input_digests": production_digests,
        "verification_input_digests": verification_digests,
        "artifacts": artifacts,
    }
    path = manifest_path(root, project)
    write_manifest(path, manifest)
    return path


def interval_state_digest(label: str, state: dict[str, Any]) -> str:
    hasher = hashlib.sha256()
    hash_bytes(
        hasher,
        "interval-state-schema",
        str(INTERVAL_STATE_SCHEMA).encode("ascii"),
    )
    hash_bytes(
        hasher,
        label,
        json.dumps(state, sort_keys=True, separators=(",", ":")).encode("utf-8"),
    )
    return f"sha256:{hasher.hexdigest()}"


def verification_start_state(
    root: Path,
    registry: dict[str, Any],
    project: dict[str, Any],
) -> tuple[str, dict[str, Any], str]:
    manifest = validate_manifest(root, registry, project, require_verified=False)
    current_verification = verification_fingerprint(root, project)
    state = {
        "project": project["id"],
        "source_fingerprint": manifest["source_fingerprint"],
        "artifacts": manifest["artifacts"],
        "verification_fingerprint": current_verification,
    }
    return (
        interval_state_digest("verification-start", state),
        manifest,
        current_verification,
    )


def run_verification(
    root: Path,
    registry: dict[str, Any],
    project: dict[str, Any],
) -> Path:
    started_from, manifest, _ = verification_start_state(root, registry, project)
    manifest["verified"] = False
    manifest.pop("verified_at", None)
    manifest["verify_commands"] = []
    path = manifest_path(root, project)
    write_manifest(path, manifest)

    command = run_registered_entry(root, project, "verify_script", "verify")
    current_start, manifest, current_verification = verification_start_state(
        root, registry, project
    )
    if started_from != current_start:
        raise ContractError(
            "source, artifacts, or verification inputs changed during verification; "
            "rerun verify-production.sh"
        )

    manifest["verification_fingerprint"] = current_verification
    manifest["verification_input_digests"] = input_digests(
        root, verification_input_patterns(root, project)
    )
    manifest["verified"] = True
    manifest["verified_at"] = utc_now()
    manifest["verify_commands"] = [command]
    write_manifest(path, manifest)
    return path


def installed_status(
    root: Path,
    manifest: dict[str, Any],
    mappings: list[str],
) -> list[str]:
    recorded = {artifact["path"]: artifact for artifact in manifest.get("artifacts", [])}
    messages = []
    for mapping in mappings:
        if "=" not in mapping:
            raise ContractError(f"invalid --installed mapping: {mapping!r}")
        source, target_text = mapping.split("=", 1)
        if source not in recorded:
            raise ContractError(f"--installed references an unregistered artifact: {source}")
        target = Path(os.path.expanduser(target_text)).absolute()
        if not target.exists():
            messages.append(f"MISSING {target}")
            continue
        digest, _, _ = artifact_digest(target, source)
        if digest == recorded[source]["sha256"]:
            messages.append(f"OK {target}")
        else:
            messages.append(f"DIFFERENT {target}")
    return messages


def parser() -> argparse.ArgumentParser:
    default_registry = Path(__file__).resolve().parent.parent / "docs" / "projects.toml"
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--registry", type=Path, default=default_registry)
    subparsers = result.add_subparsers(dest="command", required=True)

    build = subparsers.add_parser("run-build")
    build.add_argument("project")

    check = subparsers.add_parser("check")
    check.add_argument("project")
    check.add_argument("--require-verified", action="store_true")

    verify = subparsers.add_parser("run-verification")
    verify.add_argument("project")

    check_inputs = subparsers.add_parser("check-inputs")
    check_inputs.add_argument(
        "--root",
        type=Path,
        help="repository the registry describes (default: two levels above it)",
    )

    status_parser = subparsers.add_parser("status")
    status_parser.add_argument("project")
    status_parser.add_argument("--installed", action="append", default=[])
    return result


def main() -> int:
    args = parser().parse_args()
    if args.command == "check-inputs":
        try:
            root, registry, _projects = load_registry(args.registry)
            if args.root is not None:
                root = args.root.resolve()
            errors = input_contract_errors(root, registry)
        except ContractError as error:
            errors = [str(error)]
        for error in errors:
            print(f"{ERROR_PREFIX}{error}", file=sys.stderr)
        if errors:
            return 1
        print(
            "production inputs: every buildable project declares them, "
            "with each Cargo path package its artifact links"
        )
        return 0
    try:
        root, registry, project = project_contract(args.registry, args.project)
        # The refusal leaves `check` alone: it only reads, and its callers are
        # production-common.sh's deploy helper and scripts/land-unit.py.
        if (
            args.command in {"run-build", "run-verification", "status"}
            and session_worktree_marker(root) is not None
        ):
            raise ContractError(
                "this is a session worktree; production runs happen at landing "
                "(scripts/land-unit.py)"
            )
        if args.command == "run-build":
            path = run_build(root, registry, project)
            print(f"manifest: {path.relative_to(root)} (pending verification)")
        elif args.command == "check":
            validate_manifest(
                root,
                registry,
                project,
                require_verified=args.require_verified,
            )
            print(f"artifact: {project['id']} current")
        elif args.command == "run-verification":
            path = run_verification(root, registry, project)
            print(f"manifest: {path.relative_to(root)} (verified)")
        elif args.command == "status":
            manifest = validate_manifest(root, registry, project, require_verified=True)
            print(f"artifact: {project['id']} current and verified")
            messages = installed_status(root, manifest, args.installed)
            for message in messages:
                print(f"installed: {message}")
            if any(not message.startswith("OK ") for message in messages):
                return 1
        else:
            raise AssertionError(args.command)
    except ContractError as error:
        print(f"{ERROR_PREFIX}{error}", file=sys.stderr)
        for detail in error.details:
            print(f"{ERROR_PREFIX}  {detail}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
