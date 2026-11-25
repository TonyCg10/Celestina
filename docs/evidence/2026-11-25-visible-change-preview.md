# Evidence: preview visible changes before building them (VIS-1-A)

- **Date:** 2026-11-25
- **Scope:** documents only, on a sensitive path (the root `AGENTS.md`), so an ordinary unit; `VIS-1-A`: the section "Visible changes: preview before building" in `AGENTS.md` and one light-unit line in `docs/standards/verification.md`; the VIS-1 plan, opened and closed here; the root roadmap and status
- **Environment:** worktree `suite-VIS-1-A` on `b912de11`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-staged-units.py --root . docs/inventories/2026-11-25-visible-change-preview/VIS-1-A.numstat.tsv
python3 scripts/agent-context.py celestina
```

## Result

- **Exit:** every command exits 0; the root contract is 15642 bytes, under its preflight budget of 16384
- **Review:** 1 round (Sonnet); fixed: the rule had nowhere to link a light unit's preview, said nothing on where screenshots live, on `VALIDATION.md` or on fixes that restore an approved look, and named states and scaling loosely; deferred: none

## Limits

- A text rule: no guard checks that a preview was shown, and no unit has used it yet.
- Sealed by hand with `scripts/landing.py`'s `numstat_rows`, `render_inventory` and `sealed_diffstat`: a born-archived plan has no open row for `land-unit.py`. The commit must reach `main` by fast-forward with parent `b912de11`.

## Follow-up

None.
