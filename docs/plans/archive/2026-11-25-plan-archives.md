# ARC-1 — Archive the finished project plans

- **Opened:** 2026-11-25
- **Plan ID:** plan-archives
- **Closed:** 2026-11-25
- **Successor:** none; the author named no next suite checkpoint. Its one row (ARC-1-A) is done
- **Status:** done
- **Authorization:** on 2026-11-25 the author asked which open plans could be
  archived and then to archive the four whose every row was done. Opened and
  closed in one commit: one documents-only unit, merged by fast-forward. A
  suite unit, because documents of the suite link to one of the plans
- **Scope:** suite
- **Implementation checkpoint:** ARC-1
- **Author-validation checkpoint:** none

## Hypothesis

Four project plans have every ledger row done yet stay active, so their
roadmaps name a checkpoint nobody is working on. Archiving them in one suite
unit leaves each project idle and every link to them resolving.

## Tangible outcome

The four plans under their projects' `docs/plans/archive/`, with `Closed`,
`Successor` and `Status: done`:

- `CEL-2` (the shell's parity with Noctalia): [the archived plan](../../../celestina/docs/plans/archive/2026-10-16-cel-2-parity.md)
- `CUP-1` (Cuprita's foundation): [the archived plan](../../../cuprita/docs/plans/archive/2026-10-08-cup-1-foundation.md)
- `GRA-H1` (Grafita's hardening after the monorepo audit): [the archived plan](../../../grafita/docs/plans/archive/2026-09-26-hardening.md)
- `RS-H1` (the shared owners after the monorepo audit): [the archived plan](../../../celestina-rs/docs/plans/archive/2026-09-26-hardening.md)

## Scope

- `ARC-1-A` — the four moves, each project's roadmap idle, its active-plans
  index and the status lines that named the checkpoint active, and every
  Markdown link to the moved plans outside the sealed inventories.

## Exclusions

- STYLE-G7, MAG-D1 and AND-6, which still hold rows that are not done.
- The sealed inventories, which keep naming the plans' old paths.

## Build order

1. `ARC-1-A`.

## Implementation exit

The checkpoint closes when `ARC-1-A` is `done`: the documentation guard and the
staged-units check pass on the sealed commit.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| ARC-1-A | `suite:` | done | [inventory](../../inventories/2026-11-25-plan-archives/ARC-1-A.numstat.tsv) | 47 files, +593/-379 | Archive the four finished project plans and mark their roadmaps idle | [evidence](../../evidence/2026-11-25-plan-archives.md) | None |

This plan records intent; it grants no authority.
