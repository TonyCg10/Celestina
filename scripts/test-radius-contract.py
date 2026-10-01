#!/usr/bin/env python3
"""Hermetic fixtures for the radius guard: each rule positive and negative, and the ratchet."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import radius_contract as guard  # noqa: E402

THEME = textwrap.dedent(
    """
    pragma Singleton
    import QtQuick
    QtObject {
        readonly property int radiusXs: 3
        readonly property int radiusSm: 8
        readonly property int radiusMd: 12
        readonly property int radiusButton: 10
        readonly property int radiusLg: 20
        readonly property int radiusPill: 9999
        readonly property int radiusInput: radiusPill
        readonly property int spaceXs: 4
        readonly property int spaceSm: 8
        readonly property int spaceMd: 12
        readonly property int spaceCardInset: 8
        readonly property int windowMargin: 16
        function cornerInset(radius) { return Math.ceil(radius * 0.3) }
    }
    """
)


class RuleTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "CelestinaTheme.qml").write_text(THEME, encoding="utf-8")
        self.tokens = guard.theme_tokens(self.root / "CelestinaTheme.qml")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def scan(self, body: str) -> list[str]:
        path = self.root / "Sample.qml"
        path.write_text("import QtQuick\n" + textwrap.dedent(body), encoding="utf-8")
        return [f"{f.rule}: {f.detail}" for f in guard.scan_file(path, self.tokens)]

    def test_corner_inset_matches_the_theme(self) -> None:
        self.assertEqual(guard.corner_inset(20), 6)
        self.assertEqual(guard.corner_inset(12), 4)
        self.assertEqual(guard.corner_inset(8), 3)

    def test_theme_tokens_resolve_aliases(self) -> None:
        self.assertEqual(self.tokens["radiusInput"], 9999)
        self.assertEqual(self.tokens["radiusLg"], 20)

    def test_text_anchored_to_a_rounded_parent_without_inset_fails(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusMd
                Text { anchors.left: parent.left; anchors.bottom: parent.bottom; text: "name" }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertTrue(findings[0].startswith("inset: Text at 0 px needs cornerInset(radiusMd) = 4"))

    def test_text_with_a_token_inset_below_the_corner_inset_fails(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusLg
                Text { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceXs; text: "x" }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertIn("spaceXs (4) is below cornerInset(radiusLg) = 6", findings[0])

    def test_text_with_enough_inset_passes(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Text { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceSm; text: "x" }
                }
                """
            ),
            [],
        )

    def test_parent_padding_counts_as_the_inset(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Pane {
                    padding: CelestinaTheme.spaceSm
                    background: Rectangle { radius: CelestinaTheme.radiusLg }
                    Text { anchors.fill: parent; text: "x" }
                }
                """
            ),
            [],
        )

    def test_vertically_centred_child_is_not_a_corner_contact(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Text { anchors.verticalCenter: parent.verticalCenter; anchors.left: parent.left; anchors.leftMargin: CelestinaTheme.spaceXs; text: "x" }
                }
                """
            ),
            [],
        )

    def test_full_bleed_image_is_clipped_artwork(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusMd
                    clip: true
                    Image { anchors.fill: parent }
                }
                """
            ),
            [],
        )

    def test_partially_anchored_image_is_still_a_glyph(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusLg
                Image { anchors.left: parent.left; anchors.top: parent.top }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertTrue(findings[0].startswith("inset: "))

    def test_focus_ring_may_enter_the_inset(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    CelestinaFocusRing { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceXs }
                }
                """
            ),
            [],
        )

    def test_non_concentric_child_radius_fails(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusLg
                Rectangle { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceXs; radius: CelestinaTheme.radiusMd }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertIn("concentric: radiusLg (20) != radiusMd (12) + spaceXs (4)", findings[0])

    def test_concentric_child_radius_passes(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Rectangle { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceCardInset; radius: CelestinaTheme.radiusMd }
                }
                """
            ),
            [],
        )

    def test_a_pill_inside_anything_passes(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Rectangle { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceXs; radius: CelestinaTheme.radiusPill }
                }
                """
            ),
            [],
        )

    def test_numeric_margin_inside_a_rounded_surface_fails(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusLg
                Text { anchors.fill: parent; anchors.margins: 10; text: "x" }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertTrue(findings[0].startswith("literal: anchors.margins: 10"))

    def test_a_non_token_radius_is_not_inspected(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: height / 2
                    Text { anchors.fill: parent; text: "x" }
                }
                """
            ),
            [],
        )

    def test_centred_child_is_not_a_corner_contact(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Text { anchors.centerIn: parent; text: "x" }
                }
                """
            ),
            [],
        )

    def test_pill_owner_is_exempt_from_the_inset_rule(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusPill
                    Text { anchors.fill: parent; text: "x" }
                }
                """
            ),
            [],
        )

    def test_pill_owner_still_refuses_literals(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusPill
                Text { anchors.fill: parent; anchors.margins: 3 }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertTrue(findings[0].startswith("literal: anchors.margins: 3"))

    def test_comments_are_ignored(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    // Text { anchors.fill: parent; text: "x" }
                    /* Rectangle { anchors.margins: 3 } */
                }
                """
            ),
            [],
        )


class RatchetTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "CelestinaTheme.qml").write_text(THEME, encoding="utf-8")
        (self.root / "app").mkdir()
        (self.root / "app" / "Bad.qml").write_text(
            'import QtQuick\nRectangle { radius: CelestinaTheme.radiusLg; Text { anchors.fill: parent; text: "x" } }\n',
            encoding="utf-8",
        )
        self.baseline = self.root / "radius-baseline.tsv"

    def tearDown(self) -> None:
        self.temp.cleanup()

    def run_guard(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(HERE / "radius_contract.py"),
                "--theme",
                str(self.root / "CelestinaTheme.qml"),
                "--baseline",
                str(self.baseline),
                *extra,
                f"app={self.root / 'app'}",
            ],
            text=True,
            capture_output=True,
        )

    def test_missing_baseline_row_is_zero_and_fails(self) -> None:
        self.baseline.write_text("# findings\tproject\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("app: 1 finding(s) over the baseline 0", result.stderr)
        self.assertIn("Bad.qml:2: inset:", result.stdout)

    def test_matching_baseline_passes(self) -> None:
        self.baseline.write_text("# findings\tproject\n1\tapp\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Radius contract: OK", result.stdout)

    def test_stale_baseline_must_fall(self) -> None:
        self.baseline.write_text("# findings\tproject\n3\tapp\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 1)
        self.assertIn("app: baseline 3 exceeds the 1 finding(s) found; lower it in this commit", result.stderr)

    def test_malformed_baseline_row_exits_2(self) -> None:
        self.baseline.write_text("# findings\tproject\nmany app\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn(f"{self.baseline}: malformed row: many app", result.stderr)

    def test_write_baseline_records_reality(self) -> None:
        self.baseline.write_text("# findings\tproject\n", encoding="utf-8")
        result = self.run_guard("--write-baseline")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("1\tapp", self.baseline.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
