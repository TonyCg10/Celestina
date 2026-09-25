<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Siderita Folder Usage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Siderita's properties dialog and space-bar quick look show, for a folder, the occupation Hematita's storage analyzer computes: totals, a size-ordered list and a navigable treemap, read-only, with a hand-off to Hematita.

**Architecture:** The pure projection (rows, remainder, tiles, unreadable counts) moves from Hematita's app into `hematita_core::usage::view`; the treemap and usage list move from Hematita's QML into `celestina-style` as `CelestinaTreemap` and `CelestinaUsageList` with a narrower, action-free contract; Siderita links `hematita-core` (as it links `grafita-core` and `fluorita-core`), owns a new `SideritaUsage` QObject that scans on a thread and reprojects in memory, and composes the shared controls in one `FolderUsage.qml` both modals instantiate. Hematita accepts a folder argument so Siderita can hand off.

**Tech Stack:** Rust 2021 (`celestina-core::CancellationToken`, `hematita-core`), CXX-Qt 0.9.1, Qt 6.9 QML, `celestina-style` tokens, `qmltestrunner`, `gtk-launch`.

**Spec:** [docs/superpowers/specs/2026-09-25-siderita-folder-usage-design.md](../specs/2026-09-25-siderita-folder-usage-design.md)

## Global Constraints

- **Three local plans, not a suite plan.** The root roadmap already names `PRD-1` as its one active checkpoint, and the change policy says several projects require several local plans unless one suite unit owns a cross-suite invariant. So: `celestina-style` extends its active `STYLE-G7` plan with units `STYLE-G7-G`; Hematita opens checkpoint `S3` with plan `hematita/docs/plans/active/2026-09-25-s3-shared-usage.md`; Siderita opens checkpoint `SID-U1` with plan `siderita/docs/plans/active/2026-09-25-folder-usage.md`. One `suite-maintenance` commit carries the architecture standard line and the registry change (Task 0). This supersedes §9 of the spec, which is corrected in Task 0.
- **Never touch or read `celestina/` or `celestina-rs/crates/celestina-shell-core`** (author's standby order).
- **Builds only where a task changes a deployable app:** Task 1 runs `celestina-style`'s verify (a small CMake module build); Task 2 runs Hematita's `build-production.sh` + `verify-production.sh` once; Task 3 runs Siderita's pair once; Task 4 runs each app's `complete-production.sh` once. No `cargo` under `hematita/` or `siderita/` outside those. `cd celestina-rs && cargo test -p hematita-core` is cheap and allowed at any time.
- **Rust:** no `unsafe`; no production `unwrap`/`expect`/`panic!`; no `#[allow]`, TODO, FIXME; typed errors keep source; no IO on the Qt thread; workers named, cancellable, results guarded by generation; every domain feature tested in its unit.
- **QML:** colours, radii, typography, motion only from `CelestinaTheme`; `required property` and signals, no parent-id reach, no `x: x`; every new file registered in `build.rs` (apps) or `qmldir` + `CMakeLists.txt` (style); qmllint at 0 with no suppressions; Spanish only inside `qsTr()` (both apps' new copy uses `qsTr()`; Siderita's legacy bare strings stay as they are); `helpText` is only an accessible name, never a floating label (author's rule: no tooltips).
- **Language:** English everywhere else (identifiers, comments, docs, commit subjects). Commit subjects `<prefix>-<kind>: <Imperative>` with a recognized verb (`Add`, `Fix`, `Publish`, `Move`, `Record`; not `Bind`, `Release`, `Book`). Trailer `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- **Units and inventories:** each unit is one commit with an immutable inventory made by `python3 /tmp/claude-1000/-home-toni-CODIGO-CELESTINA/0baf873c-48ee-460a-bacd-945bb3d1687d/scratchpad/mkinv.py UNIT INVENTORY PLAN PATH...` (the plan row must already read `| done |` with the inventory link first; a deleted path is written by hand as `0<TAB><lines><TAB>deleted<TAB>path`; a renamed plan is a deleted row plus an added row). Verify with `python3 scripts/check-staged-units.py INVENTORY` after `git add` of exactly the unit's paths. Never `git stash`. Never commit without the author's word; leave units staged and report.
- **Other sessions:** another session has been committing under `siderita/` (`d666e89`, `17b801f`). If `git status` shows dirty files that are not this unit's, do not touch, revert or include them; stop and report.
- **Versions at closure:** `celestina-style` 1.8.7 → 1.9.0 (`milestone`, in Task 1); Hematita 1.1.1 → 1.2.0 (`milestone`, Task 4); Siderita 1.6.1 → 1.7.0 (`milestone`, Task 4). Bumps via `python3 scripts/version_tool.py bump <owner> milestone --unit <UNIT> --summary "<subject after the colon>"`.

---

## File structure

| Path | Responsibility |
|---|---|
| `docs/standards/architecture.md` | one new sentence: Siderita may consume `hematita-core::usage` as a domain |
| `docs/projects.toml` | `celestina-rs/crates/hematita-core` added to Siderita's `production_inputs` |
| `celestina-rs/crates/hematita-core/src/usage/view.rs` (new) | pure projection: `Row`, `children_rows`, `flat_rects`, `unreadable_below`, `REMAINDER_ID`, `MAX_ROWS` |
| `celestina-rs/crates/hematita-core/src/usage/mod.rs` | `pub mod view;` |
| `celestina-style/CelestinaTreemap.qml` (new) | tiles as buttons; arrows, Enter, Backspace; `entered`/`chosen`/`upRequested`; optional `markedIds`/`dimmedIds` |
| `celestina-style/CelestinaUsageList.qml` (new) | rows with share bars; same signals; `reset`/`keepViewport`/`follow`/`takeFocus`/`currentId` |
| `celestina-style/qmldir`, `CMakeLists.txt`, `DESIGN.md`, `tests/tst_treemap.qml`, `tests/tst_usagelist.qml` | registration, contract, tests |
| `hematita/src/analysis_view.rs`, `analysis_session.rs` | use the crate's `view`; local copies removed |
| `hematita/qml/components/StoragePage.qml` | consumes `CelestinaUsageList`/`CelestinaTreemap`; Space/Delete handled by the page |
| `hematita/qml/CelestinaTreemap.qml`, `hematita/qml/CelestinaUsageList.qml` (symlinks) | canonical consumption path |
| `hematita/qml/components/Treemap.qml`, `UsageList.qml` | deleted |
| `hematita/src/analysis.rs` | `open_path(path)` invokable: browse a folder handed on the command line |
| `hematita/src/activation.rs`, `src/main.rs`, `qml/Main.qml`, `org.celestina.Hematita.desktop` | folder argument, `Open(path)` hand-off, `Exec=hematita %f` |
| `siderita/src/usage_session.rs` (new) | `UsageSession`: plain state (tree, current, generation, crumbs) and projection; unit-tested |
| `siderita/src/usage.rs` (new) | `SideritaUsage` QObject: scan thread, progress, publication |
| `siderita/src/properties.rs`, `src/controller/selection.rs`, `src/controller/state.rs` | `directory_size` and its worker removed; `prop_size` reads from the hub |
| `siderita/qml/dialogs/FolderUsage.qml` (new) | the occupation section both modals instantiate |
| `siderita/qml/dialogs/PropertiesDialog.qml`, `QuickLookView.qml`, `siderita/qml/components/folder/FolderActions.qml` | composition; one `SideritaUsage` per folder view |
| `siderita/qml/CelestinaTreemap.qml`, `siderita/qml/CelestinaUsageList.qml` (symlinks), `siderita/build.rs`, `siderita/src/main.rs`, `siderita/Cargo.toml` | registration and dependency |
| `siderita/tests/qml/tst_folder_usage.qml` (new) | composition test with a stub hub |

---

### Task 0: Authorize the dependency — `suite-maintenance`

**Files:**
- Modify: `docs/standards/architecture.md` (the paragraph after the table, line ~17)
- Modify: `docs/projects.toml` (siderita `production_inputs`, line ~182)
- Modify: `docs/superpowers/specs/2026-09-25-siderita-folder-usage-design.md` §9

**Interfaces:** none; documents only.

- [ ] **Step 1: Add the standard's sentence.** After the sentence "Integrated consumers share narrow domain and seams while retaining their own Qt state and QML composition." add:

```markdown
Siderita is such a consumer of `grafita-core`, `fluorita-core` and
`hematita-core::usage` (the folder scan, tree and layout only; never
`usage::remove`, `hematita/src` or Hematita's QML).
```

- [ ] **Step 2: Register the input.** In `docs/projects.toml`, in the `[projects.siderita]` (or `[[projects]]` with `id = "siderita"`) entry's `production_inputs` list, append `"celestina-rs/crates/hematita-core"` after the last `celestina-rs/crates/...` entry, keeping the list on one line as it is today.

- [ ] **Step 3: Correct the spec's §9.** Replace the paragraph of §9 with:

```markdown
Three local plans, in order: `celestina-style` extends its active `STYLE-G7`
plan with the two controls (`STYLE-G7-G`, milestone 1.9.0); Hematita opens
`S3` (crate view, consumption of the shared controls, the folder argument;
`S3-A`, then `S3-Z` at 1.2.0); Siderita opens `SID-U1` (hub, session,
modals; `SID-U1-A`, then `SID-U1-Z` at 1.7.0). One `suite-maintenance`
commit carries the architecture standard's sentence and the registry input.
The root roadmap keeps `PRD-1` as its one active checkpoint, so no suite plan
is opened. Builds only where a unit changes a deployable app.
```

- [ ] **Step 4: Guards and stage.**

```bash
bash scripts/check-architecture-contract.sh && bash scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py && git add docs/standards/architecture.md docs/projects.toml docs/superpowers/specs/2026-09-25-siderita-folder-usage-design.md
```

Commit subject when authorized: `suite-maintenance: Allow Siderita to consume the storage domain and record the three local plans`.

---

### Task 1: The shared controls — `STYLE-G7-G` (`celestina-style:`)

**Files:**
- Create: `celestina-style/CelestinaTreemap.qml`, `celestina-style/CelestinaUsageList.qml`
- Create: `celestina-style/tests/tst_treemap.qml`, `celestina-style/tests/tst_usagelist.qml`
- Modify: `celestina-style/qmldir`, `celestina-style/CMakeLists.txt` (`QML_FILES`, and the `project(... VERSION ...)` line through `version_tool.py`), `celestina-style/DESIGN.md` §6.1, `celestina-style/STATUS.md`, `celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md` (unit row `STYLE-G7-G`), `docs/version-history.tsv`
- Create: `celestina-style/docs/evidence/2026-09-25-usage-controls.md`, `celestina-style/docs/inventories/2026-08-04-shared-reading-controls/STYLE-G7-G.numstat.tsv`

**Interfaces:**
- Produces `CelestinaTreemap`:
  - `required property var tiles` — array of `{ id: int, x, y, w, h: real (0..1), name: string, tone: string }`; `id < 0` is the merged remainder, drawn disabled and named `qsTr("otros")`.
  - `required property var toneColors` — object tone → color.
  - `required property int currentId`, `required property string currentName`.
  - `property var markedIds: []`, `property var dimmedIds: []` — ids drawn selected / at `CelestinaTheme.unavailableContentOpacity`; `-1` in `dimmedIds` dims the remainder.
  - `signal entered(int id)` (double click / Enter), `signal chosen(int id)` (click / arrow focus), `signal upRequested()` (Backspace).
  - Keys handled: Left/Up/Right/Down, Return/Enter, Backspace. **Space and Delete are not accepted** and bubble to the host.
- Produces `CelestinaUsageList`:
  - `required property var usageRows` — array of `{ id, name, kind: "dir"|"file"|"other", tone, share (0..1), size: string, percent: string, detail: string }`; `detail` may be `""`.
  - `required property var toneColors`; `property var markedIds: []`; `property string emptyText: qsTr("Carpeta vacía")`.
  - signals `entered(int id)`, `chosen(int id)`, `upRequested()`; functions `reset(apply)`, `keepViewport(apply)`, `follow(id)`, `takeFocus()`, `currentId()`.
  - Keys: Up/Down (ListView), Return/Enter, Backspace. Space and Delete bubble.

- [ ] **Step 1: Write `celestina-style/CelestinaTreemap.qml`.** Start from `hematita/qml/components/Treemap.qml` (read it whole) and apply exactly these changes:
  - imports: `pragma ComponentBehavior: Bound`, `import QtQuick`, `import QtQuick.Controls` — no `org.celestina.*` import (the module is implicit, as in `CelestinaLineGutter.qml`).
  - header comment in English describing the contract above.
  - replace `required property var selectedIds` by `property var markedIds: []` and add `property var dimmedIds: []`.
  - delete `signal toggled(int id)` and `signal trashRequested()`; delete the `Qt.Key_Space` and `Qt.Key_Delete` cases from `Keys.onPressed` (so they fall to `default: return`, unaccepted).
  - `marked: map.markedIds.indexOf(tile.tileData.id) >= 0`; `opacity: map.dimmedIds.indexOf(tile.tileData.id) >= 0 ? CelestinaTheme.unavailableContentOpacity : 1` (replaces `tileData.matches`).
  - `Accessible.name` of a tile: `tile.remainder ? qsTr("otros") : qsTr("%1, carpeta").arg(tile.tileData.name)` when `tileData.kind === "dir"` else `qsTr("%1, archivo").arg(name)`; add `kind` to the tile object contract (`tone` stays for colour). Add `Accessible.description: tile.tileData.name`.
  - map `Accessible.name: qsTr("Mapa de ocupación de %1").arg(map.currentName)`.
  - Everything else (order, `step`, geometry, focus ring, tokens) unchanged.

- [ ] **Step 2: Write `celestina-style/CelestinaUsageList.qml`.** Start from `hematita/qml/components/UsageList.qml` and apply:
  - same import rule; English header.
  - `required property var selectedIds` → `property var markedIds: []`; delete `required property bool filtered`; add `property string emptyText: qsTr("Carpeta vacía")`.
  - delete `toggled`, `trashRequested`, `Keys.onSpacePressed`, and the `Qt.Key_Delete` branch (keep Backspace).
  - row object: `copies` → `detail: string`; the caption text becomes `row.rowData.detail.length > 0 ? row.rowData.percent + " · " + row.rowData.detail : row.rowData.percent`; `emptyRow` gains `detail: ""` and loses `copies`.
  - `Accessible.name` of a row: `qsTr("%1, %2, %3, %4").arg(name).arg(size).arg(kind === "dir" ? qsTr("carpeta") : qsTr("archivo")).arg(percent)`.
  - list `Accessible.name: qsTr("Contenido por tamaño")`.
  - the empty text: `text: card.emptyText`.
  - `marked: card.markedIds.indexOf(row.rowData.id) >= 0`.

- [ ] **Step 3: Register.** `qmldir`: add `CelestinaTreemap 1.0 CelestinaTreemap.qml` and `CelestinaUsageList 1.0 CelestinaUsageList.qml` after `CelestinaLineGutter`. `CMakeLists.txt` `QML_FILES`: add both after `CelestinaLineGutter.qml`.

- [ ] **Step 4: Tests.** Create `celestina-style/tests/tst_treemap.qml` following `tests/tst_menuitem.qml`'s shape (`TestCase`, a `Window`, `when: testWindow.visible`, `CelestinaTheme.reducedMotion = true` in `init()`):

```qml
import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// Three tiles in reading order; the arrows walk them, Enter enters, Backspace
// asks to go up, Space is left to the host, the remainder is disabled.
TestCase {
    id: testCase
    name: "CelestinaTreemap"
    when: testWindow.visible

    property var chosenIds: []
    property var enteredIds: []
    property int ups: 0
    property int spaces: 0

    Window {
        id: testWindow
        width: 400
        height: 300
        visible: true

        Item {
            anchors.fill: parent
            Keys.onSpacePressed: testCase.spaces++
            CelestinaTreemap {
                id: map
                anchors.fill: parent
                tiles: [
                    { id: 10, x: 0, y: 0, w: 0.5, h: 1, name: "a", kind: "dir", tone: "dir" },
                    { id: 11, x: 0.5, y: 0, w: 0.5, h: 0.5, name: "b", kind: "file", tone: "file" },
                    { id: -1, x: 0.5, y: 0.5, w: 0.5, h: 0.5, name: "", kind: "other", tone: "other" }
                ]
                toneColors: ({ dir: CelestinaTheme.glyphAccentBlue, file: CelestinaTheme.glyphAccentViolet, other: CelestinaTheme.textFaint })
                currentId: 10
                currentName: "root"
                onChosen: function(id) { testCase.chosenIds.push(id) }
                onEntered: function(id) { testCase.enteredIds.push(id) }
                onUpRequested: testCase.ups++
            }
        }
    }

    function init() {
        CelestinaTheme.reducedMotion = true
        testCase.chosenIds = []
        testCase.enteredIds = []
        testCase.ups = 0
        testCase.spaces = 0
        map.contentItem.forceActiveFocus()
    }

    function test_arrows_walk_reading_order() {
        keyClick(Qt.Key_Right)
        compare(testCase.chosenIds, [11])
    }

    function test_enter_enters_and_backspace_goes_up() {
        keyClick(Qt.Key_Return)
        compare(testCase.enteredIds, [10])
        keyClick(Qt.Key_Backspace)
        compare(testCase.ups, 1)
    }

    function test_space_reaches_the_host() {
        keyClick(Qt.Key_Space)
        compare(testCase.spaces, 1)
    }

    function test_remainder_is_named_and_disabled() {
        const remainder = findChild(map, "")   // replaced below
    }
}
```

  Replace the last test by walking `map.contentItem.children` for the `AbstractButton` whose `tileData.id < 0` and asserting `enabled === false` and `Accessible.name === qsTr("otros")` (in a test, compare against the literal `"otros"`). Write `tests/tst_usagelist.qml` the same way with three rows (`id` 10 dir, 11 file, 12 file), asserting: `Key_Down` from the first row emits `chosen(11)`; `Key_Return` emits `entered(11)`; `Key_Backspace` increments `ups`; `Key_Space` reaches the host; an empty `usageRows` shows `emptyText`.

- [ ] **Step 5: Wire the tests.** In `celestina-style/CMakeLists.txt`, the `BUILD_TESTING` block runs one QuickTest executable over `tests/`; confirm (read the block, lines ~200-220) that it runs every `tst_*.qml` in `QUICK_TEST_SOURCE_DIR`. If it does, nothing to add. If the block names files, add both.

- [ ] **Step 6: DESIGN.md §6.1** — add two rows/entries for `CelestinaTreemap` and `CelestinaUsageList` in the same format as `CelestinaLineGutter`'s, stating: anatomy from tokens (`radiusSm`, `spaceXs`, `accentSoftOpacity`, `surfaceSelected`, `contentHover`, `rowHeight`, `badgeFill`, `unavailableContentOpacity`), one Tab stop each, keys handled and keys left to the host, tone colours supplied by the consumer, no selection or action semantics.

- [ ] **Step 7: Verify.**

```bash
bash celestina-style/scripts/verify-production.sh
```

Expected: qmllint of the module at 0, `ctest` green including the two new tests, `check-style-contract.sh` OK. If the verify says the production inputs changed, run `bash celestina-style/scripts/build-production.sh` first, then verify again.

- [ ] **Step 8: Version, ledger, evidence, inventory.**

```bash
python3 scripts/version_tool.py bump celestina-style milestone --unit STYLE-G7-G --summary "Publish the shared treemap and usage list"
```

  Add row `STYLE-G7-G` to the active style plan's ledger (status `done`, inventory link first in Files/areas, diffstat filled by `mkinv.py`, evidence link, `VAL-STYLE-04`). Add a "Build order" item 8 naming the two controls and their second consumer (Siderita). STATUS.md: `Delivered as 1.9.0: STYLE-G7-G` paragraph. Evidence `2026-09-25-usage-controls.md`: the contract, what changed from Hematita's originals, the tests, the verify tail. Then:

```bash
python3 /tmp/claude-1000/-home-toni-CODIGO-CELESTINA/0baf873c-48ee-460a-bacd-945bb3d1687d/scratchpad/mkinv.py STYLE-G7-G celestina-style/docs/inventories/2026-08-04-shared-reading-controls/STYLE-G7-G.numstat.tsv celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md celestina-style/CelestinaTreemap.qml celestina-style/CelestinaUsageList.qml celestina-style/tests/tst_treemap.qml celestina-style/tests/tst_usagelist.qml celestina-style/qmldir celestina-style/CMakeLists.txt celestina-style/DESIGN.md celestina-style/STATUS.md celestina-style/docs/evidence/2026-09-25-usage-controls.md docs/version-history.tsv
python3 scripts/check-staged-units.py celestina-style/docs/inventories/2026-08-04-shared-reading-controls/STYLE-G7-G.numstat.tsv
```

Commit subject when authorized: `celestina-style-milestone: Publish the shared treemap and usage list`.

---

### Task 2: Hematita consumes the crate view and the shared controls, and accepts a folder — `S3-A` (`hematita:`)

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/usage/view.rs`
- Modify: `celestina-rs/crates/hematita-core/src/usage/mod.rs`
- Modify: `hematita/src/analysis_view.rs` (remove `REMAINDER_ID`, `unreadable_below`, `flat_rects` and their tests), `hematita/src/analysis_session.rs` (`view()` uses `hematita_core::usage::view`), any other caller found by `rg -n "flat_rects|unreadable_below|REMAINDER_ID" hematita/src`
- Delete: `hematita/qml/components/Treemap.qml`, `hematita/qml/components/UsageList.qml`
- Create symlinks: `hematita/qml/CelestinaTreemap.qml -> ../../celestina-style/CelestinaTreemap.qml`, `hematita/qml/CelestinaUsageList.qml -> ../../celestina-style/CelestinaUsageList.qml`
- Modify: `hematita/build.rs` (`QML_FILES`), `hematita/qml/components/StoragePage.qml`
- Modify: `hematita/src/analysis.rs` (`open_path`), `hematita/src/activation.rs` (`Open(path)`), `hematita/src/main.rs` (argument), `hematita/qml/Main.qml`, `hematita/org.celestina.Hematita.desktop`
- Create: `hematita/docs/plans/active/2026-09-25-s3-shared-usage.md`, `hematita/docs/evidence/2026-09-25-s3-shared-usage.md`, `hematita/docs/inventories/2026-09-25-s3-shared-usage/S3-A.numstat.tsv`
- Modify: `hematita/ROADMAP.md` (status `active`, checkpoint `S3`, a `## S3 — Shared usage view and the folder argument` section), `hematita/STATUS.md`, `hematita/docs/plans/active/README.md`, `hematita/VALIDATION.md` (`VAL-S3`)

**Interfaces:**
- Produces `hematita_core::usage::view`:

```rust
pub const REMAINDER_ID: f64 = -1.0;
/// Children listed before the rest merge into one remainder row.
pub const MAX_ROWS: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// `None` is the merged remainder.
    pub id: Option<NodeId>,
    pub name: OsString,
    pub kind: Kind,
    pub allocated: u64,
    pub apparent: u64,
    /// `allocated` over the parent's `allocated`; 0 when the parent is empty.
    pub share: f64,
    pub files_below: u64,
    pub unreadable_below: u32,
    pub other_device: bool,
    /// Children merged into this row; 0 for a real child.
    pub merged: usize,
}

/// `current`'s children biggest first; beyond `limit` rows the rest become
/// one remainder row that keeps their sums. `unreadable` is
/// [`unreadable_below`]'s output, indexed by node.
pub fn children_rows(tree: &Tree, current: NodeId, unreadable: &[u32], limit: usize) -> Vec<Row>;

/// The treemap as QML reads it: `[id, x, y, w, h]` per tile; a `None` id and
/// squarify's own remainder both carry [`REMAINDER_ID`].
pub fn flat_rects(ids: &[Option<NodeId>], tiles: &[Tile]) -> Vec<f64>;

/// Per node: how many unreadable directories sit at or below it.
pub fn unreadable_below(tree: &Tree) -> Vec<u32>;
```

- Produces on `HematitaAnalysis`: `#[qinvokable] fn open_path(self: Pin<&mut HematitaAnalysis>, path: &QString)` — browses `path` (stack = `[path]`, mode `browsing`); ignored with a stderr diagnostic when the path is not a readable directory.
- Produces on the D-Bus interface `org.celestina.Hematita`: method `Open(s path)` → the window raises and browses the path. `activation::hand_off(path: Option<&str>) -> bool`.
- Produces `Main.qml` initial property `startPath: string` (empty = none).
- Consumes Task 1's controls.

- [ ] **Step 1: Write the crate tests first** in `view.rs`'s `#[cfg(test)]` module. Move the existing tests of `flat_rects` and `unreadable_below` from `hematita/src/analysis_view.rs` (they build a `Tree` by hand with `Node { .. }` literals; keep that helper) and add:

```rust
#[test]
fn children_rows_orders_by_size_and_merges_the_rest() {
    // root(0) with four children: 40, 30, 20, 10 bytes allocated.
    let tree = four_children();
    let unreadable = unreadable_below(&tree);
    let rows = children_rows(&tree, tree.root, &unreadable, 3);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].allocated, 40);
    assert_eq!(rows[1].allocated, 30);
    let rest = &rows[2];
    assert_eq!(rest.id, None);
    assert_eq!(rest.allocated, 30); // 20 + 10
    assert_eq!(rest.merged, 2);
    assert!((rows.iter().map(|r| r.share).sum::<f64>() - 1.0).abs() < 1e-9);
}

#[test]
fn children_rows_under_the_limit_have_no_remainder() {
    let tree = four_children();
    let rows = children_rows(&tree, tree.root, &unreadable_below(&tree), 40);
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().all(|r| r.id.is_some() && r.merged == 0));
}

#[test]
fn children_rows_of_an_empty_folder_have_zero_shares() {
    let tree = leaf_only();
    let rows = children_rows(&tree, tree.root, &unreadable_below(&tree), 40);
    assert!(rows.is_empty());
}

#[test]
fn flat_rects_names_a_none_id_as_the_remainder() {
    let ids = [Some(NodeId(3)), None];
    let tiles = squarify(&[70, 30], Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 });
    let flat = flat_rects(&ids, &tiles);
    assert_eq!(flat[0], 3.0);
    assert_eq!(flat[5], REMAINDER_ID);
}
```

- [ ] **Step 2: Run them to see them fail.** `cd celestina-rs && cargo test -p hematita-core view` → compile error (module missing).

- [ ] **Step 3: Implement `view.rs`.** `unreadable_below` and `flat_rects` are the Hematita functions moved verbatim, `flat_rects` taking `&[Option<NodeId>]` (`Some(id)` → `f64::from(id.0)`, `None` → `REMAINDER_ID`, and a tile whose `index` is `None` → `REMAINDER_ID`). `children_rows`:

```rust
pub fn children_rows(tree: &Tree, current: NodeId, unreadable: &[u32], limit: usize) -> Vec<Row> {
    let Some(parent) = tree.node(current) else { return Vec::new(); };
    let total = parent.allocated;
    let share = |allocated: u64| if total > 0 { allocated as f64 / total as f64 } else { 0.0 };
    let children = tree.children_by_size(current);
    let (listed, rest) = if children.len() > limit && limit > 0 {
        children.split_at(limit - 1)
    } else {
        (children.as_slice(), &[][..])
    };
    let mut rows: Vec<Row> = listed
        .iter()
        .filter_map(|id| tree.node(*id).map(|node| Row {
            id: Some(*id),
            name: node.name.clone(),
            kind: node.kind,
            allocated: node.allocated,
            apparent: node.apparent,
            share: share(node.allocated),
            files_below: node.files_below,
            unreadable_below: unreadable.get(id.0 as usize).copied().unwrap_or(0),
            other_device: node.other_device,
            merged: 0,
        }))
        .collect();
    if !rest.is_empty() {
        let mut remainder = Row { id: None, name: OsString::new(), kind: Kind::Other, allocated: 0, apparent: 0, share: 0.0, files_below: 0, unreadable_below: 0, other_device: false, merged: rest.len() };
        for id in rest {
            if let Some(node) = tree.node(*id) {
                remainder.allocated = remainder.allocated.saturating_add(node.allocated);
                remainder.apparent = remainder.apparent.saturating_add(node.apparent);
                remainder.files_below = remainder.files_below.saturating_add(node.files_below);
                remainder.unreadable_below = remainder.unreadable_below.saturating_add(unreadable.get(id.0 as usize).copied().unwrap_or(0));
            }
        }
        remainder.share = share(remainder.allocated);
        rows.push(remainder);
    }
    rows
}
```

  Add `pub mod view;` to `usage/mod.rs` and a sentence in its doc. `cargo test -p hematita-core` green; `cargo clippy -p hematita-core --all-targets -- -D warnings` clean.

- [ ] **Step 4: Hematita uses the crate view.** In `analysis_view.rs` delete `REMAINDER_ID`, `unreadable_below`, `flat_rects` and their tests (moved in Step 1); import `hematita_core::usage::view::{flat_rects, unreadable_below}` where they were used (`findings()` and any other site). In `analysis_session.rs` `view()`:

```rust
let ids: Vec<Option<NodeId>> = children.iter().map(|id| Some(*id)).collect();
view.rects = hematita_core::usage::view::flat_rects(&ids, &squarify(&sizes, unit));
```

  `rg -n "flat_rects|unreadable_below|REMAINDER_ID" hematita/src` must show only crate-qualified uses.

- [ ] **Step 5: The shared controls in Hematita.** Delete `hematita/qml/components/Treemap.qml` and `UsageList.qml` (`git rm`). Create the two symlinks (relative, like the existing ones: `ln -s ../../celestina-style/CelestinaTreemap.qml hematita/qml/CelestinaTreemap.qml`). In `build.rs`'s `QML_FILES` remove the two component lines and add `"qml/CelestinaTreemap.qml"`, `"qml/CelestinaUsageList.qml"` in the shared block. In `StoragePage.qml`:
  - `UsageList { ... }` → `CelestinaUsageList { id: usageList; usageRows: page.usageRows; markedIds: page.analysis.selectedIds; toneColors: page.toneColors; emptyText: page.filtered ? qsTr("Nada coincide con el filtro") : qsTr("Carpeta vacía"); onChosen…; onEntered…; onUpRequested… }` — delete `onToggled` and `onTrashRequested` bindings.
  - `Treemap { ... }` → `CelestinaTreemap { tiles: page.tiles; markedIds: page.analysis.selectedIds; dimmedIds: page.dimmedIds; toneColors: …; currentId: …; currentName: …; onChosen…; onEntered…; onUpRequested… }`.
  - In `weaveAnalysed()`, each row gains `detail: copies > 0 ? qsTr("%1 copias").arg(copies) : ""` and each tile gains `kind: row ? row.kind : "other"`; replace the `matches` field by collecting `page.dimmedIds`: a new `property var dimmedIds: []` set to the ids of tiles whose row is undefined plus `-1` when `page.filtered`.
  - Space and Delete: wrap the `StackLayout` holding the list and the one holding the map (or the `RowLayout` above both) in an `Item`/use the existing parent with:

```qml
Keys.onSpacePressed: function(event) {
    if (page.currentId >= 0) { page.analysis.toggleSelected(page.currentId); event.accepted = true }
}
Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Delete && page.canAct) { page.requestTrash(); event.accepted = true }
}
```

  placed on the nearest common ancestor of both controls so the unaccepted key from either child reaches it. Confirm with `rg -n "toggled|trashRequested" hematita/qml` → no matches.

- [ ] **Step 6: The folder argument.**
  - `analysis.rs`: add to the bridge `#[qinvokable] fn open_path(self: Pin<&mut HematitaAnalysis>, path: &QString);` and implement:

```rust
pub fn open_path(mut self: Pin<&mut Self>, path: &QString) {
    let path = PathBuf::from(path.to_string());
    if !path.is_dir() {
        eprintln!("hematita: not a folder, ignored: {}", path.display());
        return;
    }
    self.as_mut().rust_mut().stack = vec![path];
    self.browse();
}
```

  (`is_dir` is one `stat` on the Qt thread at startup, the same cost `browse` pays for its listing; acceptable and said in the doc comment.)
  - `activation.rs`: the `Activation` interface gains `fn open(&self, path: String)` that queues `activation.open_requested(QString::from(path.as_str()))`, a new `#[qsignal] fn open_requested(self: Pin<&mut HematitaActivation>, path: QString)`. `hand_off(path: Option<&str>) -> bool`: with `Some(path)` call `"Open"` with `&(path,)`, otherwise `"Activate"` as today.
  - `main.rs`: `let start_path = std::env::args().nth(1).unwrap_or_default();` before `hand_off`; pass `hand_off(Some(&start_path).filter(|p| !p.is_empty()))`; add initial property `startPath` (`QVariant::from(&QString::from(start_path.as_str()))`).
  - `Main.qml`: `required property string startPath`; in `HematitaActivation`, `onOpenRequested: function(path) { window.currentSection = 5; analysisHub.openPath(path); window.show(); window.raise(); window.requestActivate() }`; in `Component.onCompleted`, after the hubs start: `if (window.startPath.length > 0) { window.currentSection = 5; analysisHub.openPath(window.startPath) }`. (Section index 5 is the storage page, as `printStorageShape` already assumes.)
  - `.desktop`: `Exec=hematita %f`.

- [ ] **Step 7: Tests in the app** (`hematita/src/analysis.rs` or `analysis_session.rs` test module, run by verify): none can drive a QObject; the argument path is covered by the smoke: extend `hematita/scripts/smoke.sh` so one run passes the scratch home as the argument and greps the storage shape line; read the script first and mirror how it sets `HEMATITA_SMOKE_SECTIONS`. Expected line unchanged: `hematita-storage <n>`; add a second expectation that stderr contains no `not a folder, ignored`.

- [ ] **Step 8: Documents.** Open `S3` in `hematita/ROADMAP.md` (status `active`, checkpoint `S3`, section with hypothesis: "a second consumer proves the projection and the two controls are shared anatomy, and a folder handed on the command line lands in the storage section"), STATUS focus line, `VAL-S3` in VALIDATION.md (launch `hematita ~/Descargas` → the storage section browses it; launch it again while running → the running window raises and browses; the storage page behaves as in `VAL-S1` with the shared controls; Space still toggles and Delete still asks). Plan `2026-09-25-s3-shared-usage.md` mirroring the S2 plan's front matter (Plan ID `s3-shared-usage`, checkpoint `S3`, units `S3-A`, `S3-Z`). Evidence `2026-09-25-s3-shared-usage.md`.

- [ ] **Step 9: The one build.**

```bash
bash hematita/scripts/build-production.sh && bash hematita/scripts/verify-production.sh
```

  Expected: tests green (crate 88+4, app 57), clippy clean, qmllint 0, smoke OK with the storage shape line, manifest verified. Then the three guards and the inventory:

```bash
bash scripts/check-architecture-contract.sh && bash scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py
python3 /tmp/claude-1000/-home-toni-CODIGO-CELESTINA/0baf873c-48ee-460a-bacd-945bb3d1687d/scratchpad/mkinv.py S3-A hematita/docs/inventories/2026-09-25-s3-shared-usage/S3-A.numstat.tsv hematita/docs/plans/active/2026-09-25-s3-shared-usage.md <every changed path, including the two deleted QML files, the two symlinks, the crate files, the desktop entry, ROADMAP, STATUS, VALIDATION, the plan README, the evidence>
python3 scripts/check-staged-units.py hematita/docs/inventories/2026-09-25-s3-shared-usage/S3-A.numstat.tsv
```

Commit subject when authorized: `hematita-maintenance: Move the usage projection into the crate, consume the shared controls and accept a folder argument`.

---

### Task 3: Siderita's hub, session and modals — `SID-U1-A` (`siderita:`)

**Files:**
- Create: `siderita/src/usage_session.rs`, `siderita/src/usage.rs`, `siderita/qml/dialogs/FolderUsage.qml`, `siderita/tests/qml/tst_folder_usage.qml`
- Create symlinks: `siderita/qml/CelestinaTreemap.qml`, `siderita/qml/CelestinaUsageList.qml` → `../../celestina-style/...`
- Modify: `siderita/Cargo.toml`, `siderita/Cargo.lock`, `siderita/build.rs` (`QML_FILES` + `.files([...])`), `siderita/src/main.rs` (`mod usage; mod usage_session;`)
- Modify: `siderita/src/properties.rs` (delete `directory_size` and its test; `gather` unchanged), `siderita/src/controller/selection.rs` (`open_properties` no longer spawns; `prop_size` for a folder is `""`), `siderita/src/controller/state.rs` (delete `prop_size_cancel`), `siderita/src/controller/shell.rs` (`open_in_hematita`), `siderita/src/controller.rs` (declare `open_in_hematita`)
- Modify: `siderita/qml/dialogs/PropertiesDialog.qml`, `siderita/qml/dialogs/QuickLookView.qml`, `siderita/qml/components/folder/FolderActions.qml`
- Create: `siderita/docs/plans/active/2026-09-25-folder-usage.md`, `siderita/docs/evidence/2026-09-25-folder-usage.md`, `siderita/docs/inventories/2026-09-25-folder-usage/SID-U1-A.numstat.tsv`
- Modify: `siderita/ROADMAP.md` (`active`, `SID-U1`, section), `siderita/STATUS.md`, `siderita/docs/plans/active/README.md`, `siderita/VALIDATION.md` (`VAL-SID-U1`)

**Interfaces:**
- Consumes `hematita_core::usage::{walk::{scan, Progress, ScanError}, tree::{Tree, NodeId, Kind}, layout::{squarify, Rect}, view::{children_rows, flat_rects, unreadable_below, MAX_ROWS}, mounts::{parse_mountinfo, mount_targets}}` and `celestina_core::CancellationToken`.
- Produces `UsageSession` (plain, in `usage_session.rs`):

```rust
pub struct UsageSession {
    pub generation: u64,
    pub root: Option<PathBuf>,
    pub tree: Option<Tree>,
    pub current: Option<NodeId>,
    pub unreadable: Vec<u32>,
    pub cancel: Option<CancellationToken>,
}
pub struct Projection {
    pub crumbs: Vec<String>,          // names from the scanned root
    pub current_path: String,
    pub total_bytes: f64, pub files_below: f64, pub folders_below: f64,
    pub unreadable_below: f64, pub other_devices: f64,
    pub row_ids: Vec<f64>, pub row_names: Vec<String>, pub row_kinds: Vec<String>,
    pub row_allocated: Vec<f64>, pub row_shares: Vec<f64>, pub row_files: Vec<f64>,
    pub row_unreadable: Vec<f64>, pub row_merged: Vec<f64>,
    pub rects: Vec<f64>,
}
impl UsageSession {
    /// Bumps the generation, cancels the running scan, forgets the tree unless `root` is the one already scanned.
    pub fn begin(&mut self, root: PathBuf) -> Option<(u64, CancellationToken)>; // None: same root, tree kept
    pub fn accept(&mut self, generation: u64, tree: Tree) -> bool;   // false when stale
    pub fn close(&mut self);
    pub fn enter(&mut self, id: i32) -> bool;   // only a live Dir child of current
    pub fn up(&mut self) -> bool;
    pub fn path_of(&self, id: i32) -> Option<PathBuf>;
    pub fn project(&self) -> Projection;         // rows via children_rows(.., MAX_ROWS), tiles via squarify over the rows' allocated
}
```

- Produces `SideritaUsage` (`usage.rs`, `#[qobject] #[qml_element]` in `org.celestina.siderita`): properties `mode: QString` (`idle|scanning|analysed|failed`), `failure: QString`, `root: QString`, `currentPath: QString`, `crumbs: QStringList`, `progressEntries: f64`, `progressBytes: f64`, `totalBytes: f64`, `filesBelow: f64`, `foldersBelow: f64`, `unreadableBelow: f64`, `otherDevices: f64`, `rowIds/rowAllocated/rowShares/rowFiles/rowUnreadable/rowMerged: QVariant` (lists of doubles), `rowNames/rowKinds: QStringList`, `tiles: QVariant` (flat `[id,x,y,w,h]`), `revision: i32`; invokables `open(path: &QString)`, `close()`, `enter(id: i32)`, `up()`, `pathOf(id: i32) -> QString`; signal `navigateRequested(path: QString)` is not needed — QML calls `controller.openLocation(usage.pathOf(id))` and closes the modal itself.
- Produces `FolderUsage.qml`: `required property var usage`, `required property var controller`, `signal navigated()` (emitted after `controller.openLocation`), `signal hematitaRequested(string path)`.
- Produces `SideritaController::open_in_hematita(path: &QString)` (`shell.rs`): `crate::apps::launch_with("org.celestina.Hematita", &path)`; on `Ok` a notice `Abriendo en Hematita…` via `push_notice`, on `Err` `set_op_error`.

- [ ] **Step 1: Dependency and modules.** `siderita/Cargo.toml` under `fluorita-engine`:

```toml
# The folder occupation the properties dialog and the quick look show: the
# scan, tree and treemap layout live in Hematita's domain crate; this app only
# runs the scan on its own thread and marshals the projection to Qt. Never
# `usage::remove` — Siderita's own trash and delete stay its only removals.
hematita-core = { path = "../celestina-rs/crates/hematita-core" }
```

  `main.rs`: `mod usage;` and `mod usage_session;` in alphabetical place. `build.rs`: `"qml/CelestinaTreemap.qml"`, `"qml/CelestinaUsageList.qml"` in the shared block, `"qml/dialogs/FolderUsage.qml"` after `QuickLookView.qml`, and `"src/usage.rs"` in `.files([...])`. Symlinks as in Task 2.

- [ ] **Step 2: Session tests first** (`usage_session.rs` `#[cfg(test)]`), scanning a real temporary folder with the crate's `scan` so the test needs no Qt:

```rust
fn scanned(dir: &Path) -> Tree {
    let mut progress = |_: Progress| {};
    scan(dir, &HashSet::new(), &CancellationToken::new(), &mut progress).expect("scan a temp dir")
}

#[test]
fn begin_returns_a_token_and_a_later_begin_of_the_same_root_keeps_the_tree() {
    let dir = tempdir();                       // std::env::temp_dir() + unique name, created by the test
    let mut session = UsageSession::default();
    let (generation, _token) = session.begin(dir.clone()).expect("first begin scans");
    assert!(session.accept(generation, scanned(&dir)));
    assert!(session.begin(dir.clone()).is_none());
    assert!(session.tree.is_some());
}

#[test]
fn a_stale_tree_is_refused() {
    let dir = tempdir();
    let mut session = UsageSession::default();
    let (old, _t) = session.begin(dir.clone()).unwrap();
    let (_new, _t2) = session.begin(tempdir()).unwrap();
    assert!(!session.accept(old, scanned(&dir)));
    assert!(session.tree.is_none());
}

#[test]
fn enter_and_up_move_inside_the_tree_without_a_second_scan() {
    // dir/{a/{x: 3000 bytes}, y: 1000 bytes}
    let dir = fixture();
    let mut session = UsageSession::default();
    let (g, _t) = session.begin(dir.clone()).unwrap();
    assert!(session.accept(g, scanned(&dir)));
    let projection = session.project();
    assert_eq!(projection.crumbs.len(), 1);
    let first = projection.row_ids[0] as i32;      // biggest first: `a`
    assert_eq!(projection.row_kinds[0], "dir");
    assert!(session.enter(first));
    assert_eq!(session.project().crumbs, vec![name_of(&dir), "a".to_owned()]);
    assert!(!session.enter(session.project().row_ids[0] as i32)); // `x` is a file
    assert!(session.up());
    assert!(!session.up());                         // at the root
}

#[test]
fn close_forgets_everything() {
    let dir = tempdir();
    let mut session = UsageSession::default();
    let (g, _t) = session.begin(dir.clone()).unwrap();
    assert!(session.accept(g, scanned(&dir)));
    session.close();
    assert!(session.tree.is_none() && session.root.is_none() && session.current.is_none());
}

#[test]
fn totals_count_a_hard_link_once() {
    // dir/{f: 4096 bytes, g: hard link to f}
    let dir = hard_link_fixture();
    let mut session = UsageSession::default();
    let (g, _t) = session.begin(dir.clone()).unwrap();
    assert!(session.accept(g, scanned(&dir)));
    assert_eq!(session.project().files_below, 1.0);
}
```

  (`fixture()` writes the files with `std::fs`; `hard_link_fixture()` uses `std::fs::hard_link`; each test removes its folder at the end.)

- [ ] **Step 3: Implement `usage_session.rs`.** `begin`: if `self.root.as_deref() == Some(&root) && self.tree.is_some()` → `None`; else cancel the old token, `generation += 1`, set `root`, clear `tree`/`current`/`unreadable`, create a token, store and return `(generation, token)`. `accept`: `generation == self.generation` else `false`; set `tree`, `current = Some(tree.root)`, `unreadable = unreadable_below(&tree)`, `cancel = None`. `close`: cancel, clear all, bump generation. `enter(id)`: `u32::try_from(id)` → `NodeId`; must be a child of `current` with `Kind::Dir` and not `other_device`; then `current = Some(id)`. `up`: `current = tree.node(current).parent` when `current != tree.root`. `project`: crumbs = names from `tree.root` to `current` (root's name is the root path's `file_name` or the whole path when none); totals from `tree.node(current)` (`allocated`, `files_below`, folders below = count of `Kind::Dir` descendants — compute by a single pass over `tree.nodes` counting dirs whose ancestor chain reaches `current`; simpler and exact: count dirs among the subtree by walking `children` iteratively), `unreadable[current]`, other devices = descendants with `other_device`; rows = `children_rows(tree, current, &self.unreadable, MAX_ROWS)`; `rects = flat_rects(&rows.iter().map(|r| r.id).collect::<Vec<_>>(), &squarify(&rows.iter().map(|r| r.allocated).collect::<Vec<_>>(), unit))`. `path_of(id)` → `tree.path_of(NodeId)` when live. All lossy name conversion here (`to_string_lossy`).

- [ ] **Step 4: Run** `cd siderita && cargo test usage_session` **is forbidden** (it builds the Qt app). Instead defer running to the one build in Step 10; write the code carefully and keep the tests independent of Qt.

- [ ] **Step 5: Implement `usage.rs`.** Mirror `hematita/src/usage_worker.rs::spawn_scan` and `hematita/src/lists.rs` (`strings`, `doubles` helpers — copy these two small functions into `usage.rs` as private fns; they are marshalling, not domain):

```rust
#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" { include!("cxx-qt-lib/qstring.h"); type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h"); type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qvariant.h"); type QVariant = cxx_qt_lib::QVariant; }
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject] #[qml_element]
        #[qproperty(QString, mode)] #[qproperty(QString, failure)] #[qproperty(QString, root)]
        #[qproperty(QString, current_path)] #[qproperty(QStringList, crumbs)]
        #[qproperty(f64, progress_entries)] #[qproperty(f64, progress_bytes)]
        #[qproperty(f64, total_bytes)] #[qproperty(f64, files_below)] #[qproperty(f64, folders_below)]
        #[qproperty(f64, unreadable_below)] #[qproperty(f64, other_devices)]
        #[qproperty(QVariant, row_ids)] #[qproperty(QStringList, row_names)] #[qproperty(QStringList, row_kinds)]
        #[qproperty(QVariant, row_allocated)] #[qproperty(QVariant, row_shares)] #[qproperty(QVariant, row_files)]
        #[qproperty(QVariant, row_unreadable)] #[qproperty(QVariant, row_merged)] #[qproperty(QVariant, tiles)]
        #[qproperty(i32, revision)]
        type SideritaUsage = super::SideritaUsageRust;
        #[qinvokable] fn open(self: Pin<&mut SideritaUsage>, path: &QString);
        #[qinvokable] fn close(self: Pin<&mut SideritaUsage>);
        #[qinvokable] fn enter(self: Pin<&mut SideritaUsage>, id: i32);
        #[qinvokable] fn up(self: Pin<&mut SideritaUsage>);
        #[qinvokable] fn path_of(self: &SideritaUsage, id: i32) -> QString;
    }
    impl cxx_qt::Threading for SideritaUsage {}
}
```

  `open`: if `path` is empty → `close()`; `PathBuf::from`; `match session.begin(path)`: `None` → republish (same root); `Some((generation, token))` → `mode = "scanning"`, zero the progress, publish, spawn thread `siderita-usage`: read `/proc/self/mountinfo` there (`parse_mountinfo` + `mount_targets`, empty set on error), `scan(&root, &boundaries, &token, &mut report)` with `report` coalesced to 250 ms via `qt.queue(|hub| hub.apply_progress(generation, files, bytes))`, then `qt.queue(|hub| hub.apply_tree(generation, result))`. `apply_progress`: drop when `generation != session.generation`; set the two numbers, bump revision. `apply_tree`: `Ok(tree)` → `session.accept` (drop if stale) → `mode = "analysed"`, publish; `Err(ScanError::Cancelled)` → nothing (a close or a newer open already published); other `Err(e)` when current → `mode = "failed"`, `failure = e.to_string()`, publish. If the thread cannot spawn → `mode = "failed"`, `failure` from the `io::Error`. `close`: `session.close()`, `mode = "idle"`, publish empty. `enter`/`up`: on `true` publish. `publish`: `let p = session.project();` set every property from it, `revision += 1` last. `path_of`: `session.path_of(id)` lossy or `""`.

- [ ] **Step 6: Retire `directory_size`.** In `selection.rs` `open_properties`, replace the `match props.size { … }` block by:

```rust
self.as_mut().set_prop_size(QString::from(
    props.size.map(|size| crate::format::size_full(size)).unwrap_or_default().as_str(),
));
```

  (a folder's size row now reads from the hub, see Step 8). Delete the `prop_size_cancel` take in `open_properties` and `close_properties`, the field in `state.rs` and its initializer, `directory_size` and its test in `properties.rs`, and the `CancellationToken` import if unused. `rg -n "directory_size|prop_size_cancel" siderita/src` → nothing.

- [ ] **Step 7: `open_in_hematita`.** In `controller.rs`'s bridge add `#[qinvokable] fn open_in_hematita(self: Pin<&mut SideritaController>, path: &QString);`; in `shell.rs`:

```rust
/// Hands a folder to Hematita's storage section through its desktop entry,
/// which applies `%f`; a missing Hematita is reported, never guessed at.
pub fn open_in_hematita(mut self: Pin<&mut Self>, path: &QString) {
    let path = PathBuf::from(path.to_string());
    match crate::apps::launch_with("org.celestina.Hematita", &path) {
        Ok(()) => { self.as_mut().push_notice("Abriendo en Hematita…", "share-2", super::notices::NoticeTone::Info, false); }
        Err(error) => self.as_mut().set_op_error(QString::from(error.as_str())),
    }
}
```

  (`push_notice`'s text is product copy inside a Rust literal: `shell.rs` already carries such literals; if the language guard flags the new line, mark the file head `language-contract: product-copy` only if it is not already marked, otherwise leave.)

- [ ] **Step 8: `FolderUsage.qml`.**

```qml
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import org.celestina.siderita 1.0

// The occupation of a folder, as Hematita computes it: where the person is
// inside the scanned tree, its totals, the biggest children as a list beside
// a treemap, and a way out to Hematita. Presents `usage`; decides nothing.
Item {
    id: section

    required property var usage
    required property var controller
    signal navigated()

    readonly property bool scanning: section.usage.mode === "scanning"
    readonly property bool analysed: section.usage.mode === "analysed"
    readonly property bool failed: section.usage.mode === "failed"
    property int currentId: -1
    property var usageRows: []
    property var tiles: []
    readonly property var toneColors: ({ dir: CelestinaTheme.glyphAccentBlue, file: CelestinaTheme.glyphAccentViolet, unreadable: CelestinaTheme.textFaint, other: CelestinaTheme.textFaint })

    function bytesText(bytes) { /* the same four thresholds as Hematita's StoragePage.bytesText, with qsTr("%1 B") */ }
    function weave() {
        const published = section.usage
        const count = Math.min(published.rowIds.length, published.rowNames.length, published.rowKinds.length,
                               published.rowAllocated.length, published.rowShares.length, published.rowMerged.length)
        const rows = []
        const byId = {}
        for (let index = 0; index < count; ++index) {
            const merged = published.rowMerged[index]
            const share = published.rowShares[index]
            const row = { id: published.rowIds[index],
                          name: merged > 0 ? qsTr("otros (%1)").arg(merged) : published.rowNames[index],
                          kind: published.rowKinds[index],
                          tone: published.rowUnreadable[index] > 0 ? "unreadable" : (published.rowKinds[index] === "dir" || published.rowKinds[index] === "file" ? published.rowKinds[index] : "other"),
                          share: share,
                          size: section.bytesText(published.rowAllocated[index]),
                          percent: qsTr("%1 %").arg((share * 100).toLocaleString(Qt.locale(), "f", 1)),
                          detail: published.rowUnreadable[index] > 0 ? qsTr("%1 no legibles").arg(published.rowUnreadable[index]) : "" }
            rows.push(row); byId[row.id] = row
        }
        const rects = published.tiles
        const tiles = []
        for (let at = 0; at + 4 < rects.length; at += 5) {
            const id = rects[at]; const row = byId[id]
            tiles.push({ id: id, x: rects[at + 1], y: rects[at + 2], w: rects[at + 3], h: rects[at + 4],
                         name: row ? row.name : "", kind: row ? row.kind : "other", tone: row ? row.tone : "other" })
        }
        section.tiles = tiles
        usageList.reset(function() { section.usageRows = rows })
        section.currentId = rows.length > 0 ? rows[0].id : -1
    }
    function goTo(id) {
        const path = section.usage.pathOf(id)
        if (path.length === 0) return
        section.controller.openLocation(path)
        section.navigated()
    }

    Connections { target: section.usage; function onRevisionChanged() { section.weave() } }

    ColumnLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceSm

        // Crumbs: one ghost button per folder from the scanned root.
        Row { id: crumbs; spacing: CelestinaTheme.spaceXs; Accessible.role: Accessible.ToolBar; Accessible.name: qsTr("Ruta")
            Repeater { model: section.usage.crumbs.length
                delegate: Row { id: crumb; required property int index; spacing: CelestinaTheme.spaceXs
                    CelestinaIcon { visible: crumb.index > 0; width: CelestinaTheme.iconSm; height: width; name: "chevron-right"; tone: CelestinaIcon.Secondary; anchors.verticalCenter: parent.verticalCenter }
                    CelestinaButton { role: CelestinaButton.Ghost; text: section.usage.crumbs[crumb.index]; helpText: text
                        font.weight: crumb.index === section.usage.crumbs.length - 1 ? CelestinaTheme.weightDemiBold : Font.Normal
                        onClicked: { for (let steps = section.usage.crumbs.length - 1 - crumb.index; steps > 0; --steps) section.usage.up() } } } } }

        Text { Layout.fillWidth: true; textFormat: Text.PlainText; color: CelestinaTheme.textMuted; font.family: CelestinaTheme.sansFamily; font.pixelSize: CelestinaTheme.fontCaption; elide: Text.ElideRight; Accessible.role: Accessible.StaticText
            text: section.scanning ? qsTr("Calculando… %1 entradas · %2").arg(section.usage.progressEntries.toLocaleString(Qt.locale(), "f", 0)).arg(section.bytesText(section.usage.progressBytes))
                : section.failed ? qsTr("No legible: %1").arg(section.usage.failure)
                : qsTr("%1 · %2 archivos · %3 carpetas · %4 no legibles · %5 en otros dispositivos")
                    .arg(section.bytesText(section.usage.totalBytes)).arg(section.usage.filesBelow.toLocaleString(Qt.locale(), "f", 0))
                    .arg(section.usage.foldersBelow.toLocaleString(Qt.locale(), "f", 0)).arg(section.usage.unreadableBelow).arg(section.usage.otherDevices) }

        RowLayout { Layout.fillWidth: true; Layout.fillHeight: true; spacing: CelestinaTheme.spaceSm; visible: !section.failed
            CelestinaUsageList { id: usageList; Layout.fillHeight: true; Layout.preferredWidth: 42; Layout.horizontalStretchFactor: 42
                usageRows: section.usageRows; toneColors: section.toneColors
                emptyText: section.scanning ? qsTr("Calculando…") : qsTr("Carpeta vacía")
                onChosen: function(id) { section.currentId = id }
                onEntered: function(id) { if (!section.usage.enter(id)) {} }
                onUpRequested: section.usage.up() }
            CelestinaTreemap { Layout.fillHeight: true; Layout.preferredWidth: 58; Layout.horizontalStretchFactor: 58
                tiles: section.tiles; toneColors: section.toneColors; currentId: section.currentId
                currentName: section.usage.crumbs.length > 0 ? section.usage.crumbs[section.usage.crumbs.length - 1] : ""
                onChosen: function(id) { section.currentId = id; usageList.follow(id) }
                onEntered: function(id) { section.usage.enter(id) }
                onUpRequested: section.usage.up() }
        }

        RowLayout { Layout.fillWidth: true
            Item { Layout.fillWidth: true }
            CelestinaButton { text: qsTr("Abrir en Hematita"); role: CelestinaButton.Tonal; enabled: section.usage.root.length > 0
                onClicked: section.controller.openInHematita(section.usage.currentPath) }
        }
    }

    // Ctrl+Enter on the current entry navigates Siderita there and closes.
    Keys.onPressed: function(event) {
        if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && (event.modifiers & Qt.ControlModifier) && section.currentId >= 0) {
            section.goTo(section.currentId); event.accepted = true
        }
    }
}
```

  `usage.enter(id)` is a void invokable in the bridge above; make it return `bool` (`#[qinvokable] fn enter(...) -> bool`) so QML can ignore a file. Keep `onEntered` as `section.usage.enter(id)`. Double click on a folder tile/row enters (the control's `entered`); the hand-off to Siderita is Ctrl+Enter or the crumb's context: add `onDoubleClicked` semantics? No: the controls already map double click to `entered` = drill (spec §5). Navigation to Siderita is Ctrl+Enter only, plus a second button `qsTr("Ir a la carpeta")` beside `Abrir en Hematita`, enabled when `section.currentId >= 0`, calling `section.goTo(section.currentId)`. Add it.

- [ ] **Step 9: The two modals and the owner.**
  - `FolderActions.qml`: add `SideritaUsage { id: folderUsage }` beside `mediaPlayerState`; pass `usage: folderUsage` to both dialogs.
  - `PropertiesDialog.qml`: `property var usage`; `width: Math.min(controller.propIsDir ? 760 : 500, owner.width - 48)`; `height` for a folder: `Math.min(owner.height - 64, propertiesColumn.implicitHeight + propHeading.height + 90 + 300)`; after the `Column` of `PropRow`s (inside the `Flickable`'s content, or better as a sibling below the `Flickable` with a fixed `height: 280`) add `FolderUsage { visible: controller.propIsDir; usage: propertiesView.usage; controller: propertiesView.controller; onNavigated: controller.closeProperties() }`. The size row (`"Tamaño"`): `value: controller.propIsDir ? (propertiesView.usage.mode === "analysed" ? propertiesView.bytesTextOf(propertiesView.usage.totalBytes) : (propertiesView.usage.mode === "failed" ? qsTr("No legible") : qsTr("Calculando…"))) : controller.propSize` — put `bytesTextOf` in `FolderUsage` as a function the dialog calls through its id (`folderUsageSection.bytesText(...)`). Scan lifecycle: `onShownChanged: if (shown && controller.propIsDir) usage.open(controller.propPath); else usage.close()` and `Connections { target: controller; function onPropPathChanged() { if (propertiesView.shown) propertiesView.usage.open(controller.propIsDir ? controller.propPath : "") } }`. `PropRow` values are bare strings today (legacy); the two new samples are already `qsTr()` calls.
  - `QuickLookView.qml`: `property var usage`; body (4) keeps the glyph for non-folders only: `visible: … && quickLookView.qlKind !== "directory"`; add `FolderUsage { anchors.fill: parent; visible: quickLookView.qlKind === "directory"; usage: quickLookView.usage; controller: quickLookView.controller; onNavigated: owner.quickLookOpen = false }`. Lifecycle: extend `syncPlayer()` into `sync()` that also does `if (shown && qlKind === "directory" && qlPath.length > 0) usage.open(qlPath); else usage.close()`. The `Keys.onPressed` of the modal stays as is (↑↓←→ step entries, Space/Esc close, Enter activates) — it runs only while the modal surface has focus; when Tab moved focus into the list or map, their own handlers accept the arrows, Enter and Backspace, and Space/Esc bubble up to the modal (close). Footer text becomes `qsTr("Espacio o Esc para cerrar · ↑ ↓ para navegar · Tab, flechas y Retroceso dentro del mapa · Ctrl+Intro para ir")` for a folder; keep the old text for files. Since the file today has a bare-string footer (legacy), the new folder text is `qsTr()` and the legacy one is untouched.

- [ ] **Step 10: QML test** `siderita/tests/qml/tst_folder_usage.qml`, in the style of `tst_compress_dialog.qml`, with a `QtObject` stub `usageStub` exposing the properties `FolderUsage` reads (`mode: "analysed"`, `crumbs: ["home", "a"]`, `rowIds: [10, 11, -1]`, `rowNames`, `rowKinds`, `rowAllocated`, `rowShares`, `rowMerged: [0, 0, 2]`, `rowUnreadable: [0, 0, 0]`, `tiles: [10, 0,0,0.5,1, 11, 0.5,0,0.5,0.5, -1, 0.5,0.5,0.5,0.5]`, totals, `revision: 1`, functions `enter(id)` recording calls and returning `id === 10`, `up()`, `pathOf(id)` returning `"/home/a/x"`), a `controllerStub` recording `openLocation` and `openInHematita`, then tests: after `usageStub.revision++` the list shows three rows and the third is named `otros (2)`; `Key_Return` on the first row calls `enter(10)`; Ctrl+Return calls `openLocation("/home/a/x")` and emits `navigated`; the button `Abrir en Hematita` calls `openInHematita`. Run with `bash siderita/scripts/qml-tests.sh` (no app build; qmltestrunner over the sources — the symlinked shared controls are picked up by the generated `qmldir`).

- [ ] **Step 11: Documents.** Open `SID-U1` in `siderita/ROADMAP.md` (`active`, checkpoint `SID-U1`, section "SID-U1 — The folder's occupation in the properties dialog and the quick look": problem, boundary (`hematita-core::usage` domain; own hub and composition), outcome). STATUS focus. `VAL-SID-U1` in VALIDATION.md with the spec's §8 author steps. Plan `siderita/docs/plans/active/2026-09-25-folder-usage.md` (Plan ID `folder-usage`, units `SID-U1-A`, `SID-U1-Z`), plan README, evidence `2026-09-25-folder-usage.md` noting the visible change: a folder's size now counts a hard link once (matches `du`), where the old sum counted every name.

- [ ] **Step 12: The one build.**

```bash
bash siderita/scripts/build-production.sh && bash siderita/scripts/verify-production.sh
```

  Expected: `cargo test` green including the five session tests; clippy `-D warnings` clean; `qmllint-cxxqt.sh siderita` 0; `qml-tests.sh` green with the new test; `smoke.sh` OK. Fix and rebuild incrementally only if the pair fails. Then guards and inventory as in Task 2 (`SID-U1-A`, plan `siderita/docs/plans/active/2026-09-25-folder-usage.md`, inventory `siderita/docs/inventories/2026-09-25-folder-usage/SID-U1-A.numstat.tsv`; include `Cargo.lock`, the symlinks, the QML test, all docs).

Commit subject when authorized: `siderita-maintenance: Add the folder occupation to the properties dialog and the quick look`.

---

### Task 4: Implementation exits — `S3-Z` (`hematita:`) and `SID-U1-Z` (`siderita:`)

**Files:** per project, as the S2-Z closing (`git show f929387` is the template): version source + `docs/version-history.tsv`, ROADMAP (`idle`/`none`, `## S3 — closed <date>` / `## SID-U1 — closed <date>`), STATUS (`Delivered as 1.2.0: S3` / `Delivered as 1.7.0: SID-U1`), plan archived with `git mv` to `docs/plans/archive/` (same basename, `Successor: none`), both plan READMEs, evidence `2026-09-25-s3-production-completion.md` / `2026-09-25-folder-usage-production-completion.md`, inventory `S3-Z` / `SID-U1-Z`.

- [ ] **Step 1: Hematita.**

```bash
python3 scripts/version_tool.py bump hematita milestone --unit S3-Z --summary "Add the shared usage view and the folder argument"
bash hematita/scripts/complete-production.sh
```

  Documents, evidence (complete-production tail, deployed digest line), plan row `S3-Z` done, archive, inventory (`mkinv2.py` in the scratchpad handles the deleted plan path; if it is gone, add the deleted row by hand), `check-staged-units.py`. Commit subject: `hematita-milestone: Add the shared usage view and the folder argument`.

- [ ] **Step 2: Siderita.**

```bash
python3 scripts/version_tool.py bump siderita milestone --unit SID-U1-Z --summary "Add the folder occupation to the properties dialog and the quick look"
bash siderita/scripts/complete-production.sh
```

  Same closing steps. Commit subject: `siderita-milestone: Add the folder occupation to the properties dialog and the quick look`.

- [ ] **Step 3: Guards after both:** `bash scripts/check-architecture-contract.sh`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/check-language-contract.py`, `python3 scripts/version_tool.py check`.

---

## Self-review

**Coverage.** Spec §1 items 1–6 → Task 3 (1–4), Task 1 + Task 2 (5), Task 2 (6). §2 table → Tasks 0–3 file structure; `usage::remove` never linked (Task 3 Cargo comment). §3 hub API → Task 3 Steps 3, 5 (`navigateRequested` replaced by `pathOf` + `controller.openLocation` in QML; `layout(width,height)` replaced by unit-square rects scaled in QML, as Hematita does — both simplifications keep the hub free of pixels). §4 controls → Task 1; `FolderUsage` → Task 3 Step 8; dialog widths → Step 9. §5 keys → Task 1 (controls), Task 3 Step 8–9 (Ctrl+Enter, focus flow, footer). §6 table → Step 5 (`failed`, cancel, generation, thread spawn), crate (`boundaries`, `nlink`). §7 → Task 2 Step 6. §8 tests → Task 2 Step 1, Task 3 Steps 2, 10; Hematita smoke argument in Task 2 Step 7 (replaces the spec's "test that a path argument leaves the analysis in browsing", which no headless test can reach without a QObject); Siderita's two `siderita-usage` shape lines from the spec are replaced by the QML composition test, because Siderita's smoke has no shape gate. §9 → superseded by three local plans (Task 0 corrects the spec).

**Placeholders.** None: every code step carries its code; the two `/* … */` in `FolderUsage.qml` name the exact function to copy (`bytesText`, four thresholds) and are not deferred work.

**Types.** `Row.id: Option<NodeId>` ↔ `flat_rects(&[Option<NodeId>], …)` ↔ Hematita's `ids` vector; `children_rows(tree, current, &[u32], usize)` ↔ session `project`; `SideritaUsage.enter -> bool` ↔ `FolderUsage.onEntered`; `pathOf(id) -> QString` ↔ `goTo`; tile objects carry `kind` in both consumers; row objects carry `detail` in both.
