<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Siderita folder usage — design

- **Date:** 2026-09-25
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Product:** the folder occupation of Hematita's storage analyzer, shown
  inside Siderita's properties dialog and its space-bar quick look
- **Owners:** `siderita` (consumer), `celestina-style` (shared controls),
  `hematita` and `hematita-core` (domain and projection); a `suite:` plan
- **Versions at closure:** Siderita 1.7.0, Hematita 1.2.0, `celestina-style`
  per its own contract

## 1. Goal and scope

Siderita's properties dialog shows a folder's size as a single number that a
hand-rolled sum computes (`siderita/src/properties.rs`, `directory_size`),
and its quick look shows a folder as a large icon and the word `Carpeta`.
Hematita's storage analyzer already answers the better question — what fills
this folder — with a scanned tree, a size-ordered list and a treemap. The
author wants that answer inside both Siderita modals, read-only, with
navigation into subfolders and a hand-off to Hematita for the rest.

In scope:

1. an automatic, cancellable scan of the folder when either modal opens on
   it, replacing the `Calculando…` sum; the same walk the sum did today, but
   the tree is kept while the modal is open;
2. a section `"Ocupación"` in both modals: breadcrumbs, totals, a size-ordered
   list beside a treemap of the current folder within the scanned tree;
3. drilling into subfolders in the modal without rescanning, going back up,
   and a hand-off that closes the modal and navigates Siderita to a folder;
4. a button `Abrir en Hematita` that launches Hematita browsing that folder;
5. the treemap and usage list extracted from Hematita into `celestina-style`
   as shared controls, and the pure projection they need moved into
   `hematita-core::usage::view`, Hematita consuming both from there;
6. Hematita accepting a folder path as its argument.

Out of scope:

- deletion, trash, move or rename from the section (Siderita's own paths
  remain the only ones; Hematita keeps the suite's only permanent deletion);
- duplicates and empty folders (counted in the totals only; listed in
  Hematita);
- a shared Qt bridge crate: the two hubs differ (modes, filters, selection and
  actions in Hematita; scan, drill and navigate in Siderita) and CXX-Qt
  covers both, so no `fluorita-qt`-style exception is needed.

Decisions the author took in brainstorming: layout **2** (breakdown plus
navigable treemap) in both modals; scan **automatic on open**; a click on a
folder **drills inside the modal**, a double click or Enter-with-modifier
navigates Siderita there.

## 2. Boundaries and ownership

| Piece | Home | Reason |
|---|---|---|
| Walk, tree arena, `squarify`, mount boundaries | `hematita_core::usage` (exists) | Pure domain; unchanged. |
| Pure projection tree → rows and tiles | new `hematita_core::usage::view` | Moved from `hematita/src/analysis_view.rs`: only the real intersection (`children_rows`, `flat_rects`, `unreadable_below`). Duplicates, marks, selection and `forget_removed` stay in Hematita's app. Tested on hand-built trees in the crate; the moved tests move with them. |
| Siderita's Qt state: scan, live tree, current folder, cancellation | new `siderita/src/usage.rs`, QObject `SideritaUsage` | Its own file: `SideritaController` is a legacy coordinator under the size baseline and must not grow. The controller only hands it a path and receives `navigateRequested(path)`. |
| `CelestinaTreemap`, `CelestinaUsageList` | `celestina-style/` | Two consumers with the same semantics: the standard's extraction threshold. Hematita imports them from the canonical path and deletes its copies in the same unit. |
| Properties dialog, quick look, `FolderUsage.qml` | `siderita/qml/dialogs/` | Siderita's own composition of the shared controls over `SideritaUsage`. |
| `directory_size` in `siderita/src/properties.rs` | removed | Replaced by the tree; a refactor leaves no second live path. |

Dependencies: `siderita/Cargo.toml` adds `hematita-core` with a manifest
justification like the existing `grafita-core` and `fluorita-core` entries.
`docs/standards/architecture.md` adds `hematita-core::usage` to the domains
Siderita may consume, with the same bound as the other two: domain only,
never `hematita/src` or its QML. `celestina-style` depends on nothing, as
today. `usage::remove` is not linked from Siderita.

`hematita/qml/components/PathCrumbs.qml` is compared with Siderita's path
pill before the plan is written; Siderita's pill is an editor with distinct
semantics, so the crumbs are expected to stay local to each app. If they do
match, they are extracted as `CelestinaCrumbs` in the same unit as the other
two controls.

## 3. The `SideritaUsage` hub

Typed properties and index-aligned lists, `revision` set last, following the
Hematita hubs.

**Invokables:** `open(path)`, `close()`, `enter(id)`, `up()`,
`openInSiderita(id)` → signal `navigateRequested(path)`,
`layout(width, height)` → recomputes `tiles` for the current folder.

**State:** `mode` (`idle | scanning | analysed | failed`), `failure`, `root`,
`currentPath`, `crumbs` (names from the scanned root), `progressEntries`,
`progressBytes`, `totalBytes`, `filesBelow`, `foldersBelow`,
`unreadableBelow`, `otherDevices`, `emptyBelow`.

**Rows of the current folder:** `rowIds`, `rowNames`, `rowSizes` (text),
`rowFractions`, `rowIsDir`, `rowTones`, `rowUnreadable`. **Tiles:** `tiles`
as `QVariant(QVariantList)` of `{id, x, y, w, h, name, tone}`, id `-1` the
merged remainder `otros` — the same contract as Hematita, so the shared
control does not change.

**Flow.** `open(path)` bumps `generation`, cancels the previous scan through
its token, records the path and spawns `walk::scan` on a thread named
`siderita-usage`, reading the mount boundaries from `mountinfo` on that
thread (as Hematita's `spawn_scan`). Progress arrives through
`qt_thread().queue` coalesced to a fixed interval; the finished tree arrives
in one queue and is applied only when the generation is current. `close()`
cancels and drops the tree: no thread keeps reading the disk after the modal
closes. ↑↓ in the quick look is `close()` + `open(other)`; a folder whose
`root` already has a tree is not rescanned; a Siderita refresh invalidates
it. Nothing is spawned for a file.

**Projection.** `enter`/`up` reproject from the tree in memory:
`view::children_rows(tree, current, MAX_ROWS)` orders by `allocated` and
merges the rest into `otros`; `squarify` over those fractions gives the tiles
for the rectangle QML passes to `layout`. Boundaries, other-device counts,
`unreadable` and the `nlink` gate come from `scan`; Siderita adds no domain
rule.

**Cost.** One scan per open (the walk `directory_size` did) plus the arena in
memory while the modal is open, released on close.

## 4. Shared controls and the modals

The Hematita controls carry multi-selection (`selectedIds`, `toggled`,
`trashRequested`, `matches`), which is action semantics. The shared controls
are narrower:

- `CelestinaTreemap`: `required tiles`, `required toneColors`,
  `required currentId`; optional `markedIds: []`, `dimmedIds: []`; signals
  `entered(id)`, `chosen(id)`, `upRequested()`. One Tab stop; the four arrows
  walk tiles in reading order; Backspace goes up.
- `CelestinaUsageList`: `required usageRows`
  (`{id, name, size, fraction, isDir, tone, unreadable}`), `required toneColors`,
  `required currentId`; the same optional lists and signals `entered`,
  `chosen`. Fraction bar in the row's tone, tabular size.

Hematita fills `markedIds`/`dimmedIds` and binds the Delete key on
`currentId` from its page; `trashRequested` leaves the control. Both are
registered in `qmldir` and the module's CMake, documented in
`celestina-style/DESIGN.md`, covered by `tst_treemap.qml` and
`tst_usagelist.qml`.

**`FolderUsage.qml`** (Siderita, `required property var usage`) is the one
composition both modals instantiate: breadcrumbs, a totals line
(`qsTr("%1 · %2 archivos · %3 carpetas · %4 no legibles · %5 en otros dispositivos")`),
the list on the left and the treemap on the right, `Abrir en Hematita` at the
foot. While `mode == scanning` the totals read `Calculando…` and the treemap
shows an indeterminate bar from the theme; `failed` shows the cause and no
controls.

**Properties dialog.** When `propIsDir`, the dialog widens (up to 760 px or
the parent's width minus margin) and appends `"Ocupación"` under the existing
rows; the `"Tamaño"` row reads `totalBytes` from the hub. Files are unchanged.

**Quick look.** For a folder, the body replaces the big icon and `Carpeta`
with `FolderUsage` over the whole surface; the footer keeps
`Espacio o Esc para cerrar · ↑ ↓ para navegar` and adds
`← → baldosas · Retroceso subir · Enter abrir`.

## 5. Keyboard and accessibility

- Focus order inside the occupation section: crumbs → list → treemap →
  `Abrir en Hematita` → `Cerrar`. Each control is one Tab stop.
- Quick look: ↑↓ keep changing the folder entry while focus is on the modal
  surface (the initial state); Tab enters the controls, where the arrows
  belong to the control; a first Esc returns focus to the surface, a second
  closes. Space closes from the surface and from the controls.
- Enter on a folder row or tile drills; Backspace goes up; Ctrl+Enter or a
  double click navigates Siderita there and closes. Enter on a file does
  nothing.
- Names: rows and tiles are `Accessible.role: Button` with name
  `"<name> · <size> · carpeta|archivo · <percent>"` and the path as description;
  the treemap is named `"Mapa de ocupación de <carpeta>"`, the list
  `"Contenido por tamaño"`; totals are static text; crumbs are buttons named by
  their partial path. `helpText` is only the accessible name.
- Visible focus through `CelestinaFocusRing` with `visualFocus`; contrast as
  Hematita already validated in VAL-S1.
- Motion: the dialog growth and the section's appearance use the universal
  exit of the motion contract and honour `reducedMotion`; a subfolder change
  does not animate tiles.
- Mouse: click makes current, double click drills, mouse back button goes up.
  No context menu in this checkpoint.

## 6. Errors and edge cases

| Case | Behaviour |
|---|---|
| Root missing or unreadable | `mode = failed`, `failure` from `ScanError::Root`; the size row reads `"No legible"`; the rest of the dialog works. |
| Unreadable subfolders | The scan continues; `unreadable` climbs the ancestors; totals and rows say so. |
| Mounts inside (bind, USB, network) | Boundaries: counted as `otherDevices`, never crossed; a slow network mount cannot stall the scan. Btrfs subvolumes are boundaries too (known limit). |
| Symbolic links | Listed with their own size, never followed. |
| Hard links | `nlink > 1` counted once per `(dev, ino)`; the total matches `du`, unlike today's sum. Recorded as a visible change. |
| Modal closed mid-scan | Token cancelled; the thread stops at its next check; a late result is dropped by generation. |
| ↑↓ to another entry mid-scan | Same; the old progress is ignored. |
| Same folder reopened | Not rescanned while a tree exists; a Siderita refresh invalidates it. |
| Huge folder | Coalesced progress; only the current folder's projection crosses to Qt; `MAX_ROWS` with `otros` bounds rows and tiles. |
| Arena memory | Not bounded; the same risk exists in Hematita; documented, not mitigated. |
| `openInSiderita` on a vanished folder | Siderita navigates and reports its own navigation error. |
| `Abrir en Hematita` without Hematita installed | The launch fails and Siderita shows its usual error toast. |
| Thread cannot be spawned | `mode = failed` with the `io::Error`; no panic. |

## 7. Hematita argument

`hematita <folder>` opens the storage section in `browsing` mode on that
folder; a missing or unreadable argument is ignored with a diagnostic, not an
error. The desktop entry becomes `Exec=hematita %f`. Siderita launches it
through the desktop entry.

## 8. Tests and evidence

- **Crate:** `usage::view` on hand-built trees: ordering, `otros` merge
  preserving the sum, tile/id alignment, unreadable propagation.
- **Siderita app** (`--release`, one build per batch): `SideritaUsage`
  headless on a temporary folder with files, a subfolder, a symbolic link and
  a hard link → exact totals; `enter`/`up` without a second scan; `close`
  cancels and a late result is dropped; same path not rescanned; missing path
  → `failed`. The `directory_size` test goes with the function.
- **Siderita offscreen gate:** two shape lines, `siderita-usage <rows> <tiles>`,
  after opening properties and quick look on a fixture folder.
- **Hematita:** unchanged verify; the storage smoke proves the page builds
  with the imported controls; a test that a path argument leaves the analysis
  in `browsing` on it.
- **`celestina-style`:** `tst_treemap.qml`, `tst_usagelist.qml`: arrow order,
  `chosen` on Enter, `upRequested` on Backspace, accessible names; qmllint 0.
- **Guards:** architecture (with the new standard line), documentation,
  language, version; the three projects' verify scripts;
  `check-staged-units.py` per unit.
- **Author validation `VAL-SID-U1`:** properties on the home, totals against
  `du -sh` and `du -sh --apparent-size` and against Hematita on the same
  folder; close mid-scan and confirm no thread keeps reading; ↑↓ during a
  scan; full keyboard walk in the order of §5; `Abrir en Hematita`; a screen
  reader on a tile and a row; a folder with a mount inside.

## 9. Delivery

A `suite:` plan with units per project, in order: `hematita-core` view
(crate only), `celestina-style` controls, Hematita consuming them plus the
argument (one build), Siderita hub and modals (one build), closing units with
version bumps (`siderita` milestone 1.7.0, `hematita` milestone 1.2.0,
`celestina-style` per contract) and `complete-production.sh` for Siderita and
Hematita. Builds only where a batch changes a deployable app.
