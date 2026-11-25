# Evidence: the status keeps the current product (SID-H1-T)

- **Date:** 2026-11-25
- **Scope:** light unit; `SID-H1-T`: `siderita/STATUS.md`, `siderita/docs/history/status-through-2026-11-25.md`, this unit's ledger row
- **Environment:** session worktree on `daa55af2`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
python3 scripts/preflight_budget.py measure
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
```

The 24 per-unit bullets of "Current checkout truth" (`Delivered as …`,
`Committed (recorded before its landing) …` and the `SID-H1-…` records) moved
verbatim to the history, their relative links rewritten for the new depth.
The header's checkpoint line now says what the ledger says: every `SID-H1`
row is done but `SID-H1-D`. One bullet names the version of the checkout.

## Result

- **Exit:** every command exits 0
- **Size:** `siderita/STATUS.md` 30 472 B before, 8 383 B after; the `now` tier of `siderita` from 51 266 B to 29 276 B (limit 53 248, unchanged)
- **Review:** 1 round (Sonnet, all three status trims together): nothing lost, no broken link, no false statement; the open limits that had moved only to the history are named again in the status, with the pending validations; deferred: none

## Limits

- The budget row stays at 53 248: `scripts/preflight-baseline.tsv` is not a ratchet a `siderita:` commit may lower.

## Follow-up

None.

## Landing

- **Base revision:** `dd9827125c986c51ef303a6f85107ec8a5c864e3`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: the toolchain changed since the build; run build-production.sh
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:23efc5fefe89b521935feec6093ada4089aaf40f396e74426eaa0247b3b47b4b, verification_fingerprint sha256:df6b621ae12bd7df71d4532d7bd90539fa98ca17f32c257f9f559f40f1a3ac90
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
