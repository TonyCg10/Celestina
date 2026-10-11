# SEL-2 — The corner preview

- **Opened:** 2026-10-10
- **Plan ID:** sel-2-preview
- **Status:** active
- **Authorization:** the author approved the capture preview design in
  brainstorming on 2026-10-10 and asked for the program to be implemented;
  the design is
  [the spec](../../../../docs/superpowers/specs/2026-10-10-capture-preview-design.md)
  (§4 is Selenita's) and the task breakdown is
  [the plan](../../../../docs/superpowers/plans/2026-10-10-capture-preview.md)
  (Task 2). The program is the suite's `PRV-1`, whose ledger is
  [the suite plan](../../../../docs/plans/active/2026-10-10-capture-preview.md)
- **Scope:** selenita
- **Implementation checkpoint:** SEL-2
- **Author-validation checkpoint:** VAL-SEL-PREVIEW in
  [VALIDATION.md](../../../VALIDATION.md) is pending (it does not block)

## Hypothesis

A capture is most often taken to be sent or touched up at once. A small
preview in the corner of the screen after every capture and every
recording lets the file be dragged into any program that takes files (a
video, which the clipboard cannot carry, reaches a chat that way) and a
click opens it in Fluorita's floating editor, while leaving it alone
changes nothing.

## Tangible outcome

After every capture saved to a file and every finished recording, however
it was started, a frameless `Vista previa` window shows the picture (or the
recording's first frame, its length and the film glyph) for five seconds,
held while the pointer rests on it; dragging it offers the file as a copy,
a click hands it to Fluorita (`org.celestina.Fluorita1.Edit`, or
`fluorita --edit`), the × closes it and a trashed file closes it. A
key-binding launch no longer shows the main window. `Selenita1.Adopt`
lets Fluorita's edited copy join the history.

## Scope

- `SEL-2-A` — the corner preview (`PreviewWindow.qml`, the controller's
  preview state, `Report::Preview` after every publish, the poster through
  `gst-launch-1.0`), the hand-off to Fluorita (`preview.rs`), `Adopt` on
  `org.celestina.Selenita1`, and the key-binding launch that shows only the
  preview.

## Exclusions

- Everything the capture preview design's §1 lists as out of scope.
- The niri window rule that places the preview: it goes into the author's
  configuration in the suite's exit unit `PRV-1-E`, after the author's
  approval; this plan only documents it.
- Any new external dependency, and any reference to the Celestina shell.

## Build order

1. `SEL-2-A`, in parallel with Fluorita's `FLU-P1-A`; the two meet only at
   `Fluorita1.Edit` and `Selenita1.Adopt`, fixed by the suite's `PRV-1-A`.

## Implementation exit

The checkpoint closes when `SEL-2-A` is `done`: the preview, the hand-off
and `Adopt` are in with their tests, the smoke reports the preview, and
`scripts/complete-production.sh` succeeds on the landed tree.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SEL-2-A | `selenita:` | done | [inventory](../../inventories/2026-10-10-sel-2-preview/SEL-2-A.numstat.tsv) | 33 files, +3075/-143 | Add the corner preview with drag, edit and adopt: the frameless `Vista previa` window after every published capture and recording (picture or poster, length, film glyph, five seconds held by the pointer, × and trash close it), the `text/uri-list` copy-only drag, the click's hand-off to Fluorita (`Fluorita1.Edit` or `fluorita --edit`), `Selenita1.Adopt`, and no main window after a key-binding launch | [evidence](../../evidence/2026-10-10-preview.md) | VAL-SEL-PREVIEW pending |
