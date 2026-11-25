# Evidence: the status keeps the current product (HEM-H1-H)

- **Date:** 2026-11-25
- **Scope:** light unit; `HEM-H1-H`: `hematita/STATUS.md`, `hematita/docs/history/status-through-2026-11-25.md`, this unit's ledger row
- **Environment:** session worktree on `daa55af2`
- **Artifact:** not applicable (no production input changes)

## Procedure

```sh
python3 scripts/preflight_budget.py measure
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
```

Moved verbatim to the history, their relative links rewritten for the new
depth: from the header, the `Delivered as …` and `Closed phase …` bullets, the
`HEM-H1-G` smoke fix and the old checkpoint line, which still called
`HEM-H1-A` and `HEM-H1-B` "not landed"; from "Current checkout truth", the
`As of …` and `… closed the checkpoint` records and the bullets that named
1.2.2 as the version built and installed. The new checkpoint line says what
the ledger says (every `HEM-H1` row is done) and one bullet names the
checkout's version, 1.3.2.

## Result

- **Exit:** every command exits 0
- **Size:** `hematita/STATUS.md` 41 842 B before, 6 816 B after; the `now` tier of `hematita` from 63 475 B to 28 548 B (limit 65 536, unchanged)
- **Review:** 1 round (Sonnet, all three status trims together): nothing lost, no broken link, no false statement; the open limits that had moved only to the history are named again in the status, with the pending validations; deferred: none

## Limits

- The budget row stays at 65 536: `scripts/preflight-baseline.tsv` is not a ratchet a `hematita:` commit may lower.
- The `HEM-H1` plan has every row done and is still active; archiving it is not part of this unit.

## Follow-up

None.

## Landing

- **Base revision:** `ca5192c17e5bbac6770e932d689a3edc1f25270d`
- **Check:** `production_artifact.py check hematita --require-verified` exit 1: production-artifact: the toolchain changed since the build; run build-production.sh
- **Build:** hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4a7402abb258b13ab31b1aebcfee131d323c228f3ffd1c138c946a5583498406, verification_fingerprint sha256:c26ca6b61830100aa68ae607bbe972359fc53954c57899e80c4c07b8745a196e
- **Deploy:** after the push: hematita: deploy-production.sh, status-production.sh
