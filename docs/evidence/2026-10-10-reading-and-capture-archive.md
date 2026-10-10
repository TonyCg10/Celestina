# Evidence: archive the delivered reading and capture plan

- **Date:** 2026-10-10
- **Scope:** `EXT-1-D` of the
  [reading and capture plan](../plans/archive/2026-10-09-reading-and-capture.md)
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

- All four EXT-1 ledger rows (`EXT-1-A`, `EXT-1-B`, `EXT-1-S` and `EXT-1-C`)
  were `done`, and both application plans had closed at 1.0.0 (Calcita 1.0.0
  and Selenita 1.0.0); no implementation work remained open.
- The root roadmap and status still named EXT-1 as the active checkpoint, and
  the roadmap, status and the Calcita and Selenita skeleton evidence and
  foundation plans linked the plan's `active/` path, while the delivered plan
  occupied `active/`.
- The superpowers spec and plan and the `EXT-1-C` evidence mention the old
  path only in code spans that record past instructions, so they were left
  unchanged; inventories are immutable.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `EXT-1-D` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `CONV-1-G` and `AUD-1-S` were.
- The two `HOST-HYGIENE.md` findings of 2026-10-09 stay `pending`: the pin of
  `application/pdf` to `org.celestina.Calcita.desktop` and the installation
  of `gst-plugins-good` for the recording are the author's decisions, and this
  unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
