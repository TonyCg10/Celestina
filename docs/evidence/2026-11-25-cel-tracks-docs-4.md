# Evidence: the focus indicator in the capsule sheets design (CEL-TRACKS-DOCS-4)

- **Date:** 2026-11-25
- **Scope:** light unit; `CEL-TRACKS-DOCS-4`: §4.4 of `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md`, Task B5 in `docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md`, and the TRK-1 row
- **Why:** the author approved the focus indicator ("solo foco": I5 of the preview); the design had described a spotlight from a misreading, which CEL-D-002 shipped and CEL-D-003 replaced
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-4`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
git diff --numstat origin/main... -- . ':!*.md'
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** every command exits 0; 0 lines outside documents
- **Review:** 1 round (the controller read the diff against the landed code of CEL-D-003: `Indicator::dim`, `child_opacity`, `paint_row`); nothing to fix

## Limits

- Documents only: no code, build or nested run.

## Follow-up

None.

## Landing

- **Base revision:** `dafe4aace3bd0858000193a8e82275221fd8f2f9`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
