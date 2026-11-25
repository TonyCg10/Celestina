# Evidence: the smoke's phases as verification inputs and sensitive paths (DEV-1-D)

- **Date:** 2026-11-25
- **Scope:** `DEV-1-D`, an ordinary unit (it changes `docs/projects.toml` and `docs/standards/`): the shell's `verification_inputs`, the sensitive-path list, the task plan of `SMOKE-PHASES` copied into the repository
- **Environment:** session worktree on `8a37eb46`; Python 3 from the host
- **Artifact:** not applicable

## Procedure

```sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/test-documentation-contract.sh
python3 scripts/test-preflight-budget.py
sh scripts/test-production-artifacts.sh
```

The smoke's own files were listed with `grep` over every `$here/…` path in
`celestina/scripts/smoke.sh`, `celestina/scripts/smoke/*.sh` and the helpers
they call. The review added what the tests read: `tests/duty_files.rs` walks
all of `celestina/scripts` and `tests/lock_scripts.rs` reads
`lock-rehearsal.sh` and `wallpaper-backdrop-probe.sh`, so the whole directory
is registered. `inputs_changed` and `verification_fingerprint` of
`scripts/production_artifact.py` were then called on this tree.

## Result

- **Exit:** each command exits 0
- **Added inputs:** all of `celestina/scripts` (it replaces the two files listed before) and `celestina/examples` (the fake system bus and the fake MPRIS player the smoke builds and drives)
- **Observed:** a change to `celestina/scripts/smoke/bar.sh`, `nest-run.sh`, `lock-rehearsal.sh` or an example counts as a changed input; editing a phase file changes the verification fingerprint
- **Review:** 1 round; 2 findings fixed (the tests read scripts the first version left out; the sensitive list missed the probes and the fake server), 0 deferred

## Limits

- The landing of this unit re-verifies the shell; that run is the proof that the registry still resolves.

## Follow-up

None.

## Landing

- **Base revision:** `8a37eb463a5bb4f3e0713b4910bd795f55d87277`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:e39e41fd026c2ab518a284d55ce375e6dc6bbf5a124309f6201eb4be278ae64e, verification_fingerprint sha256:9cb503fa0582d5d7fcc6ab7f0294ed7cfeaad8f5d650976028acb97ae542c8b4
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
