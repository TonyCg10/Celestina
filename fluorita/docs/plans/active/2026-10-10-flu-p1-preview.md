# FLU-P1 — The floating editor of the capture preview

- **Opened:** 2026-10-10
- **Plan ID:** flu-p1-preview
- **Status:** active
- **Authorization:** the author approved the capture preview design in
  brainstorming on 2026-10-10 and asked for the program to be implemented;
  the design is
  [the spec](../../../../docs/superpowers/specs/2026-10-10-capture-preview-design.md)
  (§5 and §6 are Fluorita's), the task breakdown is
  [the plan](../../../../docs/superpowers/plans/2026-10-10-capture-preview.md)
  and the suite program is
  [PRV-1](../../../../docs/plans/active/2026-10-10-capture-preview.md)
- **Scope:** fluorita
- **Implementation checkpoint:** FLU-P1
- **Author-validation checkpoint:** `VAL-FLU-EDIT-WINDOW` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md); the trim adds its own

## Hypothesis

A capture is most often touched up and sent at once. Fluorita already edits
pictures and saves them in the two ways the author wants; opening that editor
on any path, in a window of its own that niri floats, lets Selenita's corner
preview hand a capture straight to it, and a video trimmed by an `ffmpeg`
child covers the recording.

## Tangible outcome

`org.celestina.Fluorita1.Edit(key)` and `fluorita --edit PATH` open one file
in a floating window titled «Editar — nombre», beside an untouched library
window; it saves «Guardar ambas» (a copy beside, which then joins Selenita's
history) or «Guardar solo la editada» (the original to the Trash), asks those
two and «Descartar» before closing with changes, and the saved result can be
dragged out as a file. A video opened the same way can be trimmed
frame-accurately.

## Scope

- `FLU-P1-A` — the floating editor on any path: `Fluorita1.Edit` served on
  the activation connection, `--edit`, the edit window per file, the save
  labels and the close question, Selenita's adoption of a copy, and the
  drag out (spec §5).
- `FLU-P1-B` — the video trim through an `ffmpeg` child, and ADR 0009's
  amendment for that one operation (spec §6).

## Exclusions

- Everything the design's §1 lists as out of scope, and the niri window rules
  themselves, which the suite exit `PRV-1-E` adds after the author approves.
- Any new external dependency: `selenita-core` is a path crate of this
  repository, and the trim runs `/usr/bin/ffmpeg` as a child, never linked.

## Build order

1. `FLU-P1-A`, in parallel with Selenita's `SEL-2-A` once `PRV-1-A` fixed the
   two interfaces.
2. `FLU-P1-B`, stacked on `FLU-P1-A`.

## Implementation exit

Each row's `Automated evidence` names its exit; every unit ends with
Fluorita's `scripts/complete-production.sh` at landing.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| FLU-P1-A | `fluorita:` | done | [inventory](../../inventories/2026-10-10-flu-p1-preview/FLU-P1-A.numstat.tsv) | 28 files, +2961/-143 | Add the floating editor that opens any path: `org.celestina.Fluorita1.Edit` beside the shared activation, `fluorita --edit`, one `EditWindow` per file beside an untouched library window, «Guardar ambas» and «Guardar solo la editada» with the close question, a copy adopted into Selenita's history, and the saved result dragged out as a copy | [evidence](../../evidence/2026-10-10-floating-editor.md) | `VAL-FLU-EDIT-WINDOW` |
| FLU-P1-B | `fluorita:` | planned | — | — | Add the frame-accurate video trim through an `ffmpeg` child, and amend ADR 0009 for that one operation | — | — |

This plan records intent; it grants no authority.
