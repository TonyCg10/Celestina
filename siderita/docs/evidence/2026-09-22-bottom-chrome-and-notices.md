<!-- language-contract: product-copy — the Spanish below is product copy quoted as string literals -->
# Evidence: 2026-09-22 bottom chrome and the notice stack

- **Date:** 2026-09-22
- **Scope:** `SID-B1-A`; plan
  [bottom-chrome-and-notices](../plans/archive/2026-09-22-bottom-chrome-and-notices.md)
- **Environment:** Arch-derived Linux, Qt 6.9, `cargo` stable, release profile.
  QML interaction under `qmltestrunner` (Qt6) on the `offscreen` platform; no
  window was opened on the author's session.
- **Artifact:** `siderita/target/release/siderita`
- **Delivered with:** [the heading's scroll](2026-09-22-heading-scroll.md),
  asked for after this work was verified but before it was committed, and so
  part of the same unit

## What the author demonstrated

Two screenshots of a running copy and an extraction. Each is announced twice:
once by its ring in the operations dock, and once by a wide strip across the
bottom bar reading `"Moviendo..."` / `"Extrayendo..."` — `FolderBottomStatus`'s
`statusPill`, bound to the controller's `status_text`.

## What reading the code added

**`status_text` carried four unrelated kinds of message**, which is why the
strip could not simply be deleted:

| kind | examples | where it goes now |
|---|---|---|
| measurable job | `"Moviendo…"`, `"Extrayendo…"` | deleted; the ring already says it |
| activity with no knowable end | `"Montando…"`, `"Desmontando…"`, `"Leyendo la papelera…"`, `"Leyendo Recientes…"`, `"Leyendo carpeta…"` | a running notice with an owner |
| outcome | `"Operación cancelada"`, `"Pegado cancelado"`, `"El elemento ya está en esta carpeta"`, `"Abriendo X…"` | a settled notice |
| standing state | `"12 de 340"`, `"Vigilancia perdida · instantánea"` | deleted / the folder heading |

**Two defects found while reading, both closed here:**

- `"Desmontando…"` was **never cleared**. `mounts.rs` set it before spawning the
  worker and no completion path reset it, so the word stayed until something
  else overwrote the line. Every notice now has an owner, and
  `controller::notices::tests::nothing_still_claims_to_be_running_once_every_action_has_ended`
  is the regression test.
- `volume_busy` is published and **no QML reads it**, so unmounting a disk had
  no indicator of its own at all. It now has one.

**One duplication that removed work rather than adding it:** `FolderHeading`
already renders `N VISIBLES DE M ELEMENTOS`, so the `12 de 340` the strip
carried had nowhere to move to and was deleted.

**The author's screenshots predate the checkout.** They show `"Ocultos"` and
`"Tamaño"` as text labels; commit `1c86dd55` (2026-09-03) already made both
icon-only and the installed `95b0fa84` carries it. Only the sort field's
`"Nombre"` was still text.

## What was built

One transient column anchored to the bottom right of the content frame, growing
upward: the rings at its base (`OperationsDock`, unchanged for one to three
jobs), self-retiring notices above them, and the two error properties rendered
as danger notices above everything. This replaced three separately-placed
surfaces whose `y` bindings chained off each other's visibility, two of them
full-width bands across the rows.

`ActivityNotice` is one pill with two tenses: born with a turning mark while the
action runs, mutating into a word when it settles. Its rules are about time —
nothing appears before 500 ms, an action that settles inside that window is
never drawn, a settled notice dwells 4 s (10 s for an error) and a press retires
it at once.

The bottom bar became one glass capsule of three ghost icons — hidden entries,
the current view mode (which opens the merged view-and-sort menu), and the
sizes popup, moved from the far right. Four pills and roughly 330 px became one
surface of 102 px.

## Decisions taken during implementation that the plan did not contain

1. **`src/controller.rs` was split.** It sat at exactly its inventoried ceiling
   of 1072 lines, and the notice contract could not fit: cxx-qt requires every
   property in the bridge module, and even with no comments at all the minimum
   addition was +7 net. The struct, its `Default` and its four Qt-free lookups
   moved to `src/controller/state.rs` (347 lines) with `pub(super)` fields;
   `controller.rs` fell to 754 and its baseline row was lowered to lock that in.
   Approved by the author before the move.
2. **One published list, not five.** The queue publishes
   `id\ticon\ttone\trunning\ttext` rows, cut at the first four tabs so a name
   carrying a tab survives — the pattern `path_crumbs` already uses in the same
   bridge. No `revision` property: the Repeater reads the list's own length.
3. **The password wait lost its line.** `"Esperando la contraseña…"` was a
   notice behind a modal that already names the archive — the same duplication
   this checkpoint removes — so it was deleted rather than converted.
4. **The merged menu uses section labels, not separators.** A `MenuSeparator`
   is a local Qt control the architecture baseline does not carry for this file
   and the ratchet may not grow; `CelestinaSectionLabel` is shared, already used
   by `SizePopup`, and names the groups (`VISTA`, `ORDENAR POR`, `SENTIDO`),
   which a merged menu wants anyway.
5. **Archiving `SID-A4` needed more than a move.** The contract requires
   `Closed:` and `Successor:` metadata on an archived plan, and three documents
   linked to its old path (two evidence records and its predecessor's
   `Successor`). All four were corrected.
6. **The hidden-entries toggle got one owner.** Two surfaces now draw it (the
   picker's floating pill and the capsule's ghost icon), so its glyph and its
   Spanish name moved to the `HiddenToggleDefs` singleton.

## The build budget was overrun, and by how much

The plan allowed one application build cycle for this unit. It took more, and
the reason is recorded rather than smoothed over:

1. `build-production.sh` — failed with eight errors. Two causes, both trivial:
   a `pub(super)` the state-split script had added to `impl Default`'s `default`
   (a trait item may not carry one), and `controller/notices.rs` missing
   `use cxx_qt::CxxQtType;`. The diagnostics were lost to a truncated capture.
2. `cargo check --release` — to recover the diagnostics without spending a
   second release build on them. Cheaper by construction: no codegen, and the
   C++ and Qt objects were already cached.
3. `cargo check --release` — confirmed the two fixes.
4. `build-production.sh` — succeeded in 36.7 s, because everything but the
   Rust crate was cached. It left one `unused_mut` warning: `start_job` no
   longer needs `mut self` now that it does not set the status line.
5. `cargo check --release`, then `build-production.sh` and
   `verify-production.sh` — which found three more things, each needing another
   Rust-only rebuild of well under a minute:
   - **rustfmt.** Two hunks in `notices.rs`. `cargo fmt` changed production
     inputs, so the artifact had to be rebuilt before it could be verified.
   - **A flaky neighbour.** `fluorita-engine`'s
     `a_seek_is_reported_as_completed_only_after_the_backend_restarts` failed
     under load and passed on its own; nothing in `fluorita` was touched here.
     Siderita's verify runs the whole workspace, so another project's
     timing-sensitive test can fail this gate.
   - **A stale id the QML tests could not see.** `FolderActions.qml:28` still
     named `folderSortMenu` in `navigationBlocked`, an expression no unit test
     instantiates. The offscreen smoke gate caught it:
     `ReferenceError: folderSortMenu is not defined`. This is exactly the class
     of defect that gate exists for — a rename that compiles, lints and passes
     every interaction test, and breaks the window on construction.

The lesson worth keeping: the expensive part of this application's build is the
C++ and Qt side, and it caches. A Rust-only recompile after the first one costs
well under a minute, so protecting the first build with a `cargo check` is
cheaper than spending a second one on a path error.

## Accepted costs

- **The sort direction loses its permanent arrow.** It is read by opening the
  menu, and in details mode the column header still shows it. The author was
  offered the alternative (the middle icon showing sort instead of view) and
  declined it.
- **A retired heading takes the watch warning with it.** It is a standing
  condition, not an alarm, and the alternative is a permanent surface over the
  rows — the thing this checkpoint removes.
- **The portal picker keeps its own chrome.** It runs no write operations, so
  `HiddenTogglePill` survives for it and the two surfaces disagree about how
  "show hidden" looks until a later unit aligns them.

## Resolved debt

- **Resolved language debt:** `siderita/qml/components/chrome/BottomControls.qml`
- **Resolved language debt:** `siderita/qml/components/folder/FolderBottomStatus.qml`
- **Resolved language debt:** `siderita/qml/menus/FolderSortMenu.qml`

All three were rewritten with `qsTr()`, so their rows left
`scripts/language-baseline.tsv`. `scripts/architecture-baseline.tsv` lost the
`control … BottomControls.qml:BusyIndicator` row (the indicator is gone; folder
loading is a notice) and its `lines siderita/src/controller.rs` row fell from
1072 to 754.

## Procedure

1. `scripts/qml-tests.sh` — the real components under Qt6's `qmltestrunner`
   with synthetic pointer input, on the `offscreen` platform.
2. `scripts/check-language-contract.py`,
   `scripts/check-architecture-contract.sh` and
   `scripts/check-documentation-contract.sh` — the three static gates, over the
   whole worktree.
3. `scripts/build-production.sh` then `scripts/verify-production.sh` — the
   release build, the release-profile workspace tests, clippy, `qmllint`
   against its ratchet, and the eight-second offscreen smoke run.

## Result

```
scripts/qml-tests.sh
Totals: 140 passed, 0 failed, 0 skipped, 0 blacklisted
```

New interaction coverage: `tst_activity_notice` (5 functions — the threshold,
the never-drawn short action, the immediate outcome, press-to-dismiss without
reaching the file underneath, the danger wrap), `tst_activity_stack` (6 — empty
height, order, the error above the announcement, the rings as the base, the tab
in a name surviving the cut, dismissal by id), `tst_bottom_capsule` (4 — three
icons on one surface, the eye's binding surviving its own toggle, the middle
glyph following the view mode, the sizes popup staying inside the window), and
five added to `tst_operations_dock` (three rings stay a row, a fourth collapses,
a narrow frame collapses at any count, expand and close, cancel by job id).

```
scripts/check-language-contract.py      Language contract: OK (148 legacy file(s) ratcheted)
scripts/check-architecture-contract.sh  Architecture contract: OK
scripts/check-documentation-contract.sh Documentation contract: OK

scripts/build-production.sh             Finished `release` profile in 36.74s
scripts/verify-production.sh            manifest: siderita/target/production-artifact.toml (verified)
smoke                                   OK — binario vivo 8 s, sin errores QML, sin auto-bindings
```

`scripts/qmllint-baseline.tsv`'s `siderita` row fell from 268 to 260: the four
pills and two banners that went away took eight warnings with them.

## Limits

The offscreen smoke run proves the window constructs and survives eight
seconds without a QML error; it does not prove anything about appearance.
`ShaderEffectSource` and `MultiEffect` render nothing under `offscreen`, so no
glass was exercised. `qmltestrunner` delivers synthetic pointer events, which
is not the same as a real pointer on a real compositor, and it exercises no
assistive technology. The timings in a notice were asserted against
`tryVerify` deadlines, not measured. Whether the 500 ms threshold, the two-line
error and the lost sort arrow are right in daily use is `VAL-SID-15`.

## Not taken: the headless stills

The plan asked for one `grabToImage` still per state. They were not taken. The
probe has to live in `Main.qml`, which is a production input, so each still
costs a rebuild and a re-verify to put the tree back — and under `offscreen`
`ShaderEffectSource` renders nothing, so every pill would have come out without
its glass. The smoke gate already proves the whole window constructs headlessly
with no QML error, and appearance is `VAL-SID-15` either way.

## Not covered here

Appearance, glass, reduced motion, assistive technology and whether the 500 ms
threshold feels right against a real fast unmount belong to `VAL-SID-15`. No
window was opened on the author's session.
