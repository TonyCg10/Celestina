# Evidence: the open-menu indicator after the author's live test (CEL-TRACKS-DOCS-5)

- **Date:** 2026-11-26
- **Scope:** `CEL-TRACKS-DOCS-5`: §4.2 to §4.4, §5 and §7 of `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md`, the plan `docs/superpowers/plans/2026-11-26-celestina-cel-d-008-plan.md` and the TRK-1 row
- **Why:** after trying celestina 1.23.0 the author asked for the pieces around an open sheet at full opacity and a single pill on the open piece; the author approved variants O1 and P1 of the preview on 2026-11-26 (CEL-D-007 implements them); the same day the author found that a narrowed capsule hid the other buttons and approved version 4 of the whole-bar preview: the capsule never narrows, the open piece flies down as the menu's subtitle and the bar closes its slot without a gap (CEL-D-008 implements it)
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-5`
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
- **Review:** 1 round (the controller read the sections and the plan against the approved previews)

## Limits

- Documents only: no code, build or nested run.

## Follow-up

None.

## Landing

- **Base revision:** `2fa7cdfe9d4030230be12c32b72570766977a69f`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
