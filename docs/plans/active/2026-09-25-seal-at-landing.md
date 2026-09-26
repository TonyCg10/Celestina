# LND-1 — seal at landing

- **Opened:** 2026-09-25
- **Plan ID:** seal-at-landing
- **Status:** active
- **Scope:** suite
- **Implementation checkpoint:** LND-1
- **Author-validation checkpoint:** none

## Hypothesis

The closure of a unit is a function of the commit that will be its parent, so
producing it at landing time removes every reason a second session has to wait
for the first.

## Tangible outcome

Sessions work in their own worktrees and never build production there;
`scripts/land-unit.py` lands one unit from the canonical checkout on `main`:
rebase, semantic merge of the hot files, version bump against the current
`main`, the one production run the artifact contract needs (none when the
product's inputs did not move), inventory and ledger closure, the full guard
chain, one typed commit, push, and automatic retry when `main` moved. No guard
changes.

## Scope

- One worktree per session, outside the repository, with a shared Cargo cache
  for the fast checks a session runs, and a refusal to run production entries
  there (`scripts/worktree.sh`, `scripts/production_artifact.py`).
- `scripts/land-unit.py`, which lands one unit on `main` from the canonical
  checkout.
- Tests for both scripts with fixture repositories and fake production
  entries.
- A decision record and the document changes that move closure from "compute
  the inventory by hand" to "request the landing".

## Exclusions

- Any change to a hook, a guard, the inventory format, the version contract or
  the one-active-checkpoint rule; linear history, base = direct parent,
  immutable inventories and one commit per unit stay exactly as they are.
- Landing by merge; it opens the audit gap `.github/workflows/README.md`
  documents and does not solve same-product versions.
- Reserving version numbers when a unit opens.
- More than one active checkpoint per project.
- A shared cache for CMake builds (the shell and `celestina-style`); those
  build only at landing, in the canonical checkout, as today.
- Merging prose conflicts; a `STATUS.md`, `ROADMAP.md`, `VALIDATION.md` or plan
  index conflict stops the landing and is resolved by the author.

## Build order

1. `LND-1-A`: `scripts/worktree.sh`, the `scripts/production_artifact.py`
   refusal and their tests. The worktree directory is outside the repository,
   so no ignore rule is needed.
2. `LND-1-B`: `scripts/land-unit.py` with the merge functions,
   `scripts/test-land-unit.sh`, `scripts/test-land-unit.py` and the CI step.
3. `LND-1-C`: ADR 0011, `docs/contracts/landing.md`, the registry row in
   `suite.shared_rules`, and the amendments to `AGENTS.md`, `CONTRIBUTING.md`,
   `change-policy.md`, `plan.md` and `docs/README.md`.

## Implementation exit

```sh
bash scripts/check-architecture-contract.sh
sh scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
bash scripts/test-production-artifacts.sh
bash scripts/test-worktree.sh
bash scripts/test-land-unit.sh
python3 scripts/check-staged-units.py
```

No product version moves: every unit is `suite-maintenance` and ships no
product behaviour.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LND-1-A | `suite:` | done | [inventory](../../inventories/2026-09-25-seal-at-landing/LND-1-A.numstat.tsv) | 7 files, +510/-0 | Add the session worktree entry with its shared Cargo cache and refuse production runs inside a session worktree | [evidence](../../evidence/2026-09-25-seal-at-landing.md) | None |
| LND-1-B | `suite:` | done | [inventory](../../inventories/2026-09-25-seal-at-landing/LND-1-B.numstat.tsv) | 9 files, +3102/-4 | Add the landing tool that rebases, merges the hot files, bumps, builds only when the artifact is stale, seals, guards, commits and pushes one unit | [evidence](../../evidence/2026-09-25-seal-at-landing-tool.md) | None |
| LND-1-C | `suite:` | done | [inventory](../../inventories/2026-09-25-seal-at-landing/LND-1-C.numstat.tsv) | 12 files, +520/-23 | Record the decision and move closure from hand-computed inventories to the landing | [evidence](../../evidence/2026-09-25-seal-at-landing-documents.md) | None |
| LND-1-D | `suite:` | done | [inventory](../../inventories/2026-09-25-seal-at-landing/LND-1-D.numstat.tsv) | 15 files, +709/-133 | Fix what the final review found before the first landing: keep session plan edits, build affected consumers, close after a landing, guard before building, refuse a landed unit, and align the mandatory documents | [evidence](../../evidence/2026-09-25-seal-at-landing-review.md) | None |
| LND-1-E | `suite:` | done | [inventory](../../inventories/2026-09-25-seal-at-landing/LND-1-E.numstat.tsv) | 16 files, +1750/-251 | Fix the findings parked by the landing reviews: scope close to the owner, verify without rebuilding, merge lockfiles and ratchet comments soundly, refuse early, recover on every interruption, and align the remaining documents | [evidence](../../evidence/2026-09-25-seal-at-landing-parked.md) | None |

## Paired suite transition

The root roadmap may name one active implementation checkpoint, and exactly one
active plan may name it. PRD-1's unit was delivered in `79cb124`, but its plan
stayed in `active/`. Its own administrative `PRD-1-B` unit therefore owns the
archive transition, evidence and exact inventory. `LND-1-A` shares the suite
commit and prefix without claiming those paths.
