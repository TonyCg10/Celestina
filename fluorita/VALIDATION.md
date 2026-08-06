# Fluorita author validation

This manual lane does not contain implementation and does not block
[ROADMAP.md](ROADMAP.md).

## VAL-FLU-TEARDOWN — closing video without taking the process with it

- **Status:** pending
- **Related implementation:** F6-B
- **Requires:** the deployed Fluorita binary, a real Wayland session with GPU
  rendering, one file that plays, and one file mpv cannot open (an empty or
  truncated `.mkv` serves)
- **Procedure:** play the good file and close it with Escape; play it again and
  close the window while it is still playing; open the unplayable file, let it
  reach its error state, then press Escape and afterwards open the good file
  from the same window; finally, with a file playing, run
  `playerctl --player=fluorita status` and `metadata`, then seek and watch
  whether both change without polling
- **Pass condition:** no crash and no abort on any close path, no lingering process
  after the window is gone, the unplayable file shows a visible error instead of
  staying on "abriendo", the good file opened right after it plays normally, and
  `playerctl` reflects status, metadata and position changes as they happen
- **Result:** not run
- **Evidence:** none

The corrections behind this row are compiled and unit-tested in the
[render-context lifecycle evidence](docs/evidence/2026-08-05-render-context-lifecycle.md),
which cannot reach any of it: the defect was an ordering between two teardown
paths on real GPU state. A failure records the observation here and opens a
linked corrective unit.

## VAL-FLU-SOURCES — the source-first library in a real session

- **Status:** pending
- **Related implementation:** F5
- **Requires:** the deployed Fluorita binary, a real Wayland session, a working
  `org.freedesktop.portal.FileChooser` backend, and at least one folder of
  pictures and one of music outside the seeded XDG directories
- **Procedure:** launch Fluorita with no argument; select each folder in the
  sidebar in turn; press "Add folder…" and choose a folder outside the seeded
  ones; restart Fluorita; remove that folder again; then click once on an item,
  double-click another, and repeat both with Tab, the arrow keys and Enter
- **Pass condition:** the sidebar lists the mapped folders; selecting one shows
  exactly its supported media, with the grid for pictures and video, artists
  and albums for music, and both for a folder holding both; the chosen folder
  appears as an entry and is still there after the restart; removing it drops
  it and its items from the library while every file stays on disk; one click
  opens or plays an item; a double click does not restart what it just opened;
  and the keyboard reaches and activates the same things as the pointer
- **Result:** not run
- **Evidence:** none

The agent lane proved compilation, the domain rules, QML construction offscreen
and that the folder configuration reaches disk. It cannot prove appearance,
pointer behaviour or the desktop's own dialog. A failure records the
observation here and opens a linked corrective implementation unit; it does not
reopen F5.

## Closed historical observations

`VAL-FLU-PLAYBACK`, `VAL-FLU-INPUT`, `VAL-FLU-PRESENT`,
`VAL-FLU-LIFECYCLE` and `VAL-FLU-MPRIS` are preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).

## Coverage intentionally outside the current plan

An exhaustive codec/hardware matrix, full AT-SPI/reduced-motion review and an
hours-long idle playback soak are not pending version-1 milestones. If the
author requests one, add a bounded `VAL-FLU-*` row here; a failure then opens a
separate corrective implementation unit.
