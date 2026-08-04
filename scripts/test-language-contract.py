#!/usr/bin/env python3
"""Hermetic fixtures for the repository language guard."""

# language-contract: allow-non-english
# These fixtures intentionally contain rejected non-English samples.

from __future__ import annotations

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

    def run_guard(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(SCRIPT), "--root", str(self.root)],
            text=True,
            capture_output=True,
        )

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

    def test_an_unmarked_source_keeps_the_old_rule(self) -> None:
        self.write_baseline()
        (self.root / "src/other.rs").write_text(
            'pub const EMPTY: &str = "Sin álbum";\n', encoding="utf-8"
        )
        result = self.run_guard()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("new non-English repository text", result.stdout)


if __name__ == "__main__":
    unittest.main()
