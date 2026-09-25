# Evidence: archive the delivered desktop-entry registration plan

- **Date:** 2026-09-25
- **Scope:** `PRD-1-B` of the
  [desktop-entry registration plan](../plans/archive/2026-08-05-desktop-entry-registration.md)
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
checkpoint, completed product unit and stable inventory/evidence links, while
the root roadmap advances to the successor LND-1 checkpoint.

## Observed facts

- PRD-1-A landed in `79cb124`; no implementation work remained open.
- The root roadmap still named PRD-1 as its active checkpoint and the root
  status still named LNG-1, while the delivered PRD-1 plan occupied `active/`.
- LND-1-A is a separate unit in the same suite-prefixed commit and does not
  claim either archive endpoint.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
