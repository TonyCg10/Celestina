#!/usr/bin/env python3
"""Hermetic fixtures for the glass-canvas guard: both findings and the ratchet."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import glass_canvas_contract as guard  # noqa: E402

GLASS = textwrap.dedent(
    """
    import QtQuick
    // color: CelestinaTheme.canvas is a comment, and never counts.
    ApplicationWindow {
        id: window
        color: CelestinaTheme.clear
        Item {
            anchors.fill: parent
            CelestinaBackdrop {
                anchors.fill: parent
            }
        }
    }
    """
)

OPAQUE = textwrap.dedent(
    """
    import QtQuick
    ApplicationWindow {
        id: window
        color: CelestinaTheme.canvas
        // CelestinaBackdrop { } in a comment is not a backdrop.
        Item {
            color: CelestinaTheme.clear
        }
    }
    """
)

NO_COLOUR = textwrap.dedent(
    """
    import QtQuick
    ApplicationWindow {
        CelestinaBackdrop { anchors.fill: parent }
    }
    """
)


class RuleTests(unittest.TestCase):
    def kinds(self, text: str) -> list[str]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "Main.qml"
            path.write_text(text, encoding="utf-8")
            return [finding.kind for finding in guard.scan_window(path)]

    def test_glass_window_passes(self) -> None:
        self.assertEqual(self.kinds(GLASS), [])

    def test_opaque_window_without_backdrop_has_both_findings(self) -> None:
        self.assertEqual(self.kinds(OPAQUE), ["opaque", "backdrop"])

    def test_a_nested_clear_does_not_make_the_window_clear(self) -> None:
        self.assertIn("opaque", self.kinds(OPAQUE))

    def test_a_window_with_no_colour_is_opaque(self) -> None:
        self.assertEqual(self.kinds(NO_COLOUR), ["opaque"])


class RatchetTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        for label, text in (("one", OPAQUE), ("two", GLASS)):
            (self.root / label).mkdir()
            (self.root / label / "Main.qml").write_text(text, encoding="utf-8")
        self.baseline = self.root / "baseline.tsv"

    def tearDown(self) -> None:
        self.directory.cleanup()

    def run_guard(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(HERE / "glass_canvas_contract.py"),
                "--baseline",
                str(self.baseline),
                *extra,
                f"one={self.root / 'one'}",
                f"two={self.root / 'two'}",
            ],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_missing_row_is_zero_and_fails(self) -> None:
        self.baseline.write_text("", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 1)
        self.assertIn("one: 2 finding(s) over the baseline 0", result.stderr)

    def test_matching_baseline_passes(self) -> None:
        self.baseline.write_text("2\tone\n0\ttwo\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Glass-canvas contract: OK", result.stdout)

    def test_stale_baseline_must_fall(self) -> None:
        self.baseline.write_text("3\tone\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 1)
        self.assertIn("lower it in this commit", result.stderr)

    def test_write_baseline_records_reality(self) -> None:
        result = self.run_guard("--write-baseline")
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = guard.read_baseline(self.baseline)
        self.assertEqual(rows, {"one": 2, "two": 0})

    def test_missing_main_window_exits_2(self) -> None:
        (self.root / "two" / "Main.qml").unlink()
        self.baseline.write_text("2\tone\n", encoding="utf-8")
        self.assertEqual(self.run_guard().returncode, 2)


if __name__ == "__main__":
    unittest.main()
