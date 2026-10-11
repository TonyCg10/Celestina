# Evidence: archive the delivered FLU-P1 preview plan

- **Date:** 2026-10-10
- **Scope:** `FLU-P1-C` of the
  [FLU-P1 plan](../plans/archive/2026-10-10-flu-p1-preview.md)
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
checkpoint, completed units and stable inventory/evidence links, while the
Fluorita roadmap becomes idle with no active checkpoint, because the author
named no successor.

## Observed facts

- Both FLU-P1 delivery rows were `done`: `FLU-P1-A`, the floating editor,
  landed as 1.6.0 (cf2668ac), and `FLU-P1-B`, the video trim, as 1.7.0
  (6a05c7f6), the version in `fluorita/Cargo.toml`; the suite's capture
  preview program `PRV-1` closed with `PRV-1-E` (67b45f52) and was archived;
  no implementation work remained open.
- The Fluorita roadmap still named FLU-P1 as the active checkpoint with both
  of its boxes unticked, the status still described `FLU-P1-B` as awaiting
  its landing, the active-plan index named FLU-P1, and the roadmap and the
  status linked the plan's `active/` path, while the delivered plan occupied
  `active/`.
- The superpowers plan of the capture preview and the suite's `PRV-1-E`
  evidence and archived plan mention the old path only in code spans that
  record past instructions, so they were left unchanged; inventories are
  immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `FLU-P1-C` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `FLU-H1-G` was.
- `VAL-FLU-EDIT-WINDOW` and `VAL-FLU-TRIM` stay open for the author, beside
  `FLU-H1`'s `VAL-FLU-EDIT`, `VAL-FLU-METADATA` and `VAL-FLU-TEARDOWN`; they
  do not block, and this unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
