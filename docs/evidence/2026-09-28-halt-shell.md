# Evidence: the Celestina shell is halted

- **Date:** 2026-09-28
- **Scope:** `HALT-SHELL` of the
  [monorepo hardening plan](../plans/archive/2026-09-26-monorepo-hardening.md):
  the author's halt of the Celestina shell (2026-09-27) recorded in
  [the root contract](../../AGENTS.md#halted-projects) and `docs/projects.toml`
  (`halted`), read by `scripts/project_registry.py`, and honoured by
  `scripts/commit_scope.py`, `scripts/landing.py`, `scripts/agent-context.py`
  and `scripts/documentation_contract.py`; the root and shell documents say
  so, and the audit's shell rows are withdrawn
- **Environment:** session worktree `unit/suite/HALT-SHELL` on the author's
  machine (Linux 7.2.8); Python 3.14.7, Git 2.55.0. No production entry ran:
  the fixtures build their own repositories in temporary directories
- **Artifact:** not applicable; the halt builds, verifies and deploys nothing

## Landing correction

The branch first stopped at the landing's `preflight` with
`the branch must change exactly one active plan; found 2`, because it also
rewrote the ledger of the shell's
[SURF-1 plan](../../celestina/docs/plans/active/2026-08-20-persistent-carriers.md),
setting its five unfinished rows to `blocked`, and it had no ledger row or
evidence record of its own. The correction restores that plan byte for byte
to `origin/main`, so its ledger stays exactly as the halt found it
(`SURF-1-A`, `SURF-1-B` and `SURF-1-C` `active`, `SURF-1-E` and `SURF-1-F`
`planned`, all frozen by the halt), rewords the shell roadmap, status and
plan index, this plan's exclusion and the audit addendum to say so, and adds
this unit's row and this record.

## Procedure

```sh
bash scripts/test-commit-scope.sh
sh scripts/test-documentation-contract.sh
python3 scripts/test-land-unit.py
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/version_tool.py check
```

## Result

- **Exit:** every command above exited 0 on the final tree. The
  documentation guard exited 1 only before this record existed, with the
  single error `broken local link: ../../evidence/2026-09-28-halt-shell.md`.
- `test-commit-scope.sh` printed `Commit scope: OK`,
  `test-documentation-contract.sh` printed
  `Documentation contract and agent-context: OK`, and `test-land-unit.py`
  printed `OK`.

## Limits

- The halt stops only what the committed rules read. Until this unit is on
  `main`, the landing tool that runs is `main`'s, which does not know the
  `halted` key, so this unit's own landing still marks the shell as affected
  and runs its registered production entries; the author accepted that one
  run, without activation, on 2026-09-28.
- Nothing here exercises the shell, a compositor or a live session.

## Landing

- **Base revision:** `616245f1157da7fabcc31b6fdb192397ce72ae6e`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina-style verify: verify-production.sh exit 0, manifest source_fingerprint sha256:a07296319641a7bb878fddba20ba6faa4c0bbda5e9a3377366b2145606354c2d, verification_fingerprint sha256:a39198dd80847c11a00e42bf2e8bd274277fda99ece3686436e715b6965ce4c4; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4ac5523bc556c02252a6b668fe8af67ce2291a6738b3eb92285034765622b5d8, verification_fingerprint sha256:2f31c98aedde3f61643a23efa260b5a20551cd6e5922ebdf47abfc205da0ab8f; siderita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:41961220fa5cf9dc774e80fac2444fd1a638d1e05abdada292cc2169eb7f42e3, verification_fingerprint sha256:5f146269db5ff6852038744763f506205bf0d340cdfc00f29c9529f1408de0f1; magnetita build: complete-production.sh exit 0, manifest source_fingerprint sha256:5bcadc527a911cac20f39372bf5991c0116655bb246225f307771f1dd5eebab5, verification_fingerprint sha256:96d0308902aece319e934f56ccbb981a2bb22dd0f33cc9e69e93d53bbb54551b; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:9f4d1b9aa5339d08bb040f7de96d3804b3a8258fb61c54135bdae4fa3dcc3f5d, verification_fingerprint sha256:0f4de089c51a5e14060b99ab2bf3790f3755ff9991a50df16bd54152ce6ba42a; grafita build: complete-production.sh exit 0, manifest source_fingerprint sha256:d72431eb2e5d044437b424701d584d14733a7f63f5cd26141401b3d694b0f263, verification_fingerprint sha256:fab881c835239ab001de0b71556c55ad5dcba385c5c7716abea1fbbf0647a129; fluorita build: complete-production.sh exit 0, manifest source_fingerprint sha256:2f7a2eba3545ce64966cde62acd14a12f9d0b1b46ee05c5d2667fcf09360d601, verification_fingerprint sha256:aa43ea735274b41eedf84155e9e876fa8aca3e060ecca89655ca8e230737560e; hematita verify: verify-production.sh exit 0, deploy-production.sh exit 0, status-production.sh exit 0, manifest source_fingerprint sha256:5ad19471f5da22e1c07cc2dc4bc3df93b78d795cfbbbdbce8ee7f4f16193a0ae, verification_fingerprint sha256:716e3362c7d5f75ee256921911e429b1be81908769f2f8554652c51065a5aa5f
