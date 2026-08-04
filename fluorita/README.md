# Fluorita

Celestina's local media library and player: Gallery and Music in a standalone
application, plus a bounded image/video/audio surface embedded in Siderita.

## User contract

- Index only configured local roots, initially seeded from existing XDG
  Pictures, Videos and Music directories; never crawl the whole filesystem or
  delete a source when it leaves the catalogue.
- Gallery projects images and video; Music projects artists, albums and tracks.
  Direct activation opens and starts an individual item without hiding the
  library.
- In Siderita, `Space` views/plays media in place and double-click/`Enter`
  launches standalone Fluorita. The canonical mapping is the
  [content-activation contract](../docs/contracts/content-activation.md).
- Fluorita is not a streaming service, tag editor, social catalogue, general
  file manager or codec implementation.

Static image thumbnails, video posters and embedded covers use the freedesktop
PNG cache. Live video trailers are separate, bounded and cancelable; normal
navigation never starts the decoder merely to show a row.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/fluorita-core` | Media identity/kind, catalogue projections, capabilities, playback truth and generation-stamped resource contracts; no Qt/decode |
| `../celestina-rs/crates/fluorita-engine` | Bounded scan/watch, persisted catalogue, metadata, artwork, trailers and libmpv playback |
| `../celestina-rs/crates/fluorita-qt` | Shared C++/Qt Quick framebuffer/render seam for libmpv |
| `src/` | Standalone CXX-Qt adapters, owned workers, activation and MPRIS2 |
| `qml/` | Gallery, Music and complete player composition |
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
