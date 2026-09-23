# Author validation — Hematita

This queue contains no implementation work and never blocks `ROADMAP.md`.

## VAL-H1 — Live CPU and memory on the real session

- **Status:** passed
- **Related implementation:** H1
- **Requires:** the deployed Hematita on the real session
- **Procedure:** launch `hematita`; watch the CPU graph for a minute while
  compiling something; switch to Memory with the pointer and with the arrow
  keys from the strip; read the strip with the screen reader; check the
  monitor's own CPU in another tool while idle
- **Pass condition:** the graph advances once per second without stutter;
  values match another monitor within a few percent; the strip is reachable by
  Tab and walkable by arrows; the screen reader names each section and its
  checked state; Hematita idles under 1 % CPU
- **Result:** passed — the remediation VIS-2 was validated through VAL-VIS-2
- **Evidence:** author's screenshots, 2026-09-23, recorded in [visual feedback](docs/evidence/2026-09-23-visual-feedback.md)
- **Remediation:** `VIS-1` in [plan](docs/plans/archive/2026-09-23-visual-feedback.md)
- **Remediation validated:** VAL-VIS-2

## VAL-H2 — Every resource, live, on the real session

- **Status:** pending
- **Related implementation:** H2
- **Requires:** the deployed Hematita 0.3.0 on the real session; a USB disk to
  plug in; the Wi-Fi interface up
- **Procedure:** launch `hematita`; read the side list; copy a large file
  between two disks and watch both rows; download something and watch the
  interface; run a GPU load and watch the GPU row; open the processor and
  toggle the per-core grid; plug a USB disk in and out; make `/proc/diskstats`
  unreadable is not possible, so instead unplug the Wi-Fi and confirm its row
  says it is down rather than vanishing; watch Hematita's own CPU while idle;
  walk the resource list by Tab and by screen reader from the strip down to
  the last interface; read a disk's Capacidad and a Wi-Fi row's Velocidad
  and Estado and confirm they are plausible numbers, not "NaN" or blanks
- **Pass condition:** every whole disk and interface appears with its model or
  name; rates match another tool within a few percent; the selected row stays
  selected across a hot-plug; the grid shows one graph per core; an unreadable
  or absent source is a Spanish sentence in its own row, not a frozen number;
  Hematita idles under 1 % CPU; every row is reachable by keyboard and named
  by the screen reader; no fact reads NaN
- **Result:** not run
- **Evidence:** none

## VAL-H3 — Processes and applications on the real session

- **Status:** failed
- **Related implementation:** H3
- **Requires:** the deployed Hematita 0.4.1 on the real session; a process of
  the author's to end (for example `sleep 600` in a terminal); one root
  process visible
- **Procedure:** open Procesos; type part of a name in the search and watch
  the table narrow; click each column title and confirm the order flips;
  select the `sleep` row and press Terminar; select another own process and
  press Matar, then cancel in the dialog with Escape, then confirm; select a
  root process and read the bar; walk the table by Tab and arrows and by
  screen reader; tab into the table once, arrow down, wait ten seconds, arrow
  down again and confirm the selection moved one row; open Aplicaciones,
  confirm the running desktop applications appear with their icons and names,
  expand one and read its processes, then fold and unfold one from the
  keyboard with Left and Right; leave the table open for a minute and confirm
  the selection and scroll position do not jump; check Hematita's own CPU
  while idle on Procesos
- **Pass condition:** the search narrows live; every column sorts both ways;
  Terminar ends `sleep` within a second; the dialog contains focus, Escape
  cancels, the confirm kills; a root process shows the not-actionable words
  and no dialog opens; every row and control is reachable by keyboard and
  named by the screen reader; every running desktop application has an icon
  (a missing icon is a `VAL` failure to record, not a crash); the table keeps
  its place across ticks; Hematita idles under 2 % CPU on Procesos
- **Result:** failed on 2026-09-23 — the search field, the action capsule and the table read as one flat surface; the rows ran into the card's rounded corners; path-named processes (`Z:\mnt\...\x.exe`, `/usr/lib/...`) were unreadable; the capsule's glyphs sat too close together; every one of the author's own processes read "—" for its disk rates; two `hematita` processes were running. The rest of the procedure was not reported
- **Evidence:** author's screenshots, 2026-09-23, recorded in [visual feedback](docs/evidence/2026-09-23-visual-feedback.md)
- **Remediation:** `VIS-1` in [plan](docs/plans/archive/2026-09-23-visual-feedback.md)
- **Remediation validated:** VAL-VIS-2

## VAL-H4 — Sensors on the real session

- **Status:** failed
- **Related implementation:** H4
- **Requires:** the deployed Hematita 0.5.1 on the real session; a GPU load
  and a compile to heat things up
- **Procedure:** open Sensores; count the chip cards against
  `ls /sys/class/hwmon/*/name`; read the processor's Tctl, the GPU's edge and
  junction, the NVMe composites, the six board fans and the three labelled
  board voltages; run a GPU load for a minute and watch the GPU temperature,
  fan and power rows move and their session maximum rise; hover nothing (no
  tooltips exist); walk the cards by keyboard and screen reader; on Procesos,
  terminate a `sleep` and confirm the confirmation sentence stays until you
  select another row; type a name quickly and confirm the table does not
  stutter; check Hematita's idle CPU on Sensores; Tab into Sensores once and
  arrow through the cards; unplug nothing, but confirm that the page, not its
  rows, is the single Tab stop
- **Pass condition:** every chip in `/sys/class/hwmon` has a card and every
  `_input` channel a row with a plausible value and unit; labelled channels
  show the kernel's label, unlabelled ones a numbered word; limits appear
  where the kernel has them; session minimum and maximum move; the GPU rows
  change under load; every row is reachable and named; the terminate
  sentence persists; typing is smooth; idle CPU under 2 %
- **Result:** failed on 2026-09-23 — channels read as the kernel's abbreviations (`Tctl`, `vddgfx`, `PPT`); every value was the same colour whatever its kind; the NVMe rows showed a maximum of 65 261.8 °C, the kernel's sentinel, and a fan's line showed two maxima side by side and a thousands separator that reads as a decimal mark. The rest of the procedure was not reported
- **Evidence:** author's screenshots, 2026-09-23, recorded in [visual feedback](docs/evidence/2026-09-23-visual-feedback.md)
- **Remediation:** `VIS-1` in [plan](docs/plans/archive/2026-09-23-visual-feedback.md)
- **Remediation validated:** VAL-VIS-2

## VAL-H5 — Services and foreign processes on the real session

- **Status:** pending
- **Related implementation:** H5
- **Requires:** the deployed Hematita 0.6.1; a session with or without an
  authentication agent (state which)
- **Procedure:** open Servicios; compare the count of user services with
  `systemctl --user list-units --type=service`; stop and start a harmless
  user service (`at-spi-dbus-bus.service` restarts on demand) and watch its
  state change; filter by name; toggle Sistema and Usuario; try to restart a
  system service and read the outcome sentence (with no agent it must name
  the missing agent, not fail silently); on Procesos, select a root process
  and press Terminar, then read the sentence; walk the list by keyboard and
  screen reader; confirm Escape cancels the confirming dialog
- **Pass condition:** counts match; the user service's state follows the
  action within a second; the filter and toggles narrow live; a system
  action without an agent says so in Spanish; with an agent, the polkit
  dialog actually appears — for a system unit and for a foreign process —
  and the action follows the answer;
  every row and control reachable and named; idle CPU under 2 % on Servicios
- **Result:** not run
- **Evidence:** none

## VAL-VIS-1 — The corrected screens on the real session

- **Status:** failed
- **Related implementation:** VIS-1
- **Requires:** the deployed Hematita 0.6.2 on the real session; one Windows
  executable or `/usr/lib` binary running as the author
- **Procedure:** launch `hematita`; look at the strip; look at the
  Performance list and each detail (processor, memory, GPU, a disk, an
  interface) and the per-core grid; open Procesos and read the bar, the
  card, the column titles and a path-named process; copy a large file and
  watch a process of the author's own gain a read or write rate; read a
  foreign process's rates; open Servicios and read a unit's two lines; open
  Sensores and read the processor, GPU, NVMe and board rows; launch
  `hematita` a second time from a terminal
- **Pass condition:** each strip item's glyph and word sit centred in its
  pill; no sparkline or core graph reaches its rounded corners; the
  processor, memory, GPU, disk and network each have their own colour and a
  high load still turns amber or red; the bar sits on the canvas with a
  spaced capsule, and the table is one card whose titles, hairline and rows
  stay inside its corners; a path-named process reads its file name and the
  screen reader still reads its full path; an own idle process reads
  "0 B/s" and a busy one a rate, a foreign one "—"; a unit leads with its
  description; known sensor labels read as words, each kind has its dot and
  colour, no limit is absurd, and no reading has a thousands separator; the
  second launch raises the first window, or says on stderr why it could not
- **Result:** failed on 2026-09-23 — every data refresh scrolled the process table back to its top, and the Aplicaciones page opened with every application unfolded. The rest of the procedure was not reported
- **Evidence:** author's report, 2026-09-23, recorded in [scroll and folds](docs/evidence/2026-09-23-scroll-and-folds.md)
- **Remediation:** `VIS-2` in [plan](docs/plans/archive/2026-09-23-scroll-and-folds.md)

## VAL-VIS-2 — The lists keep their place and applications open folded

- **Status:** passed
- **Related implementation:** VIS-2
- **Requires:** the deployed Hematita 0.6.3 on the real session
- **Procedure:** launch `hematita`; open Procesos, scroll the table well
  down and wait ten seconds without touching it; open Aplicaciones and look
  at every group; move the cursor with the arrows past the bottom and top of
  the view; on an application row press Right, then Left; repeat the scroll
  and wait on Servicios and Sensores
- **Pass condition:** the scrolled table stays where it was across every
  refresh; every application is folded when the page opens; the arrows
  still bring the cursor into view; Right unfolds the application under the
  cursor and Left folds it; Servicios and Sensores also keep their place
- **Result:** the author scrolled the process table, waited, and it stayed;
  applications opened folded; no flicker on refresh (2026-09-23)
- **Evidence:** author's report, 2026-09-23, recorded in [release evidence](docs/evidence/2026-09-23-release-1.md)
