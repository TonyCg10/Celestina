# Evidence: archiving the SID-H1 plan (SID-H1-U)

- **Date:** 2026-11-25
- **Scope:** light unit; `SID-H1-U`: the SID-H1 plan moved to `siderita/docs/plans/archive/` with `Closed`, `Successor` and `Status`, its scope, build order and ledger rows; `SID-H1-D`, never started, moved with its ledger text to the planned implementation debt in `siderita/STATUS.md`; the roadmap idle; the status's checkpoint line; the active-plans index; links in four evidence records retargeted to the archive
- **Environment:** worktree `siderita-SID-H1-U` on `de9bf3da`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-staged-units.py --root . siderita/docs/inventories/2026-09-26-hardening/SID-H1-U.numstat.tsv
```

## Result

- **Exit:** every command exits 0
- **Exit condition of the plan:** every remaining row, `SID-H1-A` to `SID-H1-C` and `SID-H1-E` to `SID-H1-T`, is `done`, each with its inventory and evidence; `SID-H1-D` left the ledger, as an archived plan holds only done units
- **Correction:** the status's checkpoint line, written by `SID-H1-T`, called `SID-H1-D` "the portal parenting"; that is `SID-M1`'s planned coverage, a separate item. The line now names `SID-H1-D` for what it is
- **Review:** 1 round (Sonnet): 2 findings fixed (a broken sentence in the status's checkpoint line; a roadmap line still putting `SID-H1` first); the moved `SID-H1-D` bullet also names its files now; deferred: none

## Limits

- Sealed by hand with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and `sealed_diffstat`: `land-unit.py` refuses a unit that moves its own plan. The commit must reach `main` by fast-forward with parent `de9bf3da`.
- The sealed inventories of `SID-H1-A` to `SID-H1-T` still name the plan's old path; they are immutable.

## Follow-up

`SID-H1-D` waits in the planned debt until a `celestina-core` local-time owner exists and a plan takes it.
