# Evidence: CEL-2-G-SYNC — the clipboard worker's start under load

- **Date:** 2026-10-17
- **Scope:** `celestina/src/clipboard/worker.rs` only (the worker's first sync and its connect), this record and the ledger row.
- **Environment:** Linux 7.2.10-1-cachyos, host load average about 7 from other sessions; nested niri through `scripts/smoke.sh` (`dev-session.sh --own-bus`), never the author's session; debug binary copied from the shared target after `cargo build --locked`.
- **Artifact:** `celestina` debug build of this worktree, copied to the session scratchpad.

## Procedure

The landing of another unit failed in the smoke's clipboard run with
`the compositor did not answer a sync within 1000 ms` followed by
`clipboard: unavailable`: the nested compositor, starting beside other
processes, answered the worker's first sync after more than the pump's one
second. The worker's first sync now waits `STARTUP_SYNC_TIMEOUT` (5 s, on the
worker's own thread, so nothing at rest changes) and the connection is tried
twice when only that answer was late and no stop was asked
(`connect_with_retry`). Every other loss is final and the check that the duty
works is unchanged. Run three times in a row:

```sh
cd celestina && cargo fmt --check && cargo clippy --workspace --all-targets --locked -- -D warnings
scripts/smoke.sh --binary <copy of the debug binary>   # x3
```

## Result

- fmt and clippy clean.
- Smoke, three runs under load 7: two `smoke: OK`; one stopped at the
  clipboard run's **rest-frame** gate (3 frames, at most 2) because of a frame
  at about +2 s with no clipboard event at all in the journal. The sync timeout
  did not recur in any run.

## Limits

- The retry has no unit test: it wraps the real Wayland connect, which the
  fake transports of the worker's tests do not exercise.
- The rest-frame gate remains sensitive to host load (a frame unrelated to the
  clipboard); it was not loosened.
- Nothing was run on the author's session.

## Landing

- **Base revision:** `8b09c6655fb58d4399e395913927c42d9f9df612`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh
- **Build:** celestina verify: verify-production.sh exit 0, manifest source_fingerprint sha256:3698c4217fbcc1b8b9be6d3e24fa44fef3314a5989121a817ce10055195eb5bc, verification_fingerprint sha256:d75b34fc12b6b3a2acecf2787a830ce263a8e4ae6a19dd7c2b3e89d33cd2115e
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
