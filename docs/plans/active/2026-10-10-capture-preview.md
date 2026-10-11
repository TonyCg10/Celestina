# PRV-1 — Capture preview

- **Opened:** 2026-10-10
- **Plan ID:** capture-preview
- **Status:** active
- **Authorization:** the author approved the design in brainstorming on
  2026-10-10 and asked for the program to be implemented; the design is
  [the spec](../../superpowers/specs/2026-10-10-capture-preview-design.md)
  and the task breakdown is
  [the plan](../../superpowers/plans/2026-10-10-capture-preview.md)
- **Scope:** suite
- **Implementation checkpoint:** PRV-1
- **Author-validation checkpoint:** none at suite level; each application's
  `VALIDATION.md` holds its own entries

## Hypothesis

A capture is most often taken to be sent or touched up at once. A small
preview in the corner of the screen, as macOS and Android show, lets the file
be dragged into any program that takes files (which is how a video, which the
clipboard cannot carry, reaches a chat), and a click opens it in a floating
Fluorita editor; leaving it alone changes nothing.

## Tangible outcome

After every Selenita capture or finished recording the result appears in the
bottom-right corner without taking focus; dragging it offers the file as a
copy; clicking it opens Fluorita's floating editor, which saves both versions
(«Guardar ambas») or only the edited one («Guardar solo la editada») and can
trim a video frame-accurately; an edited copy joins Selenita's history.

## Scope

- `PRV-1-A` — fix the interfaces between the two applications
  (`org.celestina.Fluorita1.Edit` and `org.celestina.Selenita1.Adopt`) in
  [ADR 0012](../../decisions/0012-suite-conventions.md#follow-ups), teach the
  activation scanner their literals, and open this checkpoint.
- `PRV-1-E` — the exit: the two niri window rules in the author's
  configuration after the author's approval, the host-hygiene record, the
  README cross-references and the program's close.

The application units are rows of each project's own plan, each created by
its first unit: `SEL-2-A` (the corner preview, the `Edit` hand-off and
`Adopt`) in Selenita's plan
`selenita/docs/plans/active/2026-10-10-sel-2-preview.md`, and `FLU-P1-A` (the
floating editor on any path) and `FLU-P1-B` (the video trim through an
`ffmpeg` child and the ADR 0009 amendment) in Fluorita's plan
`fluorita/docs/plans/active/2026-10-10-flu-p1-preview.md`.

## Exclusions

- Everything the design's §1 lists as out of scope: cropping a video's area,
  removing its sound, speed changes, joining clips, annotations on a video, a
  preview for anything other than Selenita's own results, sharing targets
  inside the preview.
- Any new external dependency: no layer-shell-qt and no crate that links a
  media library.
- The halted Celestina shell.

## Build order

`PRV-1-A`, then `SEL-2-A` and `FLU-P1-A` in parallel (they meet only at the
two D-Bus methods `PRV-1-A` fixes), then `FLU-P1-B`, then `PRV-1-E`. Before
`SEL-2-A` the author archives Selenita's SEL-1 plan by hand, and before
`FLU-P1-A` Fluorita's hardening plan, so each application has one active
plan.

## Implementation exit

The checkpoint closes when every row below is `done`, both application plans'
rows are `done` and, on the landed `main`,
`bash scripts/check-architecture-contract.sh`,
`bash scripts/check-documentation-contract.sh`,
`python3 scripts/check-language-contract.py` and
`python3 scripts/test-activation-contract.py` exit 0.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| PRV-1-A | `suite:` | done | [inventory](../../inventories/2026-10-10-capture-preview/PRV-1-A.numstat.tsv) | 11 files, +646/-11 | Fix the capture preview's interfaces between Selenita and Fluorita | [evidence](../../evidence/2026-10-10-capture-preview-interfaces.md) | None |
| PRV-1-S | `suite:` | done | [inventory](../../inventories/2026-10-10-capture-preview/PRV-1-S.numstat.tsv) | 4 files, +61/-1 | Support edit FLU-P1-A needs before it can land: `celestina-rs/crates/selenita-core` added to Fluorita's `production_inputs` in `docs/projects.toml`, because Fluorita links `selenita-core` to append an edited copy to Selenita's history when Selenita is not running | [evidence](../../evidence/2026-10-10-capture-preview-support.md) | None |
| PRV-1-T | `suite:` | done | [inventory](../../inventories/2026-10-10-capture-preview/PRV-1-T.numstat.tsv) | 4 files, +100/-0 | ADR 0009's amendment for FLU-P1-B: a video's duration trim is the one editing operation that re-encodes, raster-class, by an `ffmpeg` child never linked; pictures keep every rule | [evidence](../../evidence/2026-10-10-capture-preview-adr-0009.md) | None |
| PRV-1-E | `suite:` | planned | — | — | Close the capture preview program with the niri window rules, the host-hygiene record and the README cross-references. | — | None |
