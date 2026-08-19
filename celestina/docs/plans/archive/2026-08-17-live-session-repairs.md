# Live session repairs — what three monitors broke that one never could

- **Opened:** 2026-08-17
- **Closed:** 2026-08-17
- **Plan ID:** live-session-repairs
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** LIVE-1
- **Author-validation checkpoint:** VAL-R8
- **Successor:** [BUBBLE-1 shell bubble integration](2026-08-17-melibea-bubbles.md)

## Hypothesis

The nest is one output, and the author's session is three. Every defect the
first live migrations exposed is a consequence of that difference or of the
compositor being older than the one the shell was verified against — not of
the shell being wrong in ways a nested session could have shown.

If that is right, each defect has a mechanism that becomes visible the moment
it is measured on the real session rather than reasoned about, and none of
them needs a redesign.

## Tangible outcome

The shell survives ordinary use on three monitors: it does not die when a menu
closes, its membrane connects on every output rather than only the primary
one, the panel shows Wi-Fi and Bluetooth, the session's colours are its own,
and the veil and the dense cards are two visibly different materials.

## Scope

Two units, grouped by where the defect lives rather than by how it was found.

- **LIVE-1-A — The shell survives three monitors.** One predicate owns whether
  compositor blur may be spoken to at all; the attachment lease records its
  output at acquisition instead of re-deriving it from a surface Qt cannot
  place yet; and the connectivity group's presence comes from the readings
  rather than from its own children's rendered visibility.
- **LIVE-1-Z — The closed checkpoint releases its slot.** `LOCK-1` is archived
  and the governing documents move onto this checkpoint. Administrative: it
  changes no behaviour.
- **LIVE-1-B — Two providers stop misreading the machine.** A young gamma
  controller is no longer mistaken for a broken one and a failing output serves
  a bounded backoff; the device listing is read with the escaping nmcli
  actually uses.

### Measured facts this plan is built on

- The crash is `ext_background_effect_surface_v1: error 0: wl_surface was
  destroyed`, and Qt Wayland keeps `QPlatformWindow` alive across exactly the
  window in which the surface is gone.
- The author's primary screen is `HDMI-A-1`; the membrane worked there and
  nowhere else.
- Both connectivity providers published while both indicators were invisible.
- The compositor patch delivers `passes` and `offset` to the shader; the live
  configuration simply carried no global `blur {}` block, so the veil ran at
  niri's default and matched the dense profile.

## Exclusions

- **No compositor change.** The patch was investigated and found correct; the
  binary is unchanged and the tracing used to prove it is removed.
- **No new provider, channel or protocol.** Every repair is to code that
  already existed.
- **No configuration is owned here.** The author's `niri` configuration is
  theirs; what this plan records is which values the material was tuned
  against and why the defaults do not express it.
- **No claim about a full day of use.** That is `VAL-R8`.

## Build order

`LIVE-1-A` first, because the crash ended every session before anything else
in it could be observed, and the other two surface defects were only visible
once a session survived. `LIVE-1-B` is independent of it and was found while
reading for the others.

## Implementation exit

- `LIVE-1-A`: opening and closing menus across three outputs for minutes
  produces no protocol error, where the same use killed the shell in under a
  minute before.
  The author confirms the membrane on all three monitors, and the panel shows
  Wi-Fi and Bluetooth whenever the control centre does.
- `LIVE-1-B`: the suite covers both repairs, and `celestina-shell-core`
  carries the author's own device listing as a regression.
- Whole checkpoint: Rust tests, Clippy, `cargo fmt`, QML lint and CTest pass.
  `scripts/complete-production.sh` is deliberately *not* run here — its smoke
  starts the real host and probes DDC on the live machine, and this checkpoint
  was developed against a build-tree shell on the author's own session.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LIVE-1-A | `celestina:` | done | [inventory](../../inventories/2026-08-17-live-session-repairs/LIVE-1-A.numstat.tsv) | Refuse every compositor-blur request to a surface Qt destroyed while still allowing a region to be armed before its surface is shown; keep the membrane on outputs Qt cannot place yet; derive connectivity presence from the readings | 12 files, +458/-26 | [evidence](../../evidence/2026-08-17-three-monitors.md) | `VAL-R8` |
| LIVE-1-B | `celestina:` | done | [inventory](../../inventories/2026-08-17-live-session-repairs/LIVE-1-B.numstat.tsv) | Stop the night light rebuilding a controller that has not yet reported its size, give a failing output a bounded backoff, and read nmcli's escaped field separators | 6 files, +311/-20 | [evidence](../../evidence/2026-08-17-provider-truthfulness.md) | `VAL-R8` |
| LIVE-1-Z | `celestina:` | done | [inventory](../../inventories/2026-08-17-live-session-repairs/LIVE-1-Z.numstat.tsv) | Archive the closed LOCK-1 plan so this checkpoint may hold the one active slot, and move the roadmap, status and plan index onto LIVE-1 | 15 files, +446/-315 | [evidence](../../evidence/2026-08-17-lock-plan-closure.md) | `VAL-LOCK-1` |
| LIVE-1-Y | `celestina:` | done | [inventory](../../inventories/2026-08-17-live-session-repairs/LIVE-1-Y.numstat.tsv) | Archive this closed plan itself, releasing its slot to `BUBBLE-1` | 4 files, +138/-95 | [archival evidence](../../evidence/2026-08-19-live-session-repairs-archival.md) | `VAL-R8` |
