# Evidence: archive the delivered seal-at-landing plan

- **Date:** 2026-09-26
- **Scope:** `LND-1-F` of the
  [seal-at-landing plan](../plans/archive/2026-09-25-seal-at-landing.md)
- **Environment:** repository documentation and Git history only
- **Artifact:** archived plan transition

## Procedure

```sh
sh scripts/check-documentation-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
python3 scripts/check-staged-units.py
```

## Result

The unit records both endpoints of the transition: deletion of the active plan
and addition of the archived plan. The plan keeps its basename, Plan ID,
checkpoint, completed units and stable inventory/evidence links, while the root
roadmap becomes idle with no active checkpoint, because the author named no
successor.

## Observed facts

- LND-1-A landed in `9f15c6f`, LND-1-B in `2162080`, LND-1-C in `52050c0`,
  LND-1-D in `eced4f9` and LND-1-E in `a6e89ac`; no implementation work
  remained open.
- The root roadmap and status still named LND-1 as the active checkpoint, and
  the five LND-1 evidence records linked the plan's `active/` path, while the
  delivered plan occupied `active/`.
- The unit was sealed on its session branch with `scripts/landing.py`'s
  `numstat_rows`, `render_inventory` and `diffstat` functions, not landed by
  `scripts/land-unit.py`: its preflight stops on a branch that deletes its
  active plan (`discover_unit`), and this session cannot push `main`.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
