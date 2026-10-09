#!/usr/bin/env python3
"""Fixture tests for scripts/activation_contract.py."""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parent / "activation_contract.py"

CLAIM = (
    "fn serve(connection: &zbus::blocking::Connection) {\n"
    "    connection.request_name_with_flags(NAME, flags);\n"
    "}\n"
)


def write(root: pathlib.Path, relative: str, text: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


class ActivationContract(unittest.TestCase):
    def run_in(self, root: pathlib.Path, *roots: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(SCRIPT), *(f"{r}={r}" for r in roots)],
            cwd=root, capture_output=True, text=True, check=False)

    def setUp(self) -> None:
        self.scratch = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.scratch.name)

    def tearDown(self) -> None:
        self.scratch.cleanup()

    def test_an_adapter_without_a_claim_passes(self) -> None:
        write(self.root, "grafita/src/activation.rs",
              "// request_name_with_flags lives in celestina-core now.\n"
              "pub fn claim() { celestina_core::activation::claim_or_exit(GRAFITA) }\n"
              "let worker = std::thread::Builder::new().name(\"w\".to_owned());\n")
        result = self.run_in(self.root, "grafita")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_reintroduced_claim_is_refused(self) -> None:
        write(self.root, "grafita/src/activation.rs", CLAIM)
        result = self.run_in(self.root, "grafita")
        self.assertEqual(result.returncode, 1)
        self.assertIn("grafita/src/activation.rs:2", result.stderr)

    def test_a_claim_under_any_other_name_is_refused_too(self) -> None:
        write(self.root, "hematita/src/elsewhere.rs",
              "let _ = connection.request_name(NAME);\n")
        result = self.run_in(self.root, "hematita")
        self.assertEqual(result.returncode, 1)
        self.assertIn("hematita/src/elsewhere.rs:1", result.stderr)

    def test_sideritas_file_manager_and_portal_are_allowed_their_names_only(self) -> None:
        write(self.root, "siderita/src/dbus.rs",
              "c.request_name_with_flags(\"org.freedesktop.FileManager1\", f)?;\n")
        write(self.root, "siderita/src/portal.rs",
              "c.request_name_with_flags(\n"
              "    \"org.freedesktop.impl.portal.desktop.celestina\",\n    f,\n)?;\n")
        self.assertEqual(self.run_in(self.root, "siderita").returncode, 0)
        # The same file claiming a second name is refused.
        write(self.root, "siderita/src/dbus.rs",
              "c.request_name_with_flags(\"org.freedesktop.FileManager1\", f)?;\n"
              "c.request_name(OTHER)?;\n")
        result = self.run_in(self.root, "siderita")
        self.assertEqual(result.returncode, 1)
        self.assertIn("siderita/src/dbus.rs:2", result.stderr)

    def test_a_call_split_across_lines_is_refused(self) -> None:
        write(self.root, "grafita/src/activation.rs",
              "connection\n    .request_name_with_flags\n    (\n        NAME,\n        flags,\n    );\n")
        result = self.run_in(self.root, "grafita")
        self.assertEqual(result.returncode, 1)
        self.assertIn("grafita/src/activation.rs:2", result.stderr)

    def test_a_name_on_a_connection_builder_is_refused(self) -> None:
        write(self.root, "cuprita/src/claim.rs",
              "let c = zbus::blocking::connection::Builder::session()\n"
              "    .and_then(|builder| builder.name(NAME))\n"
              "    .and_then(zbus::blocking::connection::Builder::build);\n")
        result = self.run_in(self.root, "cuprita")
        self.assertEqual(result.returncode, 1)
        self.assertIn("cuprita/src/claim.rs:2", result.stderr)

    def test_fluoritas_mpris_name_is_allowed(self) -> None:
        write(self.root, "fluorita/src/mpris.rs",
              "zbus::blocking::connection::Builder::session()\n"
              "    .and_then(|builder| builder.name(BUS_NAME));\n")
        self.assertEqual(self.run_in(self.root, "fluorita").returncode, 0)

    def test_a_suite_name_spelled_outside_the_crate_is_refused(self) -> None:
        write(self.root, "hematita/src/main.rs",
              "const APP_ID: &str = \"org.celestina.Hematita\";\n"
              "// \"org.celestina.Comment\" in a comment does not count\n"
              "const QML: &str = \"org.celestina.hematita\";\n")
        result = self.run_in(self.root, "hematita")
        self.assertEqual(result.returncode, 1)
        self.assertIn("hematita/src/main.rs:1: literal", result.stderr)
        self.assertNotIn("Comment", result.stderr)
        self.assertNotIn(":3:", result.stderr)

    def test_a_claim_after_a_uri_on_the_same_line_is_refused(self) -> None:
        write(self.root, "grafita/src/uri.rs",
              "let u = \"file:///tmp\"; connection.request_name(NAME);\n")
        result = self.run_in(self.root, "grafita")
        self.assertEqual(result.returncode, 1)
        self.assertIn("grafita/src/uri.rs:1: claim", result.stderr)

    def test_a_glob_string_does_not_hide_a_later_claim(self) -> None:
        write(self.root, "fluorita/src/glob.rs",
              "let filter = \"image/*\";\n"
              "let raw = r#\"a /* b\"#; let c = '/';\n"
              "connection.request_name(NAME);\n"
              "// end */\n")
        result = self.run_in(self.root, "fluorita")
        self.assertEqual(result.returncode, 1)
        self.assertIn("fluorita/src/glob.rs:3: claim", result.stderr)

    def test_a_claim_inside_a_comment_still_does_not_count(self) -> None:
        write(self.root, "cuprita/src/doc.rs",
              "/* outer /* nested */ connection.request_name(NAME); */\n"
              "// connection.request_name(NAME);\n")
        self.assertEqual(self.run_in(self.root, "cuprita").returncode, 0)

    def test_build_trees_are_not_scanned(self) -> None:
        write(self.root, "cuprita/target/debug/gen.rs", CLAIM)
        self.assertEqual(self.run_in(self.root, "cuprita").returncode, 0)

    def test_the_real_tree_passes(self) -> None:
        repo = SCRIPT.parent.parent
        apps = [name for name in ("siderita", "grafita", "hematita", "fluorita",
                                  "cuprita", "magnetita") if (repo / name).is_dir()]
        result = self.run_in(repo, *apps)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    os.environ.setdefault("PYTHONDONTWRITEBYTECODE", "1")
    unittest.main()
