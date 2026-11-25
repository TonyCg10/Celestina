#!/usr/bin/env python3
"""Hermetic fixtures for the debug-build pruning the landing runs after a deploy."""

from __future__ import annotations

import fcntl
import os
import shutil
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
            ["shell/target/release/shell", "style/build/libstyle.so"],
            "shell/target/production-artifact.toml",
            halted="2026-09-27",
        ),
        # An artifact registered inside a debug profile is still kept.
        project("odd", ["odd/target/debug/odd"], "odd/target/production-artifact.toml"),
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
            "app/target/release/deps/libbig.rlib",
            "app/target/release/.fingerprint/app-1/hash",
            "app/target/release/build/app-1/out/qml/Main.qml",
            "app/target/production-artifact.toml",
            "app/target/debug/app",
            "app/target/debug/incremental/state.bin",
            "app/target/aarch64-linux-android/debug/libmobile.so",
            "app/target/aarch64-linux-android/release/libmobile.so",
            "app/target/cxxqt/generated.h",
            "host/target/release/host",
            "host/target/debug/host",
            "lib/target/release/daemon",
            "lib/target/workspace/release/daemon",
            "lib/target/workspace/debug/deps/test-binary",
            "lib/target/debug/deps/test-binary",
            "style/build/libstyle.so",
            "style/build/Style/qmldir",
            "style/build/CMakeFiles/obj.o",
            "shell/target/release/shell",
            "shell/target/debug/shell",
            "odd/target/debug/odd",
            "odd/target/debug/deps/cache.rlib",
            "app/src/main.rs",
        ):
            write(self.root, relative)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def exists(self, relative: str) -> bool:
        return os.path.lexists(self.root / relative)

    def test_build_roots_come_from_the_registered_artifacts(self) -> None:
        self.assertEqual(
            build_trees.build_roots(REGISTRY),
            ("app/target", "host/target", "lib/target", "odd/target", "style/build"),
        )

    def test_prune_removes_the_debug_profiles(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        for removed in (
            "app/target/debug",
            "app/target/aarch64-linux-android/debug",
            "host/target/debug",
            "lib/target/debug",
            "lib/target/workspace/debug",
            "odd/target/debug/deps",
        ):
            self.assertFalse(self.exists(removed), removed)

    def test_prune_keeps_the_release_cache_and_everything_else(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        for kept in (
            "app/target/release/app",
            "app/target/release/deps/libbig.rlib",
            "app/target/release/.fingerprint/app-1/hash",
            "app/target/release/build/app-1/out/qml/Main.qml",
            "app/target/production-artifact.toml",
            "app/target/aarch64-linux-android/release/libmobile.so",
            "app/target/cxxqt/generated.h",
            "lib/target/release/daemon",
            "lib/target/workspace/release/daemon",
            "style/build/CMakeFiles/obj.o",
            "style/build/Style/qmldir",
            "app/src/main.rs",
        ):
            self.assertTrue(self.exists(kept), kept)

    def test_a_registered_artifact_inside_a_debug_profile_is_kept(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        self.assertTrue(self.exists("odd/target/debug/odd"))

    def test_a_halted_project_keeps_its_own_tree(self) -> None:
        build_trees.prune(self.root, REGISTRY)
        self.assertTrue(self.exists("shell/target/debug/shell"))

    def test_prune_reports_what_it_freed(self) -> None:
        report = build_trees.prune(self.root, REGISTRY)
        self.assertGreater(report.freed_bytes, 0)
        self.assertIn("app/target/debug", report.removed)

    def test_dry_run_removes_nothing(self) -> None:
        report = build_trees.prune(self.root, REGISTRY, dry_run=True)
        self.assertIn("app/target/debug", report.removed)
        self.assertTrue(self.exists("app/target/debug/app"))

    def test_a_missing_root_is_skipped(self) -> None:
        shutil.rmtree(self.root / "app")
        report = build_trees.prune(self.root, REGISTRY)
        self.assertNotIn("app/target/debug", report.removed)

    def test_a_symlink_is_never_followed(self) -> None:
        outside = write(self.root, "outside/debug/keep.txt")
        (self.root / "app/target/linked").symlink_to(outside.parent.parent)
        (self.root / "app/target/debug/link").symlink_to(outside.parent)
        build_trees.prune(self.root, REGISTRY)
        self.assertTrue(outside.exists())
        self.assertFalse(self.exists("app/target/debug"))

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


class TrimSessionTarget(unittest.TestCase):
    MEMBER_FILES = (
        "debug/incremental/my_app-1abc/x",
        "debug/deps/my_app-9f.d",
        "debug/deps/libmy_app-9f.rlib",
        "debug/.fingerprint/my-app-9f/x",
        "debug/build/my-app-9f/out",
        "debug/examples/demo",
        "debug/my-app",
    )
    KEPT_FILES = (
        "debug/deps/libserde-77.rlib",
        "debug/.fingerprint/serde-77/x",
        "debug/.cargo-lock",
    )

    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.target = Path(self.temp.name) / "target"

    def tearDown(self) -> None:
        self.temp.cleanup()

    def exists(self, relative: str) -> bool:
        return os.path.lexists(self.target / relative)

    def fill(self, relatives: tuple[str, ...]) -> None:
        for relative in relatives:
            write(self.target, relative)

    def test_under_the_cap_nothing_is_removed(self) -> None:
        self.fill(self.MEMBER_FILES + self.KEPT_FILES)
        report = build_trees.trim_session_target(self.target, 1000)
        self.assertEqual(report, build_trees.PruneReport())
        for relative in self.MEMBER_FILES + self.KEPT_FILES:
            self.assertTrue(self.exists(relative), relative)

    def test_over_the_cap_member_artifacts_go_and_dependencies_stay(self) -> None:
        self.fill(self.MEMBER_FILES + self.KEPT_FILES)
        expected = sum((self.target / relative).stat().st_size for relative in self.MEMBER_FILES)
        report = build_trees.trim_session_target(self.target, 5)
        for relative in self.MEMBER_FILES:
            self.assertFalse(self.exists(relative), relative)
        for relative in self.KEPT_FILES:
            self.assertTrue(self.exists(relative), relative)
        self.assertEqual(report.freed_bytes, expected)
        self.assertFalse(report.busy)

    def test_still_over_the_cap_removes_the_profiles(self) -> None:
        write(self.target, "debug/deps/libserde-77.rlib", "x" * 100)
        write(self.target, "release/deps/libserde-88.rlib", "x" * 100)
        report = build_trees.trim_session_target(self.target, 10)
        self.assertFalse(self.exists("debug"))
        self.assertFalse(self.exists("release"))
        self.assertEqual(report.freed_bytes, 200)
        self.assertEqual(report.removed, ["debug", "release"])

    def test_a_hard_linked_file_counts_once(self) -> None:
        binary = write(self.target, "debug/deps/my_app-9f", "x" * 100)
        os.link(binary, self.target / "debug/my_app")
        write(self.target, "debug/incremental/my_app-1abc/x", "x" * 10)
        write(self.target, "debug/deps/libserde-77.rlib", "x" * 50)
        # 160 bytes on disk; counting the link twice would read 260.
        self.assertEqual(build_trees.tree_size(self.target), 160)
        self.assertEqual(
            build_trees.trim_session_target(self.target, 200), build_trees.PruneReport()
        )
        report = build_trees.trim_session_target(self.target, 100)
        self.assertEqual(report.freed_bytes, 110)
        self.assertTrue(self.exists("debug/deps/libserde-77.rlib"))
        self.assertFalse(self.exists("debug/my_app"))
        self.assertFalse(self.exists("debug/deps/my_app-9f"))

    def test_removing_a_profile_keeps_its_cargo_lock(self) -> None:
        write(self.target, "debug/.cargo-lock", "")
        write(self.target, "debug/deps/libserde-77.rlib", "x" * 100)
        write(self.target, "release/deps/libserde-88.rlib", "x" * 100)
        report = build_trees.trim_session_target(self.target, 10)
        self.assertTrue(self.exists("debug/.cargo-lock"))
        self.assertFalse(self.exists("debug/deps"))
        self.assertFalse(self.exists("release"))
        self.assertEqual(report.freed_bytes, 200)
        self.assertEqual(report.removed, ["debug", "release"])

    def test_a_held_cargo_lock_skips_the_trim(self) -> None:
        self.fill(self.MEMBER_FILES + self.KEPT_FILES)
        with open(self.target / "debug/.cargo-lock", "w", encoding="utf-8") as holder:
            fcntl.flock(holder, fcntl.LOCK_EX)
            report = build_trees.trim_session_target(self.target, 5)
        self.assertTrue(report.busy)
        self.assertEqual(report.removed, [])
        self.assertEqual(report.freed_bytes, 0)
        for relative in self.MEMBER_FILES + self.KEPT_FILES:
            self.assertTrue(self.exists(relative), relative)

    def test_a_symlinked_target_is_left_alone(self) -> None:
        real = Path(self.temp.name) / "real"
        write(real, "debug/deps/libserde-77.rlib", "x" * 100)
        self.target.symlink_to(real)
        report = build_trees.trim_session_target(self.target, 10)
        self.assertEqual(report, build_trees.PruneReport())
        self.assertTrue((real / "debug/deps/libserde-77.rlib").exists())

    def test_a_missing_or_plain_file_target_is_left_alone(self) -> None:
        self.assertEqual(build_trees.trim_session_target(self.target, 0), build_trees.PruneReport())
        write(self.target.parent, "target", "x" * 100)
        self.assertEqual(build_trees.trim_session_target(self.target, 0), build_trees.PruneReport())
        self.assertTrue(self.target.is_file())

    def test_a_symlink_inside_the_target_is_never_followed(self) -> None:
        outside = write(Path(self.temp.name), "outside/keep/data", "x" * 100)
        write(self.target, "debug/deps/libserde-77.rlib", "x" * 100)
        write(self.target, "debug/incremental/my_app-1abc/x")
        (self.target / "debug/examples").symlink_to(outside.parent.parent)
        (self.target / "debug/deps/my_app-9f").symlink_to(outside.parent)
        (self.target / "release").symlink_to(outside.parent.parent)
        build_trees.trim_session_target(self.target, 10)
        self.assertTrue(outside.exists())
        build_trees.trim_session_target(self.target, 0)
        self.assertTrue(outside.exists())
        self.assertFalse(self.exists("release"))

    def test_a_symlinked_profile_is_not_followed(self) -> None:
        outside = write(Path(self.temp.name), "outside/deps/libmine-1.rlib", "x" * 100)
        write(self.target, "other", "x" * 100)
        (self.target / "debug").symlink_to(outside.parent.parent)
        build_trees.trim_session_target(self.target, 10)
        self.assertTrue(outside.exists())

    def test_dry_run_removes_nothing(self) -> None:
        self.fill(self.MEMBER_FILES + self.KEPT_FILES)
        report = build_trees.trim_session_target(self.target, 5, dry_run=True)
        self.assertIn("debug/incremental", report.removed)
        self.assertIn("debug/my-app", report.removed)
        self.assertGreater(report.freed_bytes, 0)
        for relative in self.MEMBER_FILES + self.KEPT_FILES:
            self.assertTrue(self.exists(relative), relative)

    def test_the_command_line_prints_what_it_freed(self) -> None:
        self.fill(self.MEMBER_FILES + self.KEPT_FILES)
        ran = subprocess.run(
            [sys.executable, str(HERE / "build_trees.py"), "trim-session-target",
             str(self.target), "--cap-gib", "0"],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(ran.returncode, 0, ran.stderr)
        self.assertIn("build-trees: freed", ran.stdout)
        self.assertFalse(self.exists("debug/my-app"))


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
        self.assertIn("removed app/target/debug", ran.stdout)
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
