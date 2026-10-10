# Author validation — Selenita

This queue contains no implementation work and never blocks `ROADMAP.md`.

Both entries below are run on the Selenita 1.0.0 binary the SEL-1-C landing
installs (`scripts/complete-production.sh`); SEL-1-C added the keyboard and
the screen-reader steps to them rather than a third entry, so one session
covers the window.

## VAL-SEL-SHOT — Capture the screen, a window and a region

- **Status:** pending
- **Related implementation:** SEL-1-A
- **Requires:** the deployed Selenita (`scripts/complete-production.sh`),
  `grim`, `slurp` and `wl-clipboard` installed, and the three key bindings of
  the README in niri's config
- **Procedure:** open Selenita; with «Pantalla», no delay and both switches
  on, press «Capturar»; paste into an image-accepting application; choose
  «Ventana» and the 3 s delay, press «Capturar» and focus another window
  during the countdown (then capture «Ventana» with no delay, the window you
  used before Selenita being the one meant); choose the region target, draw a rectangle (then repeat and
  press Escape); turn «Guardar en la carpeta Capturas» off and capture once
  more; on a second monitor, if any, capture the other output and watch for
  Selenita in the picture (the 350 ms wait after it steps aside is an
  estimate and is skipped for another output); close Selenita and press
  Print, Alt+Print and Ctrl+Print; on one
  history row press «Abrir en Fluorita», «Copiar», «Mostrar en Siderita» and
  «Mover a la papelera»; then with the keyboard only: press `2`, `3`, `1`
  and watch the target buttons, press Enter for a capture, Tab through the
  controls, Tab to a history row and press Down, Up, End, Home, Enter and
  Delete; with Orca running, read the capture card, the switches and a
  history row
- **Pass condition:** the window disappears for each capture and returns on
  the history, and Selenita appears in no picture; a key binding with
  Selenita closed never shows the window before its capture; the window
  capture with no delay is the window used before Selenita, and with the
  window target the clipboard switch is disabled with its niri hint; the pasted image is the screen; the window capture is the
  window focused at the end of the countdown; the region is the rectangle and
  Escape takes nothing and says nothing; each saved capture is a
  `Captura <date> <time>.png` in the pictures folder's `Capturas` with a
  thumbnail row, and the clipboard-only one adds no row; each key binding
  takes its capture whether Selenita was running or not; the row actions open
  the image in Fluorita, copy it, show it selected in Siderita and move it to
  the trash (restorable from Siderita), the row leaving the history; the
  digits move the checked target, Enter takes a capture like the button,
  every control reached by Tab shows the focus ring, the arrows walk the
  rows with the ring on the current one, Enter opens the row in Fluorita,
  Delete moves it to the trash and the next row keeps the focus; Orca reads
  «Captura» for the card, the switches as check boxes with their labels and
  a row as its name, kind, size and time
- **Result:** not run
- **Evidence:** none

## VAL-SEL-REC — Record the screen, with and without sound

- **Status:** pending
- **Related implementation:** SEL-1-B
- **Requires:** the deployed Selenita (`scripts/complete-production.sh`),
  `gst-plugins-good` installed (`gst-inspect-1.0 --exists mp4mux` exits 0;
  it was missing when the unit was built), `gst-plugin-pipewire`,
  `gst-plugins-ugly` or `gst-plugin-va`, `gst-libav`, `xdg-desktop-portal`
  with its ScreenCast backend, and the two recording key bindings of the
  README in niri's config
- **Procedure:** first, outside Selenita,
  `gst-launch-1.0 -e videotestsrc num-buffers=120 ! videoconvert ! x264enc ! h264parse ! mp4mux ! filesink location=/tmp/selenita-test.mp4`
  and play the file (the pipeline builder's shape, which the subagent could
  not run without the muxer); open Selenita and read the recording card
  (no missing-element line); press «Grabar», choose the output in the
  portal's dialog, wait 10 s while moving a window, press «Detener»; turn
  «Con sonido del sistema» on, play something audible and record 10 s
  more; press «Abrir en Fluorita» on the last recording; close Selenita
  and press Shift+Print, wait, Ctrl+Shift+Print; press Ctrl+Shift+Print
  again with nothing recording; cancel the portal's dialog once; on the
  recording's history row press «Mostrar en Siderita» and «Mover a la
  papelera»; press `R` to start a recording and `R` again to stop it; with
  Orca running, read the recording card after the stop
- **Pass condition:** the card shows the red dot and the time counting
  while it records, and «Grabar» again afterwards; each recording is a
  `<recording stem> <date> <time>.mp4` in the videos folder that Fluorita plays
  with the pointer drawn and, for the second, the sound heard; the history
  row shows the film glyph and the actions work; the key bindings start
  and stop with Selenita closed or open and a stop with nothing recording
  says nothing; a cancelled dialog leaves the card idle with the cancel
  notice and no file; closing the window while it records (or while the
  portal's dialog is open) finishes and publishes the file (or closes the
  dialog) and the recording is in the history on the next launch; `R`
  starts and stops like the button; Orca reads «Grabar» with «Tecla R» and
  the last recording's line as the last recording with the file's name
- **Result:** not run
- **Evidence:** none
