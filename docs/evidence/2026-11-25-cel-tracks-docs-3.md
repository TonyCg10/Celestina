# Evidence: the capsule sheets design and plan (CEL-TRACKS-DOCS-3)

- **Date:** 2026-11-25
- **Scope:** light unit; `CEL-TRACKS-DOCS-3`: `docs/superpowers/specs/2026-11-25-celestina-capsule-sheets-design.md`, `docs/superpowers/plans/2026-11-25-celestina-capsule-sheets-plan.md`, the superseded note in §6.2 to §6.5 of `docs/superpowers/specs/2026-11-24-celestina-standing-tracks-design.md`, and the TRK-1 row and authorization line
- **Approvals:** the author approved version 7 of the capsule sheets preview, the design (approach 1: one open sheet per output) and the plan with model table A, all on 2026-11-25
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-3`
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
- **Builder check:** one reader planned from the design before it reached the author; its questions became four calls in the design (a press while a sheet is open, overlap, the start order, where the spike's diff lives)
- **Review:** 1 round (Sonnet); fixed: the design's status line, the press routing (`geometry::hit` tells Row, Body and Outside; the bar tree names the piece), two preview rules now in the design (the open piece again closes; the row's glass does nothing), the overshoot clipped to the surface, a replacement table for the tests of the removed popup API, the spotlight's length and core in the design, the dropped hover and press change recorded, the existing empty-workspace copy reused, one name per renamed test, `shown_page_input`, the wheel marked new, who updates the STATUS and ROADMAP lines; deferred: none

## Limits

- Documents only: no code, build or nested run. The throwaway prototypes behind the design (the glass of a hanging panel, the progressive blur's cost) ran in a nested niri and are described in the design; their code is not in the repository.

## Follow-up

None.

## Landing

- **Base revision:** `c005272856bec91ca0eba784dc2eddc4cbf61789`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
