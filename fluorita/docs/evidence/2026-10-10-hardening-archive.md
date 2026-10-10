# Evidence: archive the delivered hardening plan

- **Date:** 2026-10-10
- **Scope:** `FLU-H1-G` of the
  [hardening plan](../plans/archive/2026-09-26-hardening.md)
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

- All six FLU-H1 delivery rows (`FLU-H1-A` to `FLU-H1-F`) were `done`: the
  two audit units `FLU-H1-A` and `FLU-H1-B` and the author's requests of
  2026-10-07, `FLU-H1-C` to `FLU-H1-F`; no implementation work remained open.
- The Fluorita roadmap still named FLU-H1 as the active checkpoint, the
  status still described `FLU-H1-A` and `FLU-H1-B` as prepared on their
  branches, the active-plan index named FLU-H1, and the roadmap, status and
  the two FLU-H1 evidence records of 2026-09-26 linked the plan's `active/`
  path, while the delivered plan occupied `active/`.
- The suite audit records, the archived monorepo hardening plan and the
  superpowers glass-canvas plan mention the old path only in code spans that
  record past instructions, so they were left unchanged; inventories are
  immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `FLU-H1-G` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `EXT-1-D` was.
- `VAL-FLU-EDIT`, `VAL-FLU-METADATA` and `VAL-FLU-TEARDOWN` stay open for the
  author; they do not block, and this unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
