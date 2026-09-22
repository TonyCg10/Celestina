<!-- language-contract: product-copy — the Spanish strings below are product copy quoted as qsTr() literals -->
# Siderita — bottom chrome and the transient notice stack

- **Date:** 2026-09-22
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Product:** Siderita, the suite's file manager
- **Proposed unit:** `SID-B1` in `siderita/ROADMAP.md`
- **Identifiers:** commit prefix `siderita`, desktop id `org.celestina.Siderita`

## 1. The problem, as the author demonstrated it

The author showed two screenshots of a running copy and an extraction. In both,
the operation is announced **twice**: once by the ring in the operations dock,
which is the surface designed for it, and once by a wide pill across the bottom
bar reading `Moviendo...` / `Extrayendo...`. That pill is
`FolderBottomStatus.qml`'s `statusPill`, bound to the controller's
`status_text`. It is a leftover of the status line the dock replaced, it looks
like a search field, and it consumes the whole centre of the bottom bar for the
duration of the job.

Removing it is not a deletion, because `status_text` is a catch-all. Four
different kinds of message travel through one property:

| kind | examples | has a ring today |
| --- | --- | --- |
| measurable job | `Moviendo…`, `Extrayendo…` | yes — the duplication |
| activity with no knowable end | `Montando…`, `Desmontando…`, `Leyendo la papelera…`, `Leyendo Recientes…`, `Leyendo carpeta…` | no |
| outcome (already finished) | `"Operación cancelada"`, `"Pegado cancelado"`, `"El elemento ya está en esta carpeta"`, `"Abriendo X…"` | not applicable |
| standing state | `"12 de 340"` while filtering, `"Vigilancia perdida · instantánea"` | no |

Two defects found while reading the code, both fixed by this design:

- `Desmontando…` is **never cleared**. `mounts.rs` sets it before spawning the
  worker and the completion handler does not reset it, so the word stays on
  screen until some other message overwrites it.
- `volume_busy` is published by the controller and **no QML reads it**.
  Unmounting a disk therefore has no indicator of its own at all; the legacy
  line was its only feedback.

And one duplication that removes work rather than adding it: the folder heading
already renders `12 VISIBLES DE 340 ELEMENTOS · 8 CARPETAS · …`
(`FolderHeading.qml`). The `N de M` hint in the status pill has nowhere to move
to — it is deleted.

## 2. What the author asked for

1. The legacy bottom strip disappears.
2. Actions that do not warrant a ring — unmounting a disk is the named case —
   get a different kind of surface, and they must get one: they cannot be left
   silent.
3. The bottom bar is compressed: sort and view mode merge into **one** icon
   that opens a menu, and the magnifier (the `zoom-in` "Tamaño" button, today at
   the bottom right) moves to the left, so the rest of the bottom is free.
4. Responsive behaviour when many operations run at once: collapse to a circle
   carrying the count, which expands upward on press. The author noted this is
   rare and not the priority.

Decisions taken during brainstorming, recorded here:

- The standing states leave the bottom bar (`N de M` is deleted as duplicate;
  `watchDegraded` moves beside the folder title).
- `Ocultos` stops being a wide text pill and becomes an eye icon — one press,
  state still visible, consistent with the suite's icon-first rule.
- Approach **B**: two surfaces separated by tense, in one stack, and the danger
  banners join it.

## 3. The design

### 3.1 One transient column, three layers

Everything transient lives in a single column anchored to the bottom right of
the content frame, growing upward:

```
        ┌───────────────────────┐  errors      (danger notices, persistent)
        ├───────────────────────┤  notices     (whispers, self-retiring)
        ├───────────────────────┤  the dock    (rings, as today)
        └───────────────────────┘
  [ eye | view+sort | size ]              ← the bottom bar's only occupant
```

This replaces three separately-positioned surfaces in `FolderBottomStatus.qml`
— `errorBanner`, `operationErrorBanner` and `statusPill` — whose `y` bindings
currently chain off each other's visibility. The two error banners are
full-width rectangles stacked over the rows: the same fault as the strip being
removed. They become notices in the danger tone.

### 3.2 The notice ("susurro")

A pill as wide as its text, never full width: an icon, one line, an optional
turning dot. One component with two tenses:

- **running** — born with a turning dot: `Desmontando…`, `Montando…`,
  `Leyendo la papelera…`.
- **settled** — the same pill *mutates* into a word: `Disco desmontado`,
  `"Pegado cancelado"`, `"El elemento ya está en esta carpeta"`. It is not a second
  notice; the running notice becomes the settled one in place, then retires.

Rules:

- **500 ms appearance threshold.** Nothing appears before it. An unmount that
  takes 300 ms and a folder that reads instantly never flash. This is what
  makes a notice safe for short actions, and it is the reason a ring was the
  wrong shape for them.
- **Retirement.** A settled notice retires after 4 s; a danger notice stays
  until pressed or until 10 s pass. Exit uses the suite's universal contract:
  `CelestinaTheme.motionExit`, `exitShrink`, `easeExit`, **one clock per
  gesture**, a single fade. `reducedMotion` skips the movement, not the
  retirement.
- **Pressing a notice retires it immediately.** This gives the input shield it
  needs a purpose: a click near the corner must not fall through onto the file
  the notice covers (the reason `CelestinaInputShield` exists on today's
  banners), and here that swallowed click dismisses instead of doing nothing.
- **Fixed track.** The stack is laid out inside an item sized for the full
  visible stack from the start, and its `Column` is **always top-anchored**.
  No conditional anchors on a live positioner: a ternary on `anchors.top` /
  `anchors.bottom` is exactly what left the Celestina toast's cards hanging
  outside their glass when a parked surface was resurrected.
- At most **three** notices visible; older ones retire early when a fourth
  arrives.

### 3.3 The dock and its overflow

1–3 jobs: the row of rings exactly as today, each with its callout on press
(`OperationsDock.qml`, `OperationRing.qml`, `OperationCallout.qml` are kept).

4 or more jobs, or a frame too narrow for the row: the dock collapses to a
single glass circle carrying the **count**, with the aggregate progress drawn
on its arc. Pressing it expands upward into a **list**, one row per job: ring,
label, and the job's own pause and cancel. The expanded list makes the
per-ring callout unnecessary while it is open — the detail is already on the
row.

### 3.4 The bottom bar

One glass capsule holding three icons, replacing four separate pills
(`HiddenTogglePill`, the sort group, the view group, and the `BusyIndicator`)
plus the `FloatingButton` at the far right. Roughly 360 px of chrome becomes
roughly 115 px.

| icon | action |
| --- | --- |
| eye / eye-off | toggles hidden entries — one press, state visible in the glyph |
| the current view mode's glyph | opens the view-and-sort menu |
| `zoom-in` | opens the existing `SizePopup`, now left-aligned |

The centre icon **is the current view mode** (grid, list or details), so that
state needs no menu to be read. The menu it opens carries, in order: the three
view modes as a row of icons; the sort field as checked rows (`"Nombre"`,
`"Tamaño"`, `"Fecha"`, `"Tipo"`); the sort direction as a final row.

**Accepted cost:** the sort direction stops being visible at rest — today it is
a permanent arrow. It is read by opening the menu, and in details mode the
column header still shows it. The author accepted this in brainstorming; the
alternative offered (centre icon shows sort instead of view) was declined.

The `BusyIndicator` bound to `controller.loading` disappears from the bar:
folder loading becomes a running notice, and therefore inherits the 500 ms
threshold that stops it flashing on fast folders.

### 3.5 Where the standing states go

- `12 de 340` — **deleted**. `FolderHeading.qml` already renders
  `N VISIBLES DE M ELEMENTOS`.
- `"Vigilancia perdida · instantánea"` — an alert icon beside the folder title in
  `FolderHeading.qml`, with the explanation as its accessible name. It is a
  standing condition of the folder, not an event, so it belongs where the
  folder describes itself. No tooltip (suite rule).

## 4. The controller contract

`status_text` is retired as a property. In its place the controller publishes a
notice queue in the suite's index-aligned list shape, with a `revision` counter:

| property | meaning |
| --- | --- |
| `notice_ids` | stable id per notice, so a queue reorder does not move a pill |
| `notice_texts` | the product copy, already composed in Rust |
| `notice_icons` | catalogue glyph name |
| `notice_tones` | `info` / `danger` |
| `notice_running` | `1` while the action is still in flight, `0` once settled |
| `notice_revision` | bumped on every change |

Call sites change as follows:

| call site | today | after |
| --- | --- | --- |
| `jobs.rs` `start_job` | `set_status_text(label)` | **removed** — the ring already says it |
| `mounts.rs` mount / unmount / open | `set_status_text("Montando…")`, never cleared | `push_notice` running, **settled by the completion handler**; `volume_busy` keeps gating re-entry |
| `trash.rs`, `scan.rs`, `archive.rs` reads | `set_status_text(…)` + manual clears | running notice, settled or dropped on completion |
| `fileops.rs` cancellations, `display::same_folder_cut_status` | `set_status_text(…)` | settled notice |
| `shell.rs` `Abriendo X…` | `set_status_text(…)` | settled notice |
| `scan.rs` `"{visible} de {total}"` | `set_status_text(…)` | **removed** |
| `error_text`, `op_error` | two full-width banners | danger notices in the same stack |

Every notice has an owner that settles it. A running notice with no completion
path is a defect, and it is the defect `Desmontando…` has today.

## 5. Files

New:

- `qml/components/folder/ActivityStack.qml` — the fixed track, the column, the
  three layers, the overflow decision.
- `qml/components/folder/ActivityNotice.qml` — one pill, its two tenses, its
  threshold and its retirement.
- `qml/menus/ViewSortMenu.qml` — the merged menu (a rename of
  `FolderSortMenu.qml`, so it is new only in name).

Changed:

- `qml/components/folder/FolderBottomStatus.qml` — loses `statusPill`,
  `errorBanner`, `operationErrorBanner` and the `sizeButton`; becomes the
  placement of `ActivityStack`.
- `qml/components/chrome/BottomControls.qml` — becomes the three-icon capsule.
- `qml/components/folder/FolderBottomChrome.qml` — the two children keep one
  anchor, as they do now.
- `qml/components/folder/OperationsDock.qml` — gains the collapsed and expanded
  forms.
- `qml/components/folder/FolderHeading.qml` — the watch-degraded alert icon.
- `qml/menus/FolderSortMenu.qml` — renamed to `ViewSortMenu.qml` and extended
  with the view-mode row. `BottomControls.qml` is its only consumer (verified),
  and the folder context menu carries no sort or view entries, so there is no
  second caller to keep in step.
- `src/controller.rs` and the modules in the table above; `build.rs` gains two
  `QML_FILES` rows (the stack and the notice), renames the sort menu's row and
  drops the hidden pill's.

`qml/components/chrome/HiddenTogglePill.qml` is **kept**: the portal picker's
chrome (`qml/components/picker/PickerChrome.qml`) is its second consumer, and
the picker is out of scope (§6.1). Only `status_text` is deleted outright.

## 6. Scope and responsive behaviour

### 6.1 Out of scope

The **portal picker** (`PickerWindow.qml`, `qml/components/picker/`) keeps the
chrome it has. It is a separate window with its own bottom row, it never runs
write operations, and folding it in would double the change for no problem the
author demonstrated. The consequence is that `HiddenTogglePill` survives and
the two surfaces disagree about how "show hidden" looks until a later unit
decides to align them. Recorded as a known, deliberate inconsistency.

### 6.2 Responsive behaviour

- The capsule is fixed width and always anchored to the left inset; it never
  competes for space.
- An info notice is **one line, always**: its maximum width is the content
  frame minus the capsule minus two gaps, and beyond that the text elides. It
  is an announcement, not a document.
- A **danger** notice is the exception: it wraps to at most two lines and may
  take the full available width. An elided error is an error the person cannot
  read, which is the one thing the removed banners did right. Past two lines
  the text elides and the notice stays until pressed, so the full message is
  still reachable — the author validates whether two lines is enough against a
  real permission failure on a long path (`VAL-SID-*`).
- When the frame is too narrow for the widest notice to stay legible (under
  roughly 320 px of free width), the stack drops to icon-only pills carrying
  the accessible name.
- The dock collapses to the counted circle when the ring row would overlap the
  capsule, independently of the job count.

## 7. Verification

- **Headless stills.** `QT_QPA_PLATFORM=offscreen` with `XDG_CONFIG_HOME`
  pointed at a scratch directory, a probe `Timer` calling `grabToImage` on
  `contentLayer` (never `window.contentItem`), one still per state: bar at
  rest, menu open, one running notice, a settled notice, a danger notice,
  three rings, the collapsed circle, the expanded list.
- **Pointer semantics.** `scripts/qml-tests.sh` under Qt6's `qmltestrunner`:
  pressing a notice retires it; the press does not reach the entry underneath;
  the capsule's three icons stay reachable; the collapsed circle expands and
  the outside press collapses it.
- **Rust.** A unit test per call site asserting that every running notice is
  settled or dropped by its completion path, which is the regression test for
  the `Desmontando…` leak.
- **qmllint** stays at its current row for `siderita`; no suppressions.
- **Author validation** (`VAL-SID-*`): appearance, the 500 ms threshold against
  a fast unmount, and the merged menu's feel are the author's call on the live
  session, not claimed here.

## 8. Decisions recorded here

1. Two surfaces, separated by tense — a ring is present continuous and
   cancellable; a notice is a short announcement. Not by importance.
2. One transient column, bottom right, growing upward; the danger banners join
   it rather than keeping their own full-width geometry.
3. A 500 ms appearance threshold is what makes short actions safe to announce,
   and is why they are not rings.
4. The centre bottom icon shows the view mode; the sort direction's permanent
   arrow is spent to buy the space.
5. `status_text` is replaced, not narrowed: a property that carried four kinds
   of message was the reason the strip could never be deleted safely.
6. No tooltips anywhere in the new surfaces; `helpText` is an accessible name.
