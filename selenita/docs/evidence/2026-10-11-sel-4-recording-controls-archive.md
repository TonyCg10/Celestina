# Evidence: archive the delivered SEL-4 recording controls plan

- **Date:** 2026-10-11
- **Scope:** `SEL-4-C` of
  [SEL-4's plan](../plans/archive/2026-10-10-sel-4-recording-controls.md)
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

- SEL-4's two delivery rows were `done`: `SEL-4-A`, the recording bar and
  the focused recording through `--pick-output`, landed as 1.2.0
  (43b05d1d), and `SEL-4-B`, pausing by segments, landed as 1.3.0
  (07feea4c); no implementation work remained open.
- The Selenita roadmap still named SEL-4 as the active checkpoint with
  `SEL-4-B` `active`, the status still said `SEL-4-B` was awaiting its
  landing, gave the checkout's version as 1.2.0, said VAL-SEL-BAR waited
  for the coordinator to put the bar's niri rule, the `V` binding and the
  portal's `chooser_cmd` in place, and left the activation scanner's
  mention of `RecordFocused` to a suite follow-up; the active-plan index
  named SEL-4, and the roadmap, the status, the recording bar evidence and
  the recording pause evidence linked the plan's `active/` path, while the
  delivered plan occupied `active/`.
- The coordinator applied the three configurations on 2026-10-11 at the
  author's request: the niri rule for the bar (`title` `^Grabando`), the
  `V` binding (`--record-focused`) in the `tools` mode and
  xdg-desktop-portal-wlr's
  `chooser_cmd=/home/toni/.local/bin/selenita --pick-output`; the suite's
  `HOST-HYGIENE.md` records them with their rollback and
  `scripts/activation_contract.py` names `RecordFocused`, both in
  87067c0a. The status now says so.
- `SEL-4-A`'s and `SEL-4-B`'s inventories name the old path as their
  pathspec; inventories are immutable, so they were left unchanged.
- The archive guard requires a `done` unit with an inventory at the same
  endpoint, so the `SEL-4-C` row was sealed `done` in the author's checkout
  with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and
  `sealed_diffstat` functions, as `SEL-3-B` was.
- `VAL-SEL-BAR`, `VAL-SEL-FIXES`, `VAL-SEL-PREVIEW`, `VAL-SEL-SHOT` and
  `VAL-SEL-REC` stay pending for the author; they do not block, and this
  unit does not take them.

## Limits

This is an administrative documentation transition. It changes no product
version, artifact, runtime configuration or live session.
