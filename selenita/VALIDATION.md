# Author validation — Selenita

This queue contains no implementation work and never blocks `ROADMAP.md`.

The first two entries below are run on the Selenita 1.0.0 binary the SEL-1-C
landing installs (`scripts/complete-production.sh`); SEL-1-C added the
keyboard and the screen-reader steps to them rather than a third entry, so
one session covers the window. VAL-SEL-PREVIEW is run on the binary the
SEL-2-A landing installs, VAL-SEL-FIXES on the one the SEL-3-A landing
installs, VAL-SEL-BAR on the one the SEL-4-A landing installs.

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
  more; capture «Pantalla» of the output Selenita is on and look for
  Selenita itself in the picture (since SEL-1-D the window never steps
  aside); close Selenita and press
  Print, Alt+Print and Ctrl+Print; on one
  history row press «Abrir en Fluorita», «Copiar», «Mostrar en Siderita» and
  «Mover a la papelera»; then with the keyboard only: press `2`, `3`, `1`
  and watch the target buttons, press Enter for a capture, Tab through the
  controls, Tab to a history row and press Down, Up, End, Home, Enter and
  Delete; with Orca running, read the capture card, the switches and a
  history row
- **Pass condition:** the window stays on screen for each capture and
  Selenita appears in the picture of its own output; a key binding with
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
- **Related implementation:** SEL-1-B, SEL-1-D, SEL-1-E, SEL-1-F (the run of
  2026-10-10 found the SEL-1-D findings; the 1.0.1 run received no frame
  from the portal; re-run on the SEL-1-E binary, started from a terminal
  as `GST_DEBUG=3 GST_DEBUG_FILE=/tmp/selenita-gst.log selenita` so a
  failure leaves the child's log, and with Selenita's own stderr kept:
  it prints `portal stream node N, remote fd yes` per recording)
- **Requires:** the deployed Selenita (`scripts/complete-production.sh`),
  the suite's portal build running (`scripts/build-portal.sh
  --install-override`, SEL-1-F; `busctl --user list` shows
  `org.freedesktop.impl.portal.desktop.wlr` owned by
  `~/.local/libexec/xdg-desktop-portal-wlr`),
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
  portal's dialog, wait 10 s while moving a window, press «Detener»; at
  once press «Grabar» again, choose the output, watch the clock start from
  0:00, wait 5 s and press «Detener» (the two-in-a-row sequence that failed
  on 2026-10-10); turn «Con sonido del sistema» on and record 10 s with
  nothing playing, then 10 s more while something audible plays; press
  «Abrir en Fluorita» on the last recording, then «Mover a la papelera» on
  its history row and look at the card; close Selenita
  and press Shift+Print, wait, Ctrl+Shift+Print; press Ctrl+Shift+Print
  again with nothing recording; cancel the portal's dialog once; on the
  recording's history row press «Mostrar en Siderita» and «Mover a la
  papelera»; press `R` to start a recording and `R` again to stop it; with
  Orca running, read the recording card after the stop
- **Pass condition:** the card shows the red dot and the time counting
  from 0:00 while it records (the second recording too), and «Grabar»
  again afterwards within a second of each «Detener»; each recording is a
  `<recording stem> <date> <time>.mp4` in the videos folder's `Grabaciones`
  (nothing in the videos root, and no new `Recordings`; SEL-3-A) that Fluorita plays with the pointer drawn,
  text legible at the screen's own resolution, the silent one with a
  silent audio track and the last with the sound heard; a recording whose
  portal node sends nothing ends by itself within about 8 s with «El
  portal no ha enviado ninguna imagen» and no file (if that happens, keep
  `/tmp/selenita-gst.log`); after the trash
  the last recording's line and «Abrir en Fluorita» are gone from the
  card; the first second of each recording is noted as it is (SEL-1-D
  records a backend fault there: black with only the parts that changed
  until the second keyframe; a full first second means the host no longer
  has it); the history
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

## VAL-SEL-PREVIEW — The corner preview: place, focus, drag and edit

- **Status:** pending
- **Related implementation:** SEL-2-A
- **Requires:** the deployed Selenita (`scripts/complete-production.sh`),
  the niri window rule of the README («La vista previa») in niri's config
  (the suite's PRV-1-E adds it after the author's approval), the key
  bindings of the README, `gst-plugins-good` (`qtdemux`, `pngenc`) and a
  decoder for H.264 (`gst-libav` or `gst-plugin-va`), WhatsApp (web or
  desktop) and Slack open in another window, and for the last steps
  Fluorita with `org.celestina.Fluorita1.Edit` installed (FLU-P1-A)
- **Procedure:** with Selenita closed, press Print and keep typing in a
  text editor while the preview shows; wait without touching it; press
  Print again and rest the pointer on the preview for ten seconds, then
  move it away; press Print and press the preview's ×; open Selenita,
  capture from the window and look at both windows; press Print, drag the
  preview into a WhatsApp chat and into a Slack message, and drop it once
  on a Siderita folder while holding Shift (a move); start one more drag
  and hold it over the desktop for ten seconds before dropping it; record 5 s with
  Shift+Print and Ctrl+Shift+Print and drag its preview into WhatsApp and
  Slack; capture, then «Mover a la papelera» on its history row while its
  preview shows; with reduced motion on in the suite's appearance, press
  Print; once Fluorita's floating editor is installed, press Print and
  click the preview, then quit Fluorita, press Print and click again;
  save a copy with «Guardar ambas» and look at Selenita's history; after a
  key-binding capture's preview has gone, run `pgrep -x selenita`; open
  Selenita, capture, and close its window while the preview shows, then
  run `pgrep -x selenita` again
- **Pass condition:** the preview appears at the bottom-right corner of
  the focused output, 24 px from its edges, about 240 px wide, and the
  editor keeps the keyboard focus (the typing goes on); a key-binding
  launch never shows Selenita's main window; the preview leaves after
  about 5 s with a short fade and slide, stays while the pointer rests on
  it and leaves about 5 s after the pointer does; the × closes it at
  once; a capture from the window leaves the window where it was and shows
  the preview too; WhatsApp and Slack receive the PNG and the MP4 as
  files (the recording plays), Siderita copies rather than moves (the
  original stays in `Capturas`); a drag held longer than 5 s keeps the
  preview on screen until the drop; a recording's preview shows its first
  frame, its length and the film glyph; the trash closes the preview;
  with reduced motion it appears and leaves without moving; the click
  closes the preview and opens the file in Fluorita's floating editor,
  whether Fluorita was running or not (a failure says «No se ha podido
  abrir Fluorita.» on the preview); the saved copy is a new history row;
  neither `pgrep` finds a Selenita (a key-binding launch ends with its
  preview, and closing the window ends Selenita and its preview)
- **Result:** not run
- **Evidence:** none

## VAL-SEL-FIXES — A sharp preview, the focused monitor and `Grabaciones`

- **Status:** pending
- **Related implementation:** SEL-3-A (the author's first live use of the
  corner preview, 2026-10-10)
- **Requires:** the deployed Selenita (the SEL-3-A landing), the README's
  key bindings and its corrected preview rule (`title="^Vista previa"`,
  no closing `$`) in niri's config, two or more monitors, and the
  coordinator's move of the old `Recordings` files done or not (either
  works)
- **Procedure:** with Selenita closed and the focus on one monitor, press
  the screen key binding (`--screenshot screen`); look at the preview and
  open the saved capture; move the focus to another monitor and press it
  again; open Selenita, choose «Pantalla» and «Todas» and press
  «Capturar», then choose one output's chip and capture again; record 5 s
  with the recording key bindings and look in the videos folder; look at
  the preview of a capture of all the monitors (from the window with
  «Todas») up close, text included, on a monitor with a scale above 1 and
  on one at 1
- **Pass condition:** each key-binding capture holds only the monitor that
  had the focus (the second one the other monitor), at that monitor's
  size; the window's «Todas» still takes every monitor and an output chip
  only that output; the recording is in the videos folder's `Grabaciones`
  and no `Recordings` folder is made again; the preview opens floating at
  the bottom-right corner without the focus (not tiled), and its picture is
  sharp at every scale, edges and text without the jagged or blurred look
  of 1.1.0, a wide three-monitor capture included
- **Result:** not run
- **Evidence:** none

## VAL-SEL-BAR — The recording bar and the focused monitor without the dialog

- **Status:** pending
- **Related implementation:** SEL-4-A
- **Requires:** the deployed Selenita (the SEL-4-A landing); in niri's
  config the README's rule for the bar (`title="^Grabando"`) and the `V`
  binding (`--record-focused`) in the `tools` mode beside `S` and `R`; in
  xdg-desktop-portal-wlr's config
  `chooser_cmd=/home/toni/.local/bin/selenita --pick-output` with the
  backend restarted (the coordinator's edits after the landing); two or
  more monitors; a text editor open on one of them
- **Procedure:** with Selenita closed and the focus in the editor on one
  monitor, press `V`; keep typing in the editor for a few seconds; look at
  the top of that monitor and watch the time; press «Detener» on the bar;
  press `V` again on the other monitor, wait 5 s and press «Descartar»;
  look in the videos folder's `Grabaciones`, at Selenita's history and
  for a preview, then run `pgrep -x selenita`; open Selenita, press
  «Grabar» in the card, choose an output in the dialog, try to close the
  bar with niri's close-window key while the pointer is on it, then press
  `R` in the window to stop; with Selenita closed press `R` in the
  `tools` mode (`--record`) and stop it from the bar; turn reduced motion
  on in the suite's appearance and record once more; with Orca running,
  move to the bar's buttons; finally put the old `chooser_cmd` back as
  the README's rollback says and press `V` once
- **Pass condition:** `V` records the focused monitor at once, with no
  output dialog, and the second `V` records the other monitor; the bar
  appears at the top centre of the monitor in focus, about 16 px from its
  edge, floating, never tiled, and the editor keeps the keyboard (the
  typing goes on); it is the only Selenita window while a key-binding
  recording runs, its dot pulses slowly and its time counts from 0:00
  like the card's; «Detener» ends the recording, the bar goes and the
  corner preview shows the recording, which is in `Grabaciones` and the
  history; «Descartar» ends it with no file in `Grabaciones` (no hidden
  `.selenita-*.mp4` either), no history row, no preview, and `pgrep`
  finds no Selenita afterwards; a recording started from the card shows
  the bar too, niri's close does not close the bar or stop the
  recording, and `R` stops it; `R` (`--record`) still shows the portal's
  dialog; with reduced motion the dot does not pulse; Orca reads «Grabando»
  and the two buttons with their descriptions; after the rollback `V`
  shows the previous chooser's dialog; the pill's corners show no square
  blur behind them (if they do, the rule wants a corner radius: report
  it)
- **Result:** not run
- **Evidence:** none
