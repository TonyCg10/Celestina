# Author validation — Selenita

This queue contains no implementation work and never blocks `ROADMAP.md`.

The later SEL-1 units add theirs: a real recording with and without sound
(SEL-1-B), and the keyboard and a screen reader on the finished window
(SEL-1-C).

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
  «Mover a la papelera»
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
  the trash (restorable from Siderita), the row leaving the history
- **Result:** not run
- **Evidence:** none
