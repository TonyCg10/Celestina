# Evidence: archive the delivered SEL-2 preview plan

- **Date:** 2026-10-10
- **Scope:** `SEL-2-B` of
  [SEL-2's plan](../plans/archive/2026-10-10-sel-2-preview.md)
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

- SEL-2's one delivery row, `SEL-2-A`, was `done`: it landed as 1.1.0
  (4c03e5ce), and the suite's capture preview program `PRV-1` closed with
  `PRV-1-E` (67b45f52) and was archived; no implementation work remained
  open.
- The Selenita roadmap still named SEL-2 as the active checkpoint with
  `SEL-2-A` `active`, the status still gave the checkout's version as 1.0.2
  and said that niri opens the preview tiled and focused until `PRV-1-E`
  adds the window rule, which has been in the author's configuration since
  `PRV-1-E`; the active-plan index named SEL-2, and the roadmap, the status
  and the preview evidence linked the plan's `active/` path, while the
  delivered plan occupied `active/`.
- The superpowers plan of the capture preview and the suite's `PRV-1-E`
  evidence and archived plan mention the old path only in code spans that
  record past instructions, so they were left unchanged; inventories are
  immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `SEL-2-B` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `SEL-1-G` was.
- `VAL-SEL-PREVIEW`, `VAL-SEL-SHOT` and `VAL-SEL-REC` stay pending for the
  author; they do not block, and this unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
