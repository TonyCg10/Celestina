# Evidence: SMOKE-FIX — the nested smoke under a throttled nest window

- **Date:** 2026-10-23
- **Scope:** `celestina/scripts/smoke.sh` (the OSD runs, the rest-frame checks
  and a retry of the whole smoke) and `celestina/scripts/dev-session.sh` (the
  nested niri's presentation in `--once` runs), plus this unit's ledger row.
  No product code changed.
- **Environment:** CachyOS, host niri on `wayland-1`, worktree
  `celestina-SMOKE-FIX`, nested niri through `scripts/smoke.sh` and
  `scripts/dev-session.sh --own-bus --monitor none --once` only (own bus, own
  compositor, display `wayland-2`); the shell was never started against the
  live session. Mesa 26.2 on amdgpu. Load average between 1.2 and 12 from
  other sessions compiling; the nest window was behind other windows the
  whole time.
- **Artifact:** none sealed by this record; the landing builds its own.

## What was wrong

The full smoke on a clean `main` (a5427f7a) failed in the OSD run with
`no frame was drawn on the OSD surface`, and some runs failed earlier in the
clipboard run with `frames were drawn at rest beyond one per duty switch (3,
at most 2)`. Both have one cause. The host niri withholds frame callbacks from
a window it does not show (one about every second instead of every refresh),
and the nested niri's winit backend then blocks in `eglSwapBuffers` on every
redraw: a hidden nest serves its Wayland clients about once a second. The
shell's start-up frames (the history load, the first readings) then land after
the one-second window `celestina_wayland::metrics` counts rest frames from, and
the OSD surface, alive for about 700 ms (`timeout_ms` 500 plus the fade and the
retire), was destroyed before the nest sent its configure: no `configure
surface=N` line for it in the journal, so no frame. The OSD run alone passed
3/3 while the nest was shown (`osd_frames` 23–24) and failed when it was not.

No product fault: layer-shell forbids a buffer before the configure, and the
OSD machine destroys a surface that never drew exactly as designed.

## Procedure

1. Reproduce: three full `scripts/smoke.sh --binary <debug binary>` runs on
   a5427f7a with a copy of the script that keeps its work directory; read the
   OSD and clipboard journals.
2. Isolate: the OSD phase alone, three runs with the nest shown, then one with
   it hidden.
3. Fix the OSD run in `smoke.sh` (hold 4000 ms, `--exit-after` a parameter,
   frames counted on the OSD's own surfaces, a distinct failure when the nest
   never configured the surface); three full runs.
4. Add the retry: `nest_throttled` reads the runtime's `frame drawn` lines (a
   frame that asked for a callback followed by the next frame on that surface
   500 ms or more later); `rest_fail` exits 75 and the script re-executes
   itself up to three attempts. Verified on the kept throttled journals (waits
   of 985–1989 ms) and on healthy ones (none).
5. Fix the cause in `dev-session.sh`: `vblank_mode=0` exported to the nested
   niri of a `--once` run, and an `environment` block appended to the generated
   profile that gives every program the nest starts the caller's value back.
   Probed with `--binary /usr/bin/env --once`.
6. Three full runs with a clean environment, twice (once with the restore
   inline in the nest's command, once with the `environment` block), then the
   guards and the commit.

Two reviews by a different model (Opus 5.5, high reasoning): the first
rejected a detector based on `metrics: animated frame comp_interval_ms` (that
interval also spans a healthy idle gap) and it was replaced; the second
accepted the `dev-session.sh` mechanism with four adjustments, all applied.

## Result

| Command | Exit | Output |
|---|---|---|
| `scripts/smoke.sh` on a5427f7a (three full runs) | 1, 1, 1 | `frames were drawn at rest beyond one per duty switch (3, at most 2)` (clipboard), then `no frame was drawn on the OSD surface` twice |
| OSD phase alone, nest shown (three runs) | 0, 0, 0 | `osd_frames` 24, 24, 23 |
| OSD phase alone, nest hidden (one run) | 1 | `no frame was drawn on the OSD surface`; no `configure` for the OSD surface in the journal |
| `scripts/smoke.sh` with the OSD fix only (three full runs) | 0, 0, 0 | `osd_frames` 13, 13, 14; later tandas with the nest hidden failed in other phases (clipboard, phone) |
| `scripts/smoke.sh` with the retry, nest hidden (three full runs) | 1, 1, 1 | every attempt `inconclusive (the nest withheld a frame callback for 988 ms)`; the third attempt fails with that note; one phone run with no chained frame failed outright |
| `vblank_mode=0 scripts/smoke.sh` (three full runs) | 0, 0, 0 | `smoke: OK`, zero retries, `osd_frames` 96, 244, 372 |
| `scripts/smoke.sh` after the `dev-session.sh` fix, clean environment (three full runs, twice) | 0, 0, 0 / 0, 0, 0 | `smoke: OK`, zero retries, media phase included, `osd_frames` 359–378 |
| `dev-session.sh --own-bus --monitor none --binary /usr/bin/env --once` | 0 | no `vblank_mode` in the nest's command; with `vblank_mode=1` from the caller, `vblank_mode=1`; `WAYLAND_DISPLAY=wayland-2` |
| `scripts/check-architecture-contract.sh` | 0 | Architecture contract: OK |
| `scripts/check-documentation-contract.sh` | 0 | Documentation contract: OK |
| `python3 scripts/check-language-contract.py` | 0 | Language contract: OK (131 legacy file(s) ratcheted) |
| `python3 scripts/check-sealed-colours.py` | 0 | Sealed colour contract: OK (4 colour(s)) |
| `sh -n` on both scripts | 0 | — |

The three-attempt retry fired zero times in every run after the
`dev-session.sh` fix; it stays as a safety net.

## Limits

- `shellcheck` is not installed on this machine; the scripts were checked
  with `sh -n` and by review only.
- The debug binary ran from a copy under a private `debug/` directory so that
  other sessions' builds could not replace it mid-run; the landing's smoke
  runs the release artifact.
- The OSD run no longer exercises the 500 ms minimum hold end to end (the
  unit test in `celestina-shell::settings` still covers the clamp), and the
  verbs now re-show the live surface instead of creating a second one.
- A throttled nest whose run has no chained animation frame is not detected
  by `nest_throttled`; with the `dev-session.sh` fix that nest no longer
  stalls, so the case did not recur.
- Unthrottled, the nest redraws as fast as it can while a client animates
  (`osd_frames` in the hundreds); the smoke gates no upper bound on animated
  frames, and the interactive nest is left as it was.
- Nothing was stashed.

## Landing

- **Base revision:** `a5427f7a889338fb26bd949f06b4d101fe023eae`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact is not verified yet; run verify-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:6733761757d5e3e11c08baa26598fe5f11bda54ad66dbc698078cd9633301433, verification_fingerprint sha256:6f747e1db4ed1d9d95a727e8cc260b25b7dc460d09988686b0450a1f0c840e1f
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
