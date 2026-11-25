# Selenita status history through 2026-11-25

Moved verbatim from [the status](../../STATUS.md) on 2026-11-25 by `SEL-5-A`:
the account of the checkout at 1.3.0, written when SEL-4 closed on 2026-10-11.
The status keeps a summary and the known issues; [the README](../../README.md)
describes the product.

## From "Current checkout truth"

- Version 1.3.0 in the checkout. The
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
- Closing the window while it records or is paused finishes the
  recording as a stop would (SIGINT, the muxer's index, the segments
  joined, the file published and written to the history file directly,
  up to `QUIT_DEADLINE`, 5 min 25 s); every entry published once the
  window has quit goes to the history file; a portal dialog still open is
  closed and the child, if any, killed with its hidden files removed.
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
- The recording bar (SEL-4-A, SEL-4-B): while a recording is in the
  `recording` or `paused` state, however it started, a frameless window titled «Grabando»
  (`RecordingBar.qml`, `Qt.WindowDoesNotAcceptFocus`, no transient
  parent) shows a glass pill with the red dot (pulsing, still under
  reduced motion and while paused), the recorded time
  (`RecordingClock.qml`, the same clock the recording card uses; it stands
  still while paused), «Pausar»/«Reanudar» (`pauseRecording`,
  `resumeRecording`; the button keeps the wider label's width), «Detener»
  (`stopRecording`: saved, the preview follows) and «Descartar»
  (`discardRecording`: the worker's discard job kills the child, removes
  every hidden segment, publishes nothing, adds no history row and shows
  no preview; a key-binding launch then ends; a file that stays is said
  in the notice, `RecordError::Discard`). The card shows a paused
  recording's held time with «en pausa» and still stops it. A compositor's close is refused while it
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
- Pausing (SEL-4-B): a pause finishes the current child (SIGINT, EOS)
  into its hidden segment (`selenita_core::segments::segment_file`,
  `-part<n>`) and keeps the portal's session; a resume asks the session
  for a fresh remote (`portal::Session::remote`, `OpenPipeWireRemote`)
  and starts the next child on the same node with the same pipeline. The
  stop (recording or paused, from the bar, the card, `R`, `--stop`, the
  stop file, `StopRecording()` or the quit) publishes one segment as it
  is and joins more with `ffmpeg -f concat -c copy` (no `+faststart`)
  over a private list in `runtime_dir/selenita/join-<tag>.txt`, within
  `join_deadline` (60 s + 10 s per GiB, 5 min at most); a failed join (or
  a failed publish of the joined file) publishes each segment as
  `<name> (n).mp4` with a notice, and a file that cannot be published
  stays where it is, named in the notice (`RecordError::Unpublished`); a
  failed resume, or a segment that dies after a resume, publishes what
  came before with a notice. The stop says `stopping` before it finishes
  the last segment; a «Descartar» asked during the finish or the join
  (`Signals`) drops everything; a toggle asked while a stop ran is
  dropped. Every segment's file is written unbuffered (`filesink
  buffer-mode=2`, `record::FILESINK_UNBUFFERED`), and a pause or a stop
  always asks the child to finish first (SIGINT, EOS): only a child that
  has not exited within `EOS_GRACE` (1 s) with its file still empty is
  killed and its segment dropped (it never received a buffer; when it was
  the first segment, the recording fails with the no-signal notice); one
  whose child fails at a pause is dropped and the recording stays paused with
  the earlier ones (`RecordError::SegmentLost`). The clock's origin moves back by the recorded
  time at each resume (`Report::Started`), and the pause reports the time
  held (`Report::Paused`, `recordingRecorded`). The recorders (real and
  fake) moved to `recorder.rs`; the fake joins its segments' bytes after
  checking the list names them.
- A second launch raises the window; a path on the command line or in an
  `Open` is ignored.
- `SELENITA_FAKE=1` routes niri, the tools, the clipboard, the portal, the
  recorder and the other applications to fakes; `SELENITA_TOOLS_DIR` points
  the real backends at stub tools.
- The spike chose `gst-launch-1.0` as a child over the `gstreamer` crate
  (the crate builds, but `mp4mux` is absent on the host and the child keeps
  the tests free of GStreamer); see the
  [recording evidence](../../docs/evidence/2026-10-09-recording.md).
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
