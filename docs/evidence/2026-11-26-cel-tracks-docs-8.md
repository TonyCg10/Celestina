# Evidence: clearing notifications, the tray fixes and the collapsible tray (CEL-TRACKS-DOCS-8)

- **Date:** 2026-11-26
- **Scope:** `CEL-TRACKS-DOCS-8` covers:
  - the design `docs/superpowers/specs/2026-11-26-celestina-notifications-tray-design.md`;
  - the plans `docs/superpowers/plans/2026-11-26-celestina-cel-i-010-plan.md`, `…-cel-i-011-plan.md` and `…-cel-i-012-plan.md`;
  - the TRK-1 row and its authorization line.
- **Why:** after trying celestina 1.26.0, the author reported three problems:
  - notifications cannot be deleted, and the unread dot stays after the window is opened;
  - many tray icons do not show, and some right-click menus do not open;
  - there is no collapsible tray, Windows style, with an option to hide apps.

  On 2026-11-26 the author approved the notifications and tray preview (variant A, list G2) and the calls in §5 of the design (CEL-I-010 to CEL-I-012 implement it).
- **Environment:** worktree `suite-CEL-TRACKS-DOCS-8`
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
- **Review:** 1 round. The controller read the design and the plans against the approved preview and the code.

## Limits

- Documents only: no code, build or nested run.
- The tray findings come from reading the code and the author's session bus. When the design was written, only shelly-notifications and solaar had registered items, so the apps the author misses (Steam, Discord, nm-applet, blueman) were not observed live.

## Follow-up

None.

## Landing

- **Base revision:** `9bf22bfb89571ceb898d74d2eb408eddcf5f490f`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
