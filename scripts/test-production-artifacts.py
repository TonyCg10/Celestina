#!/usr/bin/env python3
"""Regression fixtures for the reusable production-artifact contract."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


TOOL = Path(__file__).resolve().parent / "production_artifact.py"


class ProductionArtifactFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        for directory in ("docs", "scripts", "demo/scripts", "demo/src", "demo/target/release", "shared"):
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
deploy_script = "demo/scripts/deploy-production.sh"
status_script = "demo/scripts/status-production.sh"
artifact_manifest = "demo/target/production-artifact.toml"
artifact_paths = ["demo/target/release/demo"]
production_inputs = ["demo/src"]
verification_inputs = ["demo/tests/*.txt"]
""",
            encoding="utf-8",
        )
        (self.root / "demo/scripts/build-production.sh").write_text("build v1\n", encoding="utf-8")
        (self.root / "demo/scripts/verify-production.sh").write_text("verify v1\n", encoding="utf-8")
        (self.root / "demo/scripts/deploy-production.sh").write_text("deploy v1\n", encoding="utf-8")
        (self.root / "demo/scripts/status-production.sh").write_text("status v1\n", encoding="utf-8")
        (self.root / "scripts/production_artifact.py").write_text("contract v1\n", encoding="utf-8")
        (self.root / "scripts/production-common.sh").write_text("common v1\n", encoding="utf-8")
        (self.root / "demo/src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
        (self.root / "demo/target/release/demo").write_bytes(b"release-v1\n")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_tool(self, *arguments: str, expect: int = 0) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            [sys.executable, str(TOOL), "--registry", str(self.registry), *arguments],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.assertEqual(
            result.returncode,
            expect,
            msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
        return result

    def record_build(self) -> None:
        self.run_tool("record-build", "demo", "--build-command", "fixture build")

    def test_build_verify_and_exact_installed_copy(self) -> None:
        self.record_build()
        unverified = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("todavía no está verificado", unverified.stderr)

        self.run_tool("record-verification", "demo", "--verify-command", "fixture verify")
        self.run_tool("check", "demo", "--require-verified")

        installed = self.root / "stage/bin/demo"
        installed.parent.mkdir(parents=True)
        shutil.copy2(self.root / "demo/target/release/demo", installed)
        mapping = f"demo/target/release/demo={installed}"
        self.run_tool("status", "demo", "--installed", mapping)

        installed.write_bytes(b"different\n")
        result = self.run_tool("status", "demo", "--installed", mapping, expect=1)
        self.assertIn("DISTINTO", result.stdout)

    def test_source_and_artifact_changes_invalidate_manifest(self) -> None:
        self.record_build()
        (self.root / "demo/src/main.rs").write_text("fn main() { println!(\"changed\"); }\n", encoding="utf-8")
        source_result = self.run_tool("check", "demo", expect=1)
        self.assertIn("cambiaron inputs de producción", source_result.stderr)

        (self.root / "demo/src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
        self.record_build()
        (self.root / "demo/target/release/demo").write_bytes(b"tampered\n")
        artifact_result = self.run_tool("check", "demo", expect=1)
        self.assertIn("digest", artifact_result.stderr)

    def test_verification_change_requires_only_reverification(self) -> None:
        self.record_build()
        self.run_tool("record-verification", "demo", "--verify-command", "fixture verify")
        (self.root / "demo/scripts/verify-production.sh").write_text("verify v2\n", encoding="utf-8")

        self.run_tool("check", "demo")
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("vuelve a ejecutar verify-production.sh", stale.stderr)
        self.run_tool("record-verification", "demo", "--verify-command", "fixture verify v2")
        self.run_tool("check", "demo", "--require-verified")

    def test_shared_deploy_helper_change_requires_only_reverification(self) -> None:
        self.record_build()
        self.run_tool("record-verification", "demo", "--verify-command", "fixture verify")
        (self.root / "scripts/production-common.sh").write_text("common v2\n", encoding="utf-8")

        self.run_tool("check", "demo")
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("vuelve a ejecutar verify-production.sh", stale.stderr)

    def test_project_deploy_change_requires_only_reverification(self) -> None:
        self.record_build()
        self.run_tool("record-verification", "demo", "--verify-command", "fixture verify")
        (self.root / "demo/scripts/deploy-production.sh").write_text("deploy v2\n", encoding="utf-8")

        self.run_tool("check", "demo")
        stale = self.run_tool("check", "demo", "--require-verified", expect=1)
        self.assertIn("vuelve a ejecutar verify-production.sh", stale.stderr)

    def test_symlink_target_content_is_part_of_source_fingerprint(self) -> None:
        shared = self.root / "shared/value.qml"
        shared.write_text("Item {}\n", encoding="utf-8")
        os.symlink("../../shared/value.qml", self.root / "demo/src/value.qml")
        self.record_build()
        shared.write_text("Item { enabled: false }\n", encoding="utf-8")
        result = self.run_tool("check", "demo", expect=1)
        self.assertIn("cambiaron inputs de producción", result.stderr)


if __name__ == "__main__":
    unittest.main()
