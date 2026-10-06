#!/usr/bin/env python3
"""Hermetic fixtures for the build-tree pruning the landing runs after a deploy."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import build_trees  # noqa: E402


def project(identifier: str, artifacts: list[str], manifest: str, **extra: object) -> dict:
    table: dict[str, object] = {
        "id": identifier,
        "artifact_paths": artifacts,
        "artifact_manifest": manifest,
    }
    table.update(extra)
    return table


REGISTRY = {
    "schema_version": 1,
    "projects": [
        project("app", ["app/target/release/app"], "app/target/production-artifact.toml"),
        # Two projects share one Cargo target, as Magnetita and celestina-rs do.
        project(
            "daemon-host",
            ["host/target/release/host", "lib/target/release/daemon"],
            "host/target/production-artifact.toml",
        ),
        project(
            "lib",
            ["lib/target/workspace/release/daemon"],
            "lib/target/production-artifact.toml",
        ),
        project(
            "style",
            ["style/build/libstyle.so", "style/build/Style"],
            "style/build/production-artifact.toml",
        ),
        project(
            "halted-shell",
            ["shell/build/shell", "style/build/libstyle.so"],
            "shell/build/production-artifact.toml",
            halted="2026-09-27",
        ),
        # A project with nothing to build registers no artifact.
        {"id": "docs-only"},
    ],
}


def write(root: Path, relative: str, data: str = "x") -> Path:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(data, encoding="utf-8")
    return path


class PruneTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        for relative in (
            "app/target/release/app",
            "app/target/production-artifact.toml",
            "app/target/release/deps/libbig.rlib",
            "app/target/debug/app",
            "app/target/debug/incremental/state.bin",
            "host/target/release/host",
            "host/target/production-artifact.toml",
            "host/target/debug/host",
            "lib/target/release/daemon",
            "lib/target/release/deps/daemon-123",
            "lib/target/workspace/release/daemon",
            "lib/target/workspace/release/build/out.rs",
            "lib/target/production-artifact.toml",
            "lib/target/debug/deps/test-binary",
            "style/build/libstyle.so",
            "style/build/Style/qmldir",
            "style/build/Style/Button.qml",
            "style/build/CMakeFiles/obj.o",
            "style/build/production-artifact.toml",
            "shell/build/shell",
            "shell/build/CMakeFiles/obj.o",
            "shell/build/production-artifact.toml",
            "app/src/main.rs",
        ):
            write(self.root, relative)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def exists(self, relative: str) -> bool:
        return os.path.lexists(self.root / relative)

    def test_build_roots_come_from_the_registered_artifacts(self) -> None:
        roots = build_trees.build_roots(REGISTRY)
        self.assertEqual(
            roots,
            ("app/target", "host/target", "lib/target", "style/build"),
        )

    def test_a_halted_project_keeps_its_own_tree(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        self.assertTrue(self.exists("shell/build/CMakeFiles/obj.o"))
        self.assertTrue(self.exists("shell/build/shell"))

    def test_prune_keeps_every_registered_artifact_and_manifest(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        for kept in (
            "app/target/release/app",
            "app/target/production-artifact.toml",
            "host/target/release/host",
            "host/target/production-artifact.toml",
            "lib/target/release/daemon",
            "lib/target/workspace/release/daemon",
            "lib/target/production-artifact.toml",
            "style/build/libstyle.so",
            "style/build/Style/qmldir",
            "style/build/Style/Button.qml",
            "style/build/production-artifact.toml",
            "app/src/main.rs",
        ):
            self.assertTrue(self.exists(kept), kept)

    def test_prune_removes_everything_else_under_the_roots(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        for removed in (
            "app/target/release/deps",
            "app/target/debug",
            "host/target/debug",
            "lib/target/release/deps",
            "lib/target/workspace/release/build",
            "lib/target/debug",
            "style/build/CMakeFiles",
        ):
            self.assertFalse(self.exists(removed), removed)

    def test_a_shared_target_keeps_the_artifact_another_project_registers(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        # lib/target is lib's root, and daemon-host's artifact lives in it.
        self.assertTrue(self.exists("lib/target/release/daemon"))

    def test_prune_marks_each_pruned_project_and_reports_the_bytes(self) -> None:
        report = build_trees.prune(self.root, REGISTRY)
        self.assertGreater(report.freed_bytes, 0)
        for identifier in ("app", "daemon-host", "lib", "style"):
            self.assertTrue(build_trees.is_pruned(self.root, REGISTRY_TABLES[identifier]))
        self.assertFalse(build_trees.is_pruned(self.root, REGISTRY_TABLES["halted-shell"]))

    def test_dry_run_removes_nothing(self) -> None:
        report = build_trees.prune(self.root, REGISTRY, dry_run=True)
        self.assertGreater(report.freed_bytes, 0)
        self.assertIn("app/target/debug", report.removed)
        self.assertTrue(self.exists("app/target/debug/app"))
        self.assertFalse(build_trees.is_pruned(self.root, REGISTRY_TABLES["app"]))

    def test_a_missing_root_is_skipped(self) -> None:
        import shutil

        shutil.rmtree(self.root / "app")
        report = build_trees.prune(self.root, REGISTRY)
        self.assertNotIn("app/target", report.removed)
        self.assertFalse(build_trees.is_pruned(self.root, REGISTRY_TABLES["app"]))

    def test_a_symlink_is_removed_without_following_it(self) -> None:
        outside = write(self.root, "outside/keep.txt")
        (self.root / "app/target/debug/link").symlink_to(outside.parent)
        build_trees.prune(self.root, REGISTRY)
        self.assertTrue(outside.exists())
        self.assertFalse(self.exists("app/target/debug"))

    def test_clear_mark_forgets_the_prune(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        build_trees.clear_mark(self.root, REGISTRY_TABLES["app"])
        self.assertFalse(build_trees.is_pruned(self.root, REGISTRY_TABLES["app"]))
        self.assertTrue(build_trees.is_pruned(self.root, REGISTRY_TABLES["lib"]))

    def test_a_second_prune_frees_nothing(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        self.assertEqual(build_trees.prune(self.root, REGISTRY).freed_bytes, 0)

    def test_a_root_outside_the_checkout_is_refused(self) -> None:
        registry = {
            "schema_version": 1,
            "projects": [project("evil", ["../elsewhere/target/x"], "../elsewhere/target/m.toml")],
        }
        with self.assertRaises(build_trees.PruneError):
            build_trees.build_roots(registry)


REGISTRY_TABLES = {table["id"]: table for table in REGISTRY["projects"]}


class CommandTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        write(
            self.root,
            "docs/projects.toml",
            'schema_version = 1\n\n[[projects]]\nid = "app"\n'
            'artifact_paths = ["app/target/release/app"]\n'
            'artifact_manifest = "app/target/production-artifact.toml"\n',
        )
        write(self.root, "app/target/release/app")
        write(self.root, "app/target/debug/app", "y" * 4096)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def run_tool(self, *arguments: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(HERE / "build_trees.py"), "--root", str(self.root), *arguments],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_the_command_prunes_and_reports(self) -> None:
        ran = self.run_tool("prune")
        self.assertEqual(ran.returncode, 0, ran.stderr)
        self.assertIn("app/target/debug", ran.stdout)
        self.assertFalse((self.root / "app/target/debug").exists())
        self.assertTrue((self.root / "app/target/release/app").exists())

    def test_the_command_dry_run_keeps_the_tree(self) -> None:
        ran = self.run_tool("prune", "--dry-run")
        self.assertEqual(ran.returncode, 0, ran.stderr)
        self.assertIn("would free", ran.stdout)
        self.assertTrue((self.root / "app/target/debug/app").exists())

    def test_the_command_refuses_a_session_worktree(self) -> None:
        write(self.root, ".celestina-worktree", 'project = "suite"\n')
        ran = self.run_tool("prune")
        self.assertEqual(ran.returncode, 1)
        self.assertIn("session worktree", ran.stderr)
        self.assertTrue((self.root / "app/target/debug/app").exists())


if __name__ == "__main__":
    unittest.main()
