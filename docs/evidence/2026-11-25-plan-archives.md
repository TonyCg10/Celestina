# Evidence: archiving four finished project plans (ARC-1-A)

- **Date:** 2026-11-25
- **Scope:** light unit; `ARC-1-A`: CEL-2, CUP-1, GRA-H1, RS-H1 moved to their projects' archives; each roadmap idle; each active-plans index; the status lines that named a checkpoint active; 25 files whose Markdown links pointed at the moved plans; the ARC-1 plan, opened and closed here; the root roadmap and status
- **Environment:** worktree `suite-ARC-1-A` on `5a0c4856`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-staged-units.py --root . docs/inventories/2026-11-25-plan-archives/ARC-1-A.numstat.tsv
```

## Result

- **Exit:** every command exits 0
- **Exit condition of each plan:** every row of its ledger is `done`, each with its inventory and evidence
- **Review:** 1 round (Sonnet): the four plans are finished and no inventory changed; fixed: statements in the shell's roadmap and status and in Cuprita's roadmap that still called landed units unlanded, the blank line lost under three closed headings, and the visible text of Cuprita's evidence links; deferred: old paths named in backticks in older records, kept as history

## Limits

- Sealed by hand with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and `sealed_diffstat`: `land-unit.py` refuses a unit that moves a plan. The commit must reach `main` by fast-forward with parent `5a0c4856`.
- The sealed inventories of the plans' rows still name their old paths; they are immutable.

## Follow-up

None.
