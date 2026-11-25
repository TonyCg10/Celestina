# Evidence: light units and the review cap (DEV-1-C)

- **Date:** 2026-11-25
- **Scope:** `DEV-1-C`, an ordinary unit (it changes `docs/projects.toml`, a sensitive path): `docs/standards/verification.md`, `docs/templates/evidence-light.md`, the pointers in `AGENTS.md`, `CONTRIBUTING.md`, `docs/governance/documentation.md` and `docs/templates/README.md`, one reading trigger
- **Environment:** session worktree on `00d0861c`; Python 3 from the host
- **Artifact:** not applicable

## Procedure

```sh
wc -c AGENTS.md
python3 scripts/agent-context.py celestina/src | grep verification
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/test-documentation-contract.sh
python3 scripts/test-preflight-budget.py
```

## Result

- **Exit:** `wc -c` prints 14226 (limit 16384); the second command prints the new trigger; the five guards and tests exit 0
- **Review:** 1 round; 7 findings, 7 fixed (the sensitive list and the line count were widened beyond the plan's text), 0 deferred

## Limits

- The rules are text: no guard checks a light unit's three conditions or the cap on review rounds.
- The pointer in `AGENTS.md` is two lines, shorter than planned: the planned paragraph put Selenita 191 bytes over its reading budget (limit 45056).
- No unit has run under these rules yet; this record is the first written from the short template.

## Follow-up

The author decides whether Magnetita's network and pairing crates join the sensitive list.

## Landing

- **Base revision:** `00d0861c091c3c7b1ededb4a98e56246f1a90ee7`
- **Check:** not applicable: the suite unit changes no registered production or verification input
- **Build:** none; no registered production input changed
