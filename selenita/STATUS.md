# Selenita status

- **Updated:** 2026-10-09
- **Implementation:** SEL-1 is open; SEL-1-A (screenshots, delay,
  destinations, the history and the key-binding flags) and SEL-1-B
  (recording through the portal and GStreamer, the sound option, `--record`
  and `--stop`) are implemented and verified offscreen over the fakes;
  SEL-1-C (keyboard, accessibility, 1.0) is next
- **Author validation:** VAL-SEL-SHOT (a real capture) and VAL-SEL-REC (a
  real recording, after installing `gst-plugins-good`) pending

## Current checkout truth

- Version 0.2.0. The window is transparent over the backdrop, follows the
  suite's appearance file and scrolls three cards: capture, recording and
  history.
- The capture card takes the screen (every output, or one when there are
  several), the focused window or a region drawn with `slurp`, after no delay
  or 3, 5 or 10 s, to the clipboard and to the pictures folder's «Capturas»
  (`Captura 2026-10-09 14.32.05.png`, numbered on a collision). The window
  steps aside while it captures and comes back on the history.
- The recording card records one monitor (chosen in the ScreenCast
  portal's dialog) to `<recording stem> <date> <time>.mp4` in the videos folder
  through `gst-launch-1.0 -e` (`pipewiresrc`, `vah264enc` or `x264enc`,
  `mp4mux`), with the system's sound (`avenc_aac`) when the switch is on;
  a red dot and the elapsed time show while it records, and the last
  recording has its «Abrir en Fluorita». `mp4mux` is probed at start: when
  `gst-plugins-good` is missing (it is, on the author's host) the card says
  so and the button is disabled.
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
  times), noted before the window steps aside, through niri's own
  `screenshot-window --id`. niri copies it to the clipboard whatever is
  asked, so with that target the clipboard switch shows on and disabled
  with a hint saying so.
- A `--screenshot` launch with no Selenita running starts with the window
  hidden, takes the capture and then shows the window on the history.
- The window waits 350 ms after stepping aside (an estimate) only when the
  target may show the output it was on.
- The output choice is an inline row of buttons, not a popup menu
  (accepted in review).
- Known loose end: a `--screenshot` or `--record` launch that races an
  instance still starting finds the name taken, only raises it, and its
  request is lost.
- Not yet proven on the session: VAL-SEL-SHOT and VAL-SEL-REC (the portal's
  dialog, the PipeWire node handed on the child's standard input, the
  pipeline's audio branch and the MP4's playback).

## Known issues

- A portal request that times out or is cancelled at quit leaves its
  `selenita-portal` helper thread parked on the signal iterator (zbus's
  blocking iterator has no deadline) together with its connection until the
  process ends; a bounded wait needs the async API and two more
  dependencies, so it stays recorded here.

## Blockers

None recorded.
