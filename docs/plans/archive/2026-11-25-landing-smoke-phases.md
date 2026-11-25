# DEV-2 — Land a series unit with only its smoke phases

- **Opened:** 2026-11-25
- **Plan ID:** landing-smoke-phases
- **Closed:** 2026-11-25
- **Successor:** none; the author named no next suite checkpoint. Its one row (DEV-2-A) is done
- **Status:** done
- **Authorization:** on 2026-11-25 the author asked that a unit which is one
  of a larger series not run the whole smoke at its landing, the whole smoke
  running at the end of the series; chose that such a landing runs only the
  phases its unit declares, declared on the landing command; and approved
  [the design](../../superpowers/specs/2026-11-25-landing-smoke-phases-design.md) and
  [the plan](../../superpowers/plans/2026-11-25-landing-smoke-phases-plan.md). Opened and closed in one
  commit: TRK-1, the active suite plan, excludes code, and `land-unit.py`
  needs an open row in an active plan
- **Scope:** suite
- **Implementation checkpoint:** DEV-2
- **Author-validation checkpoint:** none

## Hypothesis

The CEL-I-004 pair took about 20 minutes, two shell verifications each ending
in the full smoke of about 4 minutes. A landing that runs only the phases of
the area its unit touched, with the full smoke kept for the last unit of the
series, saves those minutes on every unit but the last without letting a
partially smoked artifact pass for a fully smoked one.

## Tangible outcome

- `scripts/land-unit.py --smoke ID[,ID...]`, checked against the shell's
  `smoke.sh --list`, kept by `--continue`, passed to every production entry as
  `CELESTINA_SMOKE_ONLY`.
- The shell's `verify-production.sh` runs those phases and records the scope
  it ran in `celestina/target/release/smoke-scope`; a landing without
  `--smoke` verifies again an artifact whose scope is not `full`.
- The series rule in [the landing contract](../../contracts/landing.md), the
  verification standard and the shell's `AGENTS.md` and `README.md`.

## Scope

- `DEV-2-A` — the option, the verify script, their tests, the rule, the design
  and plan, and the suite's roadmap and status lines.

## Exclusions

- The repeated release build, clippy and tests of a stacked pair.
- Choosing phases from the changed files.
- `scripts/production_artifact.py` and `docs/projects.toml`, verification
  inputs of every project.

## Build order

1. `DEV-2-A`.

## Implementation exit

The checkpoint closes when `DEV-2-A` is `done`: the landing tests, the
documentation guard and the staged-units check pass on the sealed commit.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| DEV-2-A | `suite:` | done | [inventory](../../inventories/2026-11-25-landing-smoke-phases/DEV-2-A.numstat.tsv) | 15 files, +712/-16 | Land a unit of a series with only its smoke phases and keep the full smoke for the series' last landing | [evidence](../../evidence/2026-11-25-landing-smoke-phases.md) | None |

This plan records intent; it grants no authority.
