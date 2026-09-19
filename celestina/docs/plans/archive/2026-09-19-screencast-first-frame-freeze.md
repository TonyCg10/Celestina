# CAST-1 — The shared screen no longer freezes on its first frame

- **Opened:** 2026-09-19
- **Closed:** 2026-09-19
- **Plan ID:** screencast-first-frame-freeze
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** CAST-1
- **Author-validation checkpoint:** none opened; the author shared a screen
  from the live session on the patched backend and confirmed it moves
- **Predecessor:** none
- **Successor:** none

## Hypothesis

The author reported that sharing a screen from any application shows the
output and then never updates it. Celestina owns only the chooser
(`celestina --pick-output`), so if the freeze is reproducible with the chooser
out of the picture, the defect is in the capture path — `xdg-desktop-portal-wlr`
over niri's wlr-screencopy — and the shell's remedy is to ship the backend the
session needs, the way it already ships a patched compositor.

## Tangible outcome

A screen shared from the live session keeps moving. The repository carries
the backport of upstream's fix as a patch beside the compositor patch, a build
script that produces the backend from the installed release without touching
the distribution's binary, and the systemd user drop-in that runs the session
on it, all reversible by deleting one file.

## Scope

- **CAST-1 — the backend backport.** Upstream `xdg-desktop-portal-wlr` 0.8.x
  drives the PipeWire graph itself (`PW_STREAM_FLAG_DRIVER`), so `.process`
  runs only after `pw_stream_trigger_process`; only its ext-image-copy-capture
  path calls it, and niri offers wlr-screencopy alone, so the stream delivered
  the frame primed at `STREAMING` and stalled. Upstream commit c0255d7b
  (2026-08-11) adds the trigger and is in no release.
  `packaging/xdg-desktop-portal-wlr/trigger-process-after-screencopy.patch`
  carries it onto the released tag at the four places the screencopy path
  hands a buffer back; `scripts/build-patched-xdpw.sh` fetches the installed
  release's tag, applies it, builds with a private meson when the host has
  none, installs to `~/.local/libexec` and, with `--install-override`, writes
  the `xdg-desktop-portal-wlr.service` drop-in and restarts the service.
- The same unit bumps the shell to 1.3.2, because the delivered outcome is a
  defect correction in what the session ships, and records the checkpoint in
  the governing documents.

## Exclusions

- No change to the chooser, to Celestina's portal registrations or to the
  session's `xdg-desktop-portal-wlr` configuration: they were correct.
- No move back to `xdg-desktop-portal-gnome` for `ScreenCast`: it would give
  up the session's own chooser to work around a one-line upstream defect.
- No serving of `ScreenCast` by Celestina itself; that remains the Niri
  adapter's later work.
- Window sharing remains unsupported by the wlr backend; the `No supported
  targets specified` lines in the portal's journal are applications asking
  for a single window and are unrelated to the freeze.

## Build order

1. Confirm the capture path and the protocol niri exposes; find the upstream
   defect and its fix.
2. Build the released tag with the backport, install it beside the
   distribution's binary and point the user service at it.
3. Have the author share a screen from the live session.
4. Record the patch, the build script and the override recipe in the
   repository; bump, build, verify and deploy the production bundle.

## Implementation exit

- `scripts/build-patched-xdpw.sh --install-override` reproduces, from a clean
  work directory, the exact binary the session runs (same SHA-256) and leaves
  the service active on it.
- The common architecture, language and documentation guards pass, the
  version contract checks clean, and `scripts/complete-production.sh` builds,
  verifies and deploys the 1.3.2 bundle.
- Author confirmation of a moving shared screen on the live session is
  recorded in the evidence; no `VAL-*` checkpoint is opened because the
  behaviour was observed directly, not left as a pending procedure.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| CAST-1 | `celestina:` | done | [exact inventory](../../inventories/2026-09-19-screencast-first-frame-freeze/CAST-1.numstat.tsv) | Ship the patched screen-sharing backend the session needs, with its build script and override recipe, and bump the shell to 1.3.2 | 12 files, +452/-4 | [delivery evidence](../../evidence/2026-09-19-screencast-first-frame-freeze.md) | author's live share, 2026-09-19 |
