# Fluorita

Celestina's local media library and player: Gallery and Music in a standalone
application, plus a bounded image/video/audio surface embedded in Siderita.

## User contract

- Index only configured local roots. The first run seeds them from the existing
  XDG Pictures, Videos and Music directories; after that they are the user's,
  added through the desktop folder chooser and removed again, and persisted.
  Never crawl the whole filesystem, and never delete a file when its root or
  its record leaves the catalogue.
- The library is navigated by root: a sidebar of the mapped folders, each
  showing the supported media inside it. Gallery projects images and video and
  Music projects artists, albums and tracks; which of them renders follows what
  the selected folder holds. One click opens an item, and direct activation
  starts it without hiding the library.
- In Siderita, `Space` views/plays media in place and double-click/`Enter`
  launches standalone Fluorita. The canonical mapping is the
  [content-activation contract](../docs/contracts/content-activation.md).
- Editing acts on the media the library already holds, and on any file
  handed to the floating editor, in the standalone application, under
  [ADR 0009](../docs/decisions/0009-editing-without-an-encoder.md). Every
  operation is either *lossless* — it reorders the original bytes — or
  *raster* — it produces a new image, and the interface says which. A picture
  can be turned, cropped, resized, written on, drawn on and redacted, and every
  mark stays selectable and undoable. Saving offers exactly two outcomes:
  «Guardar ambas», a copy beside the original, which stays reopenable, or
  «Guardar solo la editada», a replacement, which flattens the result and
  sends the original to the desktop Trash. Leaving with unsaved changes asks
  those two or «Descartar». The output format follows a fixed rule and is
  never a question. A video handed to the floating editor is trimmed
  instead: two handles choose the span kept, to the frame, and the result is
  a new, re-encoded film (*raster*) saved by the same two outcomes. The same
  operations apply to a chosen set of pictures at once, with progress,
  cancellation and an honest count of what could not be done.
- The floating editor (ADR 0012, `PRV-1`): `fluorita --edit PATH`, or
  `org.celestina.Fluorita1.Edit(s key)` on the bus (a `celestina_core::pathkey`
  key; anything but a regular file is refused with `InvalidArgs`), opens that
  one file in a window of its own titled «Editar — nombre», one window per
  file, the library window left as it is; started by `--edit`, Fluorita shows
  only that window. Selenita's capture preview opens it on a click. A copy
  saved with «Guardar ambas» joins Selenita's history when it is a capture
  (through `Selenita1.Adopt`, or Selenita's history file when Selenita is not
  running), and the saved result can be dragged out of the window into any
  program as a file, offered as a copy only.
- The video trim (ADR 0009 as amended, `PRV-1`): a video opened in the
  floating editor plays above a bar with two handles, start and end; moving a
  handle shows that frame, the arrows move it one frame of the film's own
  rate, and play plays the chosen span only. A span that is the whole film
  saves nothing. Saving runs `/usr/bin/ffmpeg` as a child, never linked: the
  span is re-encoded to H.264 (VA-API when the machine has it, else x264,
  and x264 again when a VA-API run fails) with AAC sound, into a hidden file
  beside the original that takes its name only once `ffmpeg` succeeded; the
  progress shows and the save can be cancelled. The result is always an MP4
  holding the main video and one audio track: a film in another container,
  or with more tracks or subtitles, says so before it is saved. Without
  `ffmpeg` or an H.264 encoder the window says so and the original is
  untouched.
- What a file says about itself can be read and corrected: a track's title,
  artist, album and album artist, the cover art embedded beside them, and the
  EXIF a photograph carries — including where it was taken, which can be
  removed. The media stream is copied across byte for byte; nothing is
  re-encoded. A container this suite cannot write says so instead of being
  half-written, which today means MP3, M4A and Ogg are read and refused.
- A frame of a film can be kept as a picture beside it, at the film's own
  resolution, and is then an ordinary image the editor can work on.
- Playback offers what a file really carries: its audio tracks, its subtitles
  or none of them, and a playback speed. At the end of an item it stops,
  continues with the folder, or repeats, and it advances only when the engine
  confirms the item ended.
- A picture can be looked at closely, in the viewer and in the editor alike,
  with `Ctrl` and the wheel or the magnifier beside the other actions. Resting
  on a video's card plays a bounded, silent preview of it in place.
- `Ctrl+Shift+P` reports what the picture is actually doing — frames lost and
  frames presented late, as rates — and `Ctrl+Shift+S` writes that to a file.
  It is a diagnostic for a moment that looks wrong, off unless asked for.
- Fluorita is not a streaming service, editing suite, social catalogue, general
  file manager or codec implementation. No layers, masks or blend modes, no
  configurable brushes, no per-channel colour correction, no rich text, and no
  linked encoder: the one re-encoding operation, a video's duration trim, runs
  `ffmpeg` as a child process; every other video and audio edit is bounded to
  demux and remux.

Static image thumbnails, video posters and embedded covers use the freedesktop
PNG cache, and carry the spec's `Thumb::URI` and `Thumb::MTime` keys. Thumbnails
are automatic: images go through the shared provider in `fluorita-qt` and Qt's
image reader, and video posters and covers are produced by a bounded,
cancellable background pass after each scan. A hover preview is a live, silent,
bounded session, never a file; showing a row never starts playback.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/fluorita-core` | Media identity/kind, catalogue projections, capabilities, playback truth, the session/surface handshake and generation-stamped resource contracts; no Qt/decode |
| `../celestina-rs/crates/fluorita-engine` | Bounded scan/watch, persisted catalogue and edit recipes, metadata, artwork and the libmpv playback session loop |
| `../celestina-rs/crates/fluorita-qt` | Shared C++/Qt Quick framebuffer/render seam for libmpv and the freedesktop-thumbnail image provider |
| `src/` | Standalone CXX-Qt adapters, owned workers, the folder-chooser portal client, argv reading, the adapter of the shared single-instance activation (`celestina_core::activation`) with Fluorita's own `Fluorita1` interface, the adoption of an edited capture into Selenita's history, the video trim's `ffmpeg` worker, and MPRIS2 |
| `qml/` | Source sidebar, Gallery, Music, complete player composition and the floating edit window |
| `cpp/` | The narrow toolkit seams unavailable through CXX-Qt: the image probe and the edit canvas that draws and encodes a picture |
| `../siderita/src/media.rs`, `../siderita/qml/dialogs/` | Separate thin adapter and minimal embedded player |
| `../celestina-style` | Canonical visual tokens, controls and assets |

Requests remain pending until the engine confirms them. Scan, extraction,
decode and playback are off the GUI thread; generations prevent stale work from
replacing a new selection and every host owns deterministic shutdown.

## Build and use

Fluorita needs Rust, a compatible Qt 6 development environment and libmpv
development/runtime support. The video trim runs `/usr/bin/ffmpeg` (the
`ffmpeg` package libmpv already depends on); its tests run it and `ffprobe`
on the committed fixture `tests/fixtures/three-seconds.mp4`, which
`tests/fixtures/make-three-seconds.sh` wrote once. The canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

Build creates the release artifact once; verify exercises that exact artifact
without replacing the installed binary or registering desktop handlers; status
reports whether the verification seal still matches the current inputs; deploy
installs the already verified binary, desktop entry and icons without
recompiling. `scripts/run.sh` remains a human convenience, not the canonical
agent verification entry. A change to a shared Fluorita crate also completes
Siderita because its embedded media surface consumes the same core, engine and
Qt seam; verifying the second host without completing it would leave the
author's installed file manager stale.

Desktop-entry registration can become the effective default for an unpinned
MIME type on this desktop. Completion is authorised to register it, but the
agent must report the observed handler change. A preference pinned by the user
in `mimeapps.list` remains authoritative.

After completion, launch `fluorita [PATH]`, use the desktop entry
or control the active session through MPRIS2. `fluorita --edit PATH` opens the
floating editor on one file: a picture to edit, or a video to trim.

niri places the editor, not Fluorita: a window rule matches its title and
floats it (the suite exit `PRV-1-E` adds it to the author's configuration):

```kdl
window-rule {
    match app-id=r#"^org\.celestina\.Fluorita$"# title="^Editar — "
    open-floating true
}
```

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
- [Roadmap history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
