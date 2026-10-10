# Evidence: archive the delivered SEL-1 foundation plan

- **Date:** 2026-10-10
- **Scope:** `SEL-1-G` of
  [SEL-1's plan](../plans/archive/2026-10-09-sel-1-foundation.md)
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
Selenita roadmap becomes idle with no active checkpoint, because the author
named no successor.

## Observed facts

- All six SEL-1 delivery rows (`SEL-1-A` to `SEL-1-F`) were `done`: the
  foundation closed at 1.0.0 with `SEL-1-C`, the bug units `SEL-1-D` and
  `SEL-1-E` brought it to 1.0.2, and `SEL-1-F` (64239da0) added the suite's
  capture portal build; no implementation work remained open.
- The Selenita roadmap still named SEL-1 as the active checkpoint with
  `SEL-1-C` `active`, the status still gave the checkout's version as 0.3.0,
  the active-plan index named SEL-1, and seven Selenita evidence records
  linked the plan's `active/` path, while the delivered plan occupied
  `active/`.
- The superpowers plan of the reading and capture program mentions the old
  path only in code spans that record past instructions, so it was left
  unchanged; inventories are immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `SEL-1-G` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `EXT-1-D` was.
- `VAL-SEL-SHOT` and `VAL-SEL-REC` stay pending for the author; they do not
  block, and this unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
