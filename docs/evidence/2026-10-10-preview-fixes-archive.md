# Evidence: archive the delivered preview fixes plan

- **Date:** 2026-10-10
- **Scope:** `PRV-2-B` of the
  [preview fixes plan](../plans/archive/2026-10-10-preview-fixes.md)
- **Environment:** repository documentation and Git history only
- **Artifact:** archived plan transition

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

The unit records both endpoints of the transition: deletion of the active plan
and addition of the archived plan. The plan keeps its basename, Plan ID,
checkpoint, completed units and stable inventory/evidence links, while the root
roadmap becomes idle with no active checkpoint, because the author named no
successor.

## Observed facts

- The one PRV-2 ledger row (`PRV-2-A`) was `done`, and both application
  plans' rows were `done` (Selenita's `SEL-3-A`, landed in 69baa0f6 at
  1.1.1, and Fluorita's `FLU-P2-A`, landed in 6f5d65af at 1.7.1); the
  corrected niri rule has been in the author's configuration since 2026-10-10,
  as the plan's implementation exit records. No implementation work remained
  open.
- The root roadmap and status still named PRV-2 as the active checkpoint,
  with its three boxes unticked, and the roadmap, status, active-plan index and
  the `PRV-2-A` evidence linked the plan's `active/` path, while the delivered
  plan occupied `active/`.
- The plan's own scope mentions the Selenita and Fluorita plans' `active/`
  paths only in code spans that record where the rows lived when it was
  written, so they were left unchanged; inventories are immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `PRV-2-B` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `PRV-1-F` was.
- `VAL-SEL-FIXES` and `VAL-FLU-EDIT-WINDOW` stay pending for the author in
  each application's `VALIDATION.md`; they do not block, and this unit does
  not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
