# Evidence: archive the delivered suite conventions plan

- **Date:** 2026-10-09
- **Scope:** `CONV-1-G` of the
  [suite conventions plan](../plans/archive/2026-10-09-suite-conventions.md)
- **Environment:** repository documentation and Git history only
- **Artifact:** archived plan transition

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-staged-units.py
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

- All six CONV-1 ledger rows (`CONV-1-A` to `CONV-1-F`, with `CONV-1-C`
  delivered as Cuprita's `CUP-1-I` and described in the plan's prose) were
  `done`; no implementation work remained open.
- The root roadmap and status still named CONV-1 as the active checkpoint, and
  the roadmap, status, ADR 0012 and two Cuprita records linked the plan's
  `active/` path, while the delivered plan occupied `active/`.
- The superpowers spec and plan and the `CONV-1-F` evidence mention the old
  path only in code spans that record past instructions, so they were left
  unchanged; inventories are immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `CONV-1-G` row was sealed `done` on its session branch with
  `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `AUD-1-S` was.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
