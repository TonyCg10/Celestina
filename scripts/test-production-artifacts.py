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

    def test_symlink_target_content_is_part_of_source_fingerprint(self) -> None:
        shared = self.root / "shared/value.qml"
        shared.write_text("Item {}\n", encoding="utf-8")
        os.symlink("../../shared/value.qml", self.root / "demo/src/value.qml")
        self.run_build()
        shared.write_text("Item { enabled: false }\n", encoding="utf-8")
        result = self.run_tool("check", "demo", expect=1)
        self.assertIn("production inputs changed", result.stderr)


if __name__ == "__main__":
    unittest.main()
