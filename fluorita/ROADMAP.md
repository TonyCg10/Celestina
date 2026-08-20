# Fluorita implementation roadmap

- **Status:** planned
- **Active implementation checkpoint:** none
- **Authorised sequence:** F7-F15, opened by author decision on 2026-08-19 and
  delivered on 2026-08-20 in a single commit. Nothing is in flight; a new
  checkpoint needs a measured need and the author's word
- **Related author validation:** none; completed observations are in
  [VALIDATION.md](VALIDATION.md)

Fluorita 1.0 is implemented and its F0-F4 arc is closed. Do not reopen it,
repeat completed perceptual tests in this file or treat a manual check as
unfinished implementation.

## F5 — Source-first library and direct activation

**Measured need.** The author specified the standalone library as a sidebar of
the folders they map, each showing the supported content inside it. The shipped
surface is two fixed kind tabs over one flat catalogue: the mapped roots are
modelled in `fluorita-core` and carried on every record, but the projection
discards that column, so the axis the user configured is the one axis the
interface cannot navigate. Separately, a single click on an item selects it and
does nothing else — opening requires a double click — so the library reads as
if activation were broken. The contract change this needs is
[ADR 0006](../docs/decisions/0006-source-first-library-navigation.md),
delivered by suite checkpoint ACT-1.

**Bounded resource and lifecycle model.** No new decode, scan or watch
behaviour. Source configuration is a small persisted file read once per launch
and rewritten whole on change, guarded by the suite's atomic replacement, in
the same bounded percent-encoded format the catalogue store already uses.
Choosing a folder is one bounded portal request off the GUI thread; a desktop
with no portal degrades to a stated failure, not a hang. Adding or removing a
source reuses the existing scan and watch path instead of introducing a second
one, and removing one never touches a file on disk.

**Tangible result.** Launching Fluorita shows a sidebar of the mapped folders
with a button that opens the desktop folder chooser and keeps the chosen folder
as a persistent entry. Selecting a folder shows the supported content inside
it: the Gallery grid for a source contributing images or video, the Music
projection for one contributing audio. A single click opens the item.

Implementation is complete and both registered completion commands passed; the
[evidence](docs/evidence/2026-08-04-source-first-library.md) records where the
verified bytes were built and why. What remains is the ledger closure, which
belongs to the author's commit request.

- [x] Make configured sources user-owned and persistent, with stable identity
      across runs and first-run seeding from the existing XDG media
      directories.
- [x] Publish the sources to QML, resolve the selected source's projection, and
      add or remove a folder through the desktop file-chooser portal without
      blocking the GUI thread.
- [x] Replace the two kind tabs with the source sidebar and its adaptive
      content panel, and activate an item on a single click.

The build order, exclusions, exit and ledger are in the
[archived plan](docs/plans/archive/2026-08-04-source-first-library.md).

## F6 — Immersive content and honest catalogue

**Measured need.** Three faults the author found while using F5, and one thing
it lacked. Deleted files kept appearing — even ones emptied from the Trash —
because the catalogue marks a missing record and nothing ever forgets it; the
rule was written for a disconnected drive and could not tell that case from a
real deletion. Opening an item jumped rather than moved, so the connection
between the card clicked and the picture shown was left to the person to infer.
There was no way to act on an item: no delete, no properties. And there was no
way to reach the next item without leaving the one you were on.

**Bounded resource and lifecycle model.** No new decode and no frame grabbing.
The ambient light and the opening transition both reuse the thumbnail the card
already had, so neither costs a second read. Trashing is the suite's existing
freedesktop operation on a worker, and the record goes only when the engine
confirms the file moved. Stepping between items closes the previous session
through the render handshake `close` already defines rather than tearing an
mpv instance down under a live surface.

**Tangible result.** The library shows what is there. An item grows out of its
card and shrinks back into it. Right-click offers Trash and Properties. A
picture gets a filmstrip of the rest of the folder on approach; a video or a
track gets previous and next. Whatever is open lights the space around it.

Implementation is complete: every unit in the ledger is delivered, and the
[evidence](docs/evidence/2026-08-04-immersive-content.md) records what the agent
lane could and could not reach. The real-session observations it cannot make
are `VAL-FLU-IMMERSIVE`, `VAL-FLU-TEARDOWN` and `VAL-FLU-BYTES`; pending manual
validation does not keep this checkpoint open.

- [x] Forget what a completed scan of a reachable root did not find, and keep
      what an unreachable one holds.
- [x] Grow the open item out of its card and back, carrying the loaded
      thumbnail so nothing shows black while a decoder starts.
- [x] Offer Trash and Properties on an item, through the suite's shared menu
      and modal.
- [x] Navigate the folder without leaving the item, by filmstrip or by arrows
      according to what is open.
- [x] Light the space around the content with the content's own artwork.
- [x] Return the surface to Spanish under ADR 0007.

The build order, exclusions, exit and ledger are in the
[archived plan](docs/plans/archive/2026-08-04-immersive-content.md).

## F7 — Editing what the library already holds, closed 2026-08-20

**Measured need.** The author edits the pictures this library already shows,
and today every one of those edits leaves Fluorita: a photo that opens upside
down has to be rotated somewhere else, a screenshot that needs an arrow and a
word has to travel to another application, and a number plate or an address in
a picture about to be sent cannot be covered here even though the same surface
is already showing it and already knows how to strip the file's location. The
library can name, open, describe and trash an item; it cannot change one. That
is the gap, and it is the reason the product's own limit — "not a tag editor" —
has to be replaced by a narrower, truer one: Fluorita edits the media it
indexes and is not an editing suite.

**Contract change.** A new decision record rules what editing means here and
splits every operation into two categories the interface must not blur:
*lossless*, which reorders the original bytes, and *raster*, which produces a
new image. Rotation by EXIF orientation is the first; crop, resize and every
annotation are the second. The record also replaces the `tag editor` exclusion
in the README with the suite exclusion — no layers, no masks, no blend modes,
no configurable brushes, no per-channel colour correction, no re-encoding — and
states why the boundary sits at the encoder rather than at the feature list.

**Bounded resource and lifecycle model.** No new decode backend and no
encoder: the image writer is the toolkit that already reads and measures these
files, so the dependency closure does not change and libmpv keeps its single
role of playing media that moves. The existing byte and pixel budgets in
`fluorita/src/image.rs` are checked before an edit allocates anything, exactly
as they are before a view does. Rasterising, writing and trashing run on a
worker under the generations the engine already enforces; a result that arrives
after the item changed is discarded rather than written. A write never removes
its source before the destination is confirmed, and replacing an original sends
it to the desktop Trash through `siderita-ops` instead of unlinking it. The
edit stack is a bounded object list persisted beside the catalogue, keyed by
media identity — never a sidecar file dropped into the author's own folders.

**Tangible result.** A picture opens, and it can be turned, cropped, resized,
written on, drawn on and redacted, with every object still selectable, movable
and undoable afterwards. Saving offers exactly two outcomes: a copy beside the
original, which keeps the edit reopenable, or a replacement, which flattens the
result and sends the original to the Trash. No dialogue asks about formats.

- [x] Rule the contract: the decision record, the README's new limit, and the
      capability matrix in `fluorita-core` that answers what a given item
      admits and whether the answer is lossless or raster.
- [x] Model the edit as a composable stack in `fluorita-core` — canvas
      transformations first, annotation objects in the resulting canvas
      coordinates — with undo, redo and validation, and persist it beside the
      catalogue under the media's identity.
- [x] Write an image from the engine: rasterise the stack, apply the fixed
      output-format rule, and land the bytes through the suite's atomic
      replacement, with copy and replace as the two confirmed outcomes and the
      original reaching the Trash rather than `unlink`.
- [x] Turn, crop and resize, with rotation taken as orientation metadata when
      the file can carry it so the common case costs no pixels.
- [x] Annotate: text, freehand stroke, line and arrow, rectangle and ellipse,
      highlighter, and redaction by pixelation or blur — held as objects in
      image coordinates, so a stroke lands where it was drawn at any zoom or
      display scale.
- [x] Compose the edit mode inside the immersive viewer F6 delivered, with the
      icon-first anatomy, live preview, keyboard reach for every tool, and
      confirm or discard as the only ways out.

Implementation is complete and both registered completion commands passed:
Fluorita at 1.3.0 and Siderita, which consumes the same shared crates, are
built, verified and deployed. The
[evidence](docs/evidence/2026-08-19-bounded-media-editing.md) records what ran,
what the guards refused on the way, and what the agent lane cannot reach —
which is everything a person actually sees, and is `VAL-FLU-EDIT`. What remains
is the ledger closure, which belongs to the author's commit request.

The build order, exclusions, exit and ledger are in the
[archived plan](docs/plans/archive/2026-08-19-bounded-media-editing.md).

**F7 carried more than F7.** The single `F7-A` commit that closed it also
delivered everything in F8-F15 below: the plan's own ledger says so, and the
inventory's 61 files are the proof. The sections that follow describe what
shipped rather than what was planned, because a roadmap that named only the
editing surface would leave the next reader believing the rest was still to be
built — and rebuilding it is exactly what such a document causes.

## F8 — What a file says about itself

**Measured need.** F7 gave the library a way to change a picture, and every one
of its operations rewrites pixels or refuses to. The half of editing that
touches no pixel is still missing, and it is the half the author's own library
needs most often: a track whose tags are wrong sorts into the wrong artist and
stays there, because Music projects exactly what the file claims and Fluorita
has no way to correct it; an album with no embedded cover shows a blank card
for ever; and a photograph about to leave this machine still carries the
address it was taken at, which F7 can cover with a redaction but cannot remove.
Three different faults, one cause: the container is readable and not writable.

**Bounded resource and lifecycle model.** No rasteriser and still no encoder.
A tag write replaces a container's metadata block and copies the media stream
across untouched, which is the *lossless* class
[ADR 0009](../docs/decisions/0009-editing-without-an-encoder.md) already
defines; the audio a person owns is never re-encoded. Reading stays on the
existing bounded probe, writing runs on a worker under the same generations, and
the result lands through the suite's atomic replacement with the original
reaching the Trash only when a person asked for a replacement. Cover art is
bounded before it is embedded, by bytes and by pixels, for the same reason the
viewer bounds what it decodes. Stripping EXIF removes; it never rewrites what it
does not understand, and a block it cannot parse is refused rather than dropped.

**Tangible result.** A track's title, artist and album can be corrected and the
library re-sorts under the name it now carries. An album that has no cover can
be given one from a picture already in the library. A photograph can have its
location, its camera and its timestamps removed — as a copy or in place, on the
same two terms every other edit offers — and the library says which of those a
file is still carrying.

- [x] Read and write the tag block of the containers the library already
      classifies as audio, preserving the media stream byte for byte and
      refusing a container the writer does not fully understand. FLAC is what
      the writer covers; Ogg, ID3 and MP4 are read, reported and refused rather
      than half-written.
- [x] Correct a track's title, artist, album and album artist, with the library
      learning the change the way it learns any other external edit: the
      rewritten container has a new identity, so the scan and the watch drop
      its extracted metadata and probe the file again. Nothing predicts the new
      values.
- [x] Embed cover art, chosen through the desktop's own picker and bounded by
      bytes and pixels before anything is read. A new front cover replaces the
      one the file had rather than joining it; a back cover or a photograph of
      the artist is left alone.
- [x] Report what metadata a file carries — including whether a photograph
      still says where it was taken — and remove it, through the same
      copy-or-replace outcomes F7 established. The whole EXIF segment goes
      rather than individual tags: a picture whose camera fields were removed
      while its GPS pointer still resolved would be worse than one carrying
      nothing.
- [x] Offer it from the item menu, in the shared modal anatomy, with every
      action reachable by keyboard. The menu asks what an item admits before it
      is shown, so a video — whose tags nothing here projects — is not offered
      an entry that would refuse.

**What it does not write.** MP3, M4A and Ogg are read, reported and refused:
each needs a container writer this suite does not have, and a half-written
container is worse than one left alone. The panel says so in words rather than
disabling a control with no reason attached.

## F9 — Keeping a frame of a film

**Measured need.** A person watching something pauses on a frame and wants that
frame: as a wallpaper, to send, to annotate. Fluorita could already render one —
it does exactly that to produce a poster — and had no way to hand it over.

**Bounded resource and lifecycle model.** The poster path, with two differences:
the position is the one the engine confirmed the player is at, and no scale
filter, because this is the picture a person keeps rather than a thumbnail. One
extraction at a time, on its own backend instance off the GUI thread, into a
staging directory that is removed whether it succeeded or not, landing through
the suite's atomic replacement under the keep-both name policy.

**Tangible result.** A button beside the picture while a film is open. The frame
lands next to the film as `nombre (fotograma).png`, at the film's own
resolution, and is then an ordinary library item — which is what lets F7's
editor crop, annotate and redact it without any of that being written twice.

- [x] Render the frame at the confirmed position and land it beside the film,
      bounded, cancellable and refusing rather than writing on failure.
- [x] Offer it where the film is, and say where the picture went.

Implemented and tested in the checkout, without a ledger unit, on the same
terms as F8.

**What stays shut, and why.** Trimming between two points and removing a track
are the rest of this idea and they are *not* here: both need a muxer. An MP4's
atom tree has to be rebuilt with every offset in it corrected; a Matroska's
clusters and cues likewise. That is the same wall F8 met with ID3 and MP4 tags,
and the same answer — either done properly or not claimed. It is now the
strongest argument for the encoder decision ADR 0009 deferred, because the
person asking for a trim is asking for the one thing this suite has decided
twice not to fake.

## F10 — The same small change, to many files

**Measured need.** Two things a folder of photographs actually needs, and
neither of them is a photo-by-photo job: they are all sideways, or they are all
about to be sent to somebody and none of them should still say where it was
taken. Doing either one at a time is the work the library exists to remove.

**Bounded resource and lifecycle model.** No new writer: every item goes
through the same single-item save or metadata write, so the ordering rules have
one place to be got wrong rather than two. The run is on a worker, checks
cancellation before every item, and reports its tally after each one. A file
that refuses is counted and the run continues — forty photographs are not
abandoned because one is unreadable.

**Tangible result.** Ctrl-click picks pictures out of the grid, and a bar
appears with what can be done to all of them at once: turn, mirror, and forget
what they carry. It says how many were changed, how many could not be, and how
many failed — and it says it about a run that was stopped, too.

- [x] Offer only the operations that mean the same thing on every picture, and
      refuse the ones that are absolute by construction — a crop measured on
      one photograph names a different part of the next, and a word belongs
      where it was written.
- [x] Run over a selection off the GUI thread, with progress after every item,
      cancellation before every item, and one failure that does not end the
      run.
- [x] Pick items out of the grid and say what became of them, counting what was
      skipped rather than reporting a quiet success.

Implemented and tested in the checkout, without a ledger unit, on the same
terms as F8 and F9.

The arc continues past F10 with work that is authorised in intent but not
opened:

- trimming by entry and exit points and removing or silencing a track, which
  need a muxer and therefore the decision ADR 0009 deferred;
- re-encoding, which requires a measured need the above cannot meet, because it
  introduces an encoder into a closure that today has none.

None authorises a streaming catalogue, codec rewrite, global filesystem crawl
or duplication between standalone and Siderita. Editing belongs to the
standalone application; Siderita's embedded surface keeps content, honest state
and transport only.

## F11 — Choosing a stream and a speed

**Measured need.** A film with two audio tracks played whichever the backend
picked, and a film with subtitles played none of them, because nothing in the
interface could say otherwise. The player could start, pause, seek and set a
volume; everything else about how a film sounds and reads was decided for the
person.

**Bounded resource and lifecycle model.** No new session and no second decode:
choosing a track sets one backend property on the session that is already open,
and the value is published only when the backend confirms it. A track list is
read out of a file, so it is bounded before it is held — a count ceiling, a
label ceiling, and control characters stripped from what a container claims.
Speed is clamped to a range and offered as a few known rates rather than as a
slider, because 1.03x by accident is not something anyone asks for.

**Tangible result.** A control in the transport lists the audio tracks and the
subtitles a film carries, marks the one in use, offers turning subtitles off,
and sets the playback speed. A film with one audio track and no subtitles does
not show the control at all.

- [x] Read the tracks a file carries, bounded, and publish them with the one
      in use marked.
- [x] Select an audio track and a subtitle track, or none, confirmed by the
      backend before it is shown as selected.
- [x] Set the playback speed from a short list of rates.

## F12 — What plays next

**Measured need.** Reaching the end of a track was the end of listening: the
session stopped and the next song in the folder sat there waiting to be
clicked. A music player that cannot play an album is not one.

**Bounded resource and lifecycle model.** No queue and no second model of the
library: the folder's order is the one the filmstrip already navigates, and the
rule is a pure function over a position in it. Nothing advances on its own —
the host asks what follows only after the engine has *confirmed* the file
ended, so a track whose last seconds fail to decode is not skipped past. A
still has no end to reach, so a gallery cannot turn itself into a slideshow.

**Tangible result.** A choice of what happens at the end: stop, continue with
the folder, or repeat. Continuing stops after the last item rather than
starting again from the top, because a list that never ends is one that plays
to an empty room.

- [x] Rule what follows an item that ended, as a pure function over the
      folder's order.
- [x] Offer the three modes where the rest of playback is chosen.

## F13 — Looking closer

**Measured need.** A photograph could be looked at only at the size the window
gave it, in the viewer and in the editor alike — so checking whether a face was
sharp, or placing a redaction over something small, meant opening another
application.

**Bounded resource and lifecycle model.** One owner for the arithmetic, used by
both surfaces: zoom that behaved differently in the viewer and the editor would
be the same defect as a mark that lands where it was not drawn. Every
conversion between a pointer and the picture goes through a single scale, so a
stroke drawn while zoomed in lands where it was drawn. Enlarging asks the
reader for more pixels rather than magnifying the ones it already threw away,
bounded to four times the window and settled after the person stops moving.

**Tangible result.** `Ctrl` and the wheel, at the pointer, in the viewer and in
the editor; a magnifier beside the other actions; dragging to move around;
double-click to go back to the whole picture. Zoom is around the cursor rather
than the centre, so the gesture can be aimed instead of chased.

- [x] Own the zoom arithmetic once and use it in both surfaces.
- [x] Convert every pointer position through the zoomed scale.
- [x] Raise the decode as the picture is enlarged, bounded and settled.

## F14 — A film that moves under the pointer

**Measured need.** A grid of video posters says what a file is called and
nothing about what is in it, and the roadmap had kept this shut until the
trigger, the cancellation and the budget were defined rather than assumed.

**Bounded resource and lifecycle model.** One preview at a time, on the rule
`fluorita-core` already owned. It starts after a dwell rather than on the first
pixel of contact, and leaving cancels it. It is silent, it loops so a frozen
last frame never reads as a hang, it starts inside the film rather than in its
titles, and it decodes in software because a picture the size of a card does
not need a hardware context. It never reaches the bus: one desktop has one
media player, and a film playing because a pointer went past is not what "now
playing" means.

**Tangible result.** Resting on a video's card plays a bounded, silent preview
of it in place; moving away stops it.

- [x] Give the preview a trigger and a cancellation, and keep one at a time.
- [x] Keep it silent, looping and off the bus.

## F15 — What the picture is actually doing

**Measured need.** Presentation-timing work was shut because judder that cannot
be reproduced cannot be fixed, and "it stutters sometimes" is not a
measurement. The backend has counted dropped and delayed frames all along, and
nothing could show them to the person watching the stutter happen.

**Bounded resource and lifecycle model.** A bounded recording — a fixed number
of samples, one a second, folded by a pure function that never reads a clock.
The counters are cumulative, so what is reported is the difference between two
readings over the time between them; a counter that goes backwards is a reset
and contributes nothing, because a negative rate would read as a picture that
repaired itself. Off by default: a permanent read-out would be furniture.

**Tangible result.** `Ctrl+Shift+P` shows what the picture is doing, with
dropped and late frames as rates per minute, the display's refresh rate and the
worst jitter seen. `Ctrl+Shift+S` writes it to a file and names the path, so a
person who just saw judder has something to attach to a report rather than a
memory of it. The verdict distinguishes losing frames from presenting them
late, with its thresholds written down.

- [x] Fold cumulative counters into rates, treating a reset as a fresh start.
- [x] Show the recording while it happens and write it out on request.

## Conditions for opening the next checkpoint

A new checkpoint must begin with a measured user need, a bounded resource and
lifecycle model, and a tangible result. Three of the four ideas this section
used to hold are now above it, delivered; what remains conditional is:

- **Trimming, dropping a track, converting a format and exporting a clip** all
  need a muxer, and this suite has none. That is one decision — whether FFmpeg
  enters the dependency closure — with a cost the author has not been asked to
  pay yet: a second media stack, a new hostile-input surface, long jobs needing
  their own progress model, and a much heavier production verification. Until
  it is taken, every one of those operations is refused rather than
  approximated, and F8 and F9 are written so that taking it later adds a writer
  instead of rewriting them.
- **Correcting tags in MP3, M4A and Ogg**, which F8 reads, reports and refuses.
  Each needs its own container writer — an atom tree rebuilt with every offset
  corrected, or page checksums recomputed — and each is either done properly or
  not claimed.
- **Repairing frame pacing.** The condition used to be "reproducible judder";
  F15 is the instrument that makes it reproducible, so what is missing now is a
  captured report from a real session showing something to fix, and an account
  of Qt owning the final frame swap.
- **Shell MPRIS presentation**, which requires a shell-owned checkpoint in
  Celestina and preserves Fluorita as the single confirmed playback source.

## Implementation exit

For F7 and any newly authorised checkpoint, close implementation when focused
core/engine tests and lifecycle/resource bounds pass and every affected
deployable host completes its registered production flow. Fluorita-only work
uses `fluorita/scripts/complete-production.sh`; a shared `fluorita-core`,
`fluorita-engine` or `fluorita-qt` change also uses
`siderita/scripts/complete-production.sh`. Each command builds its release once,
verifies those exact bytes and deploys them to the author's normal test
destination. Real playback or perceptual acceptance belongs to `VALIDATION.md`
and never keeps implementation open.

## Closed evidence

The completed F0-F4 implementation, backend spike, fixes, measurements and
real-session observations are preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
