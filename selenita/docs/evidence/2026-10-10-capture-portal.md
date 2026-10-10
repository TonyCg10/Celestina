# Evidence: the capture portal Selenita records through (SEL-1-F)

- **Date:** 2026-10-10
- **Scope:** SEL-1-F of
  [SEL-1's plan](../plans/active/2026-10-09-sel-1-foundation.md):
  `selenita/packaging/xdg-desktop-portal-wlr/`, `selenita/scripts/build-portal.sh`,
  README, STATUS, VALIDATION
- **Environment:** niri 26.04 (the author's fork, `zwlr_screencopy_manager_v1`
  v3 only), xdg-desktop-portal 1.22.1, PipeWire 1.6.9, GStreamer 1.28.7,
  radeonsi VA on `/dev/dri/renderD128`, outputs DP-1, DP-2 and HDMI-A-1
- **Artifact:** `~/.local/libexec/xdg-desktop-portal-wlr`, SHA-256
  `aa062ca08cced89489310dc13d21d79e2e1ca64dd339905b8c04c8ece25157bd`
  (upstream `c0255d7b` plus the packaged patch); the previous binary kept
  as `xdg-desktop-portal-wlr.prev`

## What happened live

After SEL-1-E (Selenita 1.0.2) the author's recordings still failed: the
first one after a pause sometimes recorded with a black first second, the
next ones never received a frame. The SEL-1-E watchdog and the node line
showed the child connected to the node the portal named and nothing came.
The backend was the author's own build of xdg-desktop-portal-wlr 0.8.4
with the September backport of upstream `c0255d7b`, run through a systemd
user drop-in.

## Findings

1. **Not Selenita.** With the backend logging at `DEBUG`, the failing
   sessions show the stream reaching `streaming`, one frame copied, then
   "pipewire: out of buffers" / "unable to export buffer, dropping frame"
   1760 times in one session while the consumer received nothing.
2. **The distribution's 0.8.4** (no backport) delivers exactly one frame
   per session ("wlroots: no current buffer"): three recordings of the
   author made with it each held one frame.
3. **Upstream master** (`c0255d7`, which carries the starvation retry
   `d24d6a1`, the self-driven graph `c613a8b` and the per-buffer damage
   `a413366`) still failed: 0 of 6 sessions recorded through a D-Bus test
   client running Selenita's own pipeline, with 2 or with 8 buffers, with
   `vah264enc` or `x264enc`. Raw frames reached the producer's side; the
   consumer never left `PAUSED`.
4. **Root cause.** With `GST_DEBUG=pipewiresrc:6` on a run that happened to
   pass, the consumer's first buffer arrived flagged corrupted with
   `pts 0`, the second with `pts 53437971777181`. `pipewiresrc` takes the
   first timestamp as the stream's base, so every real frame looked about
   14 hours late; the source thread waits on the clock holding its buffer,
   the producer runs dry and the pipeline never prerolls. The frameless
   buffer comes from the backend: when the consumer deactivates the stream
   between its start-up transitions (`pipewiresrc` pauses the stream after
   `READY→PAUSED` and resumes it at `PAUSED→PLAYING`), the backend returns
   the buffer it had dequeued with `h->pts = SPA_TIMESPEC_TO_NSEC(&frame)`
   of a frame that never arrived: 0. It depends on timing, which is why
   heavy logging made some runs pass and why a first recording after a
   pause sometimes worked.
5. **The black first second.** Each new buffer in the pool received a copy
   with damage only, so the area the compositor did not redraw stayed as
   allocated (black) until every buffer had carried a whole frame.

## Change

`selenita/packaging/xdg-desktop-portal-wlr/0001-screencast-stamp-frameless-buffers.patch`
over upstream `c0255d7b`:

- a corrupted or zero-timestamp buffer is stamped with the current
  `CLOCK_MONOTONIC` (the compositor's clock) instead of 0;
- each `xdpw_buffer` carries `filled`; its first copy is
  `zwlr_screencopy_frame_v1_copy` (whole frame), later ones
  `copy_with_damage`;
- the pool asks for 8 buffers, at least 4 (was 2 and 2).

`selenita/scripts/build-portal.sh` clones or fetches upstream into
`~/.cache/selenita/xdpw-src`, checks out the pinned commit, applies every
patch of the folder (`git apply --check` first), builds with meson and
ninja (a private venv when the host has none), installs to
`~/.local/libexec` keeping `.prev`, prints the SHA-256, and with
`--install-override` writes the systemd user drop-in and restarts the
service. The halted shell's September recipe is superseded and left as it
is.

## Procedure

A test copy of the backend ran by hand with a scratch configuration
(`chooser_type=none`, `output_name=HDMI-A-1`, `max_fps=60`), so no dialog
reached the author's screen. A Python client over `Gio` drove
`CreateSession`, `SelectSources` (`types=1`, `cursor_mode=2`), `Start` and
`OpenPipeWireRemote`, then ran Selenita's argv
(`gst-launch-1.0 -e -q mp4mux … pipewiresrc fd=0 path=<node>
do-timestamp=true ! videoconvert ! vah264enc rate-control=cqp qpi=20 qpp=22
key-int-max=60 ! h264parse ! queue ! mux.`) for 3 s with the remote on its
standard input, stopped it with SIGINT, closed the session and counted the
frames with `ffprobe -count_frames`. The average luma of the first frames
came from `signalstats`; no frame was looked at. The scratch videos were
deleted.

## Result

| Backend | Sessions | Recorded | Frames per 3 s | Starvation lines |
|---|---|---|---|---|
| author's 0.8.4 + backport | live | intermittent | 0 or about 170 | 1760 in one session |
| distribution 0.8.4 | 3 (author, live) | 3 | 1 | 0 |
| upstream `c0255d7`, 2 buffers | 3 | 0 | 0 | 835 |
| upstream `c0255d7`, 8 buffers | 3 | 0 | 0 | 831 |
| `c0255d7` + timestamp fix | 3 back to back | 3 | 173, 177, 173 | 0 |
| `c0255d7` + full patch | 3 back to back | 3 | 168, 169, 173 | 0 |

First frames' average luma with the full patch: `37 37 37 37 37 37` for one
session and `16 37 37 37 37 37` for the other two (37 is the full desktop).
Installed and the service restarted; `org.freedesktop.impl.portal.desktop.wlr`
is owned by the new binary.

## Limits

- One frame near the start can still be a partial render (luma 16). niri's
  screencopy queue keeps one `OutputDamageTracker` for every client buffer
  and renders with age 0 (`render_to_dmabuf` in
  `NIRI-MELIBEA/src/render_helpers/mod.rs`), so a buffer's first frame can
  receive only the damage since another buffer's frame. The cure is the
  compositor's.
- The author's live recordings through Selenita on the installed build are
  VAL-SEL-REC's.
- `shellcheck` is not installed; the script passed `sh -n` only.

## Follow-up

- niri: track damage per screencopy buffer (or render the first frame of
  each new buffer whole), in NIRI-MELIBEA.
- Offer the timestamp fix upstream; it is independent of niri.

## Landing

- **Base revision:** `aff924ad1109054e2b5000441ea30a915727a8ae`
- **Check:** `production_artifact.py check selenita --require-verified` exit 0: artifact: selenita current
- **Build:** artifact current; no build
- **Deploy:** after the push: selenita: deploy-production.sh, status-production.sh
