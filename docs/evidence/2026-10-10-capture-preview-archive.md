# Evidence: archive the delivered capture preview plan

- **Date:** 2026-10-10
- **Scope:** `PRV-1-F` of the
  [capture preview plan](../plans/archive/2026-10-10-capture-preview.md)
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

- All four PRV-1 ledger rows (`PRV-1-A`, `PRV-1-S`, `PRV-1-T` and `PRV-1-E`)
  were `done`, and both application plans' rows were `done` (Selenita's
  `SEL-2-A` at 1.1.0, Fluorita's `FLU-P1-A` at 1.6.0 and `FLU-P1-B` at
  1.7.0); the two niri window rules have been in the author's configuration
  since `PRV-1-E` (67b45f52). No implementation work remained open.
- The root roadmap and status still named PRV-1 as the active checkpoint,
  with `PRV-1-E`'s box left for this archive, and the roadmap, status, the
  three PRV-1 evidence records that link the plan and the Selenita and
  Fluorita preview plans linked the plan's `active/` path, while the
  delivered plan occupied `active/`.
- The superpowers spec and plan of the capture preview, the `PRV-1-E`
  evidence and the plan's own scope mention the old paths only in code spans
  that record past instructions, so they were left unchanged; inventories
  are immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `PRV-1-F` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `EXT-1-D` was.
- `VAL-SEL-PREVIEW`, `VAL-FLU-EDIT-WINDOW` and `VAL-FLU-TRIM` stay pending
  for the author in each application's `VALIDATION.md`; they do not block,
  and this unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
