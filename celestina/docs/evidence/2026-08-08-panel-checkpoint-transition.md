# WMAP-1-B — archive the workspace map and open PANEL-1

- **Date:** 2026-08-08
- **Scope:** Celestina administrative unit `WMAP-1-B`
- **Base:** `d56178cecb498b5820a3c9e47b42e3ec5a96e9f5`
- **Artifact:** repository records only; no deployable artifact changed
- **Environment:** local Git checkout on Linux, with the repository guards and
  staged-unit checker
- **Plan:** [the archived workspace window map](../plans/archive/2026-08-08-workspace-window-map.md)
- **Successor:** [PANEL-1 — borderless glass panel](../plans/active/2026-08-08-panel-glass-redesign.md)
- **Version:** unchanged; this unit changes records only

## What changed

The delivered workspace-map plan moved from the active plan directory to its
stable archive endpoint. Its existing `WMAP-1-A` inventory and evidence were
not edited or moved. The roadmap, status and plan indexes now name `PANEL-1` as
the single active Celestina implementation checkpoint.

The successor is intentionally narrower than the open shell-wide design
discussion. ADR 0002 authorizes only the selected borderless-glass panel slice,
and the `PANEL-1` ledger explicitly excludes menu, overlay, clock, calendar and
weather redesign work.

## Procedure

The active and archived plan endpoints, roadmap checkpoint, successor ledger,
decision, discussion and validation row were inspected as one transition. The
staged boundary was then compared with the immutable inventory before commit.

## Automated evidence

- `bash scripts/check-architecture-contract.sh`
- `python3 scripts/version_tool.py check`
- `python3 scripts/check-staged-units.py celestina/docs/inventories/2026-08-08-workspace-window-map/WMAP-1-B.numstat.tsv`
- `git diff --cached --check`

## Result

The archive transition is complete, the prior delivery remains immutable and
the repository has one active Celestina plan for the selected panel slice. No
application bytes, installed artifact or live session changed in this unit.

## Limits

This evidence proves record consistency only. It does not validate the panel's
appearance, compositor blur, input behaviour or any installed application.
