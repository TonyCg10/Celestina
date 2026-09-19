# CAST-1 delivery — the shared screen no longer freezes on its first frame

- **Date:** 2026-09-19
- **Scope:** Celestina unit `CAST-1`
- **Artifact:** see the exact inventory at
  `docs/inventories/2026-09-19-screencast-first-frame-freeze/CAST-1.numstat.tsv`;
  the session's backend is `~/.local/libexec/xdg-desktop-portal-wlr`,
  SHA-256 `8f4cc93badd8eb7924de06b03e4c293d818b1245e022aaa4dc9392acab1ac835`
- **Environment:** the author's live niri 26.04 session (three outputs),
  `xdg-desktop-portal-wlr` 0.8.4-1.1 and `xdg-desktop-portal` 1.22.1 from the
  distribution, PipeWire 1.6.8; the host carries no meson or ninja
- **Plan:** [screencast first-frame freeze](../plans/archive/2026-09-19-screencast-first-frame-freeze.md)
- **Validation:** the author shared a screen from the live session on the
  patched backend and confirmed it moves

## Procedure

### Locating the defect

The portal routing (`~/.config/xdg-desktop-portal/niri-portals.conf`) sends
`ScreenCast` to `wlr`, whose configuration runs `celestina --pick-output` as
its chooser. The chooser exits once a name is printed, so it cannot hold a
stream. The backend's own journal carried nothing for the freeze. What niri
advertises decided the path:

```sh
strings /usr/bin/niri | grep -E '^(ext_image_copy_capture_manager_v1|zwlr_screencopy_manager_v1)$'
# zwlr_screencopy_manager_v1
strings /usr/lib/xdg-desktop-portal-wlr | grep -E '^(ext_image_copy_capture|zwlr_screencopy)' | sort -u
# both families: the backend prefers ext-image-copy-capture when offered
```

Upstream's tracker matched the symptom exactly: with `PW_STREAM_FLAG_DRIVER`
the stream's `.process` runs only after `pw_stream_trigger_process`, which
only `src/screencast/ext_image_copy.c` calls; `wlr_screencopy.c` never does,
so the stream delivers the frame primed at `STREAMING` and stalls. Fixed
upstream by c0255d7b on 2026-08-11 (one call added to the shared
`wlr_frame_done`), absent from every release up to 0.8.4.

### Building the backport

Release 0.8.4 has no `wlr_frame_done`; the four sites where the screencopy
path enqueues a buffer and destroys the frame (no current buffer, changed
constraints, `ready`, `failed`) each gained the trigger. The result is
`packaging/xdg-desktop-portal-wlr/trigger-process-after-screencopy.patch`.

```sh
sh celestina/scripts/build-patched-xdpw.sh --install-override
systemctl --user is-active xdg-desktop-portal-wlr.service
sha256sum ~/.local/libexec/xdg-desktop-portal-wlr ~/.cache/celestina/xdpw-src/build/xdg-desktop-portal-wlr
```

The script installed meson and ninja into `~/.cache/celestina/xdpw-tools`,
fetched tag `v0.8.4`, applied the patch, built, installed the binary, wrote
`~/.config/systemd/user/xdg-desktop-portal-wlr.service.d/override.conf` and
restarted the service.

### Author confirmation

The author shared a screen from an application on the live session after
the restart and reported it working: the shared picture moves.

### Repository guards and production

```sh
python3 scripts/version_tool.py bump celestina bug --unit CAST-1 --summary "Fix the shared screen that froze on its first frame"
python3 scripts/version_tool.py check
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
python3 scripts/check-staged-units.py celestina/docs/inventories/2026-09-19-screencast-first-frame-freeze/CAST-1.numstat.tsv
sh celestina/scripts/complete-production.sh
```

## Result

- **Exit:** the service is `active` on the built binary; the installed and
  freshly built binaries share one SHA-256; the author's live share moves.
  Version, architecture, language, documentation and staged-unit guards: OK.
  `complete-production.sh`: the 1.3.2 bundle built, but `verify-production.sh`
  exited 8 on `ctest` (23 of 25 test programs passed) and deploy was
  therefore withheld
- **Observed:** with the distribution's binary the first frame arrived and
  nothing followed, in every application, with no error in any journal; with
  the backport the share updates continuously

## Limits

- Four test cases fail before and after this unit, identically on three
  runs, in files this unit does not touch and nobody has touched since
  2026-08-29: `IndicatorMenuTest::aSoftMenuKeepsOneOuterGlassCard` (hidden
  menu background elevation 2, expected 0) and three `TrayItemsMenu` cases in
  `tst_trayitemsmenu.qml` (keyboard focus and the menu route). The
  CelestinaStyle deliveries of 2026-09-03 to 2026-09-07 changed the glass
  and press of context menus after the last shell evidence; the shell's own
  expectations have not followed. That is a separate unit, so the 1.3.2
  bundle is built but not deployed, and the session keeps the 1.3.1 shell.
- The freeze itself was reproduced by the author, not by an agent: a portal
  share opens the session's chooser and needs a person to answer it, and this
  suite's rule is to observe the live session, never to drive it.
- Nothing here proves window sharing; the wlr backend captures outputs only.
- The backport is verified against tag `v0.8.4`; the build script refuses
  when a later release no longer takes the patch, which is the signal to drop
  the override.

## Follow-up

None. When a release of `xdg-desktop-portal-wlr` contains c0255d7b, delete the
service drop-in and the patch becomes history.
