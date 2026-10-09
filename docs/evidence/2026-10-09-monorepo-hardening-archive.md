# Evidence: archive the delivered monorepo hardening plan

- **Date:** 2026-10-09
- **Scope:** `AUD-1-S` of the
  [monorepo hardening plan](../plans/archive/2026-09-26-monorepo-hardening.md)
- **Environment:** repository documentation and Git history only
- **Artifact:** archived plan transition

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/test-land-unit.py
python3 scripts/test-production-artifacts.py
bash scripts/test-production-common.sh
```

## Result

The unit records both endpoints of the transition: deletion of the active plan
and addition of the archived plan. The plan keeps its basename, Plan ID,
checkpoint, completed units and stable inventory/evidence links, while the root
roadmap becomes idle with no active checkpoint, because the author named no
successor.

## Observed facts

- All 20 AUD-1 ledger rows (`AUD-1-A` to `AUD-1-R` and the two `HALT-SHELL`
  rows) were `done`; no implementation work remained open.
- The root roadmap and status still named AUD-1 as the active checkpoint, and
  the audit, hardening and tooling evidence records linked the plan's
  `active/` path, while the delivered plan occupied `active/`.
- Inventories under `docs/inventories/2026-09-26-monorepo-hardening/` are
  immutable and keep their historical `active/` pathspec rows; the Cuprita
  plan and the superpowers plans link or mention only inventories or record
  past instructions, so they were left unchanged.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `AUD-1-S` row was opened `active` and sealed `done` on its
  session branch with `scripts/landing.py`'s `numstat_rows`,
  `render_inventory` and `sealed_diffstat` functions, as `LND-1-F` was.
- `scripts/test-land-unit.py` `RealGuardLanding` opened its fixture plan with an
  empty ledger when the roadmap is idle, and the documentation contract rejected
  the test's own side-note commit; that setup commit now skips the hooks, as the
  fixture plan's setup commit does. The merge hooks under test are unchanged.

## Limits

This is an administrative documentation transition. It changes one test's setup commit and no product
version, artifact, runtime configuration or live session.
