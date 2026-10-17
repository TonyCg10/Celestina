# Evidence: CEL-2-K-SMOKE — the glance smoke's rest-frame budget

- **Date:** 2026-10-17
- **Scope:** `celestina/scripts/smoke.sh` (the two `glance_run` runs) and this
  unit's ledger row. No product code changed.
- **Environment:** CachyOS, worktree `celestina-CEL-2-K-SMOKE`, nested niri
  through `scripts/smoke.sh` only (own bus, own compositor); no network (the
  fake `curl` on `PATH`).
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

The glance runs accepted at most one frame at rest. The system monitor owes
two legitimate ones inside the 14-second run: one when its first reading
appears and one when the ten-second cadence copies a changed reading to the
bar, which lands near the end of the run. Whether that second frame falls
before the exit is a matter of milliseconds, so the check failed on some runs
and passed on others (`rest_frames=2` against a limit of 1).

## Procedure

Four full runs of `sh scripts/smoke.sh --binary <release binary of main>` with
the glance limit temporarily lifted to read the real counts, then the limit set
to 2 and `sh -n` plus the guards.

## Result

The limit is now 2 for both runs, with the reasoning in the script. A bar that
redrew on every two-second sample would draw about six frames in 14 s, so the
check still catches that regression. Measured on `main` at 259821ba with the
limit lifted, glance runs reported rest_frames of 1, 1, 1, 1, 2, 1, 2, 1 across
four full smokes, and one outlier of 6 (74 frames) while another session was
compiling on the machine.

| Command | Exit | Output |
|---|---|---|
| `sh scripts/smoke.sh --binary <release>` (four runs) | 0 or 1 | glance and glance-weather within budget on every run except the loaded outlier |

## Limits

- The outlier of 6 rest frames under CPU contention is not explained by this
  unit; it is recorded, not hidden. If it recurs on an idle machine the cause is
  a real redraw and needs its own investigation.
- Three of the four full smokes ended with `the shell exited with 1 (clipboard
  run)`, which is the clipboard unit's run (CEL-2-G), not the glance runs; it
  needs its own fix by that unit.
- Nothing was stashed.

## Landing

- **Base revision:** `259821ba2afe961f74b84d084e2d1f3472ae959e`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact is not verified yet; run verify-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5867a79adba934cf26b54772feeeabb568684ba99601a6f329d4e6815665d23f, verification_fingerprint sha256:d75b34fc12b6b3a2acecf2787a830ce263a8e4ae6a19dd7c2b3e89d33cd2115e
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
