# SEL-5 — The status keeps the current product

- **Opened:** 2026-11-25
- **Plan ID:** sel-5-status-trim
- **Closed:** 2026-11-25
- **Successor:** none; the author named no next Selenita checkpoint. Its one row (SEL-5-A) is done
- **Status:** done
- **Authorization:** the author asked on 2026-11-25 to trim the status of
  Selenita, Siderita and Hematita, whose first reading tier stood at or near
  its limit in `scripts/preflight-baseline.tsv`. Opened and closed in one
  commit: one documents-only unit, merged by fast-forward
- **Scope:** selenita
- **Implementation checkpoint:** SEL-5
- **Author-validation checkpoint:** none

## Hypothesis

A session working on Selenita reads its `STATUS.md` before acting. Its
"Current checkout truth" is a 12.7 KB account of every behaviour, which the
README already gives; a summary and a pointer leave Selenita well under its
reading budget without losing a sentence.

## Tangible outcome

`selenita/STATUS.md` with a short "Current checkout truth" and its known
issues unchanged; the former account, verbatim, in
`selenita/docs/history/status-through-2026-11-25.md`.

## Scope

- `SEL-5-A` — the status, its history, this plan, the roadmap's build order
  and closed evidence, and the active-plans index.

## Exclusions

- Any source, test or production input; the known issues; the README.
- Lowering Selenita's row in `scripts/preflight-baseline.tsv`, which a
  `selenita:` commit may not change.

## Build order

1. `SEL-5-A`.

## Implementation exit

The checkpoint closes when `SEL-5-A` is `done`: the documentation guard and
the staged-units check pass on the sealed commit.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SEL-5-A | `selenita:` | done | [inventory](../../inventories/2026-11-25-sel-5-status-trim/SEL-5-A.numstat.tsv) | 7 files, +325/-189 | Move the account of the checkout out of the status into its history | [evidence](../../evidence/2026-11-25-status-trim.md) | None |

This plan records intent; it grants no authority.
