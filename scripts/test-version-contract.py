#!/usr/bin/env python3
"""Hermetic regression tests for product versions and typed deliveries."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest


SCRIPT_DIR = Path(__file__).resolve().parent
SUITE_ROOT = SCRIPT_DIR.parent
sys.path.insert(0, str(SCRIPT_DIR))

import version_contract as contract  # noqa: E402
import commit_scope  # noqa: E402
import project_registry  # noqa: E402
import version_tool as mutation_tool  # noqa: E402


HISTORY_PATH = "docs/version-history.tsv"
BASE_HISTORY = (
    "# Product version history.\n"
    "# owner\tversion\tkind\tunit\tsummary\n"
    "alpha\t1.2.3\tbaseline\tADOPT-A\tAdopt the Alpha version\n"
    "beta\t2.3.4\tbaseline\tADOPT-B\tAdopt the Beta version\n"
)


def cargo_package(name: str, version: contract.SemVer | str) -> bytes:
    return (
        "[package]\n"
        f'name = "{name}"\n'
        f'version = "{version}"\n'
        'edition = "2021"\n\n'
        "[dependencies]\n"
    ).encode()


def cargo_lock(name: str, version: contract.SemVer | str) -> bytes:
    return (
        "version = 4\n\n"
        "[[package]]\n"
        f'name = "{name}"\n'
        f'version = "{version}"\n'
    ).encode()


def cmake_project(name: str, version: contract.SemVer | str) -> bytes:
    return (
        "cmake_minimum_required(VERSION 3.20)\n"
        f"project({name} VERSION {version} LANGUAGES CXX)\n\n"
        f"qt_add_qml_module({name}\n"
        "    URI Example.Module\n"
        "    VERSION 99.7\n"
        ")\n"
    ).encode()


def make_registry() -> dict[str, object]:
    return {
        "schema_version": 1,
        "suite": {"commit_prefix": "suite"},
        "version_policy": {
            "history_file": HISTORY_PATH,
            "tag_format": "<project>-v<version>",
            "bug_increment": "patch",
            "milestone_increment": "minor",
            "release_increment": "major",
            "maintenance_increment": "none",
        },
        "projects": [
            {
                "id": "alpha",
                "commit_prefix": "alpha",
                "version_source": {
                    "kind": "cargo-package",
                    "path": "alpha/Cargo.toml",
                    "package": "alpha",
                },
                "version_mirrors": [
                    {
                        "kind": "cargo-lock",
                        "path": "alpha/Cargo.lock",
                        "package": "alpha",
                    }
                ],
                "component_commit_scopes": [
                    {"prefix": "alpha-core", "roots": ["core/alpha/"]}
                ],
            },
            {
                "id": "beta",
                "commit_prefix": "beta",
                "version_source": {
                    "kind": "cmake-project",
                    "path": "beta/CMakeLists.txt",
                    "project": "beta",
                },
                "component_commit_scopes": [],
            },
            {
                "id": "library",
                "commit_prefix": "library",
                "versioned": False,
                "component_commit_scopes": [
                    {"prefix": "library-core", "roots": ["library/core/"]}
                ],
            },
        ],
    }


class TransitionFixture:
    """HEAD/INDEX registry and blob snapshots without a Git subprocess."""

    def __init__(self) -> None:
        self.head_registry = make_registry()
        self.index_registry = deepcopy(self.head_registry)
        initial = {
            "alpha/Cargo.toml": cargo_package("alpha", "1.2.3"),
            "alpha/Cargo.lock": cargo_lock("alpha", "1.2.3"),
            "beta/CMakeLists.txt": cmake_project("beta", "2.3.4"),
            HISTORY_PATH: BASE_HISTORY.encode(),
        }
        self.blobs = {
            "HEAD": dict(initial),
            "INDEX": dict(initial),
        }
        self.changed_paths: set[str] = set()

    def read_blob(self, revision: str, path: str) -> bytes | None:
        return self.blobs[revision].get(path)

    def stage(self, path: str, raw: bytes | str) -> None:
        self.blobs["INDEX"][path] = raw.encode() if isinstance(raw, str) else raw
        self.changed_paths.add(path)

    def owner_config(self, owner: str) -> contract.OwnerConfig:
        model = contract.parse_registry(self.index_registry, "fixture registry")
        return model.owner_map()[owner]

    def index_version(self, owner: str) -> contract.SemVer:
        source = self.owner_config(owner).source
        assert source is not None
        raw = self.read_blob("INDEX", source.path)
        assert raw is not None
        return contract.read_source_version(source, raw, f"INDEX:{source.path}")

    def set_owner_version(
        self,
        owner: str,
        version: contract.SemVer,
        *,
        stage: bool = True,
    ) -> None:
        config = self.owner_config(owner)
        for source in config.sources:
            if source.kind == "cargo-package":
                raw = cargo_package(source.name or owner, version)
            elif source.kind == "cargo-lock":
                raw = cargo_lock(source.name or owner, version)
            else:
                raw = cmake_project(source.name or owner, version)
            if stage:
                self.stage(source.path, raw)
            else:
                self.blobs["INDEX"][source.path] = raw

    def append_history(
        self,
        owner: str,
        version: contract.SemVer,
        kind: str,
        unit: str,
        summary: str,
    ) -> None:
        current = self.blobs["INDEX"][HISTORY_PATH]
        row = f"{owner}\t{version}\t{kind}\t{unit}\t{summary}\n".encode()
        self.stage(HISTORY_PATH, current + row)

    def deliver(
        self,
        owner: str,
        kind: str,
        action: str,
        *,
        unit: str | None = None,
    ) -> contract.SemVer:
        new = self.index_version(owner).bumped(kind)
        self.set_owner_version(owner, new)
        self.append_history(owner, new, kind, unit or f"{owner.upper()}-NEXT", action)
        return new

    def validate(
        self,
        prefix: str,
        kind: str,
        action: str,
        *,
        wrapper: str | None = None,
        changed_paths: set[str] | None = None,
    ) -> None:
        contract.validate_staged_transition(
            self.head_registry,
            self.index_registry,
            prefix,
            kind,
            action,
            wrapper,
            self.read_blob,
            self.changed_paths if changed_paths is None else changed_paths,
        )


class StaticAndSourceTests(unittest.TestCase):
    def test_current_repository_static_contract(self) -> None:
        registry_path = SUITE_ROOT / "docs/projects.toml"
        with registry_path.open("rb") as stream:
            registry = tomllib.load(stream)
        model = contract.parse_registry(registry, str(registry_path))

        def read_worktree(_revision: str, path: str) -> bytes | None:
            try:
                return (SUITE_ROOT / path).read_bytes()
            except OSError:
                return None

        snapshot = contract.validate_snapshot(model, "WORKTREE", read_worktree)
        self.assertEqual(
            set(snapshot.versions),
            {
                "celestina",
                "celestina-style",
                "siderita",
                "magnetita",
                "grafita",
                "fluorita",
            },
        )

    def test_semver_uses_exact_patch_minor_and_major_transitions(self) -> None:
        current = contract.SemVer.parse("0.5.4", "fixture")
        expected = {
            "bug": contract.SemVer(0, 5, 5),
            "milestone": contract.SemVer(0, 6, 0),
            "release": contract.SemVer(1, 0, 0),
        }
        for kind, version in expected.items():
            with self.subTest(kind=kind):
                self.assertEqual(current.bumped(kind), version)

    def test_reads_cargo_package_lock_and_only_cmake_project_version(self) -> None:
        package = contract.SourceSpec("cargo-package", "Cargo.toml", "alpha")
        lock = contract.SourceSpec("cargo-lock", "Cargo.lock", "alpha")
        cmake = contract.SourceSpec("cmake-project", "CMakeLists.txt", "beta")
        self.assertEqual(
            contract.read_source_version(package, cargo_package("alpha", "1.2.3"), "package"),
            contract.SemVer(1, 2, 3),
        )
        self.assertEqual(
            contract.read_source_version(lock, cargo_lock("alpha", "1.2.3"), "lock"),
            contract.SemVer(1, 2, 3),
        )
        self.assertEqual(
            contract.read_source_version(cmake, cmake_project("beta", "2.3.4"), "cmake"),
            contract.SemVer(2, 3, 4),
        )


class StagedTransitionTests(unittest.TestCase):
    def test_product_accepts_each_exact_delivery_transition(self) -> None:
        expected = {
            "bug": contract.SemVer(1, 2, 4),
            "milestone": contract.SemVer(1, 3, 0),
            "release": contract.SemVer(2, 0, 0),
        }
        for kind, version in expected.items():
            with self.subTest(kind=kind):
                fixture = TransitionFixture()
                action = f"Deliver the Alpha {kind}"
                self.assertEqual(fixture.deliver("alpha", kind, action), version)
                fixture.validate(f"alpha-{kind}", kind, action)

    def test_rejects_missing_wrong_extra_and_decreasing_bumps(self) -> None:
        fixture = TransitionFixture()
        with self.subTest(case="missing"):
            with self.assertRaises(contract.VersionTransitionError):
                fixture.validate("alpha-bug", "bug", "Fix the Alpha behavior")

        fixture = TransitionFixture()
        fixture.set_owner_version("alpha", contract.SemVer(1, 2, 5))
        fixture.append_history(
            "alpha", contract.SemVer(1, 2, 5), "bug", "ALPHA-JUMP", "Fix the Alpha behavior"
        )
        with self.subTest(case="wrong"):
            with self.assertRaises(contract.VersionContractError):
                fixture.validate("alpha-bug", "bug", "Fix the Alpha behavior")

        fixture = TransitionFixture()
        action = "Fix the shared behavior"
        fixture.deliver("alpha", "bug", action)
        fixture.deliver("beta", "bug", action)
        with self.subTest(case="extra"):
            with self.assertRaises(contract.VersionTransitionError):
                fixture.validate("alpha-bug", "bug", action)

        fixture = TransitionFixture()
        fixture.set_owner_version("alpha", contract.SemVer(1, 2, 2))
        with self.subTest(case="decreasing"):
            with self.assertRaises(contract.VersionContractError):
                fixture.validate("alpha-bug", "bug", "Regress the Alpha version")

    def test_rejects_missing_staged_source_or_history_path(self) -> None:
        action = "Fix the Alpha behavior"
        fixture = TransitionFixture()
        fixture.deliver("alpha", "bug", action)
        without_source = fixture.changed_paths - {"alpha/Cargo.toml"}
        with self.subTest(case="source"):
            with self.assertRaises(contract.VersionTransitionError):
                fixture.validate("alpha-bug", "bug", action, changed_paths=without_source)

        fixture = TransitionFixture()
        fixture.deliver("alpha", "bug", action)
        without_history = fixture.changed_paths - {HISTORY_PATH}
        with self.subTest(case="history"):
            with self.assertRaises(contract.VersionTransitionError):
                fixture.validate("alpha-bug", "bug", action, changed_paths=without_history)

    def test_rejects_wrong_history_summary_or_kind(self) -> None:
        action = "Fix the Alpha behavior"
        fixture = TransitionFixture()
        fixture.deliver("alpha", "bug", "Describe another change")
        with self.subTest(case="summary"):
            with self.assertRaises(contract.VersionHistoryError):
                fixture.validate("alpha-bug", "bug", action)

        fixture = TransitionFixture()
        fixture.deliver("alpha", "bug", action)
        raw = fixture.blobs["INDEX"][HISTORY_PATH]
        fixture.stage(HISTORY_PATH, raw.replace(b"\tbug\t", b"\tmilestone\t"))
        with self.subTest(case="kind"):
            with self.assertRaises(contract.VersionContractError):
                fixture.validate("alpha-bug", "bug", action)

    def test_rejects_non_append_history(self) -> None:
        fixture = TransitionFixture()
        action = "Fix the Alpha behavior"
        fixture.deliver("alpha", "bug", action)
        raw = fixture.blobs["INDEX"][HISTORY_PATH]
        fixture.stage(
            HISTORY_PATH,
            raw.replace(b"# Product version history.\n", b"# Rewritten version history.\n"),
        )
        with self.assertRaises(contract.VersionHistoryError):
            fixture.validate("alpha-bug", "bug", action)

    def test_maintenance_rejects_version_or_history_delta(self) -> None:
        fixture = TransitionFixture()
        fixture.deliver("alpha", "bug", "Fix the Alpha behavior")
        with self.subTest(case="version"):
            with self.assertRaises(contract.VersionTransitionError):
                fixture.validate("alpha-maintenance", "maintenance", "Refactor Alpha tests")

        fixture = TransitionFixture()
        fixture.stage(
            HISTORY_PATH,
            fixture.blobs["INDEX"][HISTORY_PATH].replace(
                b"# Product version history.\n", b"# Product delivery history.\n"
            ),
        )
        with self.subTest(case="history"):
            with self.assertRaises(contract.VersionTransitionError):
                fixture.validate("alpha-maintenance", "maintenance", "Clarify Alpha history")

    def test_component_and_unversioned_owner_cannot_deliver_versions(self) -> None:
        fixture = TransitionFixture()
        with self.subTest(case="component"):
            with self.assertRaises(contract.VersionTransitionError):
                fixture.validate("alpha-core-bug", "bug", "Fix the Alpha core")
        with self.subTest(case="unversioned"):
            with self.assertRaises(contract.VersionTransitionError):
                fixture.validate("library-bug", "bug", "Fix the library")

    def test_suite_accepts_multiple_owners_with_the_same_kind(self) -> None:
        fixture = TransitionFixture()
        action = "Fix the shared product contract"
        fixture.deliver("alpha", "bug", action, unit="SUITE-A")
        fixture.deliver("beta", "bug", action, unit="SUITE-B")
        fixture.validate("suite-bug", "bug", action)

    def test_suite_maintenance_cannot_bypass_product_delivery(self) -> None:
        fixture = TransitionFixture()
        fixture.deliver("beta", "milestone", "Add the Beta milestone")
        with self.assertRaises(contract.VersionTransitionError):
            fixture.validate(
                "suite-maintenance",
                "maintenance",
                "Hide the Beta milestone in maintenance",
            )

    def test_suite_maintenance_accepts_one_new_owner_baseline(self) -> None:
        fixture = TransitionFixture()
        fixture.index_registry["projects"].append(
            {
                "id": "gamma",
                "commit_prefix": "gamma",
                "version_source": {
                    "kind": "cargo-package",
                    "path": "gamma/Cargo.toml",
                    "package": "gamma",
                },
                "component_commit_scopes": [],
            }
        )
        fixture.stage("gamma/Cargo.toml", cargo_package("gamma", "0.1.0"))
        fixture.append_history(
            "gamma",
            contract.SemVer(0, 1, 0),
            "baseline",
            "GAMMA-ADOPT",
            "Adopt the Gamma version",
        )
        fixture.changed_paths.add("docs/projects.toml")
        fixture.validate(
            "suite-maintenance",
            "maintenance",
            "Register the Gamma version baseline",
        )

    def test_rejects_policy_or_existing_owner_config_self_change(self) -> None:
        fixture = TransitionFixture()
        fixture.index_registry["version_policy"]["history_file"] = "docs/other-history.tsv"
        fixture.blobs["INDEX"]["docs/other-history.tsv"] = fixture.blobs["INDEX"][HISTORY_PATH]
        fixture.changed_paths.update({"docs/projects.toml", "docs/other-history.tsv"})
        with self.subTest(case="policy"):
            with self.assertRaises(contract.VersionRegistryError):
                fixture.validate(
                    "suite-maintenance", "maintenance", "Move the version history"
                )

        fixture = TransitionFixture()
        fixture.index_registry["projects"][0]["commit_prefix"] = "renamed-alpha"
        fixture.changed_paths.add("docs/projects.toml")
        with self.subTest(case="owner"):
            with self.assertRaises(contract.VersionRegistryError):
                fixture.validate(
                    "suite-maintenance", "maintenance", "Rename an existing owner"
                )

    def test_revert_rejects_a_versioned_delivery(self) -> None:
        fixture = TransitionFixture()
        action = "Fix the Alpha behavior"
        fixture.deliver("alpha", "bug", action)
        with self.assertRaises(contract.VersionTransitionError):
            fixture.validate("alpha-bug", "bug", action, wrapper="revert")

    def test_fixup_accepts_no_delta_and_rejects_a_second_bump(self) -> None:
        fixture = TransitionFixture()
        fixture.validate(
            "alpha-bug", "bug", "Fix the Alpha behavior", wrapper="fixup!"
        )

        fixture = TransitionFixture()
        action = "Fix the Alpha behavior"
        fixture.deliver("alpha", "bug", action)
        with self.assertRaises(contract.VersionTransitionError):
            fixture.validate("alpha-bug", "bug", action, wrapper="fixup")


class MutationTests(unittest.TestCase):
    def test_bump_repository_updates_source_mirror_and_history(self) -> None:
        registry = make_registry()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            files = {
                "alpha/Cargo.toml": cargo_package("alpha", "1.2.3"),
                "alpha/Cargo.lock": cargo_lock("alpha", "1.2.3"),
                "beta/CMakeLists.txt": cmake_project("beta", "2.3.4"),
                HISTORY_PATH: BASE_HISTORY.encode(),
            }
            for relative, raw in files.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(raw)

            old, new = mutation_tool.bump_repository(
                root,
                registry,
                "fixture registry",
                "alpha",
                "bug",
                "ALPHA-BUG-1",
                "Fix the Alpha behavior",
            )

            self.assertEqual(old, contract.SemVer(1, 2, 3))
            self.assertEqual(new, contract.SemVer(1, 2, 4))
            model = contract.parse_registry(registry, "fixture registry")
            alpha = model.owner_map()["alpha"]
            for source in alpha.sources:
                with self.subTest(source=source.path):
                    self.assertEqual(
                        contract.read_source_version(
                            source, (root / source.path).read_bytes(), source.path
                        ),
                        new,
                    )
            history = contract.parse_history(
                (root / HISTORY_PATH).read_bytes(), HISTORY_PATH
            )
            self.assertEqual(
                history[-1],
                contract.HistoryRow(
                    "alpha", new, "bug", "ALPHA-BUG-1", "Fix the Alpha behavior"
                ),
            )
            snapshot = contract.validate_snapshot(
                model,
                "WORKTREE",
                lambda _revision, path: (root / path).read_bytes(),
            )
            self.assertEqual(snapshot.versions["alpha"], new)


class PublishedHistoryAuditTests(unittest.TestCase):
    def test_subject_parser_preserves_hyphenated_base_scope(self) -> None:
        self.assertEqual(
            project_registry.parse_subject_change(
                "fixup! siderita-core-bug: Fix the shared operation"
            ),
            ("siderita-core", "bug", "Fix the shared operation", "fixup"),
        )
        with self.assertRaises(ValueError):
            project_registry.parse_subject_change("siderita: Fix the operation")

    def test_commit_scope_loads_the_version_rule_from_committed_head(self) -> None:
        registry = make_registry()
        action = "Fix the Alpha behavior"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def run_git(*arguments: str) -> int:
                return subprocess.run(
                    ["git", "-C", str(root), *arguments],
                    check=False,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                ).returncode

            files = {
                "scripts/version_contract.py": (
                    SCRIPT_DIR / "version_contract.py"
                ).read_bytes(),
                "alpha/Cargo.toml": cargo_package("alpha", "1.2.3"),
                "alpha/Cargo.lock": cargo_lock("alpha", "1.2.3"),
                "beta/CMakeLists.txt": cmake_project("beta", "2.3.4"),
                HISTORY_PATH: BASE_HISTORY.encode(),
            }
            for relative, raw in files.items():
                destination = root / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(raw)
            self.assertEqual(run_git("init", "-q"), 0)
            self.assertEqual(run_git("config", "user.name", "Fixture"), 0)
            self.assertEqual(
                run_git("config", "user.email", "fixture@example.invalid"),
                0,
            )
            self.assertEqual(run_git("config", "core.hooksPath", "/dev/null"), 0)
            self.assertEqual(run_git("add", "."), 0)
            self.assertEqual(run_git("commit", "-qm", "fixture: Establish baseline"), 0)

            (root / "alpha/Cargo.toml").write_bytes(cargo_package("alpha", "1.2.4"))
            (root / "alpha/Cargo.lock").write_bytes(cargo_lock("alpha", "1.2.4"))
            with (root / HISTORY_PATH).open("ab") as stream:
                stream.write(
                    b"alpha\t1.2.4\tbug\tALPHA-BUG-1\tFix the Alpha behavior\n"
                )
            changed = ["alpha/Cargo.toml", "alpha/Cargo.lock", HISTORY_PATH]
            self.assertEqual(run_git("add", *changed), 0)
            commit_scope.validate_version_update(
                root,
                "alpha",
                "bug",
                action,
                None,
                changed,
                registry,
                registry,
            )

    def test_audit_accepts_adoption_and_typed_commit_then_rejects_legacy(self) -> None:
        audit = SCRIPT_DIR / "audit-version-commits.py"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def git(*arguments: str) -> subprocess.CompletedProcess[str]:
                return subprocess.run(
                    ["git", "-C", str(root), *arguments],
                    check=False,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )

            def write(path: str, text: str) -> None:
                destination = root / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(text, encoding="utf-8")

            self.assertEqual(git("init", "-q").returncode, 0)
            self.assertEqual(git("config", "user.name", "Fixture").returncode, 0)
            self.assertEqual(
                git("config", "user.email", "fixture@example.invalid").returncode,
                0,
            )
            self.assertEqual(git("config", "core.hooksPath", "/dev/null").returncode, 0)
            write(
                "docs/projects.toml",
                "schema_version = 1\n\n"
                "[suite]\ncommit_prefix = \"suite\"\n\n"
                "[[projects]]\nid = \"alpha\"\ncommit_prefix = \"alpha\"\n",
            )
            write("alpha/Cargo.toml", cargo_package("alpha", "1.2.3").decode())
            write("README.md", "# Fixture\n")
            self.assertEqual(git("add", ".").returncode, 0)
            self.assertEqual(
                git("commit", "-qm", "suite: Establish the fixture").returncode,
                0,
            )

            registry_lines = [
                "schema_version = 1",
                "",
                "[suite]",
                'commit_prefix = "suite"',
                "",
                "[version_policy]",
                f'history_file = "{HISTORY_PATH}"',
                'tag_format = "<project>-v<version>"',
                'bug_increment = "patch"',
                'milestone_increment = "minor"',
                'release_increment = "major"',
                'maintenance_increment = "none"',
                "",
                "[[projects]]",
                'id = "alpha"',
                'commit_prefix = "alpha"',
                'version_source = { kind = "cargo-package", path = "alpha/Cargo.toml", package = "alpha" }',
                "component_commit_scopes = []",
                "",
            ]
            write("docs/projects.toml", "\n".join(registry_lines))
            write(
                HISTORY_PATH,
                "# Product version history.\n"
                "# owner\tversion\tkind\tunit\tsummary\n"
                "alpha\t1.2.3\tbaseline\tADOPT-A\tAdopt the Alpha version\n",
            )
            self.assertEqual(git("add", ".").returncode, 0)
            self.assertEqual(
                git("commit", "-qm", "suite: Adopt the version contract").returncode,
                0,
            )
            adoption = subprocess.run(
                [sys.executable, str(audit), "--root", str(root)],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(adoption.returncode, 0, adoption.stderr)

            write("README.md", "# Typed fixture\n")
            self.assertEqual(git("add", "README.md").returncode, 0)
            self.assertEqual(
                git("commit", "-qm", "alpha-maintenance: Update fixture prose").returncode,
                0,
            )
            typed = subprocess.run(
                [sys.executable, str(audit), "--root", str(root)],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertEqual(typed.returncode, 0, typed.stderr)

            write("README.md", "# Legacy fixture\n")
            self.assertEqual(git("add", "README.md").returncode, 0)
            self.assertEqual(
                git("commit", "-qm", "alpha: Update fixture prose again").returncode,
                0,
            )
            legacy = subprocess.run(
                [sys.executable, str(audit), "--root", str(root)],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            self.assertNotEqual(legacy.returncode, 0)
            self.assertIn("must declare a change kind", legacy.stderr)


if __name__ == "__main__":
    unittest.main()
