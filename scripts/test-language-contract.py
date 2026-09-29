#!/usr/bin/env python3
"""Hermetic fixtures for the repository language guard."""

# language-contract: allow-non-english
# These fixtures intentionally contain rejected non-English samples.

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("check-language-contract.py")


class LanguageContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        subprocess.run(["git", "init", "-q"], cwd=self.root, check=True)
        (self.root / "scripts").mkdir()
        (self.root / "docs/standards").mkdir(parents=True)
        (self.root / "src").mkdir()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def run_guard(self, compare_ref: str | None = None) -> subprocess.CompletedProcess[str]:
        environment = dict(os.environ)
        environment.pop("LANGUAGE_COMPARE_REF", None)
        if compare_ref is not None:
            environment["LANGUAGE_COMPARE_REF"] = compare_ref
        return subprocess.run(
            ["python3", str(SCRIPT), "--root", str(self.root)],
            text=True,
            capture_output=True,
            env=environment,
        )

    def git(self, *arguments: str) -> str:
        return subprocess.run(
            ["git", "-C", str(self.root), *arguments],
            check=True,
            text=True,
            capture_output=True,
        ).stdout.strip()

    def commit_fixture(self, message: str) -> str:
        self.git("config", "user.name", "Fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "core.hooksPath", "/dev/null")
        self.git("add", "-A")
        self.git("commit", "-qm", message)
        return self.git("rev-parse", "HEAD")

    def write_baseline(self, rows: str = "") -> None:
        (self.root / "scripts/language-baseline.tsv").write_text(
            "# suspicious_lines<TAB>path\n" + rows, encoding="utf-8"
        )

    def test_english_canonical_source_passes(self) -> None:
        self.write_baseline()
        (self.root / "docs/standards/example.md").write_text("# English rules\n", encoding="utf-8")
        self.assertEqual(self.run_guard().returncode, 0)

    def test_spanish_canonical_source_fails_without_baseline_escape(self) -> None:
        self.write_baseline()
        (self.root / "docs/standards/example.md").write_text(
            "# Reglas\nEl agente ejecuta la verificación.\n", encoding="utf-8"
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("non-English canonical text", result.stdout)

    def test_new_legacy_path_fails_and_existing_debt_is_exact(self) -> None:
        (self.root / "src/example.rs").write_text("// El agente ejecuta la prueba.\n", encoding="utf-8")
        self.write_baseline("1\tsrc/example.rs\n")
        self.assertEqual(self.run_guard().returncode, 0)
        (self.root / "src/example.rs").write_text(
            "// El agente ejecuta la prueba.\n// También verifica la aplicación.\n",
            encoding="utf-8",
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("language debt grew", result.stdout)

    def test_explicit_international_fixture_is_allowed(self) -> None:
        self.write_baseline()
        (self.root / "src/example.rs").write_text(
            "// language-contract: allow-non-english\nconst SAMPLE: &str = \"español\";\n",
            encoding="utf-8",
        )
        self.assertEqual(self.run_guard().returncode, 0)

    def test_qml_product_copy_is_allowed_but_only_inside_qstr(self) -> None:
        self.write_baseline()
        (self.root / "src/Surface.qml").write_text(
            'Text { text: qsTr("Añadir carpeta…") }\n', encoding="utf-8"
        )
        self.assertEqual(self.run_guard().returncode, 0)

        # A bare literal in QML is a state token, an icon name or a path — not
        # something a person reads — so it stays development truth.
        (self.root / "src/Surface.qml").write_text(
            'Text { text: qsTr("Añadir carpeta…") }\nText { text: "Imágenes" }\n',
            encoding="utf-8",
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("new non-English repository text", result.stdout)

    def test_a_wrapped_qstr_call_is_still_product_copy(self) -> None:
        # The literal sits on the line after `qsTr(`, which a line-by-line
        # scanner misses; the line numbers it reports must still be right.
        self.write_baseline()
        (self.root / "src/Surface.qml").write_text(
            "Item {\n"
            "    Accessible.description: qsTr(\n"
            '        "Quita la carpeta de la biblioteca")\n'
            "}\n",
            encoding="utf-8",
        )
        self.assertEqual(self.run_guard().returncode, 0)

        (self.root / "src/Surface.qml").write_text(
            "Item {\n"
            "    Accessible.description: qsTr(\n"
            '        "Quita la carpeta de la biblioteca")\n'
            '    property string token: "Imágenes"\n'
            "}\n",
            encoding="utf-8",
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("1 suspicious line", result.stdout)

    def test_qml_comments_are_still_development_truth(self) -> None:
        self.write_baseline()
        (self.root / "src/Surface.qml").write_text(
            '// El agente ejecuta la prueba de la aplicación.\n', encoding="utf-8"
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("new non-English repository text", result.stdout)

    def test_marked_source_exempts_literals_and_nothing_else(self) -> None:
        self.write_baseline()
        (self.root / "src/copy.rs").write_text(
            "// language-contract: product-copy\n"
            "/// English doc comment.\n"
            'pub const EMPTY: &str = "Sin álbum";\n',
            encoding="utf-8",
        )
        self.assertEqual(self.run_guard().returncode, 0)

        # The marker is not a place to park development prose.
        (self.root / "src/copy.rs").write_text(
            "// language-contract: product-copy\n"
            "// El agente ejecuta la prueba de la aplicación.\n"
            'pub const EMPTY: &str = "Sin álbum";\n',
            encoding="utf-8",
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("new non-English repository text", result.stdout)

    def test_markers_do_not_exempt_documents(self) -> None:
        # About twenty Markdown records parked development prose behind the
        # markers (TOOL-11); a document is never product copy or a fixture.
        (self.root / "notes").mkdir()
        for marker in ("product-copy", "allow-non-english"):
            with self.subTest(marker=marker):
                self.write_baseline()
                (self.root / "notes/record.md").write_text(
                    f"<!-- language-contract: {marker} -->\n"
                    "El agente ejecuta la prueba de la aplicación.\n",
                    encoding="utf-8",
                )
                result = self.run_guard()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("notes/record.md: new non-English repository text", result.stdout)

    def test_product_copy_counts_only_in_rust_and_cpp(self) -> None:
        self.write_baseline()
        (self.root / "src/copy.py").write_text(
            "# language-contract: product-copy\nEMPTY = \"Sin álbum\"\n", encoding="utf-8"
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("src/copy.py: new non-English repository text", result.stdout)
        (self.root / "src/copy.py").unlink()
        for suffix in (".rs", ".cpp", ".h"):
            with self.subTest(suffix=suffix):
                (self.root / f"src/copy{suffix}").write_text(
                    "// language-contract: product-copy\nconst char *EMPTY = \"Sin álbum\";\n",
                    encoding="utf-8",
                )
                self.assertEqual(self.run_guard().returncode, 0)

    def test_markers_never_exempt_a_canonical_path(self) -> None:
        self.write_baseline()
        (self.root / "docs/standards/example.md").write_text(
            "<!-- language-contract: allow-non-english -->\nEl agente ejecuta la verificación.\n",
            encoding="utf-8",
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("docs/standards/example.md: non-English canonical text", result.stdout)

    def test_a_document_may_cite_product_copy_in_code(self) -> None:
        # Plans and evidence quote the strings a person reads; quoted in code
        # they are the product copy they cite, and the prose around them is
        # still scanned.
        self.write_baseline()
        (self.root / "notes").mkdir()
        (self.root / "notes/record.md").write_text(
            "The notice reads `\"Operación cancelada\"` after a cancel.\n\n"
            "```qml\n"
            "Text { text: qsTr(\"Añadir carpeta…\") }\n"
            "```\n\n"
            "```rust\n"
            "assert!(queue.settle(first, \"Carpeta leída\"));\n"
            "```\n",
            encoding="utf-8",
        )
        self.assertEqual(self.run_guard().returncode, 0)
        with (self.root / "notes/record.md").open("a", encoding="utf-8") as record:
            record.write('The "Tamaño" button moved to the left.\n')
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("notes/record.md: new non-English repository text (1", result.stdout)

    def test_a_citation_does_not_exempt_prose_around_it(self) -> None:
        # The reviewer's probes: each shape the old scanner flagged must stay
        # flagged, except a literal quoted inside one inline code span in a
        # record that is not canonical, which is the citation itself.
        (self.root / "notes").mkdir()
        sentence = "Esta función está rota y hay que arreglarla"
        cases = {
            "inline span": (f'Run it and see `"{sentence}"` here.\n', False),
            "span across a blank line": (
                f'The `x flag.\n\nWe said "{sentence}" loudly.\n\nThen `y`.\n', True
            ),
            "unclosed fence": (f'```\n"{sentence}"\nmore\n', True),
            "qsTr in prose": (f'Call qsTr("{sentence}") please.\n', True),
            "plain prose": (f"{sentence}.\n", True),
        }
        for path in ("notes/record.md", "docs/standards/example.md"):
            canonical = path.startswith("docs/")
            for name, (text, flagged) in cases.items():
                with self.subTest(path=path, case=name):
                    self.write_baseline()
                    (self.root / path).write_text(text, encoding="utf-8")
                    result = self.run_guard()
                    self.assertEqual(result.returncode != 0, flagged or canonical, result.stdout)
                    (self.root / path).unlink()

    def test_international_input_fixtures_keep_their_marker(self) -> None:
        self.write_baseline()
        (self.root / "tests").mkdir()
        (self.root / "tests/tst_input.qml").write_text(
            "// language-contract: allow-non-english\nText { text: \"Imágenes\" }\n",
            encoding="utf-8",
        )
        (self.root / "src/fold.rs").write_text(
            "// language-contract: allow-non-english\n// `cancion` finds `Canción`.\n",
            encoding="utf-8",
        )
        self.assertEqual(self.run_guard().returncode, 0)

    def test_an_unmarked_source_keeps_the_old_rule(self) -> None:
        self.write_baseline()
        (self.root / "src/other.rs").write_text(
            'pub const EMPTY: &str = "Sin álbum";\n', encoding="utf-8"
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("new non-English repository text", result.stdout)


    def test_a_resolvable_compare_ref_still_ratchets_the_baseline(self) -> None:
        (self.root / "src/example.rs").write_text(
            "// El agente ejecuta la prueba.\n", encoding="utf-8"
        )
        self.write_baseline("1\tsrc/example.rs\n")
        revision = self.commit_fixture("fixture: establish language debt")
        self.assertEqual(self.run_guard(revision).returncode, 0)

        # Raising a committed row is the case the historical comparison exists
        # for, and it must still be caught.
        (self.root / "src/example.rs").write_text(
            "// El agente ejecuta la prueba.\n// También verifica la aplicación.\n",
            encoding="utf-8",
        )
        self.write_baseline("2\tsrc/example.rs\n")
        result = self.run_guard(revision)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("baseline increased from 1 to 2", result.stdout)

    def test_an_unresolvable_compare_ref_fails_closed(self) -> None:
        # The ratchet used to vanish here and the guard printed OK. CI passes
        # `github.event.before`, which is all zeros when a branch is created.
        (self.root / "src/example.rs").write_text(
            "// El agente ejecuta la prueba.\n", encoding="utf-8"
        )
        self.write_baseline("1\tsrc/example.rs\n")
        self.commit_fixture("fixture: establish language debt")

        for compare_ref in ("0" * 40, "this-ref-does-not-exist"):
            with self.subTest(compare_ref=compare_ref):
                result = self.run_guard(compare_ref)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(
                    f"cannot resolve LANGUAGE_COMPARE_REF={compare_ref}", result.stdout
                )

    def test_a_resolvable_ref_without_a_baseline_is_an_initial_baseline(self) -> None:
        # A real first commit that predates the baseline file is not a failure;
        # only a ref that cannot be resolved at all is.
        (self.root / "src/example.rs").write_text("// English comment.\n", encoding="utf-8")
        revision = self.commit_fixture("fixture: publish without a baseline")
        self.write_baseline()
        result = self.run_guard(revision)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("initial baseline; no history at", result.stdout)


if __name__ == "__main__":
    unittest.main()
