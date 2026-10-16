# Evidence: CEL-1-I — documents after the CEL-1-H landing

- **Date:** 2026-10-16
- **Scope:** documents only: `celestina/STATUS.md`, `celestina/ROADMAP.md`, the
  CEL-1-I row of `celestina/docs/plans/active/2026-10-14-cel-1-island.md` and
  this record. No source, manifest or script changed. `celestina-qt/` and the
  root documents are untouched.
- **Environment:** Linux 7.2.10-1-cachyos; session worktree
  `CELESTINA.worktrees/celestina-CEL-1-I` on `unit/celestina/CEL-1-I`, based on
  `4d15b038` (celestina 1.0.0 landed in `00c6d0bb`).
- **Artifact:** not applicable.

## Procedure

From the worktree root:

```sh
bash scripts/check-documentation-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py check
```

## Result

- STATUS no longer says the exit is "prepared on its unit branch": it states
  that CEL-1-H landed as celestina 1.0.0 (`00c6d0bb`, kind `release`), that all
  nine VAL-CEL rows are still `pending`, and that the focus is the author's
  validation and the deliveries of `docs/handover.md`.
- ROADMAP: the CEL-1-H row is `done` and CEL-1-H joins "Closed evidence".
  `Status` stays `active` with checkpoint CEL-1 on purpose: the documentation
  guard rejects an active plan under a `done` or `idle` roadmap
  (`orphan active plan`), and this unit's own open ledger row keeps the plan
  active. An administrative unit archives the plan and then sets the roadmap
  to `done` with checkpoint `none`.
- Guards: all four exit 0; see Guard runs.

## Guard runs

- `check-documentation-contract.sh`: first run exit 1 (`invalid status: open`
  in the new ledger row; the ledger vocabulary is planned, active, blocked,
  done); after the row became `active`, exit 0 (`Documentation contract: OK`).
  `erratum` notices for immutable historical inventories were printed and not
  changed.
- `check-architecture-contract.sh`: exit 0 (`Architecture contract: OK`).
- `check-language-contract.py`: exit 0 (`Language contract: OK`, 131 legacy
  files ratcheted).
- `version_tool.py check`: exit 0 (`version-contract: OK (12 owners)`).

## Limits

- Documents only; nothing was run against the shell or the author's session.
- The nine VAL-CEL rows are unchanged and pending; no agent closes them.

## Follow-up

An administrative unit archives the CEL-1 plan and sets the roadmap to `done`
once this unit has landed.

## Landing

- **Base revision:** `4d15b03830259bd75d4e11be4587d57eddfcd1d0`
- **Check:** `production_artifact.py check celestina --require-verified` exit 0: artifact: celestina current
- **Build:** artifact current; no build
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
