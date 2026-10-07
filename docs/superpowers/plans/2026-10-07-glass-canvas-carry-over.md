# Glass Canvas Carry-Over Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Grafita, Hematita, Fluorita and Magnetita (and Siderita's file picker) the window Siderita has had since 1.8.0: a transparent window the compositor blurs, with the Haze canvas painted over it, opaque `#0b0c10` boxes and Haze-tinted bars, guarded so no application window ships opaque again.

**Architecture:** One suite unit first adds a ratcheted guard, `scripts/glass_canvas_contract.py`, that reads every registered application's `qml/Main.qml` and counts two findings: a root `color` other than `CelestinaTheme.clear`, and no `CelestinaBackdrop`. Then one unit per application lowers its own ratchet row to 0 by setting `color: CelestinaTheme.clear`, placing a `CelestinaBackdrop` under all content, and fixing the few app-local fills that were made for an opaque canvas. Everything else the new style brought (card colour, pill fill, hover ladder, modal fade and focus) already lives in `celestina-style` and reached every application with STYLE-G7-O…W; this plan only verifies it.

**Tech Stack:** Qt 6.11 / QML, CXX-Qt Rust applications, Python 3 guards, bash contract scripts, Niri compositor.

**Spec:** [2026-09-30-desktop-glass-design.md](../specs/2026-09-30-desktop-glass-design.md) (§2 sealed decisions, §4 shared rules), extended by the author's live spike recorded in [siderita/docs/evidence/2026-10-05-glass-canvas.md](../../../siderita/docs/evidence/2026-10-05-glass-canvas.md) (STYLE-G7-Q, SID-H1-F). Siderita's `qml/Main.qml` is the reference implementation.

## Global Constraints

- Development text (code, comments, commit messages, docs) is English; product copy is Spanish and only inside `qsTr()`.
- QML colours, radii, spacing, durations and opacities are `CelestinaTheme` tokens only; the style guard refuses hex and numeric literals.
- A `celestina-style:` commit touches only `celestina-style/`; `<app>:` commits touch only that app plus the shared ratchet files registered in `docs/projects.toml`; `suite:` may touch anything.
- Session commits use `<prefix>-maintenance:` subjects with an allowlisted imperative verb (`python3 scripts/commit_scope.py --check "<subject>"`, paths piped on stdin). Sessions never bump versions: `scripts/land-unit.py` does.
- Work happens in `scripts/worktree.sh open <project> <unit>` worktrees; landing runs from the clean canonical `main` with `python3 scripts/land-unit.py unit/<project>/<unit> --kind <kind> --summary "<Verb …>"`.
- Ratchets only fall, and fall in the commit that earns it: `scripts/qmllint-baseline.tsv`, `scripts/radius-baseline.tsv`, `scripts/architecture-baseline.tsv`, `scripts/language-baseline.tsv`, and from Task 1 `scripts/glass-canvas-baseline.tsv`.
- An outer id used inside a nested `Component`/`layer.effect` needs `pragma ComponentBehavior: Bound`, or qmllint grows.
- No screenshots: the author judges every visual change live on a binary the session launches (`<worktree>/../.cargo-target/release/<app>`).
- Out of scope here, still owed by spec §5: the top bar, icon-only actions, margin tokens and the per-app reorganisation. This plan does not move a single control.

## The recipe (what every application task applies)

Taken verbatim from Siderita (`siderita/qml/Main.qml`):

```qml
    // Transparent: the compositor blurs what lies behind the window and
    // CelestinaBackdrop paints the Haze canvas over it (DESIGN §5.2 L0).
    color: CelestinaTheme.clear
```

and, as the first visual child under every piece of content:

```qml
        CelestinaBackdrop {
            anchors.fill: parent
        }
```

`CelestinaBackdrop` paints `glassTint` (`#b3050608`, the Haze canvas at 0.70) with the grain at `glassCanvasNoiseOpacity` (0.05). The blur comes from the author's Niri rule, which already matches every application (`match app-id=r#"^org\.celestina\."#` with `background-effect { blur true; xray false }` and `opacity 1.0` in `~/.config/niri/config.kdl`); no compositor change is needed.

Colour rules from the spike, for anything an application paints itself:

| Thing | Fill |
|---|---|
| Window canvas | `CelestinaTheme.clear` + `CelestinaBackdrop` |
| Box, card, panel, document page | `CelestinaTheme.card` (opaque `#0b0c10`), or `CelestinaSurface` (Panel/Grouped/Content/Tonal already paint `card`) |
| Bar or pill sitting on the canvas | `CelestinaTheme.pillFill` (the Haze tint), no outline |
| Floating glass over scrolling content | `GlassSurface` / `GlassCard` / `CelestinaCapsule` (already Haze) |
| Picture surfaces (video, mirror) | stay opaque `CelestinaTheme.canvas` behind the picture |

## File map

| File | Task | Change |
|---|---|---|
| `scripts/glass_canvas_contract.py` | 1 | Create: the guard |
| `scripts/test-glass-canvas-contract.py` | 1 | Create: hermetic fixtures |
| `scripts/glass-canvas-baseline.tsv` | 1, 2–5 | Create with today's debt; each app task lowers its row to 0 |
| `scripts/check-architecture-contract.sh` | 1 | Run the guard in `check_visual_contract` |
| `docs/projects.toml` | 1 | Register the baseline as a shared ratchet file |
| `.github/workflows/contracts.yml` | 1 | Run the fixtures in "Hermetic fixtures" |
| `grafita/qml/Main.qml` | 2 | Clear window, backdrop |
| `grafita/qml/components/TabStrip.qml`, `FindBar.qml` | 2 | `surface` + hairline → `pillFill`, no outline |
| `grafita/qml/components/DocumentView.qml` | 2 | Page `inputFill` → `card` |
| `hematita/qml/CelestinaBackdrop.qml` (symlink), `hematita/build.rs` | 3 | Link and register the shared backdrop |
| `hematita/qml/Main.qml` | 3 | Clear window, backdrop |
| `fluorita/qml/Main.qml` | 4 | Clear window; opaque picture backing |
| `magnetita/qml/Main.qml` | 5 | Clear window (backdrop exists); `MirrorWindow.qml` stays opaque |
| `siderita/qml/PickerWindow.qml` | 6 | Clear window, backdrop |
| `<app>/docs/plans/active/*.md`, `<app>/docs/evidence/2026-10-07-glass-canvas.md` | 2–6 | Ledger row and evidence per unit |

## Units and order

| Task | Unit | Prefix | Ledger | Landing kind |
|---|---|---|---|---|
| 1 | `AUD-1-M` | `suite:` | `docs/plans/active/2026-09-26-monorepo-hardening.md` | `maintenance` |
| 2 | `GRA-H1-C` | `grafita:` | `grafita/docs/plans/active/2026-09-26-hardening.md` | `milestone` |
| 3 | `HEM-H1-C` | `hematita:` | `hematita/docs/plans/active/2026-09-26-hardening.md` | `milestone` |
| 4 | `FLU-H1-C` | `fluorita:` | `fluorita/docs/plans/active/2026-09-26-hardening.md` | `milestone` |
| 5 | `MAG-D1-F` | `magnetita:` | `magnetita/docs/plans/active/2026-09-13-app-design.md` | `milestone` |
| 6 | `SID-H1-L` | `siderita:` | `siderita/docs/plans/active/2026-09-26-hardening.md` | `bug` |

Task 1 lands first. Tasks 2–6 depend only on Task 1, may be prepared in parallel worktrees, and land in table order. Each touches its own row of `scripts/glass-canvas-baseline.tsv`; adjacent rows can conflict on rebase: resolve by keeping every row already at 0 on `origin/main` plus this unit's 0.

---

### Task 1: The glass-canvas guard (`AUD-1-M`, suite)

**Files:**
- Create: `scripts/glass_canvas_contract.py`
- Create: `scripts/test-glass-canvas-contract.py`
- Create: `scripts/glass-canvas-baseline.tsv`
- Modify: `scripts/check-architecture-contract.sh` (inside `check_visual_contract`, after the radius block)
- Modify: `docs/projects.toml` (`shared_ratchet_files`)
- Modify: `.github/workflows/contracts.yml` ("Hermetic fixtures" step)
- Modify: `docs/plans/active/2026-09-26-monorepo-hardening.md` (ledger row + bullet)
- Create: `docs/evidence/2026-10-07-glass-canvas-contract.md`

**Interfaces:**
- Produces: `glass_canvas_contract.py --baseline TSV [--write-baseline] LABEL=QML_ROOT ...`; exit 0 when every label's findings equal its row, 1 when over or under, 2 on usage or a missing `Main.qml`. Prints `"<label>: <n> finding(s) over the baseline <floor>"` or `"<label>: baseline <floor> exceeds the <n> finding(s) found; lower it in this commit"`, and `"Glass-canvas contract: OK"` on success. Python API: `scan_window(path: pathlib.Path) -> list[Finding]`, `Finding.kind in {"opaque", "backdrop"}`.
- Produces: baseline rows `findings<TAB>project`; Tasks 2–6 set their row to `0`.

- [ ] **Step 1: Open the worktree**

```bash
cd /home/toni/CODIGO/CELESTINA && scripts/worktree.sh open suite AUD-1-M
cd /home/toni/CODIGO/CELESTINA.worktrees/suite-AUD-1-M
```

- [ ] **Step 2: Write the failing fixtures** — `scripts/test-glass-canvas-contract.py`

```python
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
```

- [ ] **Step 3: Run it to see it fail**

Run: `python3 scripts/test-glass-canvas-contract.py`
Expected: `ModuleNotFoundError: No module named 'glass_canvas_contract'`

- [ ] **Step 4: Write the guard** — `scripts/glass_canvas_contract.py`

```python
#!/usr/bin/env python3
"""The glass-canvas guard: every application window paints the Haze canvas.

An application's main window (`<qml root>/Main.qml`) is transparent, so the
compositor's blur shows through, and a `CelestinaBackdrop` paints the Haze
tint and grain over it (STYLE-G7-Q, DESIGN §5.2 L0). Two findings per window:

  opaque     the root object binds `color` to anything but
             `CelestinaTheme.clear`, or binds none (a window's default is
             opaque)
  backdrop   no `CelestinaBackdrop` object anywhere in the file

Comments never count. Findings count per project against a shrink-only
ratchet, as the radius guard's do: a project over its row fails, and a row
over reality fails too, so the unit that clears a window lowers its floor.

    glass_canvas_contract.py --baseline TSV [--write-baseline] LABEL=QML_ROOT ...
"""

from __future__ import annotations

import argparse
import dataclasses
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from architecture_scanners import strip_qml_comments  # noqa: E402
from radius_contract import BaselineError, read_baseline  # noqa: E402

COLOUR = re.compile(r"^\s*color\s*:\s*(\S.*?)\s*;?\s*$")
BACKDROP = re.compile(r"\bCelestinaBackdrop\s*\{")
CLEAR = "CelestinaTheme.clear"


@dataclasses.dataclass(frozen=True)
class Finding:
    path: pathlib.Path
    kind: str
    message: str

    def __str__(self) -> str:
        return f"{self.path}: {self.kind}: {self.message}"


def root_lines(text: str) -> list[str]:
    """The lines of the first object's own body, without nested objects."""
    start = text.find("{")
    if start < 0:
        return []
    depth = 0
    kept: list[str] = []
    for char in text[start:]:
        if char == "{":
            depth += 1
            kept.append("\n")
            continue
        if char == "}":
            depth -= 1
            kept.append("\n")
            if depth == 0:
                break
            continue
        if depth == 1:
            kept.append(char)
    return "".join(kept).splitlines()


def scan_window(path: pathlib.Path) -> list[Finding]:
    text = strip_qml_comments(path.read_text(encoding="utf-8"))
    findings: list[Finding] = []
    colours = [match.group(1) for line in root_lines(text)
               if (match := COLOUR.match(line))]
    if colours != [CLEAR]:
        shown = colours[0] if colours else "nothing"
        findings.append(Finding(path, "opaque",
                                f"the window binds color to {shown}, not {CLEAR}"))
    if not BACKDROP.search(text):
        findings.append(Finding(path, "backdrop",
                                "no CelestinaBackdrop paints the Haze canvas"))
    return findings


def write_baseline(path: pathlib.Path, counts: dict[str, int]) -> None:
    lines = [
        "# Glass-canvas debt ratchet. A row may only fall, and it falls in the",
        "# same commit that clears the window. Registered in docs/projects.toml as",
        "# a shared ratchet file so every project prefix may lower its own row.",
        "# findings<TAB>project",
    ]
    lines += [f"{count}\t{project}" for project, count in sorted(counts.items())]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--baseline", required=True, type=pathlib.Path)
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("roots", nargs="+", metavar="LABEL=QML_ROOT")
    arguments = parser.parse_args(argv)

    counts: dict[str, int] = {}
    for spec in arguments.roots:
        label, _, root = spec.partition("=")
        if not label or not root:
            print(f"expected LABEL=QML_ROOT, got {spec}", file=sys.stderr)
            return 2
        window = pathlib.Path(root) / "Main.qml"
        if not window.is_file():
            print(f"{label}: missing main window {window}", file=sys.stderr)
            return 2
        findings = scan_window(window)
        for finding in findings:
            print(finding)
        counts[label] = len(findings)

    if arguments.write_baseline:
        write_baseline(arguments.baseline, counts)
        print(f"Glass-canvas contract: baseline written to {arguments.baseline}")
        return 0

    try:
        baseline = read_baseline(arguments.baseline)
    except BaselineError as error:
        print(error, file=sys.stderr)
        return 2
    status = 0
    for label, count in counts.items():
        floor = baseline.get(label, 0)
        if count > floor:
            print(f"{label}: {count} finding(s) over the baseline {floor}", file=sys.stderr)
            status = 1
        elif count < floor:
            print(f"{label}: baseline {floor} exceeds the {count} finding(s) found; "
                  "lower it in this commit", file=sys.stderr)
            status = 1
    if status == 0:
        print("Glass-canvas contract: OK")
    return status


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 5: Run the fixtures**

Run: `python3 scripts/test-glass-canvas-contract.py`
Expected: `Ran 9 tests … OK`

- [ ] **Step 6: Record today's debt**

```bash
python3 scripts/glass_canvas_contract.py --baseline scripts/glass-canvas-baseline.tsv --write-baseline \
  siderita=siderita/qml magnetita=magnetita/qml grafita=grafita/qml fluorita=fluorita/qml hematita=hematita/qml
cat scripts/glass-canvas-baseline.tsv
```

Expected rows (after the four header comments): `1 fluorita`, `2 grafita`, `2 hematita`, `1 magnetita`, `0 siderita`. If any count differs, stop and read the printed findings: the plan's per-app tasks assume these.

- [ ] **Step 7: Wire it into the architecture contract** — in `scripts/check-architecture-contract.sh`, inside `check_visual_contract`, directly after the `radius_contract.py` block's closing `fi`:

```bash

    # The glass-canvas contract (STYLE-G7-Q): every application window is
    # transparent and paints the Haze canvas with CelestinaBackdrop, against
    # the per-project ratchet in scripts/glass-canvas-baseline.tsv.
    glass_roots=()
    while IFS=$'\t' read -r role identifier _path qml_root; do
        if [ "$role" = application ]; then
            glass_roots+=("$identifier=$qml_root")
        fi
    done < <(python3 scripts/architecture_scanners.py registry-qml-projects "$registry_file")
    if ! python3 scripts/glass_canvas_contract.py \
        --baseline scripts/glass-canvas-baseline.tsv \
        "${glass_roots[@]}"; then
        fail "the glass-canvas contract failed"
    fi
```

- [ ] **Step 8: Register the ratchet** — in `docs/projects.toml`, append to `shared_ratchet_files` after `"scripts/radius-baseline.tsv",`:

```toml
  # The glass-canvas ratchet moves for the same reason: the application unit
  # that clears its window is the one that lowers its floor.
  "scripts/glass-canvas-baseline.tsv",
```

and in `.github/workflows/contracts.yml`, "Hermetic fixtures" step, after `python3 scripts/test-language-contract.py`:

```yaml
          python3 scripts/test-glass-canvas-contract.py
```

- [ ] **Step 9: Run every guard**

```bash
python3 scripts/test-glass-canvas-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
bash scripts/test-architecture-scanners.sh
```

Expected: fixtures OK; `Glass-canvas contract: OK` inside `Architecture contract: OK`; the other three OK.

- [ ] **Step 10: Ledger and evidence**

Add under the `AUD-1-L` bullet in `docs/plans/active/2026-09-26-monorepo-hardening.md`:

```markdown
- `AUD-1-M` — at the author's request of 2026-10-07, carry Siderita's glass
  canvas to every application: a guard that counts, per application, a main
  window that is not transparent and one without `CelestinaBackdrop`, against
  a shrink-only ratchet the application units lower to 0.
```

and the ledger row after `AUD-1-L`'s:

```markdown
| AUD-1-M | `suite:` | active | — | — | Add the glass-canvas guard: every registered application's `Main.qml` binds `color: CelestinaTheme.clear` and contains a `CelestinaBackdrop`, counted per project against `scripts/glass-canvas-baseline.tsv` (shared ratchet), run by the architecture contract and CI. | [evidence](../../evidence/2026-10-07-glass-canvas-contract.md) | None |
```

Create `docs/evidence/2026-10-07-glass-canvas-contract.md` with the template sections (Date, Scope `AUD-1-M` — `suite`, Environment, Artifact "not applicable", Procedure = Step 9's commands, Result = their exits and the five baseline rows, Limits "only `Main.qml` is read; secondary windows (Siderita's picker, Magnetita's mirror) are judged by hand", Follow-up "GRA-H1-C, HEM-H1-C, FLU-H1-C, MAG-D1-F, SID-H1-L lower the rows").

- [ ] **Step 11: Commit**

```bash
git add scripts/glass_canvas_contract.py scripts/test-glass-canvas-contract.py \
  scripts/glass-canvas-baseline.tsv scripts/check-architecture-contract.sh \
  docs/projects.toml .github/workflows/contracts.yml \
  docs/plans/active/2026-09-26-monorepo-hardening.md \
  docs/evidence/2026-10-07-glass-canvas-contract.md \
  docs/superpowers/plans/2026-10-07-glass-canvas-carry-over.md
git commit -m "suite-maintenance: Add the glass-canvas guard for every application window

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 12: Land**

```bash
cd /home/toni/CODIGO/CELESTINA
python3 scripts/land-unit.py unit/suite/AUD-1-M --kind maintenance \
  --summary "Add the glass-canvas guard for every application window"
```

Expected: `land-unit: landed <sha> suite-maintenance: Add the glass-canvas guard …`.

---

### Task 2: Grafita (`GRA-H1-C`)

**Files:**
- Modify: `grafita/qml/Main.qml:25` and before `TabStrip {` (line ~272)
- Modify: `grafita/qml/components/TabStrip.qml:54-59`
- Modify: `grafita/qml/components/FindBar.qml:37-42`
- Modify: `grafita/qml/components/DocumentView.qml:86`
- Modify: `scripts/glass-canvas-baseline.tsv` (`grafita` row → 0)
- Modify: `grafita/docs/plans/active/2026-09-26-hardening.md`; Create: `grafita/docs/evidence/2026-10-07-glass-canvas.md`

**Interfaces:**
- Consumes: `glass_canvas_contract.py` and the `2 grafita` row from Task 1; `CelestinaBackdrop` (already linked and in `grafita/build.rs`).

- [ ] **Step 1: Open the worktree** — `scripts/worktree.sh open grafita GRA-H1-C`, then `cd /home/toni/CODIGO/CELESTINA.worktrees/grafita-GRA-H1-C`.

- [ ] **Step 2: Lower the row first (the failing test)** — in `scripts/glass-canvas-baseline.tsv` change `2	grafita` to `0	grafita`.

Run: `python3 scripts/glass_canvas_contract.py --baseline scripts/glass-canvas-baseline.tsv grafita=grafita/qml`
Expected: exit 1, `grafita: 2 finding(s) over the baseline 0` with an `opaque` and a `backdrop` line.

- [ ] **Step 3: Clear the window** — `grafita/qml/Main.qml`, replace `    color: CelestinaTheme.canvas` with:

```qml
    // Transparent: the compositor blurs what lies behind the window and
    // CelestinaBackdrop paints the Haze canvas over it (DESIGN §5.2 L0).
    color: CelestinaTheme.clear
```

and insert directly before `    TabStrip {`:

```qml
    // Under everything the window holds: the tab strip, the documents and
    // the dialogs all paint over the Haze canvas.
    CelestinaBackdrop {
        anchors.fill: parent
    }

```

- [ ] **Step 4: Bars take the Haze tint** — in `grafita/qml/components/TabStrip.qml` and `grafita/qml/components/FindBar.qml`, replace the background block

```qml
    Rectangle {
        anchors.fill: parent
        color: CelestinaTheme.surface
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.divider
    }
```

with

```qml
    // A bar on the glass canvas wears the Haze tint, like Siderita's bars,
    // with no outline: the spec's pure Haze has no lit edge.
    Rectangle {
        anchors.fill: parent
        color: CelestinaTheme.pillFill
    }
```

- [ ] **Step 5: The page is a box** — in `grafita/qml/components/DocumentView.qml`, inside `Rectangle { id: page … }`, replace `color: CelestinaTheme.inputFill` with:

```qml
        // A box on the glass canvas is opaque, in the bar's black: the text
        // must never sit over whatever the compositor shows behind the window.
        color: CelestinaTheme.card
```

(keep its `border` lines: the page is still an input surface).

- [ ] **Step 6: The guard passes**

Run: `python3 scripts/glass_canvas_contract.py --baseline scripts/glass-canvas-baseline.tsv grafita=grafita/qml`
Expected: `Glass-canvas contract: OK`

- [ ] **Step 7: Build, smoke, lint, guards**

```bash
(cd grafita && cargo build --release)
sh grafita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/grafita
bash scripts/qmllint-cxxqt.sh grafita
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

Expected: build finishes; smoke exit 0; `qmllint-production: OK` at the baseline; every contract OK.

- [ ] **Step 8: Ledger row and evidence** — append after `GRA-H1-B`'s row:

```markdown
| GRA-H1-C | `grafita:` | active | — | — | Give the window Siderita's glass canvas: transparent window with `CelestinaBackdrop` (STYLE-G7-Q recipe), the tab strip and find bar in the Haze `pillFill` with no outline, and the document page an opaque `card` box; lowers the glass-canvas ratchet row to 0. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-glass-canvas.md) | None |
```

Create `grafita/docs/evidence/2026-10-07-glass-canvas.md` with: Scope `GRA-H1-C` — `grafita`; Environment "the author's CachyOS, Qt 6.11.2, Niri with the `org.celestina.*` blur rule"; Procedure = Step 7's commands plus "the author opened Grafita with two documents and the find bar, focused, on the session"; Result = exits and what changed; Limits "the blur depends on the author's Niri rule"; Follow-up "spec §5.1 (top bar) stays owed".

- [ ] **Step 9: Commit**

```bash
git add grafita/qml scripts/glass-canvas-baseline.tsv grafita/docs
git commit -m "grafita-maintenance: Add the glass canvas to the window

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 10: Author check, then land** — launch `/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/grafita` with `nohup … &` and wait for the author's verdict. On approval, from the canonical checkout (rebase `--onto origin/main` first if Task 1 or an earlier app landed after this branch was cut):

```bash
python3 scripts/land-unit.py unit/grafita/GRA-H1-C --kind milestone --summary "Add the glass canvas to the window"
```

---

### Task 3: Hematita (`HEM-H1-C`)

**Files:**
- Create: `hematita/qml/CelestinaBackdrop.qml` (symlink to `../../celestina-style/CelestinaBackdrop.qml`)
- Modify: `hematita/build.rs` (`QML_FILES`)
- Modify: `hematita/qml/Main.qml:30` and before `    ColumnLayout {` (line ~46)
- Modify: `scripts/glass-canvas-baseline.tsv` (`hematita` row → 0)
- Modify: `hematita/docs/plans/active/2026-09-26-hardening.md`; Create: `hematita/docs/evidence/2026-10-07-glass-canvas.md`

**Interfaces:**
- Consumes: Task 1's guard and the `2 hematita` row.

- [ ] **Step 1: Open the worktree** — `scripts/worktree.sh open hematita HEM-H1-C`; `cd /home/toni/CODIGO/CELESTINA.worktrees/hematita-HEM-H1-C`.

- [ ] **Step 2: Lower the row (failing test)** — `2	hematita` → `0	hematita`.

Run: `python3 scripts/glass_canvas_contract.py --baseline scripts/glass-canvas-baseline.tsv hematita=hematita/qml`
Expected: exit 1, `hematita: 2 finding(s) over the baseline 0`.

- [ ] **Step 3: Link the shared backdrop**

```bash
ln -s ../../celestina-style/CelestinaBackdrop.qml hematita/qml/CelestinaBackdrop.qml
```

and in `hematita/build.rs`, in `QML_FILES` after `"qml/CelestinaFocusRing.qml",`:

```rust
    "qml/CelestinaBackdrop.qml",
```

(`haze-noise.png` already ships through the linked `icons.qrc`, which `GlassSurface` uses.)

- [ ] **Step 4: Clear the window** — `hematita/qml/Main.qml`, replace `    color: CelestinaTheme.canvas` with the recipe's three lines (comment + `color: CelestinaTheme.clear`), and insert before `    ColumnLayout {`:

```qml
    // Under the strip and every page: the Haze canvas over the compositor's
    // blur. The pages' panels are CelestinaSurface and already paint `card`.
    CelestinaBackdrop {
        anchors.fill: parent
    }

```

- [ ] **Step 5: The guard passes** — same command as Step 2. Expected: `Glass-canvas contract: OK`.

- [ ] **Step 6: Build, smoke, lint, guards**

```bash
(cd hematita && cargo build --release)
sh hematita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/hematita
bash scripts/qmllint-cxxqt.sh hematita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

Expected: all exit 0; the architecture contract's shared-style-link check accepts the new symlink (it resolves to the canonical file).

- [ ] **Step 7: Ledger row and evidence** — after `HEM-H1-B`'s row:

```markdown
| HEM-H1-C | `hematita:` | active | — | — | Give the window Siderita's glass canvas: link the shared `CelestinaBackdrop`, make the window transparent and paint the Haze canvas under the strip and every page (STYLE-G7-Q recipe); the panels already paint `card`. Lowers the glass-canvas ratchet row to 0. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-glass-canvas.md) | None |
```

Evidence `hematita/docs/evidence/2026-10-07-glass-canvas.md` as in Task 2, Procedure naming Step 6's commands and "the author walked the six sections, focused".

- [ ] **Step 8: Commit**

```bash
git add hematita/qml hematita/build.rs scripts/glass-canvas-baseline.tsv hematita/docs
git commit -m "hematita-maintenance: Add the glass canvas to the window

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 9: Author check, then land** — launch the `.cargo-target` binary; on approval:

```bash
python3 scripts/land-unit.py unit/hematita/HEM-H1-C --kind milestone --summary "Add the glass canvas to the window"
```

---

### Task 4: Fluorita (`FLU-H1-C`)

**Files:**
- Modify: `fluorita/qml/Main.qml:29` and before `    CelestinaBackdrop {` (line ~200)
- Modify: `scripts/glass-canvas-baseline.tsv` (`fluorita` row → 0)
- Modify: `fluorita/docs/plans/active/2026-09-26-hardening.md`; Create: `fluorita/docs/evidence/2026-10-07-glass-canvas.md`

**Interfaces:**
- Consumes: Task 1's guard and the `1 fluorita` row; `window.playing` (Main.qml:154) and `playerSurface.showsPicture` (PlayerSurface.qml:43), both existing.

- [ ] **Step 1: Open the worktree** — `scripts/worktree.sh open fluorita FLU-H1-C`; `cd /home/toni/CODIGO/CELESTINA.worktrees/fluorita-FLU-H1-C`.

- [ ] **Step 2: Lower the row (failing test)** — `1	fluorita` → `0	fluorita`.

Run: `python3 scripts/glass_canvas_contract.py --baseline scripts/glass-canvas-baseline.tsv fluorita=fluorita/qml`
Expected: exit 1, one `opaque` finding.

- [ ] **Step 3: Clear the window, keep the picture on black** — replace `    color: CelestinaTheme.canvas` with the recipe's three lines, and insert before the existing `    CelestinaBackdrop {`:

```qml
    // A picture is never shown over the desktop: while a video or a still is
    // on screen the window behind it is the opaque canvas it always was, so
    // letterboxing stays black. Everything else sits on the Haze canvas below.
    Rectangle {
        anchors.fill: parent
        visible: window.playing && playerSurface.showsPicture
        color: CelestinaTheme.canvas
    }

```

(The existing backdrop keeps `visible: !window.playing || !playerSurface.showsPicture`: exactly one of the two is visible.)

- [ ] **Step 4: The guard passes** — Step 2's command. Expected: `Glass-canvas contract: OK`.

- [ ] **Step 5: Build, smoke, lint, guards**

```bash
(cd fluorita && cargo build --release)
sh fluorita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/fluorita
bash scripts/qmllint-cxxqt.sh fluorita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

Expected: all exit 0.

- [ ] **Step 6: Ledger row and evidence** — after `FLU-H1-B`'s row:

```markdown
| FLU-H1-C | `fluorita:` | active | — | — | Give the library Siderita's glass canvas: transparent window over the existing `CelestinaBackdrop` (STYLE-G7-Q recipe), with an opaque `canvas` backing only while a picture is on screen so a video's letterbox stays black. Lowers the glass-canvas ratchet row to 0. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-glass-canvas.md) | None |
```

Evidence as in Task 2; Procedure adds "the author browsed the library, then played a video and opened a still, focused".

- [ ] **Step 7: Commit**

```bash
git add fluorita/qml scripts/glass-canvas-baseline.tsv fluorita/docs
git commit -m "fluorita-maintenance: Add the glass canvas to the library window

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 8: Author check, then land**

```bash
python3 scripts/land-unit.py unit/fluorita/FLU-H1-C --kind milestone --summary "Add the glass canvas to the library window"
```

---

### Task 5: Magnetita (`MAG-D1-F`)

**Files:**
- Modify: `magnetita/qml/Main.qml:20`
- Modify: `scripts/glass-canvas-baseline.tsv` (`magnetita` row → 0)
- Modify: `magnetita/docs/plans/active/2026-09-13-app-design.md`; Create: `magnetita/docs/evidence/2026-10-07-glass-canvas.md`
- Not modified, on purpose: `magnetita/qml/MirrorWindow.qml` (its window is sized to the phone's picture; it stays `canvas`)

**Interfaces:**
- Consumes: Task 1's guard and the `1 magnetita` row; the existing `CelestinaBackdrop { id: backdropLayer }` in `appSurface`.

- [ ] **Step 1: Open the worktree** — `scripts/worktree.sh open magnetita MAG-D1-F`; `cd /home/toni/CODIGO/CELESTINA.worktrees/magnetita-MAG-D1-F`.

- [ ] **Step 2: Lower the row (failing test)** — `1	magnetita` → `0	magnetita`.

Run: `python3 scripts/glass_canvas_contract.py --baseline scripts/glass-canvas-baseline.tsv magnetita=magnetita/qml`
Expected: exit 1, one `opaque` finding.

- [ ] **Step 3: Clear the window** — in `magnetita/qml/Main.qml` replace `    color: CelestinaTheme.canvas` with:

```qml
    // Transparent: the compositor blurs what lies behind the window and
    // CelestinaBackdrop paints the Haze canvas over it (DESIGN §5.2 L0).
    // The mirror is its own window and keeps the opaque canvas: it is sized
    // to the phone's picture, and nothing of the desktop belongs around it.
    color: CelestinaTheme.clear
```

Leave `MirrorWindow.qml` untouched.

- [ ] **Step 4: The guard passes** — Step 2's command. Expected: `Glass-canvas contract: OK`.

- [ ] **Step 5: Build, smoke, lint, guards**

```bash
(cd magnetita && cargo build --release)
sh magnetita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/magnetita
bash scripts/qmllint-cxxqt.sh magnetita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

Expected: all exit 0.

- [ ] **Step 6: Ledger row and evidence** — after `MAG-D1-E`'s row:

```markdown
| MAG-D1-F | `magnetita:` | active | — | — | Give the main window Siderita's glass canvas: transparent window over the existing `CelestinaBackdrop` (STYLE-G7-Q recipe); the mirror window keeps the opaque canvas around the phone's picture. Lowers the glass-canvas ratchet row to 0. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-glass-canvas.md) | None |
```

Evidence as in Task 2; Procedure adds "the author opened the devices, messages and settings pages, focused, and started the mirror".

- [ ] **Step 7: Commit**

```bash
git add magnetita/qml scripts/glass-canvas-baseline.tsv magnetita/docs
git commit -m "magnetita-maintenance: Add the glass canvas to the main window

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 8: Author check, then land** — Magnetita has a daemon: confirm with `git status` that no file outside this unit is staged (a stray `magnetitad/` change blocked a landing on 2026-10-06), then:

```bash
python3 scripts/land-unit.py unit/magnetita/MAG-D1-F --kind milestone --summary "Add the glass canvas to the main window"
```

---

### Task 6: Siderita's file picker (`SID-H1-L`)

**Files:**
- Modify: `siderita/qml/PickerWindow.qml:77` and the first child of the `Item {` at line ~488
- Modify: `siderita/docs/plans/active/2026-09-26-hardening.md`; Create: `siderita/docs/evidence/2026-10-07-picker-glass-canvas.md`

**Interfaces:**
- Consumes: nothing from Task 1 (the guard reads only `Main.qml`, already clean); the recipe.

- [ ] **Step 1: Open the worktree** — `scripts/worktree.sh open siderita SID-H1-L`; `cd /home/toni/CODIGO/CELESTINA.worktrees/siderita-SID-H1-L`.

- [ ] **Step 2: Write the failing check** — the picker is a secondary window the guard does not read, so assert it directly:

Run: `python3 -c "import sys; sys.path.insert(0,'scripts'); import glass_canvas_contract as g, pathlib; print([f.kind for f in g.scan_window(pathlib.Path('siderita/qml/PickerWindow.qml'))])"`
Expected: `['opaque', 'backdrop']`

- [ ] **Step 3: Clear the picker** — replace `    color: CelestinaTheme.canvas` (line 77) with the recipe's three lines, and make the backdrop the first child of the top-level visual `Item {` (the one holding `PickerSidebar`):

```qml
    Item {
        anchors.fill: parent

        // The picker is Siderita: the same Haze canvas under its sidebar,
        // listing and bars as the main window.
        CelestinaBackdrop {
            anchors.fill: parent
        }

        // Read-only navigation, not the full write-capable Sidebar — see
```

- [ ] **Step 4: The check passes** — Step 2's command. Expected: `[]`.

- [ ] **Step 5: Build, tests, lint, guards**

```bash
(cd siderita && cargo build --release)
sh siderita/scripts/qml-tests.sh
sh siderita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

Expected: QML tests `0 failed`; everything else exit 0.

- [ ] **Step 6: Ledger row and evidence** — after `SID-H1-K`'s row:

```markdown
| SID-H1-L | `siderita:` | active | — | — | Give the file picker the main window's glass canvas: transparent window with `CelestinaBackdrop` under its sidebar and listing (STYLE-G7-Q recipe); it was the last Siderita window on the opaque canvas. Author request of 2026-10-07; no audit finding. | [evidence](../../evidence/2026-10-07-picker-glass-canvas.md) | None |
```

Evidence as in Task 2; Procedure adds "the author opened the picker from another application (open and save), focused".

- [ ] **Step 7: Commit**

```bash
git add siderita/qml/PickerWindow.qml siderita/docs
git commit -m "siderita-maintenance: Fix the file picker so it paints the glass canvas

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 8: Author check, then land**

```bash
python3 scripts/land-unit.py unit/siderita/SID-H1-L --kind bug --summary "Fix the file picker so it paints the glass canvas"
```

---

## Already carried, verified only

These parts of the new style live in `celestina-style` and every application already renders them; each application task's author check covers them, and nothing is edited:

- Opaque `#0b0c10` boxes: `CelestinaSurface` Panel/Grouped/Content/Tonal paint `card` (STYLE-G7-Q).
- Haze pills: `CelestinaCapsule.fill` is `pillFill` (STYLE-G7-S); Hematita's strip, Grafita's find capsule and Fluorita's capsule use it.
- Hover ladder: `CelestinaButton`/`CelestinaIconButton` Ghost and Tonal (STYLE-G7-S, contrast-guarded).
- Modals: one-image frozen fade and the focus sink (STYLE-G7-T…W), through `CelestinaModalLayer` in every dialog.
- Context menus: modeless behind a pointer shield (STYLE-G7-R) wherever `GlassContextMenu` is linked.

## After the plan

Close each worktree with `scripts/worktree.sh close <project> <unit>` once landed; the last close deletes the shared `.cargo-target`. Update the memory note `celestina-desktop-glass-program` so the next session knows every window is glass and only spec §5's design units remain.
