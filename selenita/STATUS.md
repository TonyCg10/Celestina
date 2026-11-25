# Selenita status

- **Updated:** 2026-11-25
- **Implementation:** SEL-4 (the recording controls) is closed and its
  [plan](docs/plans/archive/2026-10-10-sel-4-recording-controls.md)
  archived; no checkpoint is active. SEL-4-A (the recording bar with
  «Detener» and «Descartar», the discard,
  `--record-focused` and `--pick-output` as the portal's chooser) landed
  as 1.2.0; see the
  [recording bar evidence](docs/evidence/2026-10-10-recording-bar.md).
  SEL-4-B (pausing by segments: «Pausar»/«Reanudar» in the bar, the
  segments joined by `ffmpeg` at the stop) is implemented, verified
  offscreen over the fakes and landed as 1.3.0; see the
  [recording pause evidence](docs/evidence/2026-10-11-recording-pause.md). SEL-3 is closed and its
  [plan](docs/plans/archive/2026-10-10-sel-3-preview-fixes.md) archived.
  SEL-3-A answers the author's first live use of the
  corner preview (a blurred preview, a screen key binding that took every
  monitor, the recordings folder's name) and is implemented, verified
  offscreen over the fakes and landed as 1.1.1; see the
  [preview fixes evidence](docs/evidence/2026-10-10-preview-fixes.md).
  SEL-2 is closed and its
  [plan](docs/plans/archive/2026-10-10-sel-2-preview.md) archived. SEL-2-A (the corner preview after every capture and
  recording, its drag and its hand-off to Fluorita, `Selenita1.Adopt`, no
  main window after a key-binding launch) is implemented, verified offscreen
  over the fakes and landed as 1.1.0; see the
  [preview evidence](docs/evidence/2026-10-10-preview.md). SEL-1 is
  closed and its
  [plan](docs/plans/archive/2026-10-09-sel-1-foundation.md) archived. Its
  three units are implemented and verified
  offscreen over the fakes: SEL-1-A (screenshots, delay, destinations, the
  history and the key-binding flags), SEL-1-B (recording through the portal
  and GStreamer, the sound option, `--record` and `--stop`) and SEL-1-C
  (the keyboard and accessibility pass); the SEL-1-C landing releases 1.0.0
  and closes the foundation. SEL-1-D (bug) answers the author's first live
  session of 2026-10-10: the `Recordings` folder, constant-quality encoders,
  a sound branch that always reaches its EOS, the recording clock, the last
  recording's line after a trash, and the window no longer hiding for a
  capture; see the
  [recording fixes evidence](docs/evidence/2026-10-10-recording-fixes.md)
- **Author validation:** VAL-SEL-BAR (SEL-4-A and SEL-4-B: the bar at the
  top centre without the focus, «Pausar»/«Reanudar», «Detener» and
  «Descartar», a paused recording joined into one file, `V` recording the
  focused monitor with no dialog, `R` still showing it) pending; the
  coordinator put the bar's rule, the `V` binding and the portal's
  `chooser_cmd` in place on 2026-10-11 (the suite's HOST-HYGIENE,
  87067c0a). VAL-SEL-FIXES (SEL-3-A: a sharp preview of a
  three-monitor capture, the screen key binding taking the focused
  monitor only, a recording in `Grabaciones`, the preview floating at the
  corner under the corrected rule) pending. VAL-SEL-PREVIEW (the corner without the focus,
  dragging a capture and a recording into WhatsApp and Slack, the click
  into Fluorita) pending, with the niri rule of the README in place.
  VAL-SEL-SHOT (a real capture) and VAL-SEL-REC (a
  real recording) pending; both re-run after SEL-1-D. The 1.0.1 run of
  2026-10-10 recorded nothing: the portal's node never streamed, which the
  child could not say (SEL-1-E adds the no-signal watchdog, the node line
  on stderr and the diagnosis recipe; see the
  [recording start evidence](docs/evidence/2026-10-10-recording-start.md)).
  The cause was the portal backend: SEL-1-F builds the suite's own
  xdg-desktop-portal-wlr (upstream `c0255d7b` plus a timestamp and a
  first-copy fix) with `scripts/build-portal.sh`; with it six test sessions
  in a row recorded; see the
  [capture portal evidence](docs/evidence/2026-10-10-capture-portal.md)

## Current checkout truth

- Version 1.3.0 in the checkout. Selenita takes screenshots (every output
  or one, the focused window or a region, after an optional delay) and
  records one monitor, with the system's sound when asked, through the
  ScreenCast portal; it keeps the last 100 captures and recordings, shows
  a corner preview after each, has a recording bar that pauses and
  resumes, and answers niri key bindings and the suite's activation.
  [The README](README.md) describes all of it. The full account of the
  checkout written when SEL-4 closed, behaviour by behaviour, is in
  [the status history](docs/history/status-through-2026-11-25.md).

## Known issues

- Recording depends on the ScreenCast backend. The distribution's
  xdg-desktop-portal-wlr 0.8.4 gives one frame per session under niri, and
  upstream's master loses whole recordings to a frameless buffer stamped
  with time 0 (GStreamer's `pipewiresrc` then waits for ever). The suite's
  build (`scripts/build-portal.sh --install-override`) carries the fix; on
  any other backend a recording that receives nothing fails after
  `SIGNAL_DEADLINE` (8 s) with «El portal no ha enviado ninguna imagen».
  A live failure still wants the child's log:
  `GST_DEBUG=3 GST_DEBUG_FILE=/tmp/selenita-gst.log selenita` from a
  terminal. A wrapper in `SELENITA_TOOLS_DIR` must `exec` the launcher, or
  it hands the child `/dev/null` on fd 0 and loses the portal's remote.
- One frame near the start of a recording can be a partial render (only
  what changed since another buffer's frame): niri's screencopy keeps one
  damage tracker for every client buffer. The portal build already copies
  each buffer whole on its first use; the rest is the compositor's.
- A portal request that times out or is cancelled at quit leaves its
  `selenita-portal` helper thread parked on the signal iterator (zbus's
  blocking iterator has no deadline) together with its connection until the
  process ends; a bounded wait needs the async API and two more
  dependencies, so it stays recorded here.
- Quitting while a recording is being finished can take, in the worst
  case, the muxer's `STOP_DEADLINE` (10 s), the longest join
  (`segments::JOIN_MAX`, 5 min, for 24 GiB of segments) and 15 s for the
  publish: `QUIT_DEADLINE`, 5 min 25 s. A quit past that ends the process
  with the segments still hidden in `Grabaciones`; a join past its own
  deadline is killed and the segments published one by one.
- A child killed after `EOS_GRACE` with an empty file is judged by the
  file alone: with `filesink` unbuffered, an empty file after a second
  of EOS means no encoded frame reached the muxer.
- A pause and a resume each take a child's finish (the muxer's index) and
  start: the bar shows «Pausar» until the segment is finished, and a few
  frames at each join may differ from what was on screen at the click.
  The join copies the streams; segments from one pipeline share their
  codecs, so `ffmpeg` needs no re-encoding (proved on real files in
  `selenita-core`'s tests, not on a real recording).
- A quit that arrives between two of the portal's calls (`CreateSession`,
  `SelectSources`, `Start`) finds no pending request to close; the next call
  goes out and the worker only notices the quit when it returns.
- The recording worker checks the child first on each tick: when the child
  has just exited on its own, a toggle, stop, pause or resume received in
  that same tick is dropped (it asked for what has just happened); a
  discard in that tick wins (nothing is published, SEL-4-B), and a quit
  still ends the worker.
- The history file is written from two places at quit: the recording worker
  loads, appends and saves it directly while the capture worker still holds
  its own copy; whichever saves last wins, so an entry can be lost in that
  window. (The empty history of 2026-10-10 was not this: every file it
  listed had been moved to the trash, and a trash keeps the other lines,
  as `trashing_one_entry_keeps_the_others_lines_in_the_file` shows.)
  SEL-4-B sends every entry published after the quit to the file this
  way too, so the window is the same; its fix (one owner of the file at
  quit) is deferred by the SEL-4-B review (round 2).
- Once the main window has closed, `org.celestina.Selenita` is released
  at once (`activation::release`, before `record::quit` waits up to
  `QUIT_DEADLINE`), so a launch while the old process still finishes a
  recording starts a fresh instance; the old one serves nothing more.
- A `--screenshot` or `--record` launch that races an instance still
  starting finds the name taken, only raises it, and its request is lost.
- The preview's place and focus depend on niri's rule of the README, which
  is in the author's configuration since 2026-10-10 (the suite's PRV-1-E,
  67b45f52). Without it niri opens the preview as any new window: tiled and
  focused. The first rule's `title="^Vista previa$"` never matched, as Qt
  on Wayland appends the display name («Vista previa — Selenita»); that is
  why the author's first preview opened tiled. The coordinator corrected
  the author's configuration; the README shows `title="^Vista previa"`. `Qt.WindowDoesNotAcceptFocus` is only a hint on Wayland; the
  rule's `open-focused false` is what keeps the focus where it was.
- A `--record` launch's only sign while it records is the recording bar
  (SEL-4-A), placed by niri's rule of the README, which is in the
  author's configuration since 2026-10-11 (the suite's HOST-HYGIENE,
  87067c0a); without the rule niri opens the bar as any new window (tiled
  and focused).
  `Qt.WindowDoesNotAcceptFocus` is only a hint on Wayland, as for the
  preview. The bar never takes the keyboard, so «Descartar» has no key
  of its own: the keyboard stops from the card (`R`, «Detener»).
- The offscreen platform the tests use activates every window it shows
  and ignores `Qt.WindowDoesNotAcceptFocus`, so a test that records gives
  the main window the keyboard back after the bar appears
  (`tst_keyboard.qml`); on the session niri's `open-focused false` does.
- The bar's pill is drawn inside a transparent rectangular window; whether
  the compositor's blur stops at the pill's rounded corners is the
  author's to see (VAL-SEL-BAR).
- `--record-focused` depends on the portal's chooser being
  `selenita --pick-output` (`chooser_cmd`, which the coordinator set to
  `/home/toni/.local/bin/selenita --pick-output` in the author's
  configuration on 2026-10-11); with any other chooser it shows the
  dialog like `--record`, and its request is removed once the portal has
  answered. A screencast another program
  starts within the instant between a focused start's request and the
  portal's question would take the request instead.
- The suite's `scripts/activation_contract.py` describes
  `org.celestina.Selenita1`'s methods in prose and names `RecordFocused`
  since 87067c0a (the allowlisted interface is unchanged).
- The last recording's poster stays in `runtime_dir/selenita` until the
  next preview replaces it or the session ends (the runtime folder is
  cleared at logout); a poster left by an earlier process is not removed
  by the next one.
- The hand-off's `Edit` call waits up to `HAND_OFF_TIMEOUT` (3 s) for
  Fluorita's reply. A Fluorita that opens the file but answers later than
  that makes the preview say «No se ha podido abrir Fluorita.» although
  the editor is on screen.
- Fluorita's side of `Adopt` without a running Selenita writes the history
  itself (FLU-P1-A). Since SEL-3-A both folder names are
  `selenita_core::names`'s (`CAPTURES_FOLDER`, `RECORDINGS_FOLDER`) and
  `FolderPaths::inside` applies them; Fluorita adopts that constructor
  since FLU-P2-A (6f5d65af, Fluorita 1.7.1).
- Recordings made before SEL-3-A stay in the videos folder's `Recordings`;
  `Adopt` no longer takes a file from there and the history keeps the rows
  it already had until their files move. Moving them is the coordinator's
  step after landing; the moved files' rows then drop out at the next start
  (a row whose file is gone is pruned) and can come back through `Adopt`.

## Blockers

None recorded.
