#!/usr/bin/env python3
"""Regression fixtures for the reusable production-artifact contract."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest


TOOL = Path(__file__).resolve().parent / "production_artifact.py"
SUITE_ROOT = TOOL.parent.parent


class ProductionArtifactFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        for directory in (
            "docs",
            "scripts",
            "demo/scripts",
            "demo/src",
            "demo/tests",
            "demo/target/release",
            "library/scripts",
            "library/src",
            "library/tests",
            "library/target/release",
            "shared",
        ):
            (self.root / directory).mkdir(parents=True, exist_ok=True)

        self.registry = self.root / "docs/projects.toml"
        self.registry.write_text(
            """schema_version = 1

[commit_policy]
workspace_manifests = []

[[projects]]
id = "demo"
path = "demo"
deployable = true
build_script = "demo/scripts/build-production.sh"
verify_script = "demo/scripts/verify-production.sh"
complete_script = "demo/scripts/complete-production.sh"
deploy_script = "demo/scripts/deploy-production.sh"
activate_script = "demo/scripts/activate-production.sh"
status_script = "demo/scripts/status-production.sh"
artifact_manifest = "demo/target/production-artifact.toml"
artifact_paths = ["demo/target/release/demo"]
production_inputs = ["demo/src"]
verification_inputs = ["demo/tests/*.txt"]

[[projects]]
id = "library"
path = "library"
deployable = false
build_script = "library/scripts/build-production.sh"
verify_script = "library/scripts/verify-production.sh"
status_script = "library/scripts/status-production.sh"
artifact_manifest = "library/target/production-artifact.toml"
artifact_paths = ["library/target/release/library"]
production_inputs = ["library/src"]
verification_inputs = ["library/tests/*.txt"]
""",
            encoding="utf-8",
        )
        self.write_entry_script("demo", "build", "v1")
        self.write_entry_script("demo", "verify", "v1")
        (self.root / "demo/scripts/complete-production.sh").write_text(
            "complete v1\n", encoding="utf-8"
        )
        (self.root / "demo/scripts/deploy-production.sh").write_text("deploy v1\n", encoding="utf-8")
        (self.root / "demo/scripts/activate-production.sh").write_text(
            "activate v1\n", encoding="utf-8"
        )
        (self.root / "demo/scripts/status-production.sh").write_text("status v1\n", encoding="utf-8")
        (self.root / "scripts/production_artifact.py").write_text("contract v1\n", encoding="utf-8")
        (self.root / "scripts/production-common.sh").write_text("common v1\n", encoding="utf-8")
        (self.root / "scripts/complete-production.py").write_text(
            "orchestrator v1\n", encoding="utf-8"
        )
        # A declared verification glob must match something: an input that
        # matches nothing is refused, so the fixture ships one case per project.
        (self.root / "demo/tests/case.txt").write_text("case v1\n", encoding="utf-8")
        (self.root / "library/tests/case.txt").write_text("case v1\n", encoding="utf-8")
        (self.root / "demo/src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
        (self.root / "demo/target/release/demo").write_bytes(b"release-v1\n")
        self.write_entry_script("library", "build", "v1")
        self.write_entry_script("library", "verify", "v1")
        (self.root / "library/scripts/status-production.sh").write_text(
            "status library v1\n", encoding="utf-8"
        )
        (self.root / "library/src/lib.rs").write_text("pub fn value() {}\n", encoding="utf-8")
        (self.root / "library/target/release/library").write_bytes(b"library-v1\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_entry_script(self, project: str, phase: str, version: str) -> None:
        behavior_variable = f"PRODUCTION_FIXTURE_{phase.upper()}_BEHAVIOR"
        script = self.root / project / "scripts" / f"{phase}-production.sh"
        source_file = f"{project}/src/{'main.rs' if project == 'demo' else 'lib.rs'}"
        artifact = f"{project}/target/release/{project}"
        script.write_text(
            f"""#!/bin/sh
set -eu
# Fixture entrypoint {version}.
[ "$#" -eq 1 ] && [ "$1" = "--production-runner-internal" ] || exit 64
[ "${{CELESTINA_PRODUCTION_RUNNER_PHASE:-}}" = "{phase}" ] || exit 64
printf '%s\n' '{phase} {version}' > .fixture-{project}-{phase}-ran
case "${{{behavior_variable}:-success}}" in
    success) ;;
    fail) exit 7 ;;
    change-source) printf '%s\n' 'changed during {phase}' >> {source_file} ;;
    change-artifact) printf '%s\n' 'changed during {phase}' > {artifact} ;;
    change-verification-input)
        printf '%s\n' 'changed during {phase}' > {project}/tests/changed.txt
        ;;
    *) exit 65 ;;
esac
""",
            encoding="utf-8",
        )
        script.chmod(0o755)

    @property
    def demo_manifest(self) -> Path:
        return self.root / "demo/target/production-artifact.toml"

    def run_tool(
        self,
        *arguments: str,
        expect: int = 0,
        environment: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        command_environment = os.environ.copy()
        command_environment.update(environment or {})
        result = subprocess.run(
            [sys.executable, str(TOOL), "--registry", str(self.registry), *arguments],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=command_environment,
        )
        self.assertEqual(
            result.returncode,
            expect,
            msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
        return result

    def run_build(
        self,
        project: str = "demo",
        behavior: str = "success",
    ) -> None:
        self.run_tool(
            "run-build",
            project,
            environment={"PRODUCTION_FIXTURE_BUILD_BEHAVIOR": behavior},
        )

    def run_verification(
        self,
        project: str = "demo",
        behavior: str = "success",
    ) -> None:
        self.run_tool(
            "run-verification",
            project,
            environment={"PRODUCTION_FIXTURE_VERIFY_BEHAVIOR": behavior},
        )

    def assert_missing_script_cannot_be_resealed(self, relative: str) -> None:
        self.run_build()
        self.run_verification()
        (self.root / relative).unlink()

        message = f"declared input does not exist: {relative}"
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn(message, stale.stderr)
        reseal = self.run_tool(
            "run-verification",
            "demo",
            expect=1,
        )
        self.assertIn(message, reseal.stderr)

    def test_direct_sealing_commands_do_not_exist(self) -> None:
        for command in (
            "start-build",
            "record-build",
            "start-verification",
            "record-verification",
        ):
            with self.subTest(command=command):
                result = self.run_tool(command, "demo", expect=2)
                self.assertIn("invalid choice", result.stderr)
        self.assertFalse(self.demo_manifest.exists())

    def test_internal_entry_cannot_seal_on_its_own(self) -> None:
        environment = os.environ.copy()
        environment["CELESTINA_PRODUCTION_RUNNER_PHASE"] = "build"
        result = subprocess.run(
            [
                str(self.root / "demo/scripts/build-production.sh"),
                "--production-runner-internal",
            ],
            cwd=self.root,
            env=environment,
            check=False,
        )
        self.assertEqual(result.returncode, 0)
        self.assertFalse(self.demo_manifest.exists())

    def test_all_repository_entries_delegate_sealing_to_the_runner(self) -> None:
        with (SUITE_ROOT / "docs/projects.toml").open("rb") as stream:
            registry = tomllib.load(stream)
        for project in registry["projects"]:
            for phase, key, command in (
                ("build", "build_script", "run-build"),
                ("verify", "verify_script", "run-verification"),
            ):
                with self.subTest(project=project["id"], phase=phase):
                    script = (SUITE_ROOT / project[key]).read_text(encoding="utf-8")
                    self.assertIn(
                        f'exec python3 "$artifact_tool" {command} {project["id"]}',
                        script,
                    )
                    self.assertIn("--production-runner-internal", script)
                    self.assertIn(
                        f'CELESTINA_PRODUCTION_RUNNER_PHASE:-}}" != "{phase}"',
                        script,
                    )
                    self.assertNotIn("start-build", script)
                    self.assertNotIn("record-build", script)
                    self.assertNotIn("start-verification", script)
                    self.assertNotIn("record-verification", script)

    def test_success_runs_registered_entries_before_sealing(self) -> None:
        self.run_build()
        self.assertTrue((self.root / ".fixture-demo-build-ran").is_file())
        manifest = tomllib.loads(self.demo_manifest.read_text(encoding="utf-8"))
        self.assertEqual(
            manifest["build_commands"],
            ["demo/scripts/build-production.sh --production-runner-internal"],
        )
        self.assertFalse(manifest["verified"])

        self.run_verification()
        self.assertTrue((self.root / ".fixture-demo-verify-ran").is_file())
        manifest = tomllib.loads(self.demo_manifest.read_text(encoding="utf-8"))
        self.assertEqual(
            manifest["verify_commands"],
            ["demo/scripts/verify-production.sh --production-runner-internal"],
        )
        self.assertTrue(manifest["verified"])

    def test_failed_build_entry_does_not_seal(self) -> None:
        result = self.run_tool(
            "run-build",
            "demo",
            expect=1,
            environment={"PRODUCTION_FIXTURE_BUILD_BEHAVIOR": "fail"},
        )
        self.assertIn("registered build_script failed with exit 7", result.stderr)
        self.assertFalse(self.demo_manifest.exists())

    def test_failed_verify_entry_does_not_seal(self) -> None:
        self.run_build()
        result = self.run_tool(
            "run-verification",
            "demo",
            expect=1,
            environment={"PRODUCTION_FIXTURE_VERIFY_BEHAVIOR": "fail"},
        )
        self.assertIn("registered verify_script failed with exit 7", result.stderr)
        manifest = tomllib.loads(self.demo_manifest.read_text(encoding="utf-8"))
        self.assertFalse(manifest["verified"])

    def test_failed_reverification_removes_the_previous_seal(self) -> None:
        self.run_build()
        self.run_verification()
        result = self.run_tool(
            "run-verification",
            "demo",
            expect=1,
            environment={"PRODUCTION_FIXTURE_VERIFY_BEHAVIOR": "fail"},
        )
        self.assertIn("registered verify_script failed with exit 7", result.stderr)
        manifest = tomllib.loads(self.demo_manifest.read_text(encoding="utf-8"))
        self.assertFalse(manifest["verified"])
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("artifact is not verified yet", stale.stderr)

    def test_source_change_during_build_is_rejected(self) -> None:
        result = self.run_tool(
            "run-build",
            "demo",
            expect=1,
            environment={"PRODUCTION_FIXTURE_BUILD_BEHAVIOR": "change-source"},
        )
        self.assertIn("production inputs changed during the build", result.stderr)
        self.assertFalse(self.demo_manifest.exists())

    def test_verification_input_change_during_verification_is_rejected(self) -> None:
        self.run_build()
        result = self.run_tool(
            "run-verification",
            "demo",
            expect=1,
            environment={
                "PRODUCTION_FIXTURE_VERIFY_BEHAVIOR": "change-verification-input"
            },
        )
        self.assertIn("changed during verification", result.stderr)

    def test_source_change_during_verification_is_rejected(self) -> None:
        self.run_build()
        result = self.run_tool(
            "run-verification",
            "demo",
            expect=1,
            environment={"PRODUCTION_FIXTURE_VERIFY_BEHAVIOR": "change-source"},
        )
        self.assertIn("production inputs changed", result.stderr)

    def test_artifact_change_during_verification_is_rejected(self) -> None:
        self.run_build()
        result = self.run_tool(
            "run-verification",
            "demo",
            expect=1,
            environment={"PRODUCTION_FIXTURE_VERIFY_BEHAVIOR": "change-artifact"},
        )
        self.assertIn("artifact digest or set does not match", result.stderr)

    def test_build_verify_and_exact_installed_copy(self) -> None:
        self.run_build()
        unverified = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("is not verified yet", unverified.stderr)

        self.run_verification()
        self.run_tool("check", "demo", "--require-verified")

        installed = self.root / "stage/bin/demo"
        installed.parent.mkdir(parents=True)
        shutil.copy2(self.root / "demo/target/release/demo", installed)
        mapping = f"demo/target/release/demo={installed}"
        self.run_tool("status", "demo", "--installed", mapping)

        installed.write_bytes(b"different\n")
        result = self.run_tool("status", "demo", "--installed", mapping, expect=1)
        self.assertIn("DIFFERENT", result.stdout)

    def test_source_and_artifact_changes_invalidate_manifest(self) -> None:
        self.run_build()
        (self.root / "demo/src/main.rs").write_text("fn main() { println!(\"changed\"); }\n", encoding="utf-8")
        source_result = self.run_tool("check", "demo", expect=1)
        self.assertIn("production inputs changed", source_result.stderr)

        (self.root / "demo/src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
        self.run_build()
        (self.root / "demo/target/release/demo").write_bytes(b"tampered\n")
        artifact_result = self.run_tool("check", "demo", expect=1)
        self.assertIn("digest", artifact_result.stderr)

    def test_verification_change_requires_only_reverification(self) -> None:
        self.run_build()
        self.run_verification()
        self.write_entry_script("demo", "verify", "v2")

        self.run_tool("check", "demo")
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("run verify-production.sh again", stale.stderr)
        self.run_verification()
        self.run_tool("check", "demo", "--require-verified")

    def test_shared_deploy_helper_change_requires_only_reverification(self) -> None:
        self.run_build()
        self.run_verification()
        (self.root / "scripts/production-common.sh").write_text("common v2\n", encoding="utf-8")

        self.run_tool("check", "demo")
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("run verify-production.sh again", stale.stderr)

    def test_verification_inputs_are_the_fingerprinted_set(self) -> None:
        sys.path.insert(0, str(TOOL.parent))
        import production_artifact

        _root, _registry, projects = production_artifact.load_registry(self.registry)
        demo = projects["demo"]
        # Every declared, required and shared path that exists in the fixture,
        # in the order and form the fingerprint has always hashed.
        expected = [
            "demo/scripts/activate-production.sh",
            "demo/scripts/complete-production.sh",
            "demo/scripts/deploy-production.sh",
            "demo/scripts/status-production.sh",
            "demo/scripts/verify-production.sh",
            "demo/tests/*.txt",
            "docs/projects.toml",
            "scripts/complete-production.py",
            "scripts/production-common.sh",
            "scripts/production_artifact.py",
        ]
        self.assertEqual(
            production_artifact.verification_input_patterns(self.root, demo), expected
        )
        contract = {
            "project": "demo",
            "verify_script": "demo/scripts/verify-production.sh",
            "status_script": "demo/scripts/status-production.sh",
            "complete_script": "demo/scripts/complete-production.sh",
            "deploy_script": "demo/scripts/deploy-production.sh",
            "activate_script": "demo/scripts/activate-production.sh",
            "inputs": expected,
        }
        self.assertEqual(
            production_artifact.verification_fingerprint(self.root, demo),
            production_artifact.digest_paths(self.root, expected, contract_data=contract),
        )
        library = production_artifact.verification_input_patterns(self.root, projects["library"])
        self.assertNotIn("scripts/complete-production.py", library)
        self.assertIn("scripts/production-common.sh", library)

    def test_project_deploy_change_requires_only_reverification(self) -> None:
        self.run_build()
        self.run_verification()
        (self.root / "demo/scripts/deploy-production.sh").write_text("deploy v2\n", encoding="utf-8")

        self.run_tool("check", "demo")
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("run verify-production.sh again", stale.stderr)

    def test_project_completion_change_requires_only_reverification(self) -> None:
        self.run_build()
        self.run_verification()
        (self.root / "demo/scripts/complete-production.sh").write_text(
            "complete v2\n", encoding="utf-8"
        )

        self.run_tool("check", "demo")
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("run verify-production.sh again", stale.stderr)
        self.run_verification()
        self.run_tool("check", "demo", "--require-verified")

    def test_shared_completion_change_requires_only_reverification(self) -> None:
        self.run_build()
        self.run_verification()
        (self.root / "scripts/complete-production.py").write_text(
            "orchestrator v2\n", encoding="utf-8"
        )

        self.run_tool("check", "demo")
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("run verify-production.sh again", stale.stderr)
        self.run_verification()
        self.run_tool("check", "demo", "--require-verified")

    def test_missing_project_completion_cannot_be_resealed(self) -> None:
        self.assert_missing_script_cannot_be_resealed(
            "demo/scripts/complete-production.sh"
        )

    def test_missing_shared_completion_cannot_be_resealed(self) -> None:
        self.assert_missing_script_cannot_be_resealed(
            "scripts/complete-production.py"
        )

    def test_missing_verify_script_cannot_be_resealed(self) -> None:
        self.assert_missing_script_cannot_be_resealed(
            "demo/scripts/verify-production.sh"
        )

    def test_missing_deploy_script_cannot_be_resealed(self) -> None:
        self.assert_missing_script_cannot_be_resealed(
            "demo/scripts/deploy-production.sh"
        )

    def test_missing_status_script_cannot_be_resealed(self) -> None:
        self.assert_missing_script_cannot_be_resealed(
            "demo/scripts/status-production.sh"
        )

    def test_missing_declared_activation_script_cannot_be_resealed(self) -> None:
        self.assert_missing_script_cannot_be_resealed(
            "demo/scripts/activate-production.sh"
        )

    def test_missing_required_script_declarations_cannot_be_resealed(self) -> None:
        self.run_build()
        self.run_verification()
        registered = self.registry.read_text(encoding="utf-8")

        declarations = {
            "verify_script": 'verify_script = "demo/scripts/verify-production.sh"\n',
            "status_script": 'status_script = "demo/scripts/status-production.sh"\n',
            "complete_script": 'complete_script = "demo/scripts/complete-production.sh"\n',
            "deploy_script": 'deploy_script = "demo/scripts/deploy-production.sh"\n',
        }
        for key, declaration in declarations.items():
            with self.subTest(key=key):
                self.registry.write_text(registered.replace(declaration, ""), encoding="utf-8")
                message = f"demo does not declare {key}"
                stale = self.run_tool("check", "demo", "--require-verified", expect=1)
                self.assertIn(message, stale.stderr)
                reseal = self.run_tool(
                    "run-verification",
                    "demo",
                    expect=1,
                )
                self.assertIn(message, reseal.stderr)
        self.registry.write_text(registered, encoding="utf-8")

    def test_shared_completion_does_not_invalidate_non_deployable_project(self) -> None:
        self.run_build("library")
        self.run_verification("library")
        (self.root / "scripts/complete-production.py").unlink()

        self.run_tool("check", "library", "--require-verified")

    def test_verification_glob_that_matches_nothing_is_refused(self) -> None:
        # A renamed or emptied test directory used to leave the glob matching
        # nothing, and the verification fingerprint was sealed anyway — a
        # project could lose its recorded verification with nothing said. It is
        # now refused exactly like a production input that does not exist.
        self.run_build()
        self.run_verification()
        (self.root / "demo/tests/case.txt").unlink()

        message = "declared input does not exist: demo/tests/*.txt"
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn(message, stale.stderr)
        reseal = self.run_tool("run-verification", "demo", expect=1)
        self.assertIn(message, reseal.stderr)

    def test_production_glob_that_matches_nothing_is_refused_the_same_way(self) -> None:
        self.run_build()
        registered = self.registry.read_text(encoding="utf-8")
        self.registry.write_text(
            registered.replace(
                'production_inputs = ["demo/src"]',
                'production_inputs = ["demo/src/*.toml"]',
            ),
            encoding="utf-8",
        )
        result = self.run_tool("check", "demo", expect=1)
        self.assertIn("declared input does not exist: demo/src/*.toml", result.stderr)
        self.registry.write_text(registered, encoding="utf-8")

    def test_symlink_target_content_is_part_of_source_fingerprint(self) -> None:
        shared = self.root / "shared/value.qml"
        shared.write_text("Item {}\n", encoding="utf-8")
        os.symlink("../../shared/value.qml", self.root / "demo/src/value.qml")
        self.run_build()
        shared.write_text("Item { enabled: false }\n", encoding="utf-8")
        result = self.run_tool("check", "demo", expect=1)
        self.assertIn("production inputs changed", result.stderr)

    def replace_in_registry(self, old: str, new: str) -> None:
        registered = self.registry.read_text(encoding="utf-8")
        self.assertIn(old, registered)
        self.registry.write_text(registered.replace(old, new, 1), encoding="utf-8")

    def write_file(self, relative: str, text: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def write_cargo_fixture(self) -> None:
        """A demo app that links a workspace crate the way Magnetita links magnetitad's.

        `demo` reaches `link` under a renamed key, `link` reaches `core` only
        through the workspace table, `demo` needs `helper` only to build, and
        `link` names `fixtures` only for its tests. Cargo, not this file,
        decides which of them the release artifact links.
        """
        self.write_file(
            "rs/Cargo.toml",
            """[workspace]
members = ["crates/core", "crates/link", "crates/helper", "crates/fixtures"]
resolver = "2"

[workspace.dependencies]
core = { path = "crates/core" }
""",
        )
        for name in ("core", "helper", "fixtures"):
            self.write_file(
                f"rs/crates/{name}/Cargo.toml",
                f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2021"\n',
            )
            self.write_file(f"rs/crates/{name}/src/lib.rs", "\n")
        self.write_file(
            "rs/crates/link/Cargo.toml",
            """[package]
name = "link"
version = "0.1.0"
edition = "2021"

[dependencies]
core.workspace = true

[dev-dependencies]
fixtures = { path = "../fixtures" }
""",
        )
        self.write_file("rs/crates/link/src/lib.rs", "\n")
        self.write_file(
            "demo/Cargo.toml",
            """[package]
name = "demo"
version = "0.1.0"
edition = "2021"

[dependencies]
linked = { package = "link", path = "../rs/crates/link" }

[build-dependencies]
helper = { path = "../rs/crates/helper" }
""",
        )
        self.write_file("demo/build.rs", "fn main() {}\n")
        self.replace_in_registry(
            'production_inputs = ["demo/src"]',
            'cargo_manifests = ["demo/Cargo.toml"]\n'
            'production_inputs = ["demo/Cargo.toml", "demo/build.rs", "demo/src", '
            '"rs/Cargo.toml", "rs/crates/link", "rs/crates/core", "rs/crates/helper"]',
        )

    def test_linked_cargo_packages_satisfy_the_input_guard(self) -> None:
        self.write_cargo_fixture()
        result = self.run_tool("check-inputs")
        self.assertIn("production inputs: every buildable project", result.stdout)

    def test_input_guard_names_a_missing_linked_path_package(self) -> None:
        # The TOOL-3 defect: a crate the app links only through another crate
        # and a workspace-inherited key, absent from production_inputs, so a
        # fix to it left the installed binary "current".
        self.write_cargo_fixture()
        self.replace_in_registry('"rs/crates/core", ', "")
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(
            "demo: production_inputs miss rs/crates/core (core, a linked path package)",
            result.stderr,
        )

        self.replace_in_registry(', "rs/crates/helper"', "")
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn("demo: production_inputs miss rs/crates/helper", result.stderr)

    def test_input_guard_requires_the_inherited_workspace_manifest(self) -> None:
        self.write_cargo_fixture()
        self.replace_in_registry('"rs/Cargo.toml", ', "")
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(
            "demo: production_inputs miss rs/Cargo.toml (the workspace manifest core inherits from)",
            result.stderr,
        )

    def test_input_guard_requires_the_own_targets_and_lockfile(self) -> None:
        self.write_cargo_fixture()
        self.write_file("demo/Cargo.lock", "version = 4\n")
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(
            "demo: production_inputs miss demo/Cargo.lock (the lockfile the build resolves against)",
            result.stderr,
        )
        self.replace_in_registry('"demo/build.rs", ', '"demo/Cargo.lock", ')
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(
            "demo: production_inputs miss demo/build.rs (a build target of demo)", result.stderr
        )

    def test_input_guard_requires_an_existing_cargo_manifest_to_be_declared(self) -> None:
        self.write_cargo_fixture()
        self.replace_in_registry('cargo_manifests = ["demo/Cargo.toml"]\n', "")
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(
            "demo: demo/Cargo.toml exists, but cargo_manifests does not name it", result.stderr
        )

    def test_closure_follows_normal_and_build_dependencies_only(self) -> None:
        self.write_cargo_fixture()
        sys.path.insert(0, str(TOOL.parent))
        import cargo_closure

        closure = cargo_closure.path_closure(self.root, ["demo/Cargo.toml"])
        # `fixtures` is linked into link's tests only, so it is not part of
        # what the release artifact is made from.
        self.assertEqual(
            [package.directory for package in closure.packages],
            ["demo", "rs/crates/core", "rs/crates/helper", "rs/crates/link"],
        )
        demo = closure.packages[0]
        self.assertEqual(demo.sources, ("demo/build.rs", "demo/src/main.rs"))
        self.assertEqual(demo.workspace_manifest, "demo/Cargo.toml")
        self.assertEqual(closure.packages[1].workspace_manifest, "rs/Cargo.toml")
        self.run_tool("check-inputs")

    def test_input_guard_requires_the_selected_toolchain_file(self) -> None:
        self.write_cargo_fixture()
        self.write_file("demo/rust-toolchain.toml", '[toolchain]\nchannel = "stable"\n')
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(
            "demo: production_inputs miss demo/rust-toolchain.toml "
            "(the toolchain file the build selects)",
            result.stderr,
        )
        self.replace_in_registry('"demo/build.rs", ', '"demo/build.rs", "demo/rust-toolchain.toml", ')
        self.run_tool("check-inputs")

    def test_input_guard_reports_a_manifest_cargo_cannot_read(self) -> None:
        self.write_cargo_fixture()
        self.write_file("demo/Cargo.toml", "[package\nname = \n")
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(
            "production-artifact: demo: cargo metadata failed for demo/Cargo.toml with exit ",
            result.stderr,
        )

    def test_input_guard_refuses_a_path_dependency_outside_the_repository(self) -> None:
        self.write_cargo_fixture()
        outside = tempfile.TemporaryDirectory()
        self.addCleanup(outside.cleanup)
        crate = Path(outside.name) / "stray"
        (crate / "src").mkdir(parents=True)
        (crate / "Cargo.toml").write_text(
            '[package]\nname = "stray"\nversion = "0.1.0"\nedition = "2021"\n',
            encoding="utf-8",
        )
        (crate / "src/lib.rs").write_text("\n", encoding="utf-8")
        manifest = (self.root / "demo/Cargo.toml").read_text(encoding="utf-8")
        self.write_file(
            "demo/Cargo.toml",
            manifest.replace(
                "[build-dependencies]", f'stray = {{ path = "{crate}" }}\n\n[build-dependencies]'
            ),
        )
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(
            "demo: the path dependency of demo lies outside the repository", result.stderr
        )

    def fake_rustc_reporting_its_directory(self) -> dict[str, str]:
        tools = self.root / "fake-tools"
        tools.mkdir(exist_ok=True)
        rustc = tools / "rustc"
        rustc.write_text("#!/bin/sh\nprintf 'rustc in %s\\n' \"$(pwd -P)\"\n", encoding="utf-8")
        rustc.chmod(0o755)
        return {"PATH": f"{tools}{os.pathsep}{os.environ.get('PATH', '')}"}

    def test_toolchain_is_probed_where_the_build_runs(self) -> None:
        # rustup picks the toolchain by directory, so a probe at the root
        # recorded the default compiler for an app built with a pinned one.
        environment = self.fake_rustc_reporting_its_directory()
        real = self.root.resolve()
        self.run_tool("run-build", "demo", environment=environment)
        manifest = tomllib.loads(self.demo_manifest.read_text(encoding="utf-8"))
        self.assertEqual(manifest["toolchain"]["rustc"], f"rustc in {real / 'demo'}")

        # A project whose Cargo builds run in two places records both.
        self.write_cargo_fixture()
        self.replace_in_registry(
            'cargo_manifests = ["demo/Cargo.toml"]',
            'cargo_manifests = ["demo/Cargo.toml", "rs/crates/link/Cargo.toml"]',
        )
        self.run_tool("run-build", "demo", environment=environment)
        manifest = tomllib.loads(self.demo_manifest.read_text(encoding="utf-8"))
        self.assertEqual(manifest["toolchain"]["rustc@demo"], f"rustc in {real / 'demo'}")
        self.assertEqual(
            manifest["toolchain"]["rustc@rs/crates/link"],
            f"rustc in {real / 'rs/crates/link'}",
        )
        self.assertNotIn("rustc", manifest["toolchain"])
        self.run_tool("check", "demo", environment=environment)

    def test_buildable_project_with_empty_inputs_is_refused(self) -> None:
        # Magnetita Android declared no inputs, so its fingerprint hashed only
        # the build script and the APK stayed "current" forever (TOOL-4).
        self.replace_in_registry(
            'production_inputs = ["demo/src"]', "production_inputs = []"
        )
        message = "demo declares no production_inputs"
        for arguments in (("run-build", "demo"), ("check", "demo"), ("check-inputs",)):
            with self.subTest(arguments=arguments):
                result = self.run_tool(*arguments, expect=1)
                self.assertIn(message, result.stderr)
        self.assertFalse(self.demo_manifest.exists())

        self.replace_in_registry("production_inputs = []\n", "")
        result = self.run_tool("check-inputs", expect=1)
        self.assertIn(message, result.stderr)

    def fake_rustc(self, version: str) -> dict[str, str]:
        tools = self.root / "fake-tools"
        tools.mkdir(exist_ok=True)
        rustc = tools / "rustc"
        rustc.write_text(f"#!/bin/sh\nprintf '%s\\n' 'rustc {version}'\n", encoding="utf-8")
        rustc.chmod(0o755)
        return {"PATH": f"{tools}{os.pathsep}{os.environ.get('PATH', '')}"}

    def test_toolchain_change_makes_the_artifact_stale(self) -> None:
        old = self.fake_rustc("1.0.0")
        self.run_tool("run-build", "demo", environment=old)
        self.run_tool("run-verification", "demo", environment=old)
        self.run_tool("check", "demo", "--require-verified", environment=old)

        new = self.fake_rustc("2.0.0")
        result = self.run_tool("check", "demo", "--require-verified", expect=1, environment=new)
        first, *details = result.stderr.splitlines()
        self.assertEqual(
            first,
            "production-artifact: the toolchain changed since the build; run build-production.sh",
        )
        self.assertIn(
            "production-artifact:   toolchain rustc: 'rustc 1.0.0' -> 'rustc 2.0.0'", details
        )
        # The binary itself came from the other compiler, so a verification
        # alone cannot clear it.
        reseal = self.run_tool("run-verification", "demo", expect=1, environment=new)
        self.assertIn("the toolchain changed since the build", reseal.stderr)
        self.run_tool("run-build", "demo", environment=new)
        self.run_tool("run-verification", "demo", environment=new)
        self.run_tool("check", "demo", "--require-verified", environment=new)

    def test_failed_check_names_the_changed_inputs(self) -> None:
        self.run_build()
        self.run_verification()
        (self.root / "demo/src/main.rs").write_text("fn main() { loop {} }\n", encoding="utf-8")
        (self.root / "demo/tests/new.txt").write_text("case v2\n", encoding="utf-8")

        result = self.run_tool("check", "demo", "--require-verified", expect=1)
        first, *details = result.stderr.splitlines()
        self.assertEqual(
            first,
            "production-artifact: production inputs changed; run build-production.sh; "
            "tests or rules changed; run verify-production.sh again",
        )
        self.assertIn("production-artifact:   changed production input: demo/src", details)
        self.assertIn(
            "production-artifact:   new verification input: demo/tests/new.txt", details
        )

    def test_repository_registry_declares_every_linked_cargo_package(self) -> None:
        # The registry guard over the real tree: every app's production inputs
        # hold the whole path-package closure Cargo reports for it.
        result = subprocess.run(
            [sys.executable, str(TOOL), "check-inputs"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.assertEqual(result.returncode, 0, msg=result.stderr)

    def test_session_worktree_refuses_build_verify_and_status(self) -> None:
        (self.root / ".celestina-worktree").write_text(
            'project = "demo"\n', encoding="utf-8"
        )
        for command in ("run-build", "run-verification", "status"):
            with self.subTest(command=command):
                process = self.run_tool(command, "demo", expect=1)
                self.assertIn(
                    "production-artifact: this is a session worktree; production "
                    "runs happen at landing (scripts/land-unit.py)",
                    process.stderr,
                )
        self.assertFalse((self.root / ".fixture-demo-build-ran").exists())
        self.assertFalse((self.root / ".fixture-demo-verify-ran").exists())

    def test_session_worktree_still_answers_check(self) -> None:
        self.run_build()
        (self.root / ".celestina-worktree").write_text(
            'project = "demo"\n', encoding="utf-8"
        )
        process = self.run_tool("check", "demo")
        self.assertIn("artifact: demo current", process.stdout)


if __name__ == "__main__":
    unittest.main()
