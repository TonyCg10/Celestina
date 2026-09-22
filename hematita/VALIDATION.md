# Author validation — Hematita

This queue contains no implementation work and never blocks `ROADMAP.md`.

## VAL-H1 — Live CPU and memory on the real session

- **Status:** pending
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
- **Result:** not run
- **Evidence:** none

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

- **Status:** pending
- **Related implementation:** H3
- **Requires:** the deployed Hematita 0.4.0 on the real session; a process of
  the author's to end (for example `sleep 600` in a terminal); one root
  process visible
- **Procedure:** open Procesos; type part of a name in the search and watch
  the table narrow; click each column title and confirm the order flips;
  select the `sleep` row and press Terminar; select another own process and
  press Matar, then cancel in the dialog with Escape, then confirm; select a
  root process and read the bar; walk the table by Tab and arrows and by
  screen reader; open Aplicaciones, confirm the running desktop applications
  appear with their icons and names, expand one and read its processes; leave
  the table open for a minute and confirm the selection and scroll position
  do not jump; check Hematita's own CPU while idle on Procesos
- **Pass condition:** the search narrows live; every column sorts both ways;
  Terminar ends `sleep` within a second; the dialog contains focus, Escape
  cancels, the confirm kills; a root process shows the not-actionable words
  and no dialog opens; every row and control is reachable by keyboard and
  named by the screen reader; every running desktop application has an icon
  (a missing icon is a `VAL` failure to record, not a crash); the table keeps
  its place across ticks; Hematita idles under 2 % CPU on Procesos
- **Result:** not run
- **Evidence:** none
