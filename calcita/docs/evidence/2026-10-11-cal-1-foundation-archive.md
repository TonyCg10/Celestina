# Evidence: archive the delivered CAL-1 foundation plan

- **Date:** 2026-10-11
- **Scope:** `CAL-1-E` of
  [CAL-1's plan](../plans/archive/2026-10-09-cal-1-foundation.md)
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
Calcita roadmap becomes idle with no active checkpoint until its successor,
CAL-2, Calcita editing (annotations and forms), is opened next by its own plan
`2026-10-11-cal-2-editing.md`.

## Observed facts

- CAL-1's four delivery rows were `done`: `CAL-1-A` landed as 0.2.0
  (bc9b0432), `CAL-1-B` as 0.3.0 (42595540), `CAL-1-C` closed the
  foundation as 1.0.0 (c486ecb6) and the bug unit `CAL-1-D`, every page on
  opaque paper over the glass canvas, landed as 1.0.1 (79e62e53); no
  implementation work remained open.
- `CAL-1-C`'s row planned the archive right after its landing; the author's
  report on the deployed 1.0.0 opened `CAL-1-D` first, so the plan stayed
  active until this unit.
- The Calcita roadmap still named CAL-1 as the active checkpoint with
  `CAL-1-A`, `CAL-1-B` and `CAL-1-C` `active` and no `CAL-1-D` row; the
  status was dated 2026-10-09, said the `CAL-1-C` landing would release
  1.0.0 and gave the checkout's version as 0.2.0, not installed; the
  active-plan index named CAL-1, and the five Calcita evidence records
  (skeleton, open and pages, search, exit, page contrast) linked the plan's
  `active/` path, while the delivered plan occupied `active/`.
- `calcita/scripts/status-production.sh` reports the artifact current and
  verified and `~/.local/bin/calcita` installed, so the status now gives
  1.0.1 as installed under the author's prefix.
- The suite's archived EXT-1 plan held the one real Markdown link to this
  plan outside Calcita; it was repointed at Calcita's `archive/` folder by
  76419e99 before this archive, since a `calcita:` unit may not touch it.
- The inventories of `CAL-1-A` to `CAL-1-D` and the suite's `EXT-1-A` and
  `EXT-1-D` name the old path as their pathspec; inventories are immutable,
  so they were left unchanged.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `CAL-1-E` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `SEL-3-B` was.
- `VAL-CAL-OPEN`, `VAL-CAL-SEARCH` and `VAL-CAL-DARK` stay pending for the
  author; they do not block, and this unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
