# Evidence: DEV-1-OPEN — opening the development speed plan

- **Date:** 2026-11-24
- **Scope:** documents only: the DEV-1 plan
  (`docs/plans/active/2026-11-24-dev-speed.md`), ADR 0013 and its index row,
  the approved design and the first task plan under `docs/superpowers/`, the
  RSH-1 plan moved to `docs/plans/archive/` with `Closed`, `Successor` and
  `Status` set, the root roadmap and status, the active-plans index, one link
  in `docs/evidence/2026-10-14-shell-rename.md` retargeted to the archive, and
  this unit's row in the RSH-1 ledger. No source, manifest or version changed.
- **Environment:** CachyOS, worktree `suite-DEV-1-OPEN`, branch
  `unit/suite/DEV-1-OPEN` on `d4382512`
- **Artifact:** not built; no production entry ran in the session

## Procedure

Run from the worktree root with everything staged, after the last edit:

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/test-documentation-contract.sh
python3 scripts/check-staged-units.py --root . docs/inventories/2026-10-14-rust-shell/DEV-1-OPEN.numstat.tsv
```

## Result

| Command | Exit | Output |
|---|---|---|
| `bash scripts/check-documentation-contract.sh` | 0 (second run; the first, 1) | OK; the first run found a broken link to the moved RSH-1 plan in `docs/evidence/2026-10-14-shell-rename.md`, retargeted to the archive |
| `python3 scripts/check-language-contract.py` | 0 | OK (128 legacy files ratcheted) |
| `bash scripts/check-architecture-contract.sh` | 0 | OK |
| `bash scripts/test-documentation-contract.sh` | 0 | OK |
| `python3 scripts/check-staged-units.py --root . docs/inventories/2026-10-14-rust-shell/DEV-1-OPEN.numstat.tsv` | 0 | OK |

## Limits

- The unit could not pass `scripts/land-unit.py` (the archive of a plan and a new plan in one branch); it was sealed in the session worktree with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and `sealed_diffstat`, as `CEL-2-OPEN` was. The base is `d4382512b1e405a6b778f0c780316115b8f35904`, and the commit must reach `main` by fast-forward with that parent.
- `unit/suite/CEL-TRACKS-OPEN` had not landed; the author chose on 2026-11-24 to proceed without it, so `ROADMAP.md` and `STATUS.md` carry nothing of the standing tracks.
- The archived RSH-1 plan changes only its metadata (`Closed`, `Successor`, `Status`), this unit's scope line, build-order step and ledger row.
- The copied design and task plan needed no path changes.
- Nothing was stashed.
