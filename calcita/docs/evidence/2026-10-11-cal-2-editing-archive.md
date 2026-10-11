# Evidence: archive the delivered CAL-2 editing plan

- **Date:** 2026-10-11
- **Scope:** `CAL-2-D` of
  [CAL-2's plan](../plans/archive/2026-10-11-cal-2-editing.md)
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
Calcita roadmap becomes idle with no active checkpoint; the author named no
next Calcita checkpoint, so the plan names no successor.

## Observed facts

- CAL-2's three delivery rows were `done`: `CAL-2-A`, every annotation
  shown with the annotation model and the coordinates, landed as 1.1.0
  (990853f1), `CAL-2-B`, the edit mode, the three tools and saving, as
  1.2.0 (1c47c95f) and `CAL-2-C`, forms shown, text fields and checkboxes
  filled and saved, as 1.3.0 (b512cd50); no implementation work remained
  open.
- The Calcita roadmap still named CAL-2 as the active checkpoint with
  `CAL-2-C` `active` and linked the ledger as the active plan; the status
  called CAL-2 the active checkpoint, said `CAL-2-C` was implemented in its
  session worktree and not yet landed, gave the checkout's version as 1.2.0
  installed by `CAL-2-B`'s landing and marked the stamped form copy and the
  forms bullet «not landed»; the active-plan index named CAL-2, the three
  CAL-2 evidence records (annotations shown, editing, forms) and the CAL-2
  spec and its task plan linked the ledger's `active/` path, while the
  delivered plan occupied `active/`.
- `calcita/scripts/status-production.sh` reports the artifact current and
  verified and `~/.local/bin/calcita` installed, so the status now gives
  1.3.0 as installed under the author's prefix.
- No file outside Calcita links this plan, so no suite unit was needed
  before this archive.
- The task plan's Task 1 lists the ledger among the files it creates at the
  `active/` path where it was created; that historical step is left as
  written. The CAL-1 archive's evidence names the plan's basename only and
  stays unchanged.
- The inventories of `CAL-2-A` to `CAL-2-C` name the old path as their
  pathspec; inventories are immutable, so they were left unchanged.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `CAL-2-D` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `CAL-1-E` was.
- `VAL-CAL-OPEN`, `VAL-CAL-SEARCH`, `VAL-CAL-DARK`, `VAL-CAL-ANNOT`,
  `VAL-CAL-EDIT` and `VAL-CAL-FORM` stay pending for the author; they do
  not block, and this unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
