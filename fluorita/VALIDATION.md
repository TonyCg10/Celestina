# Fluorita author validation

This manual lane does not contain implementation and does not block
[ROADMAP.md](ROADMAP.md).

## VAL-FLU-BYTES — a file whose name is not UTF-8, in a real window

- **Status:** pending
- **Related implementation:** F6-C
- **Requires:** the deployed Fluorita binary, a real Wayland session, and a
  configured root holding a picture, a video and a track whose names contain a
  byte that is not valid UTF-8 — create them with
  `touch $'foto\xff.jpg' $'clip\xff.mkv' $'pista\xff.flac'` in a mapped folder,
  and one folder named the same way to add through the chooser
- **Procedure:** rescan the library; click each of the three items once;
  right-click each one and open its properties, then close it; right-click the
  track and choose the trash entry; press the add-folder button and choose the
  folder whose name carries the byte; restart Fluorita; finally launch
  `fluorita $'/ruta/al/clip\xff.mkv'` from a terminal
- **Pass condition:** the three items appear with a replacement character in
  their visible names; one click opens or plays each of them rather than
  showing the item-is-gone notice; the properties panel fills in for each; the
  track really leaves the grid and is really in the desktop Trash; the
  chosen folder appears in the sidebar, still holds its content after the
  restart, and its removal takes no file with it; and the command line opens the
  clip in the player
- **Result:** not run
- **Evidence:** none

The agent lane proved the codec, the projection, the round trip and the
catalogue lookup in
[the byte-exact path seam evidence](docs/evidence/2026-08-06-byte-exact-path-seam.md).
It cannot prove what Qt does with these strings once a real QML surface, a real
portal backend and a real filesystem are involved. The image limit this row
originally carried is gone: `F6-D` addresses the probe by key and opens the file
by descriptor, so a picture whose name is not UTF-8 is expected to *display*,
not to be refused. A failure records the observation here and opens a linked
corrective unit.

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

## VAL-FLU-IMMERSIVE — the item as a movement, and a catalogue that forgets

- **Status:** pending
- **Related implementation:** F6
- **Requires:** the deployed Fluorita binary, a real Wayland session, one
  mapped folder holding several pictures and at least one video and one track,
  and a second mapped folder on a removable drive that can be unplugged
- **Procedure:** click a picture and watch it open, then close it; step through
  the folder with the filmstrip and with the arrow keys, crossing from a
  picture to a video; right-click an item, read its properties, close them,
  then trash another one; delete a file from the folder with the file manager
  and empty the Trash while Fluorita is open; unplug the removable drive and
  rescan; plug it back in; finally repeat the open and close with
  reduced motion enabled in the desktop
- **Pass condition:** the item grows out of the card that was clicked and
  shrinks back into it with no black frame in between, the space around it is
  lit by its own artwork, the filmstrip and the arrows walk the folder in
  projection order without wrapping, properties and trash act on the item the
  pointer named, the deleted file disappears from the library while the
  unplugged drive's items stay, and reduced motion still ends the playback
  session on close
- **Result:** not run
- **Evidence:** none

The agent lane proved the domain rules, the generations and offscreen
construction in the
[immersive content evidence](docs/evidence/2026-08-04-immersive-content.md). It
cannot prove perceived motion, ambient light or what a real removable drive
reports. A failure records the observation here and opens a linked corrective
unit; it does not reopen F6.

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

## VAL-FLU-EDIT — editing a real picture, on a real display

- **Status:** pending
- **Related implementation:** F7
- **Requires:** the deployed Fluorita binary, a real Wayland session on the
  author's 4K display at its usual scale, a mapped folder holding a photograph
  larger than the window, a screenshot, and a picture whose name contains a
  byte that is not valid UTF-8
- **Procedure:** open the photograph and rotate it, then crop it and save a
  copy; reopen the copy and check that the crop can still be undone; open the
  screenshot, write a line of text on it, draw an arrow and redact a region,
  then zoom in and out while drawing a freehand stroke; save it as a
  replacement and look for the original in the desktop Trash; open the
  non-UTF-8 picture, annotate it and save a copy; finally reach and operate
  every tool with Tab, the arrow keys and Enter, without the pointer
- **Pass condition:** every stroke, box and text lands where it was drawn at
  any zoom and at the display's real scale, not offset or the wrong thickness;
  a rotation costs no visible quality; the copy reopens with its objects intact
  and the replacement does not; the replaced original is really in the Trash
  and never merely gone; the non-UTF-8 picture writes its copy under a name the
  file manager can see; and the keyboard reaches every tool with its role,
  name and state announced
- **Result:** not run
- **Evidence:** none

The agent lane can prove the stack, the budgets, the format rule and that the
bytes land atomically. It cannot prove pointer precision, perceived quality,
what the desktop's Trash really holds, or how coordinates survive a real
display scale — the one place an annotation editor fails invisibly. A failure
records the observation here and opens a linked corrective unit.

## VAL-FLU-METADATA — correcting what a file says, on real files

- **Status:** pending
- **Related implementation:** F8
- **Requires:** the deployed Fluorita binary, a real Wayland session, a mapped
  music folder holding a track with wrong tags and an album with no embedded
  cover, and a photograph taken with a phone that recorded its location
- **Procedure:** correct the track's artist and album from Music and watch
  where it sorts; restart Fluorita and look again; give the coverless album a
  cover from a picture in the library; open the photograph's properties, read
  what it says it carries, remove its location as a copy, then repeat on
  another photograph as a replacement; finally check both results in another
  application that reads tags and EXIF
- **Pass condition:** the track re-sorts under the corrected name and is still
  there after the restart; the audio plays identically to before and another
  application agrees the stream was not re-encoded; the album shows its new
  cover; the properties panel names what each file carries; the copy has no
  location and the original still does; the replacement has none and its
  original is in the desktop Trash; and no file loses anything the edit did not
  name
- **Result:** not run
- **Evidence:** none

The agent lane can prove the container round trip, the byte-identical media
stream and the refusals. It cannot prove what another application reads back,
which is the only test that says whether the write was really lossless. A
failure records the observation here and opens a linked corrective unit.

## VAL-FLU-PACING — the two triggers, and what a stutter looks like

- **Status:** pending
- **Related implementation:** `F7-C`, and F9 and F15 in the roadmap
- **Requires:** the deployed binary, a real Wayland session, a film that
  stutters on this machine, and a video whose frame is worth keeping
- **Procedure:** open a film, press the transport's frame button, and look for
  the picture beside the film; open it and check it is the frame that was on
  screen, at the film's own size. Then press `Ctrl+Shift+P` while something is
  playing, watch the read-out for a minute, cause or wait for a stutter, and
  press `Ctrl+Shift+S`; open the file at the path it names
- **Pass condition:** the frame lands beside the film under a name that never
  overwrites an earlier one; the read-out appears, its rates change as the film
  plays, and its verdict says *dropping* or *delayed* when the picture visibly
  falters rather than staying *smooth*; the written report contains the
  readings behind that verdict
- **Result:** not run
- **Evidence:** none

The agent lane can prove the arithmetic, the bounds and the refusals, and it
can prove the surfaces construct. Whether the verdict matches what an eye sees
is the whole question, and only this lane can answer it. A verdict that reads
*smooth* through a visible stutter is a defect in the thresholds, not in the
observation.

## Closed historical observations

`VAL-FLU-PLAYBACK`, `VAL-FLU-INPUT`, `VAL-FLU-PRESENT`,
`VAL-FLU-LIFECYCLE` and `VAL-FLU-MPRIS` are preserved in the
[migration evidence](../docs/evidence/2026-08-03-migrated-author-observations.md).

## Coverage intentionally outside the current plan

An exhaustive codec/hardware matrix, full AT-SPI/reduced-motion review and an
hours-long idle playback soak are not pending version-1 milestones. If the
author requests one, add a bounded `VAL-FLU-*` row here; a failure then opens a
separate corrective implementation unit.
