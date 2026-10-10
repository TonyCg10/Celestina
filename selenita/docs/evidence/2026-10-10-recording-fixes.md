# Evidence: the recording fixes after the first live session (SEL-1-D)

- **Date:** 2026-10-10
- **Scope:** SEL-1-D of
  [SEL-1's plan](../plans/archive/2026-10-09-sel-1-foundation.md):
  `celestina-rs/crates/selenita-core` (`names.rs`, `record.rs`,
  `target.rs`) and `selenita/` (the capture, recording and controller
  seams, the capture and recording cards, the window, the QML tests, the
  smoke)
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0, niri
  26.04 (v26.04-11-g5ad4497), xdg-desktop-portal 1.22.1 with ScreenCast
  routed to xdg-desktop-portal-wlr 0.8.4, PipeWire 1.6.9, GStreamer 1.28.7
  (`gst-plugin-va`, `gst-plugins-good`, `gst-plugins-ugly`, `gst-libav`
  installed today), `/dev/dri/renderD128` on a Radeon RX 9070 XT (radeonsi)
- **Artifact:** the `selenita` release binary in the shared Cargo target,
  not installed

The author ran VAL-SEL-REC and VAL-SEL-SHOT live for the first time and
brought six findings. No recording and no capture was made here: the two
recordings the author moved to the trash were read as files, every pipeline
below ran over `videotestsrc`, `audiotestsrc` or a still of the author's
own desktop (a trashed capture) into the session scratchpad, and the fakes
carried the tests and the smoke.

## Findings and root causes

### 1. Recordings landed in the videos root

`record::Places::resolve` used `videos_dir()` as the destination, while a
capture goes to the pictures folder's «Capturas». The SEL-1-B evidence had
noted the drift from the design's §5.1 («the Videos dir's `Grabaciones`
folder») and kept the root. The author's host already has
a `Recordings` folder inside its videos folder, and the author asked for
that name on 2026-10-10.

**Fix:** `names::RECORDINGS_FOLDER` (`Recordings`), `recordings_dir_in`
and `recordings_dir`; `Places.recordings` is made at start and the hidden
file and the published file live there (`selenita/src/record.rs`). The
smoke now requires the MP4 under `videos/Recordings` and nothing in the
root. The design's §5.1 word is left as written; this record is the drift
note: the folder is `Recordings` by the author's choice.

### 2. Every recording is black for its first second

Both of the author's recordings (`<recording stem> 2026-10-10 14.55.01.mp4`,
4.9 s, and the one of `14.55.50`, 6.1 s, 1920×1080 h264 High at ~60 fps,
18–20 Mbit/s) show the same shape, frame by frame (`ffprobe
-show_frames`, frames extracted with `ffmpeg -fps_mode passthrough`, mean
luma with `signalstats`):

| Frame | pts | size | type | mean luma (0–255) |
| --- | --- | --- | --- | --- |
| 0 | 0.000 | 460 B | I | 0.00 |
| 1 | 0.017 | 10–12 KB | P | 0.12–0.18 |
| 2–9 | 0.047–0.163 | 6–95 KB | P | up to 0.27 |
| 10–59 | 0.18–0.98 | ~450–1000 B | P | 0.27–0.34 |
| 60 | 0.996 | 224–268 KB | I | 14.9 / 21.5 |
| 61… | | 0.7–18 KB | P | unchanged |

The first keyframe is 460 bytes: an SPS, a PPS and a 412-byte IDR slice
whose CABAC payload is a run of zeros, a flat black picture. The P-frames
that follow carry only the regions that changed (the portal's dialog
closing, a hovered menu: the «faint fragments» the author saw) and then
next to nothing, and the whole desktop appears exactly at the second
keyframe, forced by `key-int-max=60`, in both files.

The encoder is cleared: the same `videoconvert ! vah264enc key-int-max=60
! h264parse ! mp4mux` chain over `videotestsrc` (85 KB first keyframe,
luma 126) and over a still of the author's own desktop through `pngdec !
imagefreeze` (108 KB first keyframe, luma 23.2, the same luma the real
recordings reach at frame 60) writes a complete first picture. The frames
`pipewiresrc` handed over were black with the changed parts painted on
them for the first sixty, so the fault is upstream of GStreamer.

xdg-desktop-portal-wlr 0.8 captures through `ext-image-copy-capture-v1`
when the compositor offers it (niri 26.04 does; `src/screencast/
wlr_screencast.c` falls back to `wlr-screencopy` only without it). The
protocol says of `damage_buffer`: «When a wl_buffer is captured for the
first time, or when the client doesn't track damage, the client must
damage the whole buffer», and the compositor updates «at least the union
of the region passed by the client and the region advertised by
ext_image_copy_capture_frame_v1.damage». xdpw's `ext_image_copy.c` passes
each buffer only the damage it accumulated from the compositor's own
`damage` events; a buffer of the PipeWire pool captured for the first time
gets none, so niri paints only that frame's damage into an otherwise
black buffer, and a frame that fails with `buffer_constraints` is handed
to PipeWire unpainted. That matches a black first frame with the changed
parts following. Why a full picture arrives at the one-second mark in
both files is not pinned here (it coincides with the forced keyframe; a
compositor-side full redraw at that point is the open question) and needs
a live session with `WAYLAND_DEBUG` on the portal, which an agent must not
run.

**Pipeline-level cure:** none found. `gst-launch-1.0` has no element that
drops a leading stretch by time: `identity` has no gate (`ts-offset` with
`videosegmentclip` leaves every frame; measured, 90 of 90 frames kept),
`valve` cannot be toggled from the description, `mp4mux
start-time-selection=set start-time=1000000000` keeps every frame too
(measured), and `videotrim` does not exist. A time gate would need the
worker to own the pipeline (the `gstreamer` crate, which SEL-1-B's spike
set aside) or a second pass over the file. The finding stays in STATUS as
a known issue with the backend named; the author sees the first second
as it is.

**Quality, fixed regardless:** `x264enc` ran at its default 2048 kbit/s
ABR and `vah264enc` at CBR with a bitrate it chose itself. Both now run
at a constant quality: `x264enc … pass=qual quantizer=21` (CRF 21) and
`vah264enc rate-control=cqp qpi=20 qpp=22`, one keyframe a second. Over
the desktop still, VA CQP wrote a 221 KB first keyframe and ~1 KB P-frames
for a static screen; over `videotestsrc pattern=ball` at 1080p60 the exact
builder tokens ran to a 4 s file that stops in 0.1 s.

### 3. «Abrir en Fluorita» outlived the trashed recording

`lastRecordingId`/`lastRecordingName` were set on `Report::Finished` and
never cleared. **Fix:** on every `Report::History` the controller checks
the id is still listed (`last_recording_listed`) and forgets both
properties when it is not (a trash, or a file gone at load), so the card's
line and its button go with the row. The last recording is not persisted,
so a start always begins without one.

### 4. The second recording in a row failed

Two faults, one per symptom.

*The clock did not restart.* `Session::start` reported
`State(Recording)` before `Started(origin)`; the card's clock reads the
origin the moment it sees `recording`, so the first tick showed the
previous recording's elapsed time until the next second. **Fix:** the
origin is reported first, and the card re-reads it whenever it changes
(`Connections` on `recordingStartedAt`).

*«Terminando el archivo…» for ten seconds, nothing saved.* The sequence
the procedure asks for turns «Con sonido del sistema» on for the second
recording. When the sound branch's `pipewiresrc` hands over no buffer
(a monitor of a suspended or silent sink, a stream that never negotiates),
`gst-launch-1.0 -e` never reaches its EOS after the SIGINT: measured with
the real elements over test sources, SIGINT after 3 s —

| Audio branch | Stop |
| --- | --- |
| `audiotestsrc ! audio/x-raw ! audioconvert ! audioresample ! avenc_aac ! queue ! mux.` | 0.1 s, `video,audio`, 3.0 s |
| the same through `valve drop=true` (no caps, no buffers) | never (killed at 12 s), file without `moov` |
| the same, `valve drop-mode=forward-sticky-events` (caps, no buffers) | never |
| the same, `drop-mode=transform-to-gap` (gaps, no buffers) | never |
| no muxer at all, `… ! avenc_aac ! fakesink`, no caps | never |
| silent bed `audiotestsrc wave=silence is-live=true ! audiomixer ignore-inactive-pads=true`, monitor pad with no caps / no data / data | 0.1 s each, `video,audio` |

`mp4mux` writes nothing until every pad has delivered a buffer and does
not finish on a pad that never did; `avenc_aac` does not pass an EOS it
received before any caps. The worker's `STOP_DEADLINE` (10 s) then kills
the child and `Session::stop` removes the hidden file, which is the
author's «stays on Terminando, nothing saved»; the third recording, with
sound off, worked. The fragmented-MP4 escape (`fragment-duration`) does
not help: the muxer had not written its header either.

**Fix:** the sound branch is `audiotestsrc wave=silence is-live=true !
audio/x-raw,format=S16LE,rate=48000,channels=2 ! audiomixer name=mix
ignore-inactive-pads=true ! audioconvert ! audioresample ! avenc_aac !
queue ! mux.` with the monitor's `pipewiresrc … ! audio/x-raw !
audioconvert ! audioresample ! mix.`: the bed keeps the branch flowing
and the mixer ignores a pad with nothing on it, so the EOS always arrives;
the bed is first so stereo 48 kHz is the mixer's format and the monitor is
converted into it. A recording with sound on and nothing playing now
carries a silent track instead of losing the file. The worker's state
machine was walked twice over the fake (`a_second_recording_in_a_row_
publishes_with_its_own_origin`): both publish, the second with its own
later origin reported before its state, `active` empty between them.

### 5. The window hid for a capture

`TargetKind::hides_own_window`, `needs_settle` (350 ms), `Request.hide`,
`Progress::Hide`, `Report::HideWindow`, `hideWindowRequested` and the
`windowShown` argument of `capture()` are removed; the capture worker no
longer reads the workspaces (the `Backend::workspaces` method goes with
it). A window capture still takes the window focused before Selenita,
read from niri's focus times while Selenita is focused. The `--screenshot`
launch that starts hidden and shows on the history (`reveal`,
`showWindowRequested`) is unchanged.

### 6. The history file was empty

Not a lost update. The trash holds the three captures and the two
recordings of the session, the pictures folder's `Capturas` is empty and the history
file's mtime (14:57) is the last trash: every entry it listed had been
trashed, and an empty list is an empty file. `Action::Delete` rewrites
from the live list the worker owns, never from an empty one;
`trashing_one_entry_keeps_the_others_lines_in_the_file` proves one trash
keeps the other line and only the last trash empties the file. The quit
window of the STATUS known issue (the recording worker's direct write at
quit) stays as recorded.

## Procedure

Run from the worktree root unless a line says otherwise; every command
exited 0 except where the result says so.

```sh
cd celestina-rs && cargo clippy -p selenita-core --all-targets --locked -- -D warnings && cargo test -p selenita-core
cd selenita && cargo fmt --all --check
cd selenita && cargo clippy --all-targets --locked -- -D warnings
cd selenita && cargo test
cd selenita && cargo build --release --locked
sh selenita/scripts/qml-tests.sh          # three consecutive runs
sh selenita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/selenita
bash scripts/qmllint-cxxqt.sh selenita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
python3 scripts/test-activation-contract.py
python3 scripts/commit_scope.py --check "selenita-maintenance: Fix the recording folder, the first second, the second recording and the capture of its own window"
```

## Result

- **selenita-core:** 46 tests (45 before: three added, the two step-aside
  tests removed). `record.rs`: the encoders' constant-quality tokens and
  no `bitrate=`; the sound branch's bed, mixer and order (the bed before
  the monitor), absent without sound; the `Recordings` folder under the
  videos folder.
- **selenita:** 33 tests (30 before). The two-in-a-row recording; the
  recording landing in `videos/Recordings` with the origin reported before
  the state; a delay that counts down with no hide and no settle; a screen
  capture of Selenita's own output waiting for nothing; a trash keeping
  the other entries' lines; the last recording forgotten with its row.
- **QML:** 46 passed, 0 failed, in each of three runs (43 before). The
  recording card's clock restarting on a new origin (before or after the
  state), two recordings in a row both landing, the trashed last recording
  clearing its line; the window staying for a capture and showing after a
  launch's; the capture call without the window flag.
- **Smoke:** both runs report `cards=3 history=2 recording=idle
  lastRecording=true shown=true fake=true textScale=1 fontBody=13`, one
  PNG in the scratch `Capturas`, one MP4 in the scratch
  `videos/Recordings` and none in the videos root.
- **qmllint:** 1 warning, the baseline row.
- **Guards:** the architecture, language, documentation and activation
  contracts exit 0; the commit subject passes the scope check.

## Limits

- Nothing here asked the portal for a session or read a PipeWire node:
  the first-second mechanism is argued from the two files, the protocol
  text and xdpw's source, and the point where the full picture arrives is
  open. Only the author can confirm, live, that the sound branch records
  the monitor through the mixer, that the second recording stops within a
  second, and how the first second now looks (a backend upgrade may change
  it).
- The CQP/CRF values are a screen-recording convention, not a measurement
  on the author's eyes; the file size of a motion-heavy recording grows
  with them.
- The elapsed-clock explanation covers a one-second stale reading; if the
  author still sees the clock continue from the previous recording after
  this unit, the cause is elsewhere and needs the journal of that run.

## Follow-up

- VAL-SEL-SHOT and VAL-SEL-REC, re-run by the author on the SEL-1-D
  binary; the `Recordings` folder and the two-in-a-row sequence are in the
  procedure.
- The first second: an upstream report to xdg-desktop-portal-wlr
  (`ext_image_copy.c` not damaging a buffer whole on its first use) is the
  author's call.

## Landing

- **Base revision:** `79e62e53e154ba5253372ea7ed3a725e1f1a9b4f`
- **Check:** `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a376435a7c2fa5cbf7e88704ffcb640200f1345453c729612ac47eb2b7275811, verification_fingerprint sha256:b05b24bacf711b3a9e1599a0bafea512e6e44dbf5a9cf95bc539081b3e6e7cc0; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2ac3de44bbcc23871106302ec706c5e0506620e154eb5497ea4750e606d36d94, verification_fingerprint sha256:60abc39c3e8f8b2802469c938daf5405cc4223df5102b5c51521e0a00eb975cf
- **Deploy:** after the push: selenita: deploy-production.sh, status-production.sh
