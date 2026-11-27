# Evidence: the Island notices design and plan (CEL-TRACKS-DOCS-9)

- **Date:** 2026-11-26
- **Scope:** `CEL-TRACKS-DOCS-9` covers:
  - the design `docs/superpowers/specs/2026-11-26-celestina-island-notices-design.md`;
  - the plan `docs/superpowers/plans/2026-11-26-celestina-cel-d-011-plan.md`;
  - the TRK-1 row and its authorization line.
- **Why:** after trying celestina 1.29.0, the author asked for notifications that come out of the Island with motion and close with a right click, and for a better level display. On 2026-11-26 the author chose variant A of the Island notices preview (the Island becomes the notification); CEL-D-011 implements it.
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-9`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
git diff --numstat origin/main... -- . ':!*.md'
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** every command exits 0, and no line changes outside documents.
- **Review:** 1 round. The controller read the design and the plan against the chosen preview and a read-only map of the code.

## Limits

- Documents only: no code, build or nested run for this unit. The "today" facts come from the code and from one nested capture of 1.29.0 made for the preview (a notification with buttons, a burst of three, the volume display).
- Whether niri draws the Top layer under a fullscreen window was not checked live; the design keeps the notices on the Overlay layer, so it does not depend on it.

## Follow-up

None.

## Landing

- **Base revision:** `4a90c58ba04650c1081bf82f4626e3066dc29fbf`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
