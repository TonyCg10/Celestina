# Selenita status

- **Updated:** 2026-10-10
- **Implementation:** SEL-4 (the recording controls) is active; see its
  [plan](docs/plans/active/2026-10-10-sel-4-recording-controls.md).
  SEL-4-A (the recording bar with «Detener» and «Descartar», the discard,
  `--record-focused` and `--pick-output` as the portal's chooser) is
  implemented and verified offscreen over the fakes, awaiting its landing;
  see the [recording bar evidence](docs/evidence/2026-10-10-recording-bar.md).
  SEL-4-B (pausing by segments) is planned. SEL-3 is closed and its
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
- **Author validation:** VAL-SEL-BAR (SEL-4-A: the bar at the top centre
  without the focus, «Detener» and «Descartar», `V` recording the focused
  monitor with no dialog, `R` still showing it) pending, once the
  coordinator has put the bar's rule, the `V` binding and the portal's
  `chooser_cmd` in place. VAL-SEL-FIXES (SEL-3-A: a sharp preview of a
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

- Version 1.1.1 in the checkout. The
  window is transparent over the backdrop, follows the suite's appearance
  file and scrolls three cards: capture, recording and history.
- The capture card takes the screen (every output, or one when there are
  several), the focused window or a region drawn with `slurp`, after no delay
  or 3, 5 or 10 s, to the clipboard and to the pictures folder's «Capturas»
  (`selenita_core::names::CAPTURES_FOLDER` on disk; the card's switch
  names it with its own `qsTr`)
  (`Captura 2026-10-09 14.32.05.png`, numbered on a collision). The window
  stays where it is while it captures: a capture may be of Selenita itself
  (SEL-1-D); a key-binding launch never shows it (SEL-2-A).
- The recording card records one monitor (chosen in the ScreenCast
  portal's dialog) to `<recording stem> <date> <time>.mp4` in the videos
  folder's `Grabaciones` (`selenita_core::names::RECORDINGS_FOLDER`, since
  SEL-3-A; Selenita no longer makes `Recordings`, and what is already
  there is not moved) through `gst-launch-1.0 -e` (`pipewiresrc`,
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
  that captures with its main window hidden and stays running; a key
  binding's screen capture takes only the monitor niri has focused
  (`FocusedOutput`, then `grim -o`), the card's output choice when niri
  cannot say (logged), while the window's own «Pantalla» keeps its chips
  («Todas» and each output) (SEL-3-A); `--record`
  asks `ToggleRecording()` or starts an instance that records, its main
  window hidden too; `--stop` asks `StopRecording()` and,
  when nobody answers, touches `runtime_dir/selenita/stop`.
- The corner preview (SEL-2-A): after every capture saved to a file and
  every finished recording, a frameless window titled «Vista previa»
  (240 px wide, 140–240 px high at the picture's shape, the glass canvas)
  shows the picture, decoded at the frame's size times the screen's pixel
  ratio and drawn with smoothing and mipmaps (SEL-3-A), or the recording's first frame (a `gst-launch-1.0`
  child, 3 s at most, into `runtime_dir/selenita/poster-<n>.png`; the film
  glyph alone when it fails) with its length (the MP4's movie header) and
  the film glyph. It fades and slides in and out in 200 ms (at once with
  reduced motion), stays 5 s, held while the pointer is on it or a drag
  is under way; a new result replaces it, the × closes it and so does
  trashing its file. Dragging it offers the file's URI as `text/uri-list`
  with the copy action only; a click hands the file to Fluorita
  (`org.celestina.Fluorita1.Edit(key)` when Fluorita runs, else
  `fluorita --edit <path>` detached) and a failure says «No se ha podido
  abrir Fluorita.» in the preview and in the window's notice. Without a
  runtime folder there is no poster (the film glyph); the poster folder is
  made private (`0700`). It never
  asks for the keyboard focus (`Qt.WindowDoesNotAcceptFocus`); niri's rule
  in the README places it at the bottom-right without the focus. A
  clipboard-only capture has no file and no preview.
- `org.celestina.Selenita1.Adopt(key)` adds a regular `.png` or `.mp4`
  directly in the pictures folder's «Capturas» or the videos folder's
  `Grabaciones` to the history (once); anything else is ignored without
  error (ADR 0012, PRV-1). The rule is `selenita_core::history::adopted`
  over `Folders`, which `FolderPaths::inside(pictures, videos)` builds with
  the names of `selenita_core::names`; Fluorita's fallback applies the same
  rule.
- How Selenita ends: closing the main window ends it, as before the
  preview (the preview goes at once with it, so Qt finds no window left;
  a recording under way is finished and published on the way out, up to
  `STOP_DEADLINE`). A key-binding launch (`--screenshot`, `--record`) with
  no Selenita running keeps the main window hidden and ends by itself with
  its result: once its preview has gone (its five seconds, its ×, a trash,
  or a click whose hand-off to Fluorita has returned), with nothing
  capturing and nothing recording; a cancelled region or a capture with no
  file ends it at once. A `--record` (or `--record-focused`) launch therefore lives while it
  records, `selenita --stop`, Shift+Print or the bar's «Detener» stops and
  publishes the recording, and the recording's preview ends it; the
  bar's «Descartar» ends it at once (SEL-4-A). Until then it takes
  the next key binding itself. A failed capture, a failure after it (the
  clipboard, the history file) or a failed recording shows the main
  window with its notice; from then on, or once a plain launch has raised
  the window, the instance is a plain one and ends when the window is
  closed.
- The recording bar (SEL-4-A): while a recording is in the `recording`
  state, however it started, a frameless window titled «Grabando»
  (`RecordingBar.qml`, `Qt.WindowDoesNotAcceptFocus`, no transient
  parent) shows a glass pill with the red dot (pulsing, still under
  reduced motion), the elapsed time (`RecordingClock.qml`, the same clock
  the recording card now uses) and «Detener» (`stopRecording`: saved,
  the preview follows) and «Descartar» (`discardRecording`: the worker's
  discard job kills the child, removes the hidden file, publishes
  nothing, adds no history row and shows no preview; a key-binding
  launch then ends; a file that stays is said in the notice,
  `RecordError::Discard`). The actions are one row the pill grows with,
  for SEL-4-B's pause button. A compositor's close is refused while it
  shows; closing the main window or the end of a key-binding launch
  hides it at once so Qt can quit. A key-binding launch keeps the main
  window hidden: the bar is its only window while it records.
- `selenita --record-focused` (`Selenita1.RecordFocused()` for a running
  instance; a toggle like `--record`) asks niri for the focused output
  and writes the one-shot request `runtime_dir/selenita/pick-output`
  (`selenita_core::pick`: `output=` and `written=` in ms, private folder,
  atomic `0600` file) before asking the portal; without an answer from
  niri, or without a runtime folder, it starts without one and logs it.
  Any start removes a request nobody consumed before and after the
  portal is asked. `selenita --pick-output`, handled first in `main`
  (no Qt, no bus, no settings), consumes a request at most 30 s old and
  prints `Monitor: <name>`; otherwise it drops a stale or broken request
  and `exec`s `/bin/sh -c` with `SELENITA_FALLBACK_CHOOSER` or
  `"$HOME/.local/bin/celestina" --pick-output`, keeping its standard
  streams.
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
  pipeline's audio branch and the MP4's playback), and VAL-SEL-PREVIEW
  (the preview's place and focus under niri, a real drag into other
  programs, Fluorita's `Edit`).

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
- The preview's place and focus depend on niri's rule of the README, which
  is in the author's configuration since 2026-10-10 (the suite's PRV-1-E,
  67b45f52). Without it niri opens the preview as any new window: tiled and
  focused. The first rule's `title="^Vista previa$"` never matched, as Qt
  on Wayland appends the display name («Vista previa — Selenita»); that is
  why the author's first preview opened tiled. The coordinator corrected
  the author's configuration; the README shows `title="^Vista previa"`. `Qt.WindowDoesNotAcceptFocus` is only a hint on Wayland; the
  rule's `open-focused false` is what keeps the focus where it was.
- A `--record` launch's only sign while it records is the recording bar
  (SEL-4-A), placed by niri's rule of the README; without the rule niri
  opens the bar as any new window (tiled and focused).
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
  `selenita --pick-output` (`chooser_cmd`, the coordinator's edit); with
  any other chooser it shows the dialog like `--record`, and its request
  is removed once the portal has answered. A screencast another program
  starts within the instant between a focused start's request and the
  portal's question would take the request instead.
- The suite's `scripts/activation_contract.py` describes
  `org.celestina.Selenita1`'s methods in prose without `RecordFocused`;
  updating that reason is a suite follow-up (the allowlisted interface is
  unchanged).
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
