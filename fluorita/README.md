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
- Editing acts on the media the library already holds, in the standalone
  application, under
  [ADR 0009](../docs/decisions/0009-editing-without-an-encoder.md). Every
  operation is either *lossless* — it reorders the original bytes — or
  *raster* — it produces a new image, and the interface says which. A picture
  can be turned, cropped, resized, written on, drawn on and redacted, and every
  mark stays selectable and undoable. Saving offers exactly two outcomes: a
  copy beside the original, which stays reopenable, or a replacement, which
  flattens the result and sends the original to the desktop Trash. The output
  format follows a fixed rule and is never a question. The same operations
  apply to a chosen set of pictures at once, with progress, cancellation and an
  honest count of what could not be done.
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
  encoder: video and audio editing is bounded to demux and remux, so a cut
  lands where the keyframes are.

Static image thumbnails, video posters and embedded covers use the freedesktop
PNG cache. Live video trailers are separate, bounded and cancelable; normal
navigation never starts the decoder merely to show a row.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/fluorita-core` | Media identity/kind, catalogue projections, capabilities, playback truth and generation-stamped resource contracts; no Qt/decode |
| `../celestina-rs/crates/fluorita-engine` | Bounded scan/watch, persisted catalogue, metadata, artwork, trailers and libmpv playback |
| `../celestina-rs/crates/fluorita-qt` | Shared C++/Qt Quick framebuffer/render seam for libmpv |
| `src/` | Standalone CXX-Qt adapters, owned workers, the folder-chooser portal client, activation and MPRIS2 |
| `qml/` | Source sidebar, Gallery, Music and complete player composition |
| `cpp/` | The narrow toolkit image-probe seam unavailable through CXX-Qt |
| `../siderita/src/media.rs`, `../siderita/qml/dialogs/` | Separate thin adapter and minimal embedded player |
| `../celestina-style` | Canonical visual tokens, controls and assets |

Requests remain pending until the engine confirms them. Scan, extraction,
decode and playback are off the GUI thread; generations prevent stale work from
replacing a new selection and every host owns deterministic shutdown.

## Build and use

Fluorita needs Rust, a compatible Qt 6 development environment and libmpv
development/runtime support. The canonical production workflow is:

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
or control the active session through MPRIS2.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
- [Roadmap history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
