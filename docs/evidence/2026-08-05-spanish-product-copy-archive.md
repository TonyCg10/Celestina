# Evidence: archive the delivered Spanish product-copy plan

- **Date:** 2026-08-05
- **Scope:** `LNG-1-B` of the
  [Spanish product-copy plan](../plans/archive/2026-08-04-spanish-product-copy.md)
- **Environment:** repository documentation and Git history only
- **Artifact:** archived plan transition

## Procedure

```sh
bash scripts/check-documentation-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
python3 scripts/check-staged-units.py
```

## Result

The unit records both endpoints of the transition: deletion of the active plan
and addition of the archived plan. The plan keeps its basename, Plan ID,
checkpoint, completed product unit and stable inventory/evidence links, while
the root roadmap advances to the successor PRD-1 checkpoint.

## Observed facts

- LNG-1-A landed in `8008056`; no implementation work remained open.
- The active-plan index already described the suite plan directory as empty,
  but the delivered LNG-1 plan still occupied it.
- PRD-1-A is a separate unit in the same suite-prefixed commit and does not
  claim either archive endpoint.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
