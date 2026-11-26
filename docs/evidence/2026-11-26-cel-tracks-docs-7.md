# Evidence: the menu window that blurs out towards its piece (CEL-TRACKS-DOCS-7)

- **Date:** 2026-11-26
- **Scope:** `CEL-TRACKS-DOCS-7`: §4.2, §4.3 and §5 of `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md`, the plan `docs/superpowers/plans/2026-11-26-celestina-cel-d-010-plan.md` and the TRK-1 row
- **Why:** after trying celestina 1.26.0 the author found that a menu window closed before its subtitle came back to the bar and approved on 2026-11-26 version 3 of the menu-close preview: the window blurs out, fades and shrinks towards its piece, and the piece crossfades in the bar, blurred, to and from the bar's bottom edge (CEL-D-010 implements it)
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-7`
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
- **Review:** 1 round (the controller read the sections and the plan against the approved preview)

## Limits

- Documents only: no code, build or nested run.

## Follow-up

None.

## Landing

- **Base revision:** `9af18997fab99e0a720698dae61a2a70d0b63c56`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
