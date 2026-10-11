# Evidence: archive the delivered SEL-3 preview fixes plan

- **Date:** 2026-10-10
- **Scope:** `SEL-3-B` of
  [SEL-3's plan](../plans/archive/2026-10-10-sel-3-preview-fixes.md)
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

- SEL-3's one delivery row, `SEL-3-A`, was `done`: it landed as 1.1.1
  (69baa0f6), and the suite's preview fixes program `PRV-2` was archived
  with `PRV-2-B`; no implementation work remained open.
- The Selenita roadmap still named SEL-3 as the active checkpoint with
  `SEL-3-A` `active`, the status still said `SEL-3-A` was not landed, gave
  the checkout's version as 1.1.0 and left Fluorita's adoption of
  `FolderPaths::inside` to `FLU-P2-A`, which landed in 6f5d65af; the
  active-plan index named SEL-3, and the roadmap, the status and the preview
  fixes evidence linked the plan's `active/` path, while the delivered plan
  occupied `active/`.
- The suite's archived `PRV-2` plan mentions the old path only in a code
  span that records where the row lived when it was written, so it was left
  unchanged; inventories are immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `SEL-3-B` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `SEL-2-B` was.
- `VAL-SEL-FIXES`, `VAL-SEL-PREVIEW`, `VAL-SEL-SHOT` and `VAL-SEL-REC` stay
  pending for the author; they do not block, and this unit does not take
  them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
