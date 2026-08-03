#!/usr/bin/env python3
"""Create and validate reusable production-artifact manifests.

The project registry is the single source of paths.  Build scripts record a
manifest after producing the canonical release artifact; verification and
deployment consume that manifest instead of guessing from mtimes or silently
rebuilding another binary.
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


SCHEMA_VERSION = 1
FINGERPRINT_SCHEMA = 1
IGNORED_DIRECTORY_NAMES = {
    ".git",
    ".cache",
    "__pycache__",
    "build",
    "target",
}
IGNORED_FILE_SUFFIXES = {".pyc", ".pyo"}


class ContractError(RuntimeError):
    """A production artifact does not satisfy the repository contract."""


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
        )
    except (OSError, subprocess.CalledProcessError):
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


def toolchain(root: Path) -> dict[str, str]:
    probes = {
        "cargo": ["cargo", "--version"],
        "rustc": ["rustc", "--version"],
        "cmake": ["cmake", "--version"],
        "cxx": [os.environ.get("CXX", "c++"), "--version"],
        "qt": ["qtpaths6", "--qt-version"],
    }
    return {name: run_text(command, root) for name, command in probes.items()}


def load_registry(registry_path: Path) -> tuple[Path, dict[str, Any], dict[str, Any]]:
    registry_path = registry_path.resolve()
    try:
        data = tomllib.loads(registry_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ContractError(f"no se puede leer el registro {registry_path}: {error}") from error

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
        raise ContractError(f"proyecto desconocido {project_id!r}; válidos: {known}") from error
    return root, registry, project


def lexical_repo_path(root: Path, relative: str) -> Path:
    candidate = Path(os.path.abspath(root / relative))
    try:
        candidate.relative_to(root)
    except ValueError as error:
        raise ContractError(f"ruta fuera del repositorio: {relative}") from error
    return candidate


def expand_patterns(
    root: Path, patterns: Iterable[str], *, allow_empty_glob: bool = False
) -> list[tuple[Path, str]]:
    expanded: dict[str, Path] = {}
    for pattern in patterns:
        has_magic = glob.has_magic(pattern)
        absolute_pattern = str(lexical_repo_path(root, pattern))
        matches = sorted(glob.glob(absolute_pattern, recursive=True)) if has_magic else [absolute_pattern]
        existing = [Path(match) for match in matches if os.path.lexists(match)]
        if not existing and not (has_magic and allow_empty_glob):
            raise ContractError(f"input declarado inexistente: {pattern}")
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
        raise ContractError(f"no se puede leer {logical}: {error}") from error

    if stat.S_ISLNK(info.st_mode):
        target = os.readlink(disk_path)
        hash_bytes(hasher, f"link:{logical}", target.encode("utf-8", errors="surrogateescape"))
        try:
            resolved = disk_path.resolve(strict=True)
        except OSError as error:
            raise ContractError(f"enlace roto en inputs: {logical}: {error}") from error
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
    allow_empty_glob: bool = False,
) -> str:
    hasher = hashlib.sha256()
    hash_bytes(hasher, "fingerprint-schema", str(FINGERPRINT_SCHEMA).encode("ascii"))
    hash_bytes(
        hasher,
        "contract",
        json.dumps(contract_data, sort_keys=True, separators=(",", ":")).encode("utf-8"),
    )
    for disk_path, logical in expand_patterns(root, paths, allow_empty_glob=allow_empty_glob):
        feed_path(hasher, disk_path, logical, ignore_build_outputs=True)
    return f"sha256:{hasher.hexdigest()}"


def production_fingerprint(root: Path, registry: dict[str, Any], project: dict[str, Any]) -> str:
    inputs = list(project.get("production_inputs", []))
    build_script = project.get("build_script")
    if build_script:
        inputs.append(build_script)
    if project.get("include_workspace_manifests"):
        inputs.extend(registry.get("commit_policy", {}).get("workspace_manifests", []))
    contract = {
        "project": project["id"],
        "profile": "release",
        "artifacts": project.get("artifact_paths", []),
        "inputs": sorted(set(inputs)),
    }
    return digest_paths(root, sorted(set(inputs)), contract_data=contract)


def verification_fingerprint(root: Path, project: dict[str, Any]) -> str:
    inputs = list(project.get("verification_inputs", []))
    for path in (
        project.get("verify_script"),
        project.get("deploy_script"),
        project.get("activate_script"),
        project.get("status_script"),
        "scripts/production_artifact.py",
        "scripts/production-common.sh",
        "scripts/qmllint-cxxqt.sh",
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
    contract = {
        "project": project["id"],
        "verify_script": project.get("verify_script"),
        "inputs": sorted(set(inputs)),
    }
    return digest_paths(
        root,
        sorted(set(inputs)),
        contract_data=contract,
        allow_empty_glob=True,
    )


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
            raise ContractError(f"falta el artefacto de producción: {relative}")
        digest, size, kind = artifact_digest(path, relative)
        artifacts.append({"path": relative, "kind": kind, "size": size, "sha256": digest})
    if not artifacts:
        raise ContractError(f"{project['id']} no declara artifact_paths")
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
    raise TypeError(f"valor TOML no soportado: {type(value).__name__}")


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
        lines.append(f"{key} = {toml_value(value)}")
    for artifact in manifest.get("artifacts", []):
        lines.extend(("", "[[artifacts]]"))
        for key in ("path", "kind", "size", "sha256"):
            lines.append(f"{key} = {toml_value(artifact[key])}")
    return "\n".join(lines) + "\n"


def manifest_path(root: Path, project: dict[str, Any]) -> Path:
    relative = project.get("artifact_manifest")
    if not relative:
        raise ContractError(f"{project['id']} no declara artifact_manifest")
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
            f"falta {path.relative_to(root)}; ejecuta {project['build_script']} primero"
        )
    try:
        return tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ContractError(f"manifest inválido {path}: {error}") from error


def validate_manifest(
    root: Path,
    registry: dict[str, Any],
    project: dict[str, Any],
    *,
    require_verified: bool,
) -> dict[str, Any]:
    manifest = read_manifest(root, project)
    errors: list[str] = []
    if manifest.get("schema_version") != SCHEMA_VERSION:
        errors.append("versión de manifest incompatible")
    if manifest.get("project") != project["id"]:
        errors.append("el manifest pertenece a otro proyecto")
    if manifest.get("profile") != "release":
        errors.append("el manifest no representa el perfil release")

    current_source = production_fingerprint(root, registry, project)
    if manifest.get("source_fingerprint") != current_source:
        errors.append("cambiaron inputs de producción; ejecuta build-production.sh")

    try:
        current_artifacts = collect_artifacts(root, project)
    except ContractError as error:
        errors.append(str(error))
        current_artifacts = []
    recorded_artifacts = manifest.get("artifacts", [])
    if current_artifacts != recorded_artifacts:
        errors.append("el digest o conjunto de artefactos no coincide con el build registrado")

    if require_verified:
        if not manifest.get("verified", False):
            errors.append("el artefacto todavía no está verificado; ejecuta verify-production.sh")
        current_verification = verification_fingerprint(root, project)
        if manifest.get("verification_fingerprint") != current_verification:
            errors.append("cambiaron las pruebas o reglas; vuelve a ejecutar verify-production.sh")

    if errors:
        raise ContractError("; ".join(dict.fromkeys(errors)))
    return manifest


def record_build(
    root: Path,
    registry: dict[str, Any],
    project: dict[str, Any],
    commands: list[str],
) -> Path:
    revision, dirty = git_state(root)
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "project": project["id"],
        "profile": "release",
        "source_fingerprint": production_fingerprint(root, registry, project),
        "verification_fingerprint": verification_fingerprint(root, project),
        "git_revision": revision,
        "worktree_dirty": dirty,
        "built_at": utc_now(),
        "verified": False,
        "build_commands": commands,
        "verify_commands": [],
        "toolchain": toolchain(root),
        "artifacts": collect_artifacts(root, project),
    }
    path = manifest_path(root, project)
    write_manifest(path, manifest)
    return path


def record_verification(
    root: Path,
    registry: dict[str, Any],
    project: dict[str, Any],
    commands: list[str],
) -> Path:
    manifest = validate_manifest(root, registry, project, require_verified=False)
    manifest["verification_fingerprint"] = verification_fingerprint(root, project)
    manifest["verified"] = True
    manifest["verified_at"] = utc_now()
    manifest["verify_commands"] = commands
    path = manifest_path(root, project)
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
            raise ContractError(f"mapping --installed inválido: {mapping!r}")
        source, target_text = mapping.split("=", 1)
        if source not in recorded:
            raise ContractError(f"--installed referencia un artefacto no registrado: {source}")
        target = Path(os.path.expanduser(target_text)).absolute()
        if not target.exists():
            messages.append(f"FALTA {target}")
            continue
        digest, _, _ = artifact_digest(target, source)
        if digest == recorded[source]["sha256"]:
            messages.append(f"OK {target}")
        else:
            messages.append(f"DISTINTO {target}")
    return messages


def parser() -> argparse.ArgumentParser:
    default_registry = Path(__file__).resolve().parent.parent / "docs" / "projects.toml"
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--registry", type=Path, default=default_registry)
    subparsers = result.add_subparsers(dest="command", required=True)

    build = subparsers.add_parser("record-build")
    build.add_argument("project")
    build.add_argument("--build-command", action="append", default=[])

    check = subparsers.add_parser("check")
    check.add_argument("project")
    check.add_argument("--require-verified", action="store_true")

    verify = subparsers.add_parser("record-verification")
    verify.add_argument("project")
    verify.add_argument("--verify-command", action="append", default=[])

    status_parser = subparsers.add_parser("status")
    status_parser.add_argument("project")
    status_parser.add_argument("--installed", action="append", default=[])
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        root, registry, project = project_contract(args.registry, args.project)
        if args.command == "record-build":
            path = record_build(root, registry, project, args.build_command)
            print(f"manifest: {path.relative_to(root)} (pendiente de verificación)")
        elif args.command == "check":
            validate_manifest(
                root,
                registry,
                project,
                require_verified=args.require_verified,
            )
            print(f"artifact: {project['id']} vigente")
        elif args.command == "record-verification":
            path = record_verification(root, registry, project, args.verify_command)
            print(f"manifest: {path.relative_to(root)} (verificado)")
        elif args.command == "status":
            manifest = validate_manifest(root, registry, project, require_verified=True)
            print(f"artifact: {project['id']} vigente y verificado")
            messages = installed_status(root, manifest, args.installed)
            for message in messages:
                print(f"installed: {message}")
            if any(not message.startswith("OK ") for message in messages):
                return 1
        else:
            raise AssertionError(args.command)
    except ContractError as error:
        print(f"production-artifact: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
