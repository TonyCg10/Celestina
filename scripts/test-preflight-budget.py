#!/usr/bin/env python3
"""Hermetic fixtures for the preflight byte budget."""

from __future__ import annotations

import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

import preflight_budget  # noqa: E402


REGISTRY = {
    "suite": {
        "id": "suite",
        "agents": "AGENTS.md",
        "status": "STATUS.md",
        "now_rules": ["docs/standards/language.md"],
    },
    "projects": [
        {"id": "app", "agents": "app/AGENTS.md", "status": "app/STATUS.md"},
    ],
}


def halted_registry() -> dict:
    registry = {**REGISTRY, "projects": list(REGISTRY["projects"])}
    registry["projects"].append(
        {
            "id": "old",
            "commit_prefix": "old",
            "halted": "2026-01-01",
            "commit_roots": ["old/"],
            "agents": "old/AGENTS.md",
            "status": "old/STATUS.md",
        }
    )
    return registry


class PreflightBudgetTests(unittest.TestCase):
    def setUp(self) -> None:
        self._directory = tempfile.TemporaryDirectory()
        self.addCleanup(self._directory.cleanup)
        self.root = Path(self._directory.name)

    def write(self, relative: str, size: int) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"x" * size)

    def baseline(self, *rows: str) -> None:
        self.write("scripts/preflight-baseline.tsv", 0)
        (self.root / "scripts/preflight-baseline.tsv").write_text(
            "# header\n" + "".join(f"{row}\n" for row in rows), encoding="utf-8"
        )

    def sizes(self, suite: int = 100, app_agents: int = 100, app_status: int = 100) -> None:
        self.write("AGENTS.md", suite)
        self.write("STATUS.md", 10)
        self.write("docs/standards/language.md", 10)
        self.write("app/AGENTS.md", app_agents)
        self.write("app/STATUS.md", app_status)

    def test_a_project_reads_the_suite_contract_its_own_contract_its_status_and_the_now_rules(
        self,
    ) -> None:
        self.assertEqual(
            preflight_budget.now_documents(REGISTRY, "app"),
            ["AGENTS.md", "app/AGENTS.md", "app/STATUS.md", "docs/standards/language.md"],
        )

    def test_the_suite_reads_its_contract_status_and_now_rules(self) -> None:
        self.assertEqual(
            preflight_budget.now_documents(REGISTRY, "suite"),
            ["AGENTS.md", "STATUS.md", "docs/standards/language.md"],
        )

    def test_under_its_limit_passes(self) -> None:
        self.sizes()
        self.baseline("root-agents\t16384", "suite\t1000", "app\t1000")
        self.assertEqual(preflight_budget.check(self.root, REGISTRY), [])

    def test_over_its_limit_names_the_project_and_both_numbers(self) -> None:
        self.sizes(app_status=900)
        self.baseline("root-agents\t16384", "suite\t1000", "app\t1000")
        self.assertEqual(
            preflight_budget.check(self.root, REGISTRY),
            [
                "preflight budget: app reads 1110 bytes before acting, over its limit of 1000; "
                "move closed work out of its STATUS.md into docs/history, or shorten its AGENTS.md"
            ],
        )

    def test_a_project_without_a_row_is_an_error(self) -> None:
        self.sizes()
        self.baseline("root-agents\t16384", "suite\t1000")
        self.assertEqual(
            preflight_budget.check(self.root, REGISTRY),
            ["preflight budget: app has no row in scripts/preflight-baseline.tsv"],
        )

    def test_an_unknown_row_is_an_error(self) -> None:
        self.sizes()
        self.baseline("root-agents\t16384", "suite\t1000", "app\t1000", "ghost\t1000")
        self.assertEqual(
            preflight_budget.check(self.root, REGISTRY),
            ["preflight budget: unknown row ghost"],
        )

    def test_a_halted_project_is_not_measured(self) -> None:
        registry = halted_registry()
        self.sizes()
        self.baseline("root-agents\t16384", "suite\t1000", "app\t1000")
        self.assertNotIn("old", preflight_budget.measure(self.root, registry))
        self.assertEqual(preflight_budget.check(self.root, registry), [])

    def test_a_halted_project_keeps_its_row(self) -> None:
        self.sizes()
        self.baseline("root-agents\t16384", "suite\t1000", "app\t1000", "old\t1000")
        self.assertEqual(preflight_budget.check(self.root, halted_registry()), [])

    def test_a_byte_cell_that_is_not_ascii_digits_names_its_line(self) -> None:
        path = self.root / "scripts/preflight-baseline.tsv"
        path.parent.mkdir(parents=True)
        path.write_text("suite\t4096\u00b2\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "preflight-baseline.tsv:1"):
            preflight_budget.load_baseline(path)

    def test_no_baseline_file_checks_nothing(self) -> None:
        self.sizes(suite=10**6)
        self.assertEqual(preflight_budget.check(self.root, REGISTRY), [])

    def test_the_root_contract_has_its_own_row(self) -> None:
        self.sizes(suite=20000)
        self.baseline("root-agents\t16384", "suite\t90000", "app\t90000")
        messages = preflight_budget.check(self.root, REGISTRY)
        self.assertEqual(len(messages), 1)
        self.assertIn("root-agents reads 20000 bytes", messages[0])
        self.assertIn("limit of 16384", messages[0])

    def test_a_malformed_baseline_line_names_its_number(self) -> None:
        path = self.root / "scripts/preflight-baseline.tsv"
        path.parent.mkdir(parents=True)
        path.write_text("# header\n\nsuite\t100\nbroken line\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, r":4\b"):
            preflight_budget.load_baseline(path)

    def test_the_repository_is_within_its_budget(self) -> None:
        root = SCRIPTS.parent
        registry = tomllib.loads((root / "docs/projects.toml").read_text(encoding="utf-8"))
        self.assertEqual(preflight_budget.check(root, registry), [])


if __name__ == "__main__":
    unittest.main()
