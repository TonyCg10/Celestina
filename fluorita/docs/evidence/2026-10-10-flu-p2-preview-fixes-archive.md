# Evidence: archive the delivered FLU-P2 preview fixes plan

- **Date:** 2026-10-10
- **Scope:** `FLU-P2-B` of
  [FLU-P2's plan](../plans/archive/2026-10-10-flu-p2-preview-fixes.md)
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

- FLU-P2's one delivery row, `FLU-P2-A`, was `done`: it landed as 1.7.1
  (6f5d65af), and the suite's preview fixes program `PRV-2` was archived
  with `PRV-2-B`; no implementation work remained open.
- The Fluorita roadmap still named FLU-P2 as the active checkpoint with the
  `FLU-P2-A` box unticked, the status still said `FLU-P2-A` was in a session
  worktree and not landed and gave 1.7.0 as the checkout's version; the
  active-plan index named FLU-P2, and the roadmap and the status linked the
  plan's `active/` path, while the delivered plan occupied `active/`.
- The suite's archived `PRV-2` plan mentions the old path only in a code
  span that records where the row lived when it was written, so it was left
  unchanged; inventories are immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `FLU-P2-B` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `FLU-P1-C` was.
- `VAL-FLU-EDIT-WINDOW` and `VAL-FLU-TRIM` stay open for the author; they do
  not block, and this unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
