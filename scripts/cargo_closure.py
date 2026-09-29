#!/usr/bin/env python3
"""The Cargo path packages a release artifact links, as Cargo itself reports them.

A project's artifact contains every path package reached through the normal
and build dependencies of the manifests its build script compiles. A list
written by hand in the registry drifted from that set (TOOL-3), so the
production-input guard reads it from `cargo metadata --no-deps --offline`,
once per workspace. `--no-deps` lists each package's declared dependencies
with renamed and workspace-inherited keys already resolved to their paths,
and it needs neither the network nor the registry cache.
"""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass
import json
from pathlib import Path
import subprocess
from typing import Any


# A metadata call reads manifests only; one that takes longer is stuck.
METADATA_TIMEOUT_SECONDS = 120
# Targets Cargo builds for `cargo test`, `cargo bench` and `--example`; a
# release build of the artifact never compiles them.
NON_ARTIFACT_TARGET_KINDS = frozenset({"test", "bench", "example"})
# rustup selects the toolchain from the nearest of these, walking up from the
# directory a build runs in.
TOOLCHAIN_FILE_NAMES = ("rust-toolchain.toml", "rust-toolchain")


class CargoGraphError(RuntimeError):
    """Cargo could not describe a declared manifest, or described it unusably."""


@dataclass(frozen=True)
class PathPackage:
    """One path package of a closure, with repository-relative paths."""

    name: str
    manifest: str
    directory: str
    workspace_manifest: str
    # The source file of every target a release build compiles: lib, bin and
    # the build script, never tests, benches or examples.
    sources: tuple[str, ...]


@dataclass(frozen=True)
class CargoClosure:
    """What the release build of some manifests reads from the repository."""

    packages: tuple[PathPackage, ...]
    # The lockfile of each declared manifest's workspace, when it exists.
    lockfiles: tuple[str, ...]
    # The toolchain file rustup selects for each declared manifest, when any.
    toolchain_files: tuple[str, ...]


def _repository_path(root: Path, raw: object, label: str) -> str:
    if not isinstance(raw, str) or not raw:
        raise CargoGraphError(f"cargo metadata gave no path for {label}")
    try:
        return Path(raw).resolve().relative_to(root).as_posix()
    except ValueError as error:
        raise CargoGraphError(f"{label} lies outside the repository: {raw}") from error


class CargoMetadata:
    """`cargo metadata` answers for one repository, one call per workspace."""

    def __init__(self, root: Path, cargo: str = "cargo") -> None:
        self.root = root.resolve()
        self.cargo = cargo
        # Every package Cargo reported, keyed by its absolute manifest path,
        # with the workspace root manifest it belongs to.
        self.packages: dict[Path, tuple[dict[str, Any], Path]] = {}
        # Every manifest a call was made for, with the packages it builds: its
        # own package, or every member when it is a virtual workspace manifest.
        self.roots: dict[Path, list[Path]] = {}

    def _read(self, manifest: Path, label: str) -> None:
        command = [
            self.cargo,
            "metadata",
            "--manifest-path",
            str(manifest),
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ]
        try:
            result = subprocess.run(
                command,
                cwd=self.root,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=METADATA_TIMEOUT_SECONDS,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise CargoGraphError(f"cannot run cargo metadata for {label}: {error}") from error
        if result.returncode != 0:
            lines = [line for line in result.stderr.splitlines() if line.strip()]
            reason = lines[-1].strip() if lines else "no output"
            raise CargoGraphError(
                f"cargo metadata failed for {label} with exit {result.returncode}: {reason}"
            )
        try:
            data = json.loads(result.stdout)
        except json.JSONDecodeError as error:
            raise CargoGraphError(
                f"cargo metadata gave invalid JSON for {label}: {error}"
            ) from error
        packages = data.get("packages") if isinstance(data, dict) else None
        members = data.get("workspace_members") if isinstance(data, dict) else None
        workspace_root = data.get("workspace_root") if isinstance(data, dict) else None
        if (
            not isinstance(packages, list)
            or not isinstance(members, list)
            or not isinstance(workspace_root, str)
        ):
            raise CargoGraphError(f"cargo metadata gave an unexpected document for {label}")
        workspace_manifest = (Path(workspace_root) / "Cargo.toml").resolve()
        member_ids = {member for member in members if isinstance(member, str)}
        member_manifests: list[Path] = []
        for package in packages:
            if not isinstance(package, dict) or not isinstance(package.get("manifest_path"), str):
                raise CargoGraphError(
                    f"cargo metadata gave a package without a manifest for {label}"
                )
            package_manifest = Path(package["manifest_path"]).resolve()
            self.packages[package_manifest] = (package, workspace_manifest)
            if package.get("id") in member_ids:
                member_manifests.append(package_manifest)
        if manifest in self.packages:
            self.roots[manifest] = [manifest]
        else:
            self.roots[manifest] = sorted(member_manifests)

    def roots_of(self, manifest: Path, label: str) -> list[Path]:
        """The packages a build of `manifest` compiles as its roots."""
        manifest = manifest.resolve()
        if manifest not in self.roots:
            if manifest in self.packages:
                self.roots[manifest] = [manifest]
            else:
                self._read(manifest, label)
        return self.roots[manifest]

    def package(self, manifest: Path, label: str) -> tuple[dict[str, Any], Path]:
        manifest = manifest.resolve()
        if manifest not in self.packages:
            self._read(manifest, label)
        try:
            return self.packages[manifest]
        except KeyError as error:
            raise CargoGraphError(f"cargo metadata does not describe {label}") from error


def _nearest_toolchain_file(root: Path, start: Path) -> str | None:
    directory = start
    while True:
        for name in TOOLCHAIN_FILE_NAMES:
            candidate = directory / name
            if candidate.is_file():
                return candidate.relative_to(root).as_posix()
        if directory == root or root not in directory.parents:
            return None
        directory = directory.parent


def path_closure(
    root: Path, manifests: list[str], metadata: CargoMetadata | None = None
) -> CargoClosure:
    """Every path package, lockfile and toolchain file the build of `manifests` reads.

    `manifests` are repository-relative `Cargo.toml` paths: a package manifest,
    or a virtual workspace manifest whose every member is built. Development
    dependencies are left out, because a release build does not link them.
    """
    root = root.resolve()
    reader = metadata if metadata is not None else CargoMetadata(root)
    queue: deque[Path] = deque()
    lockfiles: set[str] = set()
    toolchain_files: set[str] = set()
    for relative in manifests:
        manifest = (root / relative).resolve()
        if not manifest.is_file():
            raise CargoGraphError(f"declared cargo manifest does not exist: {relative}")
        roots = reader.roots_of(manifest, relative)
        queue.extend(roots)
        for package_manifest in roots:
            _package, workspace_manifest = reader.package(package_manifest, relative)
            lockfile = workspace_manifest.parent / "Cargo.lock"
            if lockfile.is_file():
                lockfiles.add(_repository_path(root, str(lockfile), f"the lockfile of {relative}"))
        toolchain = _nearest_toolchain_file(root, manifest.parent)
        if toolchain is not None:
            toolchain_files.add(toolchain)

    found: dict[Path, PathPackage] = {}
    while queue:
        manifest = queue.popleft()
        if manifest in found:
            continue
        label = _repository_path(root, str(manifest), "a path package manifest")
        package, workspace_manifest = reader.package(manifest, label)
        name = str(package.get("name", label))
        targets = package.get("targets", [])
        dependencies = package.get("dependencies", [])
        if not isinstance(targets, list) or not isinstance(dependencies, list):
            raise CargoGraphError(f"cargo metadata gave an unexpected package entry for {name}")
        sources: list[str] = []
        for target in targets:
            if not isinstance(target, dict):
                continue
            kinds = target.get("kind", [])
            if not isinstance(kinds, list) or NON_ARTIFACT_TARGET_KINDS.intersection(kinds):
                continue
            sources.append(
                _repository_path(root, target.get("src_path"), f"a target of {name}")
            )
        found[manifest] = PathPackage(
            name=name,
            manifest=label,
            directory=_repository_path(root, str(manifest.parent), f"the directory of {name}"),
            workspace_manifest=_repository_path(
                root, str(workspace_manifest), f"the workspace manifest of {name}"
            ),
            sources=tuple(sorted(set(sources))),
        )
        for dependency in dependencies:
            if not isinstance(dependency, dict) or dependency.get("kind") == "dev":
                continue
            dependency_path = dependency.get("path")
            if dependency_path is None:
                continue
            if not isinstance(dependency_path, str):
                raise CargoGraphError(f"cargo metadata gave an invalid path dependency of {name}")
            dependency_manifest = (Path(dependency_path) / "Cargo.toml").resolve()
            _repository_path(root, str(dependency_manifest), f"the path dependency of {name}")
            queue.append(dependency_manifest)

    return CargoClosure(
        packages=tuple(found[manifest] for manifest in sorted(found)),
        lockfiles=tuple(sorted(lockfiles)),
        toolchain_files=tuple(sorted(toolchain_files)),
    )
