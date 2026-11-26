# Evidence: each capsule menu as its own window (CEL-TRACKS-DOCS-6)

- **Date:** 2026-11-26
- **Scope:** `CEL-TRACKS-DOCS-6`: §4.2, §4.3, §5 and §7 of `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md`, the plan `docs/superpowers/plans/2026-11-26-celestina-cel-d-009-plan.md` and the TRK-1 row
- **Why:** after trying celestina 1.25.0 the author found empty space around the capsule sheets (a sheet as wide as its capsule, page sheets at the Island page's height) and approved on 2026-11-26 version 2 of the menu-window preview: each menu its own window, as large as its content, grown out of its piece (variant B) and lined up with its capsule's outer edge (CEL-D-009 implements it)
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-6`
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

- **Base revision:** `b4a2dedafefdc07d82f8d6d188ef6adacc803641`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
