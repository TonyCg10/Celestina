# EXT-1 — Reading and capture

- **Opened:** 2026-10-09
- **Plan ID:** reading-and-capture
- **Status:** active
- **Authorization:** the author approved the design in brainstorming on
  2026-10-09 and asked for the program to be implemented; the design is
  [the spec](../../superpowers/specs/2026-10-09-reading-and-capture-design.md)
  and the task breakdown is
  [the plan](../../superpowers/plans/2026-10-09-reading-and-capture.md)
- **Scope:** suite
- **Implementation checkpoint:** EXT-1
- **Author-validation checkpoint:** none at suite level; each application's
  `VALIDATION.md` holds its own entries

## Hypothesis

Two small applications in the suite's grammar, built on the CONV-1
conventions, take the last two daily needs out of the web browser and the
terminal: reading a PDF (Calcita) and capturing or recording the screen
(Selenita).

## Tangible outcome

Calcita and Selenita are registered, deployed at 1.0 under the author's
prefix, join the suite through the shared activation and appearance, and
Siderita offers Calcita for PDF files.

## Scope

- `EXT-1-A` — register Calcita with its skeleton, icon and document set, and
  open this checkpoint.
- `EXT-1-B` — register Selenita with its skeleton, icon and document set.
- `EXT-1-C` — the exit: Calcita as Siderita's «Abrir en» target for PDFs, the
  host-hygiene record, the ADR follow-up, the documents.

The application units are rows of each project's own plan:
`CAL-1-A` to `CAL-1-C` in
[Calcita's plan](../../../calcita/docs/plans/active/2026-10-09-cal-1-foundation.md),
`SEL-1-A` to `SEL-1-C` in Selenita's (created by `EXT-1-B`).

## Exclusions

- Everything the design's §1 lists as out of scope.
- The halted Celestina shell.

## Build order

`EXT-1-A`, `CAL-1-A` to `CAL-1-C`, `EXT-1-B`, `SEL-1-A` to `SEL-1-C`,
`EXT-1-C`. Each registration is one suite commit; the author's baseline hand
commit (`version_source`, `version_mirrors`, the `0.1.0 baseline` row) follows
each one after its landing.

## Implementation exit

The checkpoint closes when every row below is `done`, both applications'
plans are closed at 1.0.0 and, on the landed `main`,
`bash scripts/check-architecture-contract.sh`,
`bash scripts/check-documentation-contract.sh` and
`python3 scripts/check-language-contract.py` exit 0.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| EXT-1-A | `suite:` | done | [inventory](../../inventories/2026-10-09-reading-and-capture/EXT-1-A.numstat.tsv) | 83 files, +3629/-10 | Register the Calcita project (`versioned = false`) with its skeleton, icon and document set; add `CALCITA` and `SELENITA` to the suite's activation names; open EXT-1. | [evidence](../../evidence/2026-10-09-calcita-registration.md) | None |
| EXT-1-B | `suite:` | done | [inventory](../../inventories/2026-10-09-reading-and-capture/EXT-1-B.numstat.tsv) | 78 files, +3197/-2 | Register the Selenita project (`versioned = false`) with its skeleton, icon and document set; add Selenita to the activation guard's real-tree test. | [evidence](../../evidence/2026-10-09-selenita-registration.md) | None |
| EXT-1-C | `suite:` | planned | — | — | Add Calcita to Siderita's open-in targets, record the host hygiene and close the program. | — | None |
