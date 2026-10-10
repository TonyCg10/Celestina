# Selenita status

- **Updated:** 2026-10-10
- **Implementation:** SEL-1's three units are implemented and verified
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
- **Author validation:** VAL-SEL-SHOT (a real capture) and VAL-SEL-REC (a
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

- Version 0.3.0 in the checkout (the SEL-1-C landing sets 1.0.0). The
  window is transparent over the backdrop, follows the suite's appearance
  file and scrolls three cards: capture, recording and history.
- The capture card takes the screen (every output, or one when there are
  several), the focused window or a region drawn with `slurp`, after no delay
  or 3, 5 or 10 s, to the clipboard and to the pictures folder's «Capturas»
  (`Captura 2026-10-09 14.32.05.png`, numbered on a collision). The window
  stays where it is while it captures: a capture may be of Selenita itself
  (SEL-1-D); only a `--screenshot` launch keeps it hidden until its capture.
- The recording card records one monitor (chosen in the ScreenCast
  portal's dialog) to `<recording stem> <date> <time>.mp4` in the videos
  folder's `Recordings` through `gst-launch-1.0 -e` (`pipewiresrc`,
  `vah264enc` at CQP 20/22 or `x264enc` at CRF 21, one keyframe a second,
  `mp4mux`), with the system's sound (`avenc_aac`) when the switch is on:
  the default sink's monitor mixed over a silent live bed in an
  `audiomixer`, so the branch flows and the stop's EOS goes through even
  when the monitor hands over nothing; a red dot and the elapsed time show
  while it records (the clock restarts with each recording), and the last
  recording has its «Abrir en Fluorita», which goes with its history row
  when that is trashed. `mp4mux` is probed at start: when `gst-plugins-good`
  is missing the card says so and the button is disabled.
- The history keeps the last 100 captures and recordings in
  `data_home/selenita/history`, a thumbnail for a capture and the film glyph
  for a recording, each with «Abrir en Fluorita», «Mostrar en Siderita» and
  «Mover a la papelera»; a capture also has «Copiar» (a recording is not a
  picture for the clipboard, so the row hides it and the worker refuses it).
- Closing the window while it records finishes the recording as a stop
  would (SIGINT, the muxer's index, the file published and written to the
  history file directly, up to 10 s); a portal dialog still open is closed
  and the child, if any, killed with its hidden file removed.
- `selenita --screenshot screen|window|region` (for niri key bindings) asks
  the running instance over `org.celestina.Selenita1.Capture`, or starts one
  that captures and stays open; `--record` asks `ToggleRecording()` or
  starts an instance that records; `--stop` asks `StopRecording()` and,
  when nobody answers, touches `runtime_dir/selenita/stop`.
- A second launch raises the window; a path on the command line or in an
  `Open` is ignored.
- `SELENITA_FAKE=1` routes niri, the tools, the clipboard, the portal, the
  recorder and the other applications to fakes; `SELENITA_TOOLS_DIR` points
  the real backends at stub tools.
- The spike chose `gst-launch-1.0` as a child over the `gstreamer` crate
  (the crate builds, but `mp4mux` is absent on the host and the child keeps
  the tests free of GStreamer); see the
  [recording evidence](docs/evidence/2026-10-09-recording.md).
- A window capture takes the window focused before Selenita (niri's focus
  times) through niri's own `screenshot-window --id`. niri copies it to the clipboard whatever is
  asked, so with that target the clipboard switch shows on and disabled
  with a hint saying so.
- A `--screenshot` launch with no Selenita running starts with the window
  hidden, takes the capture and then shows the window on the history.
- The output choice is an inline row of buttons, not a popup menu
  (accepted in review).
- The keyboard: `1`, `2` and `3` choose the screen, the window or the
  region, Enter takes the capture with the card's choices (the controller's
  own refusals apply: busy, no destination) or, on a focused button,
  presses that button, `R` starts or stops the
  recording, Tab walks the controls with the style's focus ring and Escape
  brings the focus back to the window. A history row is a Tab stop: Up and
  Down walk the rows, Home and End reach the ends, Enter opens the row in
  Fluorita, Delete moves it to the trash and the row that takes its place
  keeps the focus; a click selects a row without the ring. No key fires
  while a text field has the focus, and a modified digit is left alone.
  A control reached with the keyboard is scrolled into view.
- Every control carries a Spanish accessible name: the cards are named
  groupings, a history row reads its name, kind, size and time as one list
  item (Enter is its press action), the switches are check boxes whose
  description is their hint, the last recording's line says it is the last
  recording and names the file, the target, capture and record buttons say
  their key.
- Not yet proven on the session: VAL-SEL-SHOT and VAL-SEL-REC (the portal's
  dialog, the PipeWire node handed on the child's standard input, the
  pipeline's audio branch and the MP4's playback).

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
- Quitting while a recording is being finished can take up to about 20 s in
  the worst case: the muxer gets `STOP_DEADLINE` (10 s) after the interrupt
  and the quitting window waits the same deadline for the worker on top of
  it.
- A quit that arrives between two of the portal's calls (`CreateSession`,
  `SelectSources`, `Start`) finds no pending request to close; the next call
  goes out and the worker only notices the quit when it returns.
- The recording worker checks the child first on each tick: when the child
  has just exited on its own, a job received in that same tick (a toggle or
  a stop) is dropped with the `continue` that reports the end.
- The history file is written from two places at quit: the recording worker
  loads, appends and saves it directly while the capture worker still holds
  its own copy; whichever saves last wins, so an entry can be lost in that
  window. (The empty history of 2026-10-10 was not this: every file it
  listed had been moved to the trash, and a trash keeps the other lines,
  as `trashing_one_entry_keeps_the_others_lines_in_the_file` shows.)
- A `--screenshot` or `--record` launch that races an instance still
  starting finds the name taken, only raises it, and its request is lost.

## Blockers

None recorded.
