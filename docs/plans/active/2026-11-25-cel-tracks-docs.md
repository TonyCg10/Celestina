# TRK-1 — Documents of the shell's standing tracks

- **Opened:** 2026-11-25
- **Plan ID:** cel-tracks-docs
- **Status:** active
- **Standing:** yes. It hosts a unit whenever an entry of the shell's standing
  tracks needs a suite document, and it is not archived while the author keeps
  the tracks
- **Authorization:** the author approved
  [the standing-tracks design](../../superpowers/specs/2026-11-24-celestina-standing-tracks-design.md)
  and its first plan on 2026-11-24, and the CEL-I-004 audio design and plan on
  2026-11-25; on 2026-11-25 too, the previews of the session menu,
  the icon tabs and the per-app mute button (variant A of each); on
  2026-11-25 as well, the capsule sheets preview (version 7) and design for
  CEL-D-002 to CEL-D-005; on 2026-11-26, the preview of CEL-D-007 (O1 and P1); on 2026-11-26 too, the notifications and tray preview and design for CEL-I-010 to CEL-I-012 (variant A, list G2)
- **Scope:** suite
- **Implementation checkpoint:** TRK-1
- **Author-validation checkpoint:** none

## Hypothesis

The shell's standing tracks, design (`CEL-D`) and improvements (`CEL-I`), keep
their entries' design notes and task plans under `docs/superpowers/`, which is
outside the shell's commit roots. A standing suite plan whose documents-only
units commit those files keeps each shell entry a shell-only unit.

## Tangible outcome

- The tracks' design and plans in `docs/superpowers/`. The shell's standing
  plan, `CEL-TRACKS`, is opened by the shell's first entry, `CEL-I-004`, as
  its own open row: one landing changes one active plan.

## Scope

- `CEL-TRACKS-OPEN`: the standing-tracks design and its first plan, the
  CEL-I-004 audio design and plan, this plan, and the suite's roadmap and
  status lines.
- Later rows: one documents-only unit per batch of new design notes or task
  plans of the tracks.

## Exclusions

- Any code, and any file of a project.

## Build order

1. `CEL-TRACKS-OPEN`.
2. Later rows as the tracks need them.

## Implementation exit

Standing: the checkpoint stays open while the author keeps the tracks. Each row
closes on its own landing; the plan is archived when the author ends the
tracks.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CEL-TRACKS-OPEN | `suite:` | done | [inventory](../../inventories/2026-11-25-cel-tracks-docs/CEL-TRACKS-OPEN.numstat.tsv) | 10 files, +900/-7 | Add the standing design and improvement tracks of the shell: their design and first plan, the CEL-I-004 audio design and plan, and this plan | [evidence](../../evidence/2026-11-25-cel-tracks-open.md) | None |
| CEL-TRACKS-DOCS-2 | `suite:` | done | [inventory](../../inventories/2026-11-25-cel-tracks-docs/CEL-TRACKS-DOCS-2.numstat.tsv) | 6 files, +223/-3 | Record the audio entry's revisions (collapsible sections, the speaker mute button) in its design and add the session menu's task plan | [evidence](../../evidence/2026-11-25-cel-tracks-docs-2.md) | None |
| CEL-TRACKS-DOCS-3 | `suite:` | done | [inventory](../../inventories/2026-11-25-cel-tracks-docs/CEL-TRACKS-DOCS-3.numstat.tsv) | 6 files, +1151/-2 | Add the capsule sheets design and its task plan for CEL-D-002 to CEL-D-005, which supersede §6.2 to §6.5 of the standing-tracks design | [evidence](../../evidence/2026-11-25-cel-tracks-docs-3.md) | None |
| CEL-TRACKS-DOCS-4 | `suite:` | done | [inventory](../../inventories/2026-11-25-cel-tracks-docs/CEL-TRACKS-DOCS-4.numstat.tsv) | 5 files, +68/-8 | Replace the spotlight of the capsule sheets design with the focus indicator the author approved, and add Task B5 to its plan | [evidence](../../evidence/2026-11-25-cel-tracks-docs-4.md) | None |
| CEL-TRACKS-DOCS-5 | `suite:` | done | [inventory](../../inventories/2026-11-25-cel-tracks-docs/CEL-TRACKS-DOCS-5.numstat.tsv) | 5 files, +214/-47 | Update the capsule sheets design to the author's two requests of 2026-11-26 (full opacity around the open sheet, CEL-D-007; the capsule that never narrows, with the open piece flying down as the menu's subtitle, CEL-D-008) and add the CEL-D-008 plan | [evidence](../../evidence/2026-11-26-cel-tracks-docs-5.md) | None |
| CEL-TRACKS-DOCS-6 | `suite:` | done | [inventory](../../inventories/2026-11-25-cel-tracks-docs/CEL-TRACKS-DOCS-6.numstat.tsv) | 5 files, +184/-35 | Update the capsule sheets design to the menu as its own window the author approved on 2026-11-26 (CEL-D-009) and add the CEL-D-009 plan | [evidence](../../evidence/2026-11-26-cel-tracks-docs-6.md) | None |
| CEL-TRACKS-DOCS-7 | `suite:` | done | [inventory](../../inventories/2026-11-25-cel-tracks-docs/CEL-TRACKS-DOCS-7.numstat.tsv) | 5 files, +131/-9 | Update the capsule sheets design to the menu close the author approved on 2026-11-26 (CEL-D-010) and add the CEL-D-010 plan | [evidence](../../evidence/2026-11-26-cel-tracks-docs-7.md) | None |
| CEL-TRACKS-DOCS-8 | `suite:` | done | [inventory](../../inventories/2026-11-25-cel-tracks-docs/CEL-TRACKS-DOCS-8.numstat.tsv) | 7 files, +509/-1 | Add the notifications and tray design the author approved on 2026-11-26 and the CEL-I-010, CEL-I-011 and CEL-I-012 plans | [evidence](../../evidence/2026-11-26-cel-tracks-docs-8.md) | None |

This plan records intent; it grants no authority.
