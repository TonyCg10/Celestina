<!-- language-contract: product-copy — the Spanish below is product copy quoted as string literals -->
# Siderita bottom chrome and the notice stack — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the legacy status strip that repeats what the operation ring already says, give the actions that have no ring a transient notice of their own, and compress the bottom bar into one three-icon capsule so everything to its right belongs to a single column of transient surfaces.

**Architecture:** One column anchored to the bottom right of the content frame grows upward: the existing rings dock at its base, self-retiring notices above it, and the two error banners rejoin it as danger notices instead of spanning the window. `status_text` — a property that carried four unrelated kinds of message — is replaced by a per-controller notice queue published as index-aligned lists, where every running entry has a completion path that settles or drops it. The bottom bar's four pills collapse into one `GlassPill` holding three ghost icon buttons, and the sort menu grows the view modes so the middle icon can be the current view mode itself.

**Tech Stack:** Rust 1.98.1, cxx-qt 0.9 (`QStringList` index-aligned lists — the suite's list shape), Qt 6.9 QML, `celestina-style` tokens through `CelestinaTheme`, `qmltestrunner` (Qt6) for pointer semantics.

**Spec:** [docs/superpowers/specs/2026-09-22-siderita-bottom-chrome-design.md](../specs/2026-09-22-siderita-bottom-chrome-design.md)

## Global Constraints

- **The Celestina shell is in standby.** Never read, reuse, modify or reference `celestina/` or `celestina-rs/crates/celestina-shell-core`.
- **Language contract:** identifiers, comments, docs, tests, script messages and commit subjects in English; product copy only as Spanish string literals in QML (and in a Rust module whose head carries `//! language-contract: product-copy`, as `controller/jobs.rs` already does).
- **Build budget (author's rule):** **two** application build cycles in this checkpoint — one at the end of `SID-B1-A`, one inside `complete-production.sh` at `SID-B1-Z`. Compiling heats and slows the author's machine. Never `cargo` under `siderita/` outside those two moments; never `cargo clean`. Everything in Tasks 2–6 is verified with `scripts/qml-tests.sh`, which needs no build.
- **Never open a window on the live or nested session.** Appearance, glass, reduced motion and AT are `VAL-SID-15`. Headless stills use `QT_QPA_PLATFORM=offscreen` with `XDG_CONFIG_HOME` pointed at a scratch directory.
- **Commits:** authorised per unit after review; prefix `siderita:`; subject verb from `IMPERATIVE_VERBS` in `scripts/commit_scope.py` (`Add`, `Fix`, `Record`, `Update`, `Keep`, `Archive` — not `Book`, `Show`, `Retarget`). Inventories under `siderita/docs/inventories/2026-09-22-bottom-chrome-and-notices/`, evidence under `siderita/docs/evidence/`. Hooks are never bypassed.
- **Inventories are immutable once tracked.** `check-staged-units.py` requires `Base revision = HEAD` in the inventory being committed, so recompute an untracked inventory immediately before its commit. A correction after review is a **new unit**, never an edit of a closed one.
- **Parallel sessions:** another session is working in `hematita/` and `celestina-rs/crates/hematita-core/`. Preserve unrelated worktree changes; never `git stash`; stage only this unit's paths by name.
- **New Spanish in QML goes through `qsTr()`.** The scanner blanks only `qsTr()` arguments in a `.qml` file; a bare Spanish literal counts as debt and is tracked per file in `scripts/language-baseline.tsv` (`BottomControls.qml` 4, `FolderBottomStatus.qml` 2, `FolderSortMenu.qml` 6 today). A rewritten file must not raise its row — and because these three are rewritten with `qsTr()`, their rows **fall to zero and are deleted** in the same commit, as `commit_policy.shared_ratchet_files` requires.
- **QML invariants:** every new QML file registered in `build.rs` `QML_FILES` (and in the `rerun-if-changed` loop it already chains); `required property` for injected state; a component never reaches a parent id and never uses `x: x`; colors, radii, motion and opacity only from `CelestinaTheme`; **no tooltips** — `helpText` is an accessible name only; every action reachable by keyboard and AT; new motion honours `CelestinaTheme.reducedMotion`; the `siderita` row in `scripts/qmllint-baseline.tsv` may not rise; no lint suppressions.
- **Rust invariants:** no `unsafe`; no production `unwrap`/`expect`/`panic!`; blocking IO never on the Qt thread; workers publish only current state. Every notice pushed with `running = true` has exactly one completion path that settles or drops it — a running notice with no owner is the defect this checkpoint removes.
- **Interaction families (`celestina-style/DESIGN.md` §7):** the capsule's icons and the notice are `Control` — hover is the neutral `surfaceHover` lift, press is `pressedWash` plus the sink. Never grey-to-blue on release.
- **Motion contract:** one exit for every surface — `CelestinaTheme.motionExit`, `exitShrink`, `easeExit`, a single fade, **one clock per gesture**. Never re-anchor a live positioner with a ternary.

## File structure

| Path | Responsibility |
|---|---|
| `siderita/qml/components/folder/ActivityNotice.qml` | one pill: icon, one line, optional turning dot; the 500 ms appearance threshold, the two tenses, the dwell and the single exit |
| `siderita/qml/components/folder/ActivityStack.qml` | the column: danger notices, notices, the dock; its own placement inside the content frame; the error-derived entries |
| `siderita/qml/menus/ViewSortMenu.qml` | the merged menu (renamed from `FolderSortMenu.qml`): view modes, sort field, sort direction |
| `siderita/qml/components/chrome/HiddenToggleDefs.qml` | singleton: the one definition of the hidden-entries glyph and its Spanish name, now that two surfaces draw it |
| `siderita/qml/components/chrome/BottomControls.qml` | one `GlassPill` with three ghost icon buttons: hidden, view-and-sort, sizes |
| `siderita/qml/components/folder/FolderBottomStatus.qml` | placement only: the bottom bar item and the `ActivityStack`; loses the status pill, both banners and the size button |
| `siderita/qml/components/folder/OperationsDock.qml` | unchanged rings, plus the collapsed counted circle and the expanded labelled list |
| `siderita/qml/components/folder/FolderHeading.qml` | the watch-degraded alert icon beside the folder title |
| `siderita/src/controller/notices.rs` | the notice queue: push, settle, drop, dismiss, publish |
| `siderita/src/controller.rs` | the six notice properties replacing `status_text`; the queue's state on the Rust struct |
| `siderita/src/controller/{mounts,trash,scan,archive,fileops,navigation,shell}.rs` | call sites: every `set_status_text` becomes a notice with an owner, or is deleted |
| `siderita/tests/qml/tst_activity_notice.qml` | threshold, tenses, dwell, press-to-dismiss, the press not reaching the content |
| `siderita/tests/qml/tst_activity_stack.qml` | order, the error entries, the cap of three |
| `siderita/tests/qml/tst_bottom_capsule.qml` | the three icons stay reachable; the menu opens upward; the sizes popup is left-aligned |
| `siderita/tests/qml/tst_operations_dock.qml` | extended: collapse at four, expand, outside press collapses |

Ledger units:

| Unit | Commit prefix | Content | Build |
|---|---|---|---|
| SID-B1-A | `siderita:` | The whole change — documents opened, the capsule, the merged menu, the notice and the stack, the dock's overflow, the Rust queue, the deletions | one |
| SID-B1-Z | `siderita:` | Implementation exit, `1.6.0`, documents closed, plan archived | one |

## Inventory generator

Write this once to the scratchpad as `mkinv.py`; it is not repository content. It stages the unit's paths, computes numstat and SHA-256 from the index, writes the inventory with its own `self` row, and rewrites the ledger diffstat until the self row converges.

```python
#!/usr/bin/env python3
"""Usage: mkinv.py UNIT INVENTORY PLAN PATH... — write an exact change inventory."""
import hashlib, os, re, subprocess, sys

unit, inv, plan, *paths = sys.argv[1:]
paths = sorted(set(paths + [plan]))

def git(*args):
    return subprocess.run(["git", *args], capture_output=True, check=True).stdout.decode()

head = git("rev-parse", "HEAD").strip()
os.makedirs(os.path.dirname(inv), exist_ok=True)

def rows():
    subprocess.run(["git", "add", "--"] + paths, check=True)
    out, added, deleted = [], 0, 0
    for path in paths:
        line = git("diff", "--cached", "--numstat", "--no-renames", "HEAD", "--", path).strip()
        if not line:
            sys.exit(f"{path} has no staged change")
        a, d, _ = line.split("\t")
        mode = git("ls-files", "--stage", "--", path).split()[0]
        if mode == "120000":
            blob = os.readlink(path).encode()
        else:
            blob = subprocess.run(["git", "show", f":{path}"], capture_output=True, check=True).stdout
        out.append((a, d, hashlib.sha256(blob).hexdigest(), path))
        added += int(a); deleted += int(d)
    return out, added, deleted

def write(table, self_added):
    lines = [f"# {unit} exact change inventory", "", f"Base revision\t{head}"]
    lines += [f"Pathspec\t{p}" for p in sorted(paths + [inv])]
    lines += ["Calculation\ttracked paths use git diff --numstat --no-renames; new paths use /dev/null",
              "Hashes\tSHA-256 of final bytes; symlinks hash link-target bytes", "",
              "added\tdeleted\tcontent\tpath"]
    lines += [f"{a}\t{d}\t{h}\t{p}" for a, d, h, p in table]
    lines.append(f"{self_added}\t0\tself\t{inv}")
    with open(inv, "w") as f:
        f.write("\n".join(lines) + "\n")
    subprocess.run(["git", "add", "--", inv], check=True)
    return int(git("diff", "--cached", "--numstat", "--no-renames", "HEAD", "--", inv).split("\t")[0])

table, added, deleted = rows()
self_added = write(table, 0)
for _ in range(5):
    text = open(plan).read()
    stat = f"| {len(paths) + 1} files, +{added + self_added}/-{deleted} |"
    text = re.sub(rf"(\| {re.escape(unit)} \|[^\n]*?\| done \|[^\n]*?\|)[^|]*\|", lambda m: m.group(1) + " " + stat.strip("| ") + " |", text, count=1)
    open(plan, "w").write(text)
    table, added, deleted = rows()
    new_self = write(table, self_added)
    if new_self == self_added:
        break
    self_added = new_self
print(f"{unit}: {len(paths) + 1} files, +{added + self_added}/-{deleted}")
```

Verify with:

```bash
python3 scripts/check-staged-units.py siderita/docs/inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-A.numstat.tsv
```

---

### Task 1: Open SID-B1 in the documents and close SID-A4

`SID-A4`'s two units are both `done` and its plan is still `active`, so the roadmap's single-active-checkpoint rule blocks a second plan. Close it first, in the same commit.

**Files:**
- Create: `siderita/docs/plans/active/2026-09-22-bottom-chrome-and-notices.md`
- Move: `siderita/docs/plans/active/2026-08-20-what-the-window-costs-and-shows.md` → `siderita/docs/plans/archive/2026-08-20-what-the-window-costs-and-shows.md`
- Modify: `siderita/ROADMAP.md`, `siderita/STATUS.md`, `siderita/VALIDATION.md`, `siderita/docs/plans/active/README.md`, `siderita/docs/plans/archive/README.md`

No build; lands inside the `SID-B1-A` commit.

**Interfaces:**
- Produces: the checkpoint id `SID-B1`, the unit ids `SID-B1-A` and `SID-B1-Z`, the plan slug `2026-09-22-bottom-chrome-and-notices`, the validation id `VAL-SID-15`.

- [ ] **Step 1: Archive the SID-A4 plan**

```bash
cd /home/toni/CODIGO/CELESTINA
git mv siderita/docs/plans/active/2026-08-20-what-the-window-costs-and-shows.md \
       siderita/docs/plans/archive/2026-08-20-what-the-window-costs-and-shows.md
```

Change only its `Status:` line from `active` to `done`. The basename, `Plan ID`, checkpoint, units and links stay exactly as they are — archiving never rewrites history. Add its link to `siderita/docs/plans/archive/README.md` in the same shape the rows already there use, and remove it from `siderita/docs/plans/active/README.md`.

- [ ] **Step 2: Write the new plan ledger**

`siderita/docs/plans/active/2026-09-22-bottom-chrome-and-notices.md`:

```markdown
# Bottom chrome and the notice stack

- **Opened:** 2026-09-22
- **Plan ID:** bottom-chrome-and-notices
- **Status:** active
- **Authorization:** the author asked for the redesign on 2026-09-22 and
  approved the design in brainstorming
- **Scope:** siderita
- **Implementation checkpoint:** SID-B1
- **Author-validation checkpoint:** `VAL-SID-15` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The bottom strip repeats what the ring above it already says because
`status_text` carries four unrelated kinds of message, and only one of them is
a running job. Split them by what they are — a ring for a job, a self-retiring
notice for everything else, the heading for what is simply true of the folder —
and the strip has nothing left to show. The bar it occupied then fits in one
capsule of three icons.

## Tangible outcome

A copy shows one ring and nothing else; unmounting a disk says so and stops
saying so when it is done; an error is a pill in the corner instead of a band
across the rows; and the bottom bar is a single capsule on the left with the
whole width to its right free.

## Scope

- One transient column anchored bottom right: rings, notices, danger notices.
- `ActivityNotice` with the 500 ms appearance threshold and the two tenses.
- The three-icon capsule and the merged view-and-sort menu.
- The dock's collapsed counted circle and expanded labelled list.
- `status_text` replaced by the notice queue; every running notice owned.
- The watch-degraded warning moves to the folder heading.

## Exclusions

- The portal picker (`PickerWindow.qml`, `qml/components/picker/`) keeps the
  chrome it has; it runs no write operations. `HiddenTogglePill` survives for
  it. Aligning the two surfaces is a later decision.
- Per-notice history or a notification centre: a notice that has retired is
  gone.
- Any change to what a ring shows or to `OperationCallout`.

## Build order

1. `SID-B1-A`, then `SID-B1-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds; the installed binary shows one
surface per running job, announces an unmount and stops announcing it, and its
bottom bar is one capsule.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-B1-A | `siderita:` | planned | `siderita/qml/`, `siderita/src/`, `siderita/tests/qml/`, `siderita/build.rs`, documents | — | The transient column, the notice and its threshold, the three-icon capsule, the merged menu, the dock's overflow, the notice queue replacing `status_text`, the watch warning moved to the heading | `scripts/verify-production.sh`, `scripts/qml-tests.sh` | `VAL-SID-15` |
| SID-B1-Z | `siderita:` | planned | `siderita/`, `docs/version-history.tsv` | — | Implementation exit, `1.6.0`, documents closed, plan archived | `scripts/complete-production.sh` | `VAL-SID-15` |

Like every plan in this repository, this one records intent and grants no
authority.
```

- [ ] **Step 3: Roadmap**

In `siderita/ROADMAP.md`, set `Active implementation checkpoint: SID-B1`, and append after the `SID-M1` section:

```markdown
## SID-B1 — The bottom bar stops repeating the ring

The author showed a copy and an extraction: each is announced twice, once by
its ring and once by a strip across the bottom bar that looks like a search
field. Reading the code found why the strip could not simply be deleted —
`status_text` carries a running job, an activity with no knowable end, an
outcome that has already happened, and two standing facts about the folder,
all through one property. It also found that `"Desmontando…"` is never
cleared and that `volume_busy` is published and read by nothing, so unmounting
a disk has no indicator of its own at all.

This checkpoint separates them by what they are, compresses the bar into one
capsule of three icons, and moves the magnifier to the left so the whole width
to its right belongs to one transient column.

The plan is
[Bottom chrome and the notice stack](docs/plans/active/2026-09-22-bottom-chrome-and-notices.md).
It excludes the portal picker's own chrome, and the author's own pass, which is
`VAL-SID-15`.
```

Mark the `SID-A4` section `done` wherever the file records per-checkpoint state, leaving its text otherwise untouched.

- [ ] **Step 4: STATUS and VALIDATION**

`siderita/STATUS.md`: update `Updated:` to `2026-09-22`, and change the implementation line to name `SID-B1` as the active checkpoint with `SID-A4` closed.

`siderita/VALIDATION.md`: add at the top, in the shape the rows already there use:

```markdown
## VAL-SID-15 — The bottom bar, and what a running action says

- **Status:** pending
- **Related implementation:** `SID-B1`
- **Requires:** the deployed Siderita, a removable disk, and a folder with
  enough in it for a copy to last several seconds
- **Steps:** copy a large folder and confirm the ring is the only thing that
  says so; unmount the disk from the sidebar and confirm the notice appears,
  changes to its finished wording and retires on its own; unmount a disk that
  answers instantly and confirm nothing flashes; trigger a permission error and
  confirm it is a pill in the corner that can be read and dismissed; open the
  capsule's middle icon and change view mode and sort field from it; confirm
  the sizes popup opens from the left without leaving the window
- **Judgement:** whether the 500 ms threshold feels right, whether two lines is
  enough for a real error on a long path, and whether losing the permanent sort
  arrow is acceptable in daily use
```

- [ ] **Step 5: Check the documentation contract**

```bash
cd /home/toni/CODIGO/CELESTINA && ./scripts/check-documentation-contract.sh
```

Expected: PASS. A failure names the rule; fix the document it names rather than the guard.

---

### Task 2: `HiddenToggleDefs` — one definition of the hidden-entries glyph

Two surfaces now draw the same toggle: the picker's `HiddenTogglePill` and the capsule's ghost icon. Two consumers prove the same semantics, so the definition is extracted once rather than copied.

**Files:**
- Create: `siderita/qml/components/chrome/HiddenToggleDefs.qml`
- Modify: `siderita/qml/components/chrome/HiddenTogglePill.qml`
- Modify: `siderita/build.rs`

**Interfaces:**
- Produces: singleton `HiddenToggleDefs` with `function glyph(checked) -> string` and `function name(checked) -> string`.

- [ ] **Step 1: Write the singleton**

```qml
pragma Singleton

import QtQuick

// The one definition of the hidden-entries toggle: which glyph it wears and
// what a screen reader calls it. Two surfaces draw it — the portal picker's
// floating pill and the folder capsule's ghost icon — and a toggle that says
// one thing in one window and another in the next is a defect, not a variant.
QtObject {
    function glyph(checked) {
        return checked ? "eye" : "eye-off"
    }

    function name(checked) {
        return checked ? qsTr("Ocultar elementos ocultos")
                       : qsTr("Mostrar elementos ocultos")
    }
}
```

- [ ] **Step 2: Register it as a singleton**

In `siderita/build.rs`, beside the existing singleton registrations:

```rust
        .qml_file(
            QmlFile::from("qml/components/chrome/HiddenToggleDefs.qml")
                .version(1, 0)
                .singleton(true),
        )
```

and add `"qml/components/chrome/HiddenToggleDefs.qml"` to the extra paths chained into the `rerun-if-changed` loop, beside `"qml/CelestinaPlaceDefs.qml"`. A singleton is **not** listed in `QML_FILES` — the four existing singletons are registered only through `.qml_file(...)`, and listing it twice registers the type twice.

- [ ] **Step 3: Point the picker's pill at it**

In `siderita/qml/components/chrome/HiddenTogglePill.qml`, replace the three literal expressions:

```qml
    iconName: HiddenToggleDefs.glyph(control.toggleChecked)
    helpText: HiddenToggleDefs.name(control.toggleChecked)
    active: toggleChecked
    font.pixelSize: Math.round(CelestinaTheme.fontMini * textScale)
    Accessible.name: HiddenToggleDefs.name(control.toggleChecked)
```

The comment above them said the tooltip spells it out; there are no tooltips in this suite any more. Replace it with: `// Icon-only: the eye says it, and HiddenToggleDefs says what a screen reader hears.`

- [ ] **Step 4: Run the QML tests**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh
```

Expected: every existing test still passes. The harness generates a plugin-free `qmldir` from the source tree (so what runs is the source, not a copy), detects `pragma Singleton` by reading the first five lines of each file, and sets `QT_ASSUME_STDERR_HAS_CONSOLE=1` itself — without that variable Qt routes the runner's result to journald and a failing run looks silent.

---

### Task 3: `ViewSortMenu` — the merged menu

`FolderSortMenu.qml` is renamed and grows the view modes, so the capsule's middle icon can open one menu for both. `BottomControls.qml` is its only consumer (verified: `rg sortMenu siderita/qml`), and the folder context menu carries no sort or view entries, so there is no second caller to keep in step.

**Files:**
- Move: `siderita/qml/menus/FolderSortMenu.qml` → `siderita/qml/menus/ViewSortMenu.qml`
- Modify: `siderita/qml/components/folder/FolderActions.qml:56`, `siderita/qml/views/FolderView.qml:657`, `siderita/build.rs`

**Interfaces:**
- Consumes: `controller.sortField` (`int`), `controller.sortAscending` (`bool`), `controller.changeSortField(int)`, `controller.toggleSortDirection()`; `panel.viewMode` (`string`: `"grid"`, `"list"`, `"details"`), `panel.persist()`.
- Produces: type `ViewSortMenu` with `property var controller`, `property var panel`, inherited `required property Item backdropSource`, and `popup(parent, point)` from `Menu`.

- [ ] **Step 1: Rename the file and its registration**

```bash
cd /home/toni/CODIGO/CELESTINA
git mv siderita/qml/menus/FolderSortMenu.qml siderita/qml/menus/ViewSortMenu.qml
```

In `siderita/build.rs`, change the `QML_FILES` row `"qml/menus/FolderSortMenu.qml"` to `"qml/menus/ViewSortMenu.qml"`.

- [ ] **Step 2: Rewrite the menu**

```qml
import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── ViewSortMenu ───────────────────────────────────────────────────────────
// What the bottom bar's middle icon opens: how the folder is shown, and in what
// order. They are one question — how is this arranged — which is why they are
// one menu behind one glyph rather than two pills. The panel owns the view
// mode, the tab's controller owns the sorting; both arrive as properties and
// this component reaches no id outside itself.
// ──────────────────────────────────────────────────────────────────────────────
GlassContextMenu {
    id: root

    // The tab's controller, injected by the folder view.
    property var controller
    // The panel: owner of the view mode and of persisting it.
    property var panel

    function chooseView(mode) {
        root.panel.viewMode = mode
        root.panel.persist()
    }

    GlassMenuItem {
        text: qsTr("Cuadrícula")
        icon.name: "view-grid"
        icon.source: CelestinaTheme.fallbackIcon("view-grid")
        choice: true
        current: root.panel.viewMode === "grid"
        onTriggered: root.chooseView("grid")
    }

    GlassMenuItem {
        text: qsTr("Lista")
        icon.name: "view-list"
        icon.source: CelestinaTheme.fallbackIcon("view-list")
        choice: true
        current: root.panel.viewMode === "list"
        onTriggered: root.chooseView("list")
    }

    GlassMenuItem {
        text: qsTr("Detalles")
        icon.name: "view-details"
        icon.source: CelestinaTheme.fallbackIcon("view-details")
        choice: true
        current: root.panel.viewMode === "details"
        onTriggered: root.chooseView("details")
    }

    MenuSeparator {
        contentItem: Rectangle {
            implicitHeight: 1
            color: CelestinaTheme.divider
        }
    }

    GlassMenuItem {
        text: qsTr("Nombre")
        icon.name: "type"
        icon.source: CelestinaTheme.fallbackIcon("type")
        choice: true
        current: root.controller.sortField === 0
        onTriggered: root.controller.changeSortField(0)
    }

    GlassMenuItem {
        text: qsTr("Tamaño")
        icon.name: "hard-drive"
        icon.source: CelestinaTheme.fallbackIcon("hard-drive")
        choice: true
        current: root.controller.sortField === 1
        onTriggered: root.controller.changeSortField(1)
    }

    GlassMenuItem {
        text: qsTr("Fecha de modificación")
        icon.name: "clock-arrow-up"
        icon.source: CelestinaTheme.fallbackIcon("clock-arrow-up")
        choice: true
        current: root.controller.sortField === 2
        onTriggered: root.controller.changeSortField(2)
    }

    GlassMenuItem {
        text: qsTr("Tipo")
        icon.name: "files"
        icon.source: CelestinaTheme.fallbackIcon("files")
        choice: true
        current: root.controller.sortField === 3
        onTriggered: root.controller.changeSortField(3)
    }

    MenuSeparator {
        contentItem: Rectangle {
            implicitHeight: 1
            color: CelestinaTheme.divider
        }
    }

    // The direction loses its permanent arrow on the bar: it is read here, and
    // in details mode the column header still says it.
    GlassMenuItem {
        text: root.controller.sortAscending ? qsTr("Ascendente")
                                           : qsTr("Descendente")
        icon.name: root.controller.sortAscending
                   ? "view-sort-ascending" : "view-sort-descending"
        icon.source: CelestinaTheme.fallbackIcon(
                         root.controller.sortAscending
                         ? "view-sort-ascending" : "view-sort-descending")
        onTriggered: root.controller.toggleSortDirection()
    }
}
```

- [ ] **Step 3: Point the two consumers at the new type**

`siderita/qml/components/folder/FolderActions.qml:11,56` — rename the alias and the instance:

```qml
    property alias viewSortMenu: viewSortMenu
```

```qml
    ViewSortMenu {
        id: viewSortMenu
        controller: root.controller
        panel: root.panel
        backdropSource: root.backdropSource
    }
```

Keep whatever `backdropSource` and `controller` expressions that file already passes; only the type name, the id, the alias and the new `panel` line change. If `FolderActions` has no `panel` property, add `required property var panel` to it and pass `mainPanel` from `FolderView.qml` where it already instantiates `FolderActions`.

`siderita/qml/views/FolderView.qml:657` — `sortMenuItem: folderActions.sortMenu` becomes `viewSortMenuItem: folderActions.viewSortMenu`.

- [ ] **Step 4: Run the QML tests**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh
```

Expected: every existing test still passes (the harness sets `QT_ASSUME_STDERR_HAS_CONSOLE=1` itself and generates the `qmldir` from the source tree, detecting `pragma Singleton` automatically).

---

### Task 4: `BottomControls` — one capsule, three icons

**Files:**
- Modify: `siderita/qml/components/chrome/BottomControls.qml` (replaced wholesale)
- Modify: `siderita/qml/components/folder/FolderBottomChrome.qml`
- Test: `siderita/tests/qml/tst_bottom_capsule.qml`

**Interfaces:**
- Consumes: `HiddenToggleDefs.glyph(bool)`, `HiddenToggleDefs.name(bool)` (Task 2); type `ViewSortMenu` (Task 3).
- Produces: `BottomControls` is now a `GlassPill` with `required property var controller`, `required property var panel`, `required property Item bottomView`, `required property bool bottomFloating`, `required property Item overlayParent`, `required property var viewSortMenu`, `required property var hostWindow`. Its `implicitWidth` and `implicitHeight` are the capsule's, as before.

- [ ] **Step 1: Write the failing test**

`siderita/tests/qml/tst_bottom_capsule.qml`:

```qml
import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The bottom capsule: what is shown, how it is arranged, how big it is — three
// ghost icons on one glass surface. The eye is a toggle whose state must keep
// following the controller after it has been clicked; the middle icon wears
// the current view mode and opens the menu; the magnifier now sits on the left,
// so its popup opens to the right of its own edge instead of off the window.
TestCase {
    id: testCase
    name: "BottomCapsule"
    width: 600
    height: 400
    visible: true
    when: windowShown

    property int hiddenToggles: 0

    QtObject {
        id: controllerStub
        property bool showHidden: false
        property int sortField: 0
        property bool sortAscending: true
        function toggleHidden() {
            testCase.hiddenToggles++
            controllerStub.showHidden = !controllerStub.showHidden
        }
        function changeSortField(field) { controllerStub.sortField = field }
        function toggleSortDirection() {
            controllerStub.sortAscending = !controllerStub.sortAscending
        }
    }

    QtObject {
        id: panelStub
        property string viewMode: "grid"
        function persist() { }
    }

    Item {
        id: overlayStub
        anchors.fill: parent
    }

    ViewSortMenu {
        id: menuStub
        controller: controllerStub
        panel: panelStub
        backdropSource: overlayStub
    }

    BottomControls {
        id: capsule
        x: 10
        y: 340
        controller: controllerStub
        panel: panelStub
        bottomView: overlayStub
        bottomFloating: true
        overlayParent: overlayStub
        viewSortMenu: menuStub
        hostWindow: testCase
    }

    // The window's own sizing properties, which the capsule's popup reads.
    property real contentIconScale: 1
    property real contentTextScale: 1
    property real interfaceIconScale: 1
    property real interfaceTextScale: 1
    property real sidebarIconScale: 1
    property real sidebarTextScale: 1
    function persistSizing() { }

    function init() {
        testCase.hiddenToggles = 0
        controllerStub.showHidden = false
        panelStub.viewMode = "grid"
        menuStub.close()
        mouseMove(testCase, 590, 10)
    }

    function test_capsule_is_one_surface_of_three_icons() {
        compare(capsule.icons.children.length, 3,
                "the capsule holds exactly three icons")
        verify(capsule.implicitWidth < 130,
               "three icons and their padding stay under 130px, not the 330 the "
               + "four pills took")
    }

    function test_eye_keeps_following_the_controller_after_a_click() {
        const eye = capsule.hiddenButton
        mouseClick(eye)
        compare(testCase.hiddenToggles, 1)
        compare(eye.checked, true, "the eye is on after the first click")
        mouseClick(eye)
        compare(testCase.hiddenToggles, 2)
        compare(eye.checked, false,
                "the binding to showHidden survived the button's own toggle")
    }

    function test_middle_icon_wears_the_current_view_mode() {
        compare(capsule.viewSortButton.iconName, "view-grid")
        panelStub.viewMode = "list"
        compare(capsule.viewSortButton.iconName, "view-list")
        panelStub.viewMode = "details"
        compare(capsule.viewSortButton.iconName, "view-details")
    }

    function test_sizes_popup_opens_inside_the_window_from_the_left() {
        const popup = capsule.sizePopup
        mouseClick(capsule.sizeButton)
        verify(popup.opened, "pressing the magnifier opens the sizes popup")
        const origin = capsule.sizeButton.mapToItem(testCase, 0, 0)
        verify(origin.x + popup.width <= testCase.width,
               "the popup grows rightwards from a button on the left and stays "
               + "inside the window")
    }
}
```

- [ ] **Step 2: Run it to watch it fail**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh -functions 2>&1 | grep -i bottomcapsule
```

Expected: the file fails to construct — `BottomControls` is still a `RowLayout` with no `icons`, `hiddenButton`, `viewSortButton`, `sizeButton` or `sizePopup` aliases and no `panel`/`viewSortMenu` properties.

- [ ] **Step 3: Write the capsule**

Replace `siderita/qml/components/chrome/BottomControls.qml` entirely:

```qml
import QtQuick
import org.celestina.siderita 1.0

// ─── BottomControls ─────────────────────────────────────────────────────────
// The whole bottom bar: one glass capsule of three icons — what is shown, how
// it is arranged, how big it is. It was four separate pills and some 330px; the
// rest of the width now belongs to the column of transient surfaces on the
// right.
//
// The middle icon *is* the current view mode, so that state is read without
// opening anything; the menu it opens is where sorting lives. Everything from
// outside arrives as a property: this chrome reaches no id of the view that
// instantiates it.
// ──────────────────────────────────────────────────────────────────────────────
GlassPill {
    id: root

    required property var controller
    required property var panel
    required property Item bottomView      // the view the glass samples
    required property bool bottomFloating
    required property Item overlayParent   // where the menu anchors
    required property var viewSortMenu     // the view-and-sort menu
    required property var hostWindow       // the six scales the popup edits

    // The test and the placement above read these rather than reaching inside.
    readonly property alias icons: iconRow
    readonly property alias hiddenButton: hiddenIcon
    readonly property alias viewSortButton: viewSortIcon
    readonly property alias sizeButton: sizeIcon
    readonly property alias sizePopup: sizes

    // The glyph's own box inside the capsule, matching the sort group this
    // replaces: the control keeps the suite's hover circle, four pixels in
    // from the capsule's edge.
    readonly property int iconSide: CelestinaTheme.controlHeightSm - 4

    implicitWidth: iconRow.implicitWidth + 2 * CelestinaTheme.spaceXs
    implicitHeight: CelestinaTheme.controlHeightSm
    backdrop: root.bottomView
    floating: root.bottomFloating
    fill: CelestinaTheme.controlFill

    Row {
        id: iconRow
        anchors.centerIn: parent
        spacing: 2

        CelestinaIconButton {
            id: hiddenIcon
            width: root.iconSide
            height: width
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: HiddenToggleDefs.glyph(root.controller.showHidden)
            helpText: HiddenToggleDefs.name(root.controller.showHidden)
            checkable: true
            // The control's own toggle would otherwise assign `checked` and
            // break this binding on the first press, leaving the eye stuck.
            Binding on checked {
                value: root.controller.showHidden
                restoreMode: Binding.RestoreBindingOrValue
            }
            onClicked: root.controller.toggleHidden()
        }

        CelestinaIconButton {
            id: viewSortIcon
            width: root.iconSide
            height: width
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: root.panel.viewMode === "list"
                      ? "view-list"
                      : root.panel.viewMode === "details"
                        ? "view-details" : "view-grid"
            helpText: qsTr("Vista y orden")
            onClicked: {
                // This control sits at the bottom, so its menu opens upward.
                const menuHeight = root.viewSortMenu.height > 0
                                 ? root.viewSortMenu.height : 300
                const point = viewSortIcon.mapToItem(
                                root.overlayParent, 0, -menuHeight - 6)
                root.viewSortMenu.popup(root.overlayParent, point)
            }
        }

        CelestinaIconButton {
            id: sizeIcon
            width: root.iconSide
            height: width
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: "zoom-in"
            helpText: qsTr("Ajustar tamaños")
            // The popup closes itself on the press that lands outside it — this
            // button included — so by `clicked` it is already closed and a naive
            // toggle reopens it. Decide at press time instead.
            property bool willOpen: false
            onPressedChanged: if (pressed) willOpen = !sizes.opened
            onClicked: willOpen ? sizes.open() : sizes.close()

            SizePopup {
                id: sizes
                y: -height - 10
                // The magnifier moved to the left of the bar, so the popup
                // grows rightwards from the button's own edge. Right-aligning
                // it here, as it was, would push it off the window.
                x: 0
                backdrop: root.panel
                hostWindow: root.hostWindow
            }
        }
    }
}
```

- [ ] **Step 4: Update the placement**

`siderita/qml/components/folder/FolderBottomChrome.qml`: rename `required property var sortMenuItem` to `required property var viewSortMenuItem`, and in the `BottomControls` instance replace `sortMenu: root.sortMenuItem` with `viewSortMenu: root.viewSortMenuItem`, add `panel: root.panel` (already present) and `hostWindow: root.hostWindow`, and drop `textScale`, which the capsule no longer takes. `FolderView.qml:657` already passes `viewSortMenuItem` after Task 3.

- [ ] **Step 5: Run the tests**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh
```

Expected: PASS, including the four new `BottomCapsule` functions. If `test_eye_keeps_following_the_controller_after_a_click` still fails, the `Binding on checked` form is wrong for this Qt version — replace it with a standalone `Binding { target: hiddenIcon; property: "checked"; value: root.controller.showHidden; restoreMode: Binding.RestoreBindingOrValue }` sibling and run again.

---

### Task 5: `ActivityNotice` — one pill, two tenses, one exit

**Files:**
- Create: `siderita/qml/components/folder/ActivityNotice.qml`
- Modify: `siderita/build.rs` (`QML_FILES`)
- Test: `siderita/tests/qml/tst_activity_notice.qml`

**Interfaces:**
- Produces: type `ActivityNotice` with `required property string noticeId`, `required property string text`, `required property string iconName`, `required property bool danger`, `required property bool running`, `required property real maxWidth`, `property Item backdrop`, `property bool floating`, `signal dismissed(string id)`, `function retire()`, and the read-only `shown` used by the tests.

- [ ] **Step 1: Write the failing test**

`siderita/tests/qml/tst_activity_notice.qml`:

```qml
import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// One transient announcement. The rules that matter are about time: nothing
// appears before 500 ms, an action that settles inside that window is never
// drawn at all, a notice that was drawn mutates into its settled wording and
// then retires on its own, and a press retires it at once without reaching the
// file it covers.
TestCase {
    id: testCase
    name: "ActivityNotice"
    width: 600
    height: 400
    visible: true
    when: windowShown

    property var dismissals: []
    property int contentPresses: 0

    Item {
        id: backdropStub
        anchors.fill: parent
    }

    // What lies under the notice in the folder: a row delegate's MouseArea.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        hoverEnabled: true
        onPressed: testCase.contentPresses++
    }

    Component {
        id: noticeFactory
        ActivityNotice {
            backdrop: backdropStub
            maxWidth: 400
            onDismissed: id => testCase.dismissals.push(id)
        }
    }

    function init() {
        testCase.dismissals = []
        testCase.contentPresses = 0
        mouseMove(testCase, 590, 10)
    }

    function test_a_running_notice_is_not_drawn_before_the_threshold() {
        const notice = noticeFactory.createObject(testCase, {
            noticeId: "1", text: "Desmontando…", iconName: "unplug",
            danger: false, running: true, y: 300
        })
        compare(notice.shown, false, "nothing is drawn immediately")
        wait(300)
        compare(notice.shown, false, "still nothing at 300 ms")
        tryVerify(function() { return notice.shown }, 1000,
                  "it appears once the action has lasted past the threshold")
        notice.destroy()
    }

    function test_an_action_that_settles_inside_the_threshold_is_never_drawn() {
        const notice = noticeFactory.createObject(testCase, {
            noticeId: "2", text: "Desmontando…", iconName: "unplug",
            danger: false, running: true, y: 300
        })
        wait(200)
        notice.running = false
        compare(notice.shown, false, "a 200 ms unmount never flashes")
        compare(testCase.dismissals, ["2"],
                "and it asks to be dropped rather than lingering")
        notice.destroy()
    }

    function test_an_outcome_is_drawn_at_once_and_retires_on_its_own() {
        const notice = noticeFactory.createObject(testCase, {
            noticeId: "3", text: "Pegado cancelado", iconName: "x",
            danger: false, running: false, y: 300
        })
        compare(notice.shown, true,
                "an announcement about something already finished does not wait")
        tryVerify(function() { return testCase.dismissals.length === 1 }, 6000,
                  "and it retires without being touched")
        compare(testCase.dismissals, ["3"])
        notice.destroy()
    }

    function test_a_press_retires_it_and_never_reaches_the_content() {
        const notice = noticeFactory.createObject(testCase, {
            noticeId: "4", text: "Disco desmontado", iconName: "unplug",
            danger: false, running: false, y: 300
        })
        compare(notice.shown, true)
        mouseClick(notice, notice.width / 2, notice.height / 2)
        compare(testCase.contentPresses, 0,
                "the press stops at the notice instead of selecting a file")
        tryVerify(function() { return testCase.dismissals.length === 1 }, 1000)
        notice.destroy()
    }

    function test_a_danger_notice_wraps_but_an_info_notice_does_not() {
        const long = "No se pudo leer la carpeta: permiso denegado en "
                   + "/run/media/toni/disco/una/ruta/larga/de/verdad"
        const info = noticeFactory.createObject(testCase, {
            noticeId: "5", text: long, iconName: "info",
            danger: false, running: false, y: 200
        })
        const bad = noticeFactory.createObject(testCase, {
            noticeId: "6", text: long, iconName: "circle-alert",
            danger: true, running: false, y: 260
        })
        verify(info.implicitHeight < bad.implicitHeight,
               "an error may take two lines; an announcement takes one")
        verify(bad.implicitWidth <= bad.maxWidth,
               "and neither is ever wider than the space it was given")
        info.destroy()
        bad.destroy()
    }
}
```

- [ ] **Step 2: Run it to watch it fail**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh 2>&1 | grep -i activitynotice
```

Expected: the file fails to construct — `ActivityNotice` is not a type in the module.

- [ ] **Step 3: Write the component**

`siderita/qml/components/folder/ActivityNotice.qml`:

```qml
pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── One transient announcement ────────────────────────────────────
    // A pill as wide as its line, never as wide as the window. It has two
    // tenses and is the same object in both: it is born with a turning dot
    // while the action runs, and mutates into a word when the action settles.
    //
    // A ring means "this is running and you can stop it". This means "this
    // happened", which is why a 300 ms unmount gets one of these and not a ring
    // that would appear and vanish inside one blink. The 500 ms threshold below
    // is what makes that true: an action that settles inside it is never drawn.
Item {
    id: notice

    required property string noticeId
    required property string text
    required property string iconName
    required property bool danger
    required property bool running
    // The widest this may be: the content frame minus the capsule and its gaps.
    required property real maxWidth
    property Item backdrop
    property bool floating: true

    // Nothing appears before this. It is the whole reason a short action is
    // safe to announce, and it is measured from the moment the notice arrives.
    readonly property int appearAfter: 500
    // How long a settled notice stays. An error stays long enough to be read on
    // a long path; a press always ends either sooner.
    readonly property int dwell: notice.danger ? 10000 : 4000
    // An error may wrap; an announcement is one line. An elided error is an
    // error a person cannot read, which is the one thing the full-width banners
    // this replaces did right.
    readonly property int lines: notice.danger ? 2 : 1

    // Drawn only once the threshold has been earned. Read by the stack and the
    // tests; never assigned from outside.
    readonly property bool shown: notice.drawn
    property bool drawn: false
    property bool retiring: false

    signal dismissed(string id)

    // Retires now: plays the one exit if it was ever drawn, and otherwise just
    // asks to be dropped, because there is nothing on screen to take away.
    function retire() {
        if (notice.retiring)
            return
        if (!notice.drawn) {
            notice.dismissed(notice.noticeId)
            return
        }
        notice.retiring = true
        exit.restart()
    }

    implicitWidth: Math.min(notice.maxWidth,
                            CelestinaTheme.spaceMd + glyph.width
                            + CelestinaTheme.spaceSm + label.implicitWidth
                            + CelestinaTheme.spaceMd)
    implicitHeight: Math.max(CelestinaTheme.controlHeightSm,
                             label.implicitHeight + 2 * CelestinaTheme.spaceSm)
    width: implicitWidth
    height: implicitHeight
    visible: notice.drawn
    opacity: 0

    // An alert is announced without being focused, which is what a surface that
    // retires by itself needs: a screen reader hears it, and nothing steals the
    // keyboard from the folder. Escape dismisses the danger ones, in the stack.
    Accessible.role: Accessible.AlertMessage
    Accessible.name: notice.text

    // ── The two clocks ────────────────────────────────────────────────
    // Neither of them animates anything: the one animated gesture is the exit
    // below, which has a single clock of its own.
    Timer {
        id: appearClock
        interval: notice.appearAfter
        running: notice.running && !notice.drawn && !notice.retiring
        onTriggered: notice.show()
    }

    Timer {
        id: dwellClock
        interval: notice.dwell
        running: notice.drawn && !notice.running && !notice.retiring
        onTriggered: notice.retire()
    }

    function show() {
        notice.drawn = true
        entry.restart()
    }

    // A notice that arrives already settled is an announcement about something
    // that has happened. Delaying it would only make the application feel slow.
    Component.onCompleted: if (!notice.running) notice.show()

    onRunningChanged: {
        if (notice.running)
            return
        if (!notice.drawn) {
            // It settled inside the threshold: never drawn, and gone. The
            // dwell clock must not get a turn, or a 200 ms unmount would
            // appear after the fact.
            notice.dismissed(notice.noticeId)
        }
        // Otherwise it was drawn while it ran, its wording has already changed
        // with the model, and the dwell clock takes over by itself.
    }

    // One fade in, one fade-and-shrink out: the suite's single exit, one clock
    // per gesture, no second animation racing it.
    NumberAnimation {
        id: entry
        target: notice
        property: "opacity"
        to: 1
        duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionExit
        easing.type: CelestinaTheme.easeStandard
    }

    ParallelAnimation {
        id: exit
        NumberAnimation {
            target: notice
            property: "opacity"
            to: 0
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionExit
            easing.type: CelestinaTheme.easeExit
        }
        NumberAnimation {
            target: notice
            property: "scale"
            to: CelestinaTheme.exitShrink
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionExit
            easing.type: CelestinaTheme.easeExit
        }
        onFinished: notice.dismissed(notice.noticeId)
    }

    GlassPill {
        anchors.fill: parent
        backdrop: notice.backdrop
        floating: notice.floating
        fill: notice.danger ? CelestinaTheme.dangerFill
                            : CelestinaTheme.controlFill
        border.width: notice.danger ? CelestinaTheme.borderHairline : 0
        border.color: CelestinaTheme.dangerBorder
        // The pill's own floor already stops hover and drag reaching the file
        // underneath; the area below turns the swallowed press into a dismissal
        // instead of a dead zone.
        inputShield: false
    }

    // A press retires it. That is what makes swallowing the click honest: the
    // notice covers a row, so the press must not select or open that row, and a
    // press that did nothing at all would be a dead patch of window.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        hoverEnabled: true
        preventStealing: true
        onPressed: notice.retire()
    }

    Row {
        id: content
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceSm

        // While the action runs the glyph turns; when it settles it stops and
        // the wording has already changed. One object, two tenses.
        Item {
            id: glyph
            width: CelestinaTheme.iconSm
            height: width
            anchors.verticalCenter: parent.verticalCenter

            CelestinaIcon {
                anchors.fill: parent
                name: notice.iconName
                fallbackName: notice.iconName
                tone: notice.danger ? CelestinaIcon.Danger : CelestinaIcon.Primary
            }

            Rectangle {
                id: spinner
                visible: notice.running
                width: 6
                height: 6
                radius: 3
                color: CelestinaTheme.accent
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                RotationAnimator on rotation {
                    running: spinner.visible && !CelestinaTheme.reducedMotion
                    loops: Animation.Infinite
                    from: 0
                    to: 360
                    duration: CelestinaTheme.motionSlow * 6
                }
            }
        }

        Text {
            id: label
            width: Math.min(implicitWidth,
                            notice.maxWidth - glyph.width
                            - 2 * CelestinaTheme.spaceMd - CelestinaTheme.spaceSm)
            anchors.verticalCenter: parent.verticalCenter
            text: notice.text
            color: notice.danger ? CelestinaTheme.dangerFillInk
                                 : CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: notice.lines > 1 ? Text.Wrap : Text.NoWrap
            maximumLineCount: notice.lines
            elide: Text.ElideRight
        }
    }
}
```

- [ ] **Step 4: Register it**

Add `"qml/components/folder/ActivityNotice.qml"` to `QML_FILES` in `siderita/build.rs`, beside the other `components/folder/` rows.

- [ ] **Step 5: Run the tests**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh
```

Expected: PASS, including the five new `ActivityNotice` functions. If `test_a_danger_notice_wraps_but_an_info_notice_does_not` fails because both heights are equal, `label.width` is not clamping — check that `notice.maxWidth` reached the instance before `implicitHeight` was read, and bind `label.width` to `notice.width - glyph.width - …` instead of to `maxWidth`.

---

### Task 6: `ActivityStack` — the one transient column

**Files:**
- Create: `siderita/qml/components/folder/ActivityStack.qml`
- Modify: `siderita/build.rs` (`QML_FILES`)
- Test: `siderita/tests/qml/tst_activity_stack.qml`

**Interfaces:**
- Consumes: `ActivityNotice` (Task 5); `OperationsDock` (unchanged in this task); controller properties `noticeIds`, `noticeTexts`, `noticeIcons`, `noticeTones`, `noticeRunning` (`QStringList`, index-aligned), `errorText`, `opError` (`QString`), and the invokable `dismissNotice(string)` (Task 8).
- Produces: type `ActivityStack` with `required property var controller`, `required property Item backdrop`, `required property real maxNoticeWidth`, and an `implicitHeight` its placement reads to sit above the bottom bar.

- [ ] **Step 1: Write the failing test**

`siderita/tests/qml/tst_activity_stack.qml`:

```qml
import QtQuick
import QtTest 1.3
import org.celestina.siderita 1.0

// The one transient column: rings at its base, notices above them, errors above
// everything. It grows upward because its own height decides where its top is,
// never because a live positioner changes which end it is anchored to — that is
// what once left the shell's toast cards hanging outside their glass.
TestCase {
    id: testCase
    name: "ActivityStack"
    width: 600
    height: 400
    visible: true
    when: windowShown

    property var dismissed: []

    QtObject {
        id: controllerStub
        property string errorText: ""
        property string opError: ""
        property var noticeIds: []
        property var noticeTexts: []
        property var noticeIcons: []
        property var noticeTones: []
        property var noticeRunning: []
        property bool opRunning: false
        property var opIds: []
        property var opLabels: []
        property var opIcons: []
        property var opCurrents: []
        property var opDetails: []
        property var opPercents: []
        property var opSteps: []
        property var opPaused: []
        function dismissNotice(id) { testCase.dismissed.push(id) }
        function toggleJobPaused(id) { }
        function cancelJob(id) { }
    }

    Item {
        id: backdropStub
        anchors.fill: parent
    }

    ActivityStack {
        id: stack
        controller: controllerStub
        backdrop: backdropStub
        maxNoticeWidth: 400
        x: 180
        y: 400 - implicitHeight
    }

    function init() {
        testCase.dismissed = []
        controllerStub.errorText = ""
        controllerStub.opError = ""
        controllerStub.noticeIds = []
        controllerStub.noticeTexts = []
        controllerStub.noticeIcons = []
        controllerStub.noticeTones = []
        controllerStub.noticeRunning = []
        controllerStub.opRunning = false
        controllerStub.opIds = []
    }

    function test_an_empty_stack_takes_no_height() {
        compare(stack.implicitHeight, 0,
                "with nothing running the column is not there at all")
    }

    function test_a_settled_notice_gives_the_column_its_height() {
        controllerStub.noticeIds = ["7"]
        controllerStub.noticeTexts = ["Pegado cancelado"]
        controllerStub.noticeIcons = ["x"]
        controllerStub.noticeTones = ["info"]
        controllerStub.noticeRunning = ["0"]
        tryVerify(function() { return stack.implicitHeight > 0 }, 1000)
    }

    function test_errors_sit_above_notices_and_dismiss_through_the_controller() {
        controllerStub.noticeIds = ["8"]
        controllerStub.noticeTexts = ["Pegado cancelado"]
        controllerStub.noticeIcons = ["x"]
        controllerStub.noticeTones = ["info"]
        controllerStub.noticeRunning = ["0"]
        controllerStub.opError = "No se pudo mover el elemento"
        tryVerify(function() { return stack.errorNotice.shown }, 1000)
        verify(stack.errorNotice.y < stack.noticeRepeater.itemAt(0).y,
               "the error is above the announcement, not across the rows")
        mouseClick(stack.errorNotice, 10, 10)
        tryVerify(function() {
            return testCase.dismissed.indexOf("op-error") >= 0
        }, 1000, "pressing it asks the controller to clear the error")
    }

    function test_the_dock_is_the_base_of_the_column() {
        controllerStub.opRunning = true
        controllerStub.opIds = ["1"]
        controllerStub.opLabels = ["Moviendo…"]
        controllerStub.opIcons = ["arrow-right"]
        controllerStub.opCurrents = [""]
        controllerStub.opDetails = [""]
        controllerStub.opPercents = ["40"]
        controllerStub.opSteps = ["2"]
        controllerStub.opPaused = ["0"]
        controllerStub.noticeIds = ["9"]
        controllerStub.noticeTexts = ["Pegado cancelado"]
        controllerStub.noticeIcons = ["x"]
        controllerStub.noticeTones = ["info"]
        controllerStub.noticeRunning = ["0"]
        tryVerify(function() { return stack.dock.visible }, 1000)
        verify(stack.dock.y > stack.noticeRepeater.itemAt(0).y,
               "the rings are the base; the announcement rests on top of them")
    }
}
```

- [ ] **Step 2: Run it to watch it fail**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh 2>&1 | grep -i activitystack
```

Expected: `ActivityStack` is not a type in the module.

- [ ] **Step 3: Write the component**

`siderita/qml/components/folder/ActivityStack.qml`:

```qml
pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── The one transient column ──────────────────────────────────────
    // Everything that is temporary lives here, anchored to the bottom right of
    // the content frame and growing upward: the rings at the base, the notices
    // above them, the errors above everything.
    //
    // Before this there were three surfaces with three placements, two of them
    // full-width bands across the rows whose `y` chained off each other's
    // visibility. One column removes that chain: each layer is a child in
    // order, and the column's own height is what lifts its top away from the
    // bottom bar.
    //
    // The column is **always** top-anchored. A positioner re-anchored while it
    // is alive lays its children out from the wrong edge — that is exactly what
    // left the shell's toast drawing its cards outside their glass — so the
    // upward growth comes from the placement reading `implicitHeight`, never
    // from a ternary on `anchors`.
Item {
    id: stack

    required property var controller
    required property Item backdrop
    // The widest a notice may be: the content frame minus the bottom capsule
    // and the gaps either side of it.
    required property real maxNoticeWidth

    // Read by the placement and the tests rather than reached into.
    readonly property alias dock: rings
    // Null until there is an error to show; see the loaders below.
    readonly property Item errorNotice: opError.item
    readonly property alias noticeRepeater: notices

    implicitWidth: column.implicitWidth
    implicitHeight: column.implicitHeight

    // Escape dismisses the error that is holding the top of the column. The
    // rings own their own Escape for the callout; this one only fires when
    // there is an error to clear, so the two never compete.
    Shortcut {
        sequence: "Escape"
        enabled: stack.controller.opError.length > 0
                 || stack.controller.errorText.length > 0
        onActivated: {
            if (stack.controller.opError.length > 0)
                stack.controller.dismissNotice("op-error")
            else
                stack.controller.dismissNotice("error")
        }
    }

    Column {
        id: column
        anchors.top: parent.top
        anchors.right: parent.right
        spacing: CelestinaTheme.spaceXs

        // The two error properties are not queue entries — other surfaces read
        // them too (the picker, the empty state) — so they keep their own
        // identity and are rendered here as danger notices with reserved ids.
        // Loaders, not plain instances: a notice constructed at startup with
        // empty text would show at once, burn its ten seconds and retire, and
        // the first real error would then arrive at an object that had already
        // retired. Loading it with the error and unloading it with the error
        // also makes the exit animation come out right — the fade finishes,
        // `dismissed` clears the property, and only then does the loader empty.
        Loader {
            id: locationError
            anchors.right: parent.right
            active: stack.controller.errorText.length > 0
            sourceComponent: ActivityNotice {
                noticeId: "error"
                text: stack.controller.errorText
                iconName: "circle-alert"
                danger: true
                running: false
                maxWidth: stack.maxNoticeWidth
                backdrop: stack.backdrop
                onDismissed: stack.controller.dismissNotice("error")
            }
        }

        Loader {
            id: opError
            anchors.right: parent.right
            active: stack.controller.opError.length > 0
            sourceComponent: ActivityNotice {
                noticeId: "op-error"
                text: stack.controller.opError
                iconName: "circle-alert"
                danger: true
                running: false
                maxWidth: stack.maxNoticeWidth
                backdrop: stack.backdrop
                onDismissed: stack.controller.dismissNotice("op-error")
            }
        }

        Repeater {
            id: notices
            model: stack.controller.noticeIds.length

            ActivityNotice {
                id: entry
                required property int index
                anchors.right: parent.right
                noticeId: stack.at(stack.controller.noticeIds, entry.index)
                text: stack.at(stack.controller.noticeTexts, entry.index)
                iconName: stack.at(stack.controller.noticeIcons, entry.index)
                danger: stack.at(stack.controller.noticeTones, entry.index) === "danger"
                running: stack.at(stack.controller.noticeRunning, entry.index) === "1"
                maxWidth: stack.maxNoticeWidth
                backdrop: stack.backdrop
                onDismissed: id => stack.controller.dismissNotice(id)
            }
        }

        OperationsDock {
            id: rings
            anchors.right: parent.right
            controller: stack.controller
            backdrop: stack.backdrop
            // Always floating: the column sits over the content by definition,
            // and switching the glass off at the end of the list left it flat
            // and opaque.
            floating: true
            // The column is already as wide as the widest notice may be, so the
            // dock collapses when its rings would not fit beside the capsule.
            availableWidth: stack.maxNoticeWidth
        }
    }

    function at(list, index) {
        return list !== undefined && index >= 0 && index < list.length
               ? list[index] : ""
    }
}
```

- [ ] **Step 4: Register it**

Add `"qml/components/folder/ActivityStack.qml"` to `QML_FILES` in `siderita/build.rs`.

- [ ] **Step 5: Run the tests**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh
```

Expected: PASS — **after Task 7**. `ActivityStack` passes `availableWidth` to `OperationsDock`, which Task 7 adds; until then the runner reports an unknown property. The two tasks land in the same commit and this is their only ordering constraint, so run Task 7 and then re-run this task's tests.

---

### Task 7: `OperationsDock` — the counted circle and the labelled list

The row of rings is kept exactly as it is for one, two or three jobs. Beyond that, or when the row would not fit beside the bottom capsule, the dock collapses to one circle carrying the count and the aggregate arc; pressing it expands upward into a list with one labelled row per job. The expanded list carries each job's own pause and cancel, so the per-ring callout is not opened while it is showing.

**Files:**
- Modify: `siderita/qml/components/folder/OperationsDock.qml`
- Create: `siderita/qml/components/folder/OperationsListRow.qml`
- Modify: `siderita/build.rs` (`QML_FILES`)
- Test: `siderita/tests/qml/tst_operations_dock.qml` (extended)

**Interfaces:**
- Consumes: the controller's existing `opIds`, `opLabels`, `opIcons`, `opPercents`, `opSteps`, `opPaused`, `toggleJobPaused(string)`, `cancelJob(string)`.
- Produces: `OperationsDock` gains `property real availableWidth: 0` (`0` means "no limit"), and the read-only `collapsed`, `expanded`, `countCircle` and `jobList` aliases the tests read. `OperationsListRow` takes `required property string jobId`, `required property string label`, `required property string iconName`, `required property int percent`, `required property int steps`, `required property bool paused`, and emits `pauseRequested(string id)` and `cancelRequested(string id)`.

- [ ] **Step 1: Write the failing tests**

Append to `siderita/tests/qml/tst_operations_dock.qml`, inside the existing `TestCase`:

```qml
    function fourJobs() {
        controllerStub.opIds = ["1", "2", "3", "4"]
        controllerStub.opLabels = ["Copiando…", "Extrayendo…", "Moviendo…",
                                   "Desmontando…"]
        controllerStub.opIcons = ["copy", "file-archive", "arrow-right", "unplug"]
        controllerStub.opCurrents = ["", "", "", ""]
        controllerStub.opDetails = ["", "", "", ""]
        controllerStub.opPercents = ["60", "-1", "80", "-1"]
        controllerStub.opSteps = ["3", "7", "2", "5"]
        controllerStub.opPaused = ["0", "0", "0", "0"]
        controllerStub.opRunning = true
    }

    function test_three_jobs_stay_a_row_of_rings() {
        controllerStub.opIds = ["1", "2", "3"]
        controllerStub.opLabels = ["Copiando…", "Extrayendo…", "Moviendo…"]
        controllerStub.opIcons = ["copy", "file-archive", "arrow-right"]
        controllerStub.opCurrents = ["", "", ""]
        controllerStub.opDetails = ["", "", ""]
        controllerStub.opPercents = ["60", "-1", "80"]
        controllerStub.opSteps = ["3", "7", "2"]
        controllerStub.opPaused = ["0", "0", "0"]
        controllerStub.opRunning = true
        compare(dock.collapsed, false, "three rings still fit in a row")
    }

    function test_a_fourth_job_collapses_the_dock_to_a_counted_circle() {
        fourJobs()
        compare(dock.collapsed, true)
        compare(dock.countCircle.count, 4,
                "the circle carries how many are running")
        verify(dock.implicitWidth < 80,
               "a collapsed dock is one circle wide, not four rings wide")
    }

    function test_a_narrow_frame_collapses_the_dock_whatever_the_count() {
        controllerStub.opIds = ["1", "2"]
        controllerStub.opLabels = ["Copiando…", "Extrayendo…"]
        controllerStub.opIcons = ["copy", "file-archive"]
        controllerStub.opCurrents = ["", ""]
        controllerStub.opDetails = ["", ""]
        controllerStub.opPercents = ["60", "-1"]
        controllerStub.opSteps = ["3", "7"]
        controllerStub.opPaused = ["0", "0"]
        controllerStub.opRunning = true
        dock.availableWidth = 60
        compare(dock.collapsed, true,
                "two rings do not fit in 60px, so the circle takes over")
        dock.availableWidth = 0
    }

    function test_the_circle_expands_into_a_labelled_list_and_closes_again() {
        fourJobs()
        mouseClick(dock.countCircle)
        compare(dock.expanded, true)
        compare(dock.jobList.count, 4, "one row per job, each with its name")
        compare(dock.jobList.itemAt(0).label, "Copiando…")
        // The catcher that closes the callout closes this too.
        mouseClick(testCase, 20, 20)
        compare(dock.expanded, false)
        compare(testCase.contentPresses, 0,
                "and that closing press never reached the folder")
    }

    function test_a_row_cancels_the_job_it_names() {
        fourJobs()
        mouseClick(dock.countCircle)
        dock.jobList.itemAt(1).cancelRequested(dock.jobList.itemAt(1).jobId)
        compare(testCase.cancelled, ["2"],
                "cancel names the job's own id, not the row's position")
    }
```

- [ ] **Step 2: Run them to watch them fail**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh 2>&1 | grep -i operationsdock
```

Expected: FAIL — `dock.collapsed`, `dock.countCircle`, `dock.expanded` and `dock.jobList` do not exist.

- [ ] **Step 3: Write the list row**

`siderita/qml/components/folder/OperationsListRow.qml`:

```qml
pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── One job, as a row ─────────────────────────────────────────────
    // What the expanded dock shows instead of a ring: the same arc at reading
    // size, the job's own name beside it, and its two controls on the right.
    // A ring plus a callout is the right shape for one job a person went
    // looking at; four of them is a list, and a list says which is which
    // without being asked.
Item {
    id: row

    required property string jobId
    required property string label
    required property string iconName
    required property int percent
    required property int steps
    required property bool paused

    signal pauseRequested(string id)
    signal cancelRequested(string id)

    implicitHeight: CelestinaTheme.controlHeightSm
    implicitWidth: ring.width + CelestinaTheme.spaceSm + name.implicitWidth
                   + CelestinaTheme.spaceMd + controls.width

    OperationRing {
        id: ring
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: 26
        height: 26
        iconName: row.iconName
        percent: row.percent
        steps: row.steps
        paused: row.paused
        // In the list the ring is a read-out, not a button: the row's own
        // controls are right there, so there is nothing for a press to open.
        enabled: false
        Accessible.name: row.label
    }

    Text {
        id: name
        anchors.left: ring.right
        anchors.leftMargin: CelestinaTheme.spaceSm
        anchors.right: controls.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        text: row.label
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        elide: Text.ElideRight
    }

    Row {
        id: controls
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        CelestinaIconButton {
            width: 26
            height: 26
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: row.paused ? "media-play" : "media-pause"
            helpText: row.paused ? qsTr("Reanudar") : qsTr("Pausar")
            onClicked: row.pauseRequested(row.jobId)
        }

        CelestinaIconButton {
            width: 26
            height: 26
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: "x"
            helpText: qsTr("Cancelar")
            onClicked: row.cancelRequested(row.jobId)
        }
    }
}
```

- [ ] **Step 4: Give the dock its two other shapes**

In `siderita/qml/components/folder/OperationsDock.qml`, add beside the existing properties:

```qml
    // How much width the column can give the rings. Zero means "no limit",
    // which is what the tests and any consumer that does not measure pass.
    property real availableWidth: 0
    // Beyond three, a row of rings is a row of anonymous circles: the list says
    // which is which. A frame too narrow collapses it at any count.
    readonly property int rowLimit: 3
    readonly property real rowWidth: dock.jobIds.length * dock.ringSize
                                     + Math.max(0, dock.jobIds.length - 1) * dock.gap
                                     + 2 * dock.padding
    readonly property bool collapsed: dock.jobIds.length > dock.rowLimit
                                      || (dock.availableWidth > 0
                                          && dock.rowWidth > dock.availableWidth)
    property bool expanded: false

    readonly property alias countCircle: counted
    // The Repeater, not the Column: the tests read `count` and `itemAt`, which
    // are the Repeater's own. An alias named for the Column would also collide
    // with the Repeater's id.
    readonly property alias jobList: jobRepeater
```

Change the dock's size and the row's visibility so the three shapes are exclusive:

```qml
    implicitWidth: dock.collapsed
                   ? counted.width + 2 * dock.padding
                   : rings.width + 2 * dock.padding
    implicitHeight: (dock.collapsed && dock.expanded
                     ? jobRows.implicitHeight
                     : dock.ringSize) + 2 * dock.padding
```

`rings` gains `visible: !dock.collapsed`. A collapse while a callout is open takes the callout with it, for the same reason a job ending does — extend the existing guard:

```qml
    onCollapsedChanged: if (dock.collapsed) dock.openId = ""
    onJobIdsChanged: {
        if (dock.openId.length > 0 && dock.indexOfJob(dock.openId) < 0)
            dock.openId = ""
    }
```

The outside catcher already closes the callout; make it close the expansion too, and widen the condition that shows it:

```qml
        visible: dock.openId.length > 0 || dock.expanded
        ...
            onPressed: {
                dock.openId = ""
                dock.expanded = false
            }
```

Extend the Escape shortcut the same way:

```qml
    Shortcut {
        sequence: "Escape"
        enabled: dock.openId.length > 0 || dock.expanded
        onActivated: {
            dock.openId = ""
            dock.expanded = false
        }
    }
```

Then add the two new shapes beside `rings`:

```qml
    // The collapsed shape: one circle with the count inside and the average of
    // every measurable job on its arc. It is a button, and what it opens is the
    // list below.
    OperationRing {
        id: counted
        visible: dock.collapsed
        x: dock.padding
        y: dock.padding
        width: dock.ringSize
        height: dock.ringSize
        readonly property int count: dock.jobIds.length
        // The count replaces the glyph: with four jobs there is no single
        // action to draw, and how many there are is the one thing the circle
        // can say truthfully.
        iconName: ""
        countLabel: counted.count
        percent: {
            let total = 0
            let measured = 0
            for (let i = 0; i < dock.jobIds.length; i++) {
                const raw = parseInt(dock.at(dock.controller.opPercents, i), 10)
                if (!isNaN(raw) && raw >= 0) {
                    total += raw
                    measured++
                }
            }
            return measured > 0 ? Math.round(total / measured) : -1
        }
        steps: {
            let sum = 0
            for (let i = 0; i < dock.jobIds.length; i++) {
                const raw = parseInt(dock.at(dock.controller.opSteps, i), 10)
                sum += isNaN(raw) ? 0 : raw
            }
            return sum
        }
        active: dock.expanded
        Accessible.role: Accessible.Button
        Accessible.name: qsTr("%1 operaciones en curso").arg(counted.count)
        onClicked: dock.expanded = !dock.expanded
    }

    // The expanded shape: the same jobs, named.
    Column {
        id: jobRows
        visible: dock.collapsed && dock.expanded
        x: dock.padding
        y: dock.padding
        spacing: 2

        Repeater {
            id: jobRepeater
            model: dock.jobIds.length

            OperationsListRow {
                required property int index
                width: 260
                jobId: dock.at(dock.jobIds, index)
                label: dock.at(dock.controller.opLabels, index)
                iconName: dock.at(dock.controller.opIcons, index)
                percent: {
                    const raw = parseInt(dock.at(dock.controller.opPercents, index), 10)
                    return isNaN(raw) ? -1 : raw
                }
                steps: {
                    const raw = parseInt(dock.at(dock.controller.opSteps, index), 10)
                    return isNaN(raw) ? 0 : raw
                }
                paused: dock.at(dock.controller.opPaused, index) === "1"
                onPauseRequested: id => dock.controller.toggleJobPaused(id)
                onCancelRequested: id => dock.controller.cancelJob(id)
            }
        }
    }
```

`OperationRing` needs the count face. Add to `siderita/qml/components/folder/OperationRing.qml`:

```qml
    // When several jobs share one ring there is no single action to draw, so
    // the ring wears how many there are instead of a glyph. Below zero means
    // "draw the icon", which is every other ring in the application.
    property int countLabel: -1
```

and, beside the existing centred glyph, its alternative face — with the glyph gaining `visible: ring.countLabel < 0` so exactly one of the two is ever drawn:

```qml
    // The count's face. Tabular figures so the circle does not twitch as the
    // number changes width.
    Text {
        anchors.centerIn: parent
        visible: ring.countLabel >= 0
        text: ring.countLabel
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        font.features: ({ "tnum": 1 })
    }
```

- [ ] **Step 5: Register the new file and run the tests**

Add `"qml/components/folder/OperationsListRow.qml"` to `QML_FILES` in `siderita/build.rs`, then:

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/qml-tests.sh
```

Expected: PASS for `OperationsDock` (old and new functions) and now also for `ActivityStack`.

---

### Task 8: The notice queue replaces `status_text`

**Files:**
- Create: `siderita/src/controller/notices.rs`
- Modify: `siderita/src/controller.rs` (bridge properties, struct fields, construction, `mod` list)
- Modify: `siderita/src/controller/{mounts,trash,scan,archive,fileops,navigation,shell,jobs}.rs`

**Interfaces:**
- Produces, on `SideritaController`: properties `notice_ids`, `notice_texts`, `notice_icons`, `notice_tones`, `notice_running` (`QStringList`, index-aligned) and `notice_revision` (`i32`); invokable `dismiss_notice(id: QString)`. On the Rust side: `push_notice(&str text, &str icon, NoticeTone, bool running) -> u64`, `settle_notice(u64, &str text)`, `drop_notice(u64)`.
- Removes: the property `status_text` and every `set_status_text` call.

- [ ] **Step 1: Write the module**

`siderita/src/controller/notices.rs`:

```rust
//! language-contract: product-copy
//!
//! The transient announcements, and the one column that shows them.
//!
//! `status_text` used to carry four unrelated kinds of message through one
//! property: a running job (which the ring above it already drew), an activity
//! with no knowable end, an outcome that had already happened, and two standing
//! facts about the folder. That is why the strip across the bottom bar could
//! never simply be deleted, and why `"Desmontando…"` stayed on screen until
//! something else happened to overwrite it.
//!
//! What replaces it is a queue whose entries have owners. A notice pushed while
//! something runs is settled or dropped by the same completion path that ends
//! the work; a notice pushed after the fact is settled from birth. The surface
//! decides when a settled notice has been read and asks for it to be dropped.
//!
//! The queue belongs to the controller, not to the process: a job is global
//! (it outlives the tab that started it) but an announcement is addressed to
//! the person looking at this folder right now.
//!
//! The marker above declares the Spanish here: a notice's text is the line a
//! person reads.

use core::pin::Pin;

use cxx_qt_lib::QString;

use super::qobject;

/// Which of the two tones a notice wears. Anything a person must be able to
/// read on a long path is `Danger`: it wraps, it stays longer, and it is the
/// top of the column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoticeTone {
    Info,
    Danger,
}

impl NoticeTone {
    fn token(self) -> &'static str {
        match self {
            NoticeTone::Info => "info",
            NoticeTone::Danger => "danger",
        }
    }
}

/// One announcement.
pub(crate) struct Notice {
    pub(crate) id: u64,
    pub(crate) text: String,
    pub(crate) icon: String,
    pub(crate) tone: NoticeTone,
    /// True while the action it names is still in flight. The surface draws a
    /// running notice only once it has lasted past its appearance threshold,
    /// which is what keeps a 300 ms unmount from flashing.
    pub(crate) running: bool,
}

/// At most this many at once. A fourth would push the column into the rows, and
/// three unread announcements already means nobody is reading them.
const VISIBLE: usize = 3;

/// The queue itself, with no Qt in it.
///
/// Separated from the controller on purpose: the rule worth testing is that
/// every notice pushed while something runs is eventually settled or dropped,
/// and that rule is about this data structure, not about a QObject. A test that
/// needed a controller instance could not run at all, and one that grepped the
/// sources for call pairs would be a grep pretending to be a test.
#[derive(Default)]
pub(crate) struct NoticeQueue {
    entries: Vec<Notice>,
    next_id: u64,
}

impl NoticeQueue {
    /// Adds a notice and hands back the caller's handle for settling or
    /// dropping it later.
    pub(crate) fn push(&mut self, text: &str, icon: &str, tone: NoticeTone, running: bool) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push(Notice {
            id,
            text: text.to_owned(),
            icon: icon.to_owned(),
            tone,
            running,
        });
        while self.entries.len() > VISIBLE {
            self.entries.remove(0);
        }
        id
    }

    /// Marks a notice finished and gives it its settled wording. Answers
    /// whether it was still there: a notice evicted by the cap is not an error,
    /// and its owner must not treat the miss as one.
    pub(crate) fn settle(&mut self, id: u64, text: &str) -> bool {
        let Some(notice) = self.entries.iter_mut().find(|notice| notice.id == id) else {
            return false;
        };
        notice.running = false;
        notice.text = text.to_owned();
        true
    }

    /// Removes a notice outright: a read that simply finished, or a settled
    /// notice whose time on screen is up.
    pub(crate) fn remove(&mut self, id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|notice| notice.id != id);
        before != self.entries.len()
    }

    pub(crate) fn entries(&self) -> &[Notice] {
        &self.entries
    }

    /// The notices still claiming that something is happening. The invariant
    /// this module exists to keep is that this is empty once every started
    /// action has finished.
    #[cfg(test)]
    fn running_ids(&self) -> Vec<u64> {
        self.entries
            .iter()
            .filter(|notice| notice.running)
            .map(|notice| notice.id)
            .collect()
    }
}

impl qobject::SideritaController {
    /// Adds a notice and publishes the queue.
    pub(crate) fn push_notice(
        mut self: Pin<&mut Self>,
        text: &str,
        icon: &str,
        tone: NoticeTone,
        running: bool,
    ) -> u64 {
        let id = self.as_mut().rust_mut().notices.push(text, icon, tone, running);
        self.publish_notices();
        id
    }

    /// Marks a notice as finished and gives it its settled wording. A notice
    /// the surface has already drawn mutates in place; one it has not drawn yet
    /// is dropped by the surface without ever appearing.
    pub(crate) fn settle_notice(mut self: Pin<&mut Self>, id: u64, text: &str) {
        if self.as_mut().rust_mut().notices.settle(id, text) {
            self.publish_notices();
        }
    }

    /// Removes a notice outright.
    pub(crate) fn drop_notice(mut self: Pin<&mut Self>, id: u64) {
        if self.as_mut().rust_mut().notices.remove(id) {
            self.publish_notices();
        }
    }

    /// The surface asking for an entry to go: its dwell expired, or a person
    /// pressed it. The two error properties are not queue entries — other
    /// surfaces read them too — so they answer to their own reserved ids.
    pub fn dismiss_notice(mut self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        match id.as_str() {
            "error" => {
                self.as_mut().set_error_text(QString::default());
            }
            "op-error" => {
                self.as_mut().set_op_error(QString::default());
            }
            _ => {
                if let Ok(id) = id.parse::<u64>() {
                    self.drop_notice(id);
                }
            }
        }
    }

    /// Publishes the queue as the index-aligned lists the column consumes, and
    /// bumps the revision so a surface can tell a reorder from a redraw.
    pub(crate) fn publish_notices(mut self: Pin<&mut Self>) {
        let mut ids = cxx_qt_lib::QStringList::default();
        let mut texts = cxx_qt_lib::QStringList::default();
        let mut icons = cxx_qt_lib::QStringList::default();
        let mut tones = cxx_qt_lib::QStringList::default();
        let mut running = cxx_qt_lib::QStringList::default();
        for notice in self.as_ref().rust().notices.entries() {
            ids.append(QString::from(notice.id.to_string().as_str()));
            texts.append(QString::from(notice.text.as_str()));
            icons.append(QString::from(notice.icon.as_str()));
            tones.append(QString::from(notice.tone.token()));
            running.append(QString::from(if notice.running { "1" } else { "0" }));
        }
        self.as_mut().set_notice_ids(ids);
        self.as_mut().set_notice_texts(texts);
        self.as_mut().set_notice_icons(icons);
        self.as_mut().set_notice_tones(tones);
        self.as_mut().set_notice_running(running);
        let next = self.as_ref().notice_revision().wrapping_add(1);
        self.as_mut().set_notice_revision(next);
    }
}

#[cfg(test)]
mod tests {
    use super::{NoticeQueue, NoticeTone, VISIBLE};

    #[test]
    fn a_fourth_notice_evicts_the_oldest() {
        let mut queue = NoticeQueue::default();
        for index in 0..4 {
            queue.push(&format!("aviso {index}"), "info", NoticeTone::Info, false);
        }
        assert_eq!(queue.entries().len(), VISIBLE);
        assert_eq!(queue.entries()[0].text, "aviso 1");
    }

    #[test]
    fn settling_stops_the_running_claim_and_changes_the_wording() {
        let mut queue = NoticeQueue::default();
        let id = queue.push("Desmontando…", "unplug", NoticeTone::Info, true);
        assert_eq!(queue.running_ids(), vec![id]);
        assert!(queue.settle(id, "Disco desmontado"));
        assert!(queue.running_ids().is_empty());
        assert_eq!(queue.entries()[0].text, "Disco desmontado");
    }

    #[test]
    fn settling_or_removing_an_evicted_notice_is_not_an_error() {
        let mut queue = NoticeQueue::default();
        let first = queue.push("Leyendo carpeta…", "folder", NoticeTone::Info, true);
        for index in 0..3 {
            queue.push(&format!("aviso {index}"), "info", NoticeTone::Info, false);
        }
        assert!(!queue.settle(first, "Carpeta leída"));
        assert!(!queue.remove(first));
    }

    /// The invariant the whole module exists for. `"Desmontando…"` used to have
    /// no completion path at all and stayed on screen until something else
    /// happened to overwrite it.
    #[test]
    fn nothing_still_claims_to_be_running_once_every_action_has_ended() {
        let mut queue = NoticeQueue::default();
        let mount = queue.push("Montando…", "hard-drive", NoticeTone::Info, true);
        let read = queue.push("Leyendo la papelera…", "user-trash", NoticeTone::Info, true);
        queue.push("Pegado cancelado", "circle-stop", NoticeTone::Info, false);
        assert_eq!(queue.running_ids(), vec![mount, read]);

        // The mount finishes with something worth saying; the read does not.
        queue.settle(mount, "Disco montado");
        queue.remove(read);
        assert!(
            queue.running_ids().is_empty(),
            "a running notice outlived the action that pushed it"
        );
    }

    #[test]
    fn a_danger_notice_keeps_its_tone_through_the_queue() {
        let mut queue = NoticeQueue::default();
        queue.push("No se pudo leer la carpeta", "circle-alert", NoticeTone::Danger, false);
        assert_eq!(queue.entries()[0].tone, NoticeTone::Danger);
    }
}
```

- [ ] **Step 2: Declare the properties and the invokable**

In `siderita/src/controller.rs`, in the `extern "RustQt"` block, delete `#[qproperty(QString, status_text)]` and add beside the `op_*` group:

```rust
        /// The transient announcements, an entry per notice
        /// (`controller/notices.rs`). Index-aligned lists, as every other list
        /// this controller publishes: cxx-qt 0.9 exposes no model from Rust.
        #[qproperty(QStringList, notice_ids)]
        #[qproperty(QStringList, notice_texts)]
        #[qproperty(QStringList, notice_icons)]
        #[qproperty(QStringList, notice_tones)]
        #[qproperty(QStringList, notice_running)]
        #[qproperty(i32, notice_revision)]
```

and beside `cancel_job`:

```rust
        /// Drops one announcement: its time is up, or a person pressed it. The
        /// ids `"error"` and `"op-error"` clear the two error properties, which
        /// are not queue entries because other surfaces read them too.
        #[qinvokable]
        fn dismiss_notice(self: Pin<&mut SideritaController>, id: &QString);
```

In `SideritaControllerRust`, delete `status_text: QString,` and add:

```rust
    notice_ids: QStringList,
    notice_texts: QStringList,
    notice_icons: QStringList,
    notice_tones: QStringList,
    notice_running: QStringList,
    notice_revision: i32,
    notices: notices::NoticeQueue,
```

In the constructor, delete `status_text: QString::from("Preparando Siderita…"),` and add the seven defaults (`QStringList::default()` five times, `0` for the revision, `NoticeQueue::default()` for the queue). The startup line goes with it: a window that has just opened does not need to announce that it is opening.

Add `mod notices;` to the module list, in alphabetical position between `mod navigation;` and `mod paste;`.

- [ ] **Step 3: Convert the call sites**

| File and line | Today | After |
|---|---|---|
| `jobs.rs:190` (`start_job`) | `self.as_mut().set_status_text(QString::from(label));` | **delete the line.** The ring already carries the label; this is the duplication the author photographed. |
| `mounts.rs:271` (`mount_volume`) | `set_status_text("Montando…")` | `let notice = self.as_mut().push_notice("Montando…", "hard-drive", NoticeTone::Info, true);` then move `notice` into the worker's `qt.queue` closure and, in the completion arm, `controller.as_mut().settle_notice(notice, "Disco montado")` on `Ok`, `controller.as_mut().drop_notice(notice)` on `Err` (the error already speaks through `op_error`). |
| `mounts.rs:298` (`unmount_volume`) | `set_status_text("Desmontando…")` | the same, with `"Desmontando…"` / `"unplug"` and `"Disco desmontado"`. **This is the leak:** today nothing clears it. |
| `mounts.rs:338` (`open_volume`) | `set_status_text("Montando…")` | the same as `mount_volume`; the navigation that follows is its own outcome, so settle with `"Disco montado"` and let the dwell take it away. |
| `trash.rs:143`, `trash.rs:213` | `set_status_text("Leyendo la papelera…")` / `"Leyendo Recientes…"` | `push_notice(…, "user-trash" / "clock-arrow-up", Info, true)`, held in the controller's pending field for that read and **dropped** — not settled — in the handler that today calls `set_status_text(QString::default())` at `trash.rs:165`, `230` and `341`. A finished read has no outcome worth a word. |
| `scan.rs:131` (`"Leyendo carpeta…"`) | `set_status_text` | the same shape as the trash reads: pushed running, dropped when the scan publishes. Fast folders never draw it, because of the threshold. |
| `scan.rs:228` (`"No se pudo leer la carpeta"`) | `set_status_text` | `push_notice("No se pudo leer la carpeta", "circle-alert", NoticeTone::Danger, false)` |
| `scan.rs:381` (`"{visible} de {total}"`) | `set_status_text` | **delete the whole `status` block.** `FolderHeading.qml` already renders `N VISIBLES DE M ELEMENTOS`; keep the `set_folder_visible_count` / `set_folder_total_count` calls that follow it. |
| `navigation.rs:124` (`"La ubicación está vacía"`) | `set_status_text` | `push_notice(…, "info", NoticeTone::Info, false)` |
| `archive.rs:312` (`"Esperando la contraseña…"`) | `set_status_text` | `push_notice(…, "key", NoticeTone::Info, true)`, settled or dropped by the password dialog's answer at `archive.rs:321` and `426` |
| `archive.rs:321` (`"Extrayendo…"`) | `set_status_text` | **delete.** The extraction has a ring. |
| `archive.rs:426`, `fileops.rs:220`, `fileops.rs:675` (`"Operación cancelada"`) | `set_status_text` | `push_notice("Operación cancelada", "circle-stop", NoticeTone::Info, false)` |
| `fileops.rs:453` (`same_folder_cut_status`) | `set_status_text` | `push_notice(super::display::same_folder_cut_status(count), "info", NoticeTone::Info, false)` |
| `fileops.rs:536` (`"Pegado cancelado"`) | `set_status_text` | `push_notice(…, "circle-stop", NoticeTone::Info, false)` |
| `fileops.rs:628` (`"Cancelando…"`) | `set_status_text` | **delete.** The ring that is being cancelled is still on screen and its callout already says so. |
| `fileops.rs:679` | `set_status_text(message)` | `push_notice(message.as_str(), "info", NoticeTone::Info, false)` |
| `shell.rs:28`, `shell.rs:106` (`"Abriendo X…"`) | `set_status_text` | `push_notice(message.as_str(), "share-2", NoticeTone::Info, false)` — it is an outcome, not an activity: `xdg-open` is fire-and-forget and nothing here can tell when the other application is up. |

Each `mounts.rs` and `trash.rs` conversion needs the notice id to travel into the worker's completion closure. The id is a `u64` and the closure already moves a `Vec`/`PathBuf`, so add it to the capture list; do **not** store it in a field that a second read could overwrite.

- [ ] **Step 4: Run the crate-level tests**

The five tests in Step 1 are written **before** the call sites in Step 3 and are the reason the queue is a plain struct: they exercise the rule that matters — nothing still claims to be running once every action has ended — without needing a Qt instance, and they fail today because `NoticeQueue` does not exist.

There is no `cargo` budget for running them on their own: they are compiled and run by `verify-production.sh` in Task 9, which is this checkpoint's one build. Until then, read them against Step 3's table and confirm by eye that every row that pushes with `true` names a `settle_notice` or `drop_notice` in the same handler. The three rows that do so are `mounts.rs` (three call sites), `trash.rs` (two reads) and `scan.rs` (one read); `archive.rs`'s password wait is the fourth.

- [ ] **Step 5: Delete what is now unreachable**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && rg -n "status_text|statusText" src qml tests
```

Expected: no matches outside `src/editor.rs`, which has its own unrelated `status_text` for the embedded Grafita editor and is not touched by this checkpoint. If anything else matches, convert it before moving on — a half-removed property is exactly the "two active paths" the contract forbids.

---

### Task 9: Wire it in, build once, and close SID-B1-A

**Files:**
- Modify: `siderita/qml/components/folder/FolderBottomStatus.qml` (replaced wholesale)
- Modify: `siderita/qml/components/folder/FolderHeading.qml`
- Create: `siderita/docs/evidence/2026-09-22-bottom-chrome-and-notices.md`
- Create: `siderita/docs/inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-A.numstat.tsv`

**Interfaces:**
- Consumes: everything Tasks 1–8 produced.

- [ ] **Step 1: Replace the bottom status**

`siderita/qml/components/folder/FolderBottomStatus.qml`, entirely:

```qml
import QtQuick
import org.celestina.siderita 1.0

// ─── FolderBottomStatus ─────────────────────────────────────────────────────
// Where the transient column lives. There were three surfaces here with three
// placements — two full-width bands across the rows and a strip repeating what
// the ring already said — and each one's `y` depended on whether the one before
// it was visible. It is now one column anchored to the bottom right, and its
// own height is what lifts its top away from the bar.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    required property var controller
    required property var hostWindow
    required property Item panel
    required property Item bottomControls
    required property Item contentFrame
    required property Item bottomBar
    required property Item bottomView
    required property bool bottomFloating

    ActivityStack {
        id: stack
        controller: root.controller
        backdrop: root.bottomView
        // The widest a notice may be: the frame minus the capsule and the two
        // gaps around it.
        maxNoticeWidth: Math.max(
            160,
            root.contentFrame.width - 2 * root.panel.floatingChromeInset
            - root.bottomControls.implicitWidth - 2 * CelestinaTheme.spaceMd)
        x: root.contentFrame.x + root.contentFrame.width
           - root.panel.floatingChromeInset - width
        // It grows upward because its own height decides where its top is, not
        // because the column changes which end it is anchored to. See
        // ActivityStack.
        y: root.bottomBar.y - CelestinaTheme.compFloatingGap - height
        z: 5
    }
}
```

The `sizeButton` and its `SizePopup` moved into the capsule in Task 4; nothing here replaces them.

- [ ] **Step 2: Move the watch warning to the heading**

In `siderita/qml/components/folder/FolderHeading.qml`, in the same `Column` that holds the title, directly after the title `Text`:

```qml
        // A lost watch is not an event: it stays true of this folder for as
        // long as it lasts, so it lives where the folder describes itself and
        // not in a column that retires by itself.
        Row {
            width: parent.width
            height: visible ? implicitHeight : 0
            visible: root.controller.watchDegraded
            spacing: CelestinaTheme.spaceSm
            // Centred like the rest of the heading, except on a phone, where
            // the heading aligns left.
            anchors.horizontalCenter: root.phoneLocation
                                      ? undefined : parent.horizontalCenter

            CelestinaIcon {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.round(CelestinaTheme.iconSm
                                  * root.hostWindow.interfaceIconScale)
                height: width
                name: "circle-alert"
                fallbackName: "circle-alert"
                tone: CelestinaIcon.Danger
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Vigilancia perdida · instantánea")
                color: CelestinaTheme.dangerFillInk
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                           * root.hostWindow.interfaceTextScale)
            }
        }
```

Accepted consequence, recorded in the evidence: a retired heading takes the warning with it, so a person scrolled deep into a folder does not see it until they scroll back. It is a standing condition, not an alarm, and the alternative — a permanent surface over the rows — is the thing this checkpoint removes.

- [ ] **Step 3: Lower the ratchets this change earns**

`scripts/language-baseline.tsv` — delete all three rows, because every Spanish literal in those files is now a `qsTr()` argument and the scanner no longer counts them:

```
4	siderita/qml/components/chrome/BottomControls.qml
2	siderita/qml/components/folder/FolderBottomStatus.qml
6	siderita/qml/menus/FolderSortMenu.qml
```

`scripts/architecture-baseline.tsv` — delete the row the capsule's rewrite retires, since the `BusyIndicator` is gone and folder loading is a notice now:

```
control	siderita/qml/components/chrome/BottomControls.qml:BusyIndicator	1
```

Both files are `commit_policy.shared_ratchet_files`, so they are lowered in **this** commit: splitting them into a follow-up would publish a revision whose own guard is already red. Never raise a row to make a guard pass.

Record in the evidence, with the exact field spellings the documentation contract looks for:

```markdown
- **Resolved language debt:** `siderita/qml/components/chrome/BottomControls.qml`
- **Resolved language debt:** `siderita/qml/components/folder/FolderBottomStatus.qml`
- **Resolved language debt:** `siderita/qml/menus/FolderSortMenu.qml`
```

- [ ] **Step 4: Static gates before the one build**

```bash
cd /home/toni/CODIGO/CELESTINA
./scripts/check-language-contract.py
./scripts/check-architecture-contract.sh
./scripts/check-documentation-contract.sh
cd siderita && scripts/qml-tests.sh
```

Expected: all PASS. Fix what they name before spending the build.

- [ ] **Step 5: The one build**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita
scripts/build-production.sh && scripts/verify-production.sh
```

Expected: both succeed. `verify-production.sh` runs the release-profile tests (including the five `controller::notices::tests` functions, of which `nothing_still_claims_to_be_running_once_every_action_has_ended` is the one that guards the leak), clippy and `smoke.sh`. This is one of the checkpoint's two permitted build cycles — do not run `cargo` again for any reason.

- [ ] **Step 6: Headless stills**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita
SCRATCH=$(mktemp -d)
XDG_CONFIG_HOME="$SCRATCH" QT_ASSUME_STDERR_HAS_CONSOLE=1 QT_QPA_PLATFORM=offscreen \
  ./target/release/siderita "$HOME"
```

With a temporary probe `Timer` in `Main.qml` calling `grabToImage` on `contentLayer` — **declared in QML**, never `window.contentItem`, which is created by C++ and has no QML engine — save one still per state: the bar at rest, the menu open, a running notice, a settled notice, a danger notice, three rings, the collapsed circle, the expanded list. `ShaderEffectSource` renders nothing under `offscreen`, so the glass is blank in every still; that is expected and is why appearance is `VAL-SID-15`. **Remove the probe before staging anything.**

- [ ] **Step 7: Write the evidence**

`siderita/docs/evidence/2026-09-22-bottom-chrome-and-notices.md`, recording: the four kinds of message `status_text` carried and where each went; that `"Desmontando…"` was never cleared and `volume_busy` was read by nothing; that `FolderHeading` already rendered the `N de M` the strip repeated; that the author's screenshots predate `1c86dd55`; the qml-tests output; the `verify-production.sh` output; the stills; and the two accepted costs (no permanent sort arrow, the warning retires with the heading).

- [ ] **Step 8: Recompute the inventory and commit**

```bash
cd /home/toni/CODIGO/CELESTINA
python3 "$SCRATCH/mkinv.py" SID-B1-A \
  siderita/docs/inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-A.numstat.tsv \
  siderita/docs/plans/active/2026-09-22-bottom-chrome-and-notices.md \
  siderita/ROADMAP.md siderita/STATUS.md siderita/VALIDATION.md \
  siderita/docs/plans/active/README.md siderita/docs/plans/archive/README.md \
  siderita/docs/plans/archive/2026-08-20-what-the-window-costs-and-shows.md \
  siderita/docs/evidence/2026-09-22-bottom-chrome-and-notices.md \
  scripts/language-baseline.tsv scripts/architecture-baseline.tsv \
  siderita/build.rs \
  siderita/qml/components/chrome/HiddenToggleDefs.qml \
  siderita/qml/components/chrome/HiddenTogglePill.qml \
  siderita/qml/components/chrome/BottomControls.qml \
  siderita/qml/menus/ViewSortMenu.qml \
  siderita/qml/components/folder/ActivityNotice.qml \
  siderita/qml/components/folder/ActivityStack.qml \
  siderita/qml/components/folder/OperationsListRow.qml \
  siderita/qml/components/folder/OperationsDock.qml \
  siderita/qml/components/folder/OperationRing.qml \
  siderita/qml/components/folder/FolderBottomStatus.qml \
  siderita/qml/components/folder/FolderBottomChrome.qml \
  siderita/qml/components/folder/FolderActions.qml \
  siderita/qml/components/folder/FolderHeading.qml \
  siderita/qml/views/FolderView.qml \
  siderita/src/controller.rs siderita/src/controller/notices.rs \
  siderita/src/controller/mounts.rs siderita/src/controller/trash.rs \
  siderita/src/controller/scan.rs siderita/src/controller/archive.rs \
  siderita/src/controller/fileops.rs siderita/src/controller/navigation.rs \
  siderita/src/controller/shell.rs siderita/src/controller/jobs.rs \
  siderita/tests/qml/tst_bottom_capsule.qml \
  siderita/tests/qml/tst_activity_notice.qml \
  siderita/tests/qml/tst_activity_stack.qml \
  siderita/tests/qml/tst_operations_dock.qml
python3 scripts/check-staged-units.py \
  siderita/docs/inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-A.numstat.tsv
```

The moved `SID-A4` plan appears under its new path only; `git mv` already staged the deletion of the old one, and the generator's `git add` leaves that staged. Set the `SID-B1-A` row to `done` with the printed diffstat and the evidence link before the final recompute. Stage only this unit's paths by name — another session is working in `hematita/`; never `git stash`, never `git add -A`.

```bash
git commit -m "siderita: Add the transient notice column and compress the bottom bar

The bottom strip repeated what the ring above it already said, because
status_text carried a running job, an activity with no knowable end, an
outcome and two standing facts through one property. Each goes where it
belongs: rings keep the jobs, a self-retiring notice takes the rest, the
heading keeps what is simply true of the folder. Unmounting a disk says so
and stops saying so; it never could before. The bar those four pills took is
now one capsule of three icons on the left.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 10: SID-B1-Z — implementation exit and 1.6.0

**Files:**
- Modify: `siderita/Cargo.toml`, `siderita/Cargo.lock`, `siderita/STATUS.md`, `siderita/ROADMAP.md`, `docs/version-history.tsv`
- Move: `siderita/docs/plans/active/2026-09-22-bottom-chrome-and-notices.md` → `siderita/docs/plans/archive/…`
- Create: `siderita/docs/inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-Z.numstat.tsv`

- [ ] **Step 1: Bump the version**

`1.5.12` → `1.6.0`: this adds a surface, so it is a milestone and the minor increment applies (`version_policy.milestone_increment = "minor"` in `docs/projects.toml`). Update `siderita/Cargo.toml` and let the lock follow.

- [ ] **Step 2: Append the version history row**

`docs/version-history.tsv`, one tab-separated row:

```
siderita	1.6.0	milestone	SID-B1-Z	Add the transient notice column and compress the bottom bar
```

The summary after the colon in the commit subject must match this cell **literally** — `commit-msg` compares them.

- [ ] **Step 3: Close the documents**

`siderita/ROADMAP.md`: `Active implementation checkpoint: none`, `SID-B1` marked `done`. `siderita/STATUS.md`: a `Delivered as 1.6.0` entry naming what changed and linking the evidence. Archive the plan with `git mv` and set its `Status:` to `done`, updating both `README.md` files.

- [ ] **Step 4: The second and last build**

```bash
cd /home/toni/CODIGO/CELESTINA/siderita && scripts/complete-production.sh
```

Expected: success, and the author's `~/.local` binary updated to `1.6.0` without a further recompilation.

- [ ] **Step 5: Recompute the inventory and commit**

```bash
cd /home/toni/CODIGO/CELESTINA
python3 "$SCRATCH/mkinv.py" SID-B1-Z \
  siderita/docs/inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-Z.numstat.tsv \
  siderita/docs/plans/archive/2026-09-22-bottom-chrome-and-notices.md \
  siderita/Cargo.toml siderita/Cargo.lock \
  siderita/ROADMAP.md siderita/STATUS.md \
  siderita/docs/plans/active/README.md siderita/docs/plans/archive/README.md \
  docs/version-history.tsv
python3 scripts/check-staged-units.py \
  siderita/docs/inventories/2026-09-22-bottom-chrome-and-notices/SID-B1-Z.numstat.tsv
git commit -m "siderita-milestone: Add the transient notice column and compress the bottom bar

Implementation exit for SID-B1 as 1.6.0.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

- [ ] **Step 6: Hand `VAL-SID-15` to the author**

Tell the author that `1.6.0` is installed and that `VAL-SID-15` is waiting: the 500 ms threshold against a fast unmount, two lines against a real permission error on a long path, and whether losing the permanent sort arrow is acceptable in daily use. A failure there opens a new corrective unit; it never rewrites this one.

---

## Self-review

**Spec coverage.** §3.1 the one column → Task 6. §3.2 the notice, its threshold, its two tenses, its single exit, press-to-dismiss, the fixed top-anchored column → Tasks 5 and 6. §3.3 the dock's overflow → Task 7. §3.4 the capsule, the middle icon as the view mode, the merged menu, the `BusyIndicator` becoming a notice → Tasks 3, 4 and 8. §3.5 `N de M` deleted and the watch warning moved → Tasks 8 and 9. §4 the controller contract and every call site → Task 8. §5 the file list → Tasks 2–9. §6.1 the picker excluded → Task 1's ledger. §6.2 responsive → Tasks 5 (`maxWidth`, the danger exception) and 7 (`availableWidth`). §7 verification → Tasks 2–9 plus `VAL-SID-15` in Task 1.

**Ordering.** Task 6's tests pass only after Task 7 adds `availableWidth`; both land in the same commit and this is the only ordering constraint between tasks.

**Names used across tasks.** `HiddenToggleDefs.glyph/name` (2 → 4). `ViewSortMenu` with `controller` and `panel` (3 → 4, 9). `BottomControls` aliases `icons`, `hiddenButton`, `viewSortButton`, `sizeButton`, `sizePopup` (4 → its test). `ActivityNotice` `noticeId/text/iconName/danger/running/maxWidth/shown/retire()/dismissed(id)` (5 → 6). `ActivityStack` `controller/backdrop/maxNoticeWidth` and aliases `dock`, `errorNotice`, `noticeRepeater` (6 → 9 and its test). `OperationsDock.availableWidth`, `collapsed`, `expanded`, `countCircle`, `jobList` (7 → 6 and its test). `OperationRing.countLabel` (7). `dismissNotice(id)` with the reserved ids `"error"` and `"op-error"` (8 → 6). `push_notice/settle_notice/drop_notice` and `NoticeTone` (8).
