# Evidence: the status keeps the current product (SEL-5-A)

- **Date:** 2026-11-25
- **Scope:** light unit; `SEL-5-A`: `selenita/STATUS.md`, `selenita/docs/history/status-through-2026-11-25.md`, the SEL-5 plan (opened and closed here), `selenita/ROADMAP.md`'s build order and SEL-5 paragraph, the active-plans index
- **Environment:** worktree `selenita-SEL-5-A` on `daa55af2`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
python3 scripts/preflight_budget.py measure
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-staged-units.py --root . selenita/docs/inventories/2026-11-25-sel-5-status-trim/SEL-5-A.numstat.tsv
```

The 20 bullets of "Current checkout truth" moved verbatim to the history,
their relative links rewritten for the new depth; a summary bullet with the
version and a pointer to the README and the history replaces them. The known
issues stay.

## Result

- **Exit:** every command exits 0
- **Size:** `selenita/STATUS.md` 24 194 B before, 12 114 B after; the `now` tier of `selenita` from 45 042 B to 32 962 B (limit 45 056, unchanged)
- **Review:** 1 round (Sonnet, all three status trims together): nothing lost, no broken link, no false statement; nothing to fix in Selenita; deferred: the long implementation account in the status header, outside this unit's scope

## Limits

- Selenita has no active plan, so `land-unit.py` has nowhere to find this row. The SEL-5 plan is opened and archived in this one commit, sealed by hand with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and `sealed_diffstat`; it must reach `main` by fast-forward with parent `daa55af2`.
- The budget row stays at 45 056: `scripts/preflight-baseline.tsv` is not a ratchet a `selenita:` commit may lower.

## Follow-up

None.
