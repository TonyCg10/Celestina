# Selenita status

- **Updated:** 2026-10-09
- **Implementation:** SEL-1 is open; SEL-1-A (screenshots, delay,
  destinations, the history and the key-binding flags) is implemented and
  verified offscreen over the fakes, and SEL-1-B (recording) is next
- **Author validation:** VAL-SEL-SHOT pending (a real capture on the session)

## Current checkout truth

- Version 0.1.0. The window is transparent over the backdrop, follows the
  suite's appearance file and scrolls three cards: capture, recording (still
  a muted line) and history.
- The capture card takes the screen (every output, or one when there are
  several), the focused window or a region drawn with `slurp`, after no delay
  or 3, 5 or 10 s, to the clipboard and to the pictures folder's «Capturas»
  (`Captura 2026-10-09 14.32.05.png`, numbered on a collision). The window
  steps aside while it captures and comes back on the history.
- The history keeps the last 100 captures in `data_home/selenita/history`,
  each with a thumbnail and «Abrir en Fluorita», «Copiar», «Mostrar en
  Siderita» and «Mover a la papelera».
- `selenita --screenshot screen|window|region` (for niri key bindings) asks
  the running instance over `org.celestina.Selenita1.Capture`, or starts one
  that captures and stays open; `--record` and `--stop` are reserved for
  SEL-1-B, as is `ToggleRecording()`.
- A second launch raises the window; a path on the command line or in an
  `Open` is ignored.
- `SELENITA_FAKE=1` routes niri, the tools, the clipboard and the other
  applications to fakes; `SELENITA_TOOLS_DIR` points the real backend at
  stub tools.
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
- Known loose end: a `--screenshot` launch that races an instance still
  starting finds the name taken, only raises it, and its capture is lost.
- Not yet proven on the session: VAL-SEL-SHOT.

## Blockers

None recorded.
