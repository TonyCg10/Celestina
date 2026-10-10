# xdg-desktop-portal-wlr for Selenita

Selenita records a monitor through the ScreenCast portal. Under niri that
portal's backend is xdg-desktop-portal-wlr, which captures through
`zwlr_screencopy_manager_v1` (the only capture protocol niri offers). The
suite builds its own copy with `selenita/scripts/build-portal.sh`: upstream
at a pinned commit plus the patches in this folder.

## Base

Upstream `master` at `c0255d7b047b7263629ab5a314661045ff7f65e3`
(2026-08-11, "screencast: Trigger the graph from the wlr-screencopy
backend"). No release carries it yet. Over 0.8.4 it brings, among others:

- `c613a8b` the backend drives the PipeWire graph itself;
- `d24d6a1` a retry timer that recovers a stream starved of buffers;
- `a413366` each buffer receives only its own accumulated damage;
- `c0255d7` the wlr-screencopy path asks for the next cycle after each
  frame (0.8.4 delivered one frame and stalled; the September recipe in the
  halted shell's tree backported only this one change).

## Patches

`0001-screencast-stamp-frameless-buffers.patch`:

1. **A frameless buffer gets a real timestamp.** When the consumer pauses
   the stream between its start-up transitions, the backend returns the
   buffer it had dequeued as corrupted with `pts = 0`. GStreamer's
   `pipewiresrc` takes the first pts it sees as the stream's time base, so
   the next real frame, stamped with the compositor's `CLOCK_MONOTONIC`,
   looks hours late: the source thread waits on the clock holding the
   buffer, the backend runs out of buffers ("pipewire: out of buffers",
   "unable to dequeue buffer, dropping frame" every 16 ms) and nothing is
   recorded. The backend now stamps such a buffer with the current
   `CLOCK_MONOTONIC`.
2. **A buffer's first copy is a whole frame.** `copy_with_damage` into a
   buffer the compositor never filled leaves the undamaged area as it was
   allocated (black). Each `xdpw_buffer` carries a `filled` flag; until it
   is set the backend asks for `zwlr_screencopy_frame_v1_copy`.
3. **A larger pool.** 8 buffers, at least 4 (was 2 and 2), so an encoder
   that holds a frame for a moment does not starve the capture.

## Measurements (2026-10-10, niri 26.04, PipeWire 1.6.9, 1920×1080 at 60)

A D-Bus test client drove `CreateSession`, `SelectSources`, `Start` and
`OpenPipeWireRemote` with Selenita's own pipeline for 3 s per session:

| Backend | Sessions recorded | Frames |
|---|---|---|
| distribution 0.8.4 | every one, one frame each | 1 |
| upstream `c0255d7` | 0 of 6 without debug logging | 0 |
| `c0255d7` + this patch | 6 of 6, back to back | 168–177 |

## Known remaining issue

One frame near the start (the first or second) is still a partial render
(average luma 16 against 37 for the full frames). niri's screencopy queue
keeps one `OutputDamageTracker` for every buffer and renders into the
client's buffer with age 0; the cure belongs to the compositor.
