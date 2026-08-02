# Fluorita

The Celestina suite's local media library and player. The standalone application
organizes the author's media into two first-class sections — **Gallery** for
images and video, and **Music** for albums, artists and tracks — while still
opening and playing an individual file handed to it by Siderita or the desktop.

- **Role:** local media library + player/viewer for image, video and audio
- **Stack:** Rust · Qt Quick/QML via CXX-Qt · libmpv behind a narrow engine contract
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores ·
  [celestina-style](../celestina-style/) tokens and controls
- **Consumed by:** standalone Fluorita and Siderita's minimal media player
- **Speaks:** freedesktop thumbnail cache · MPRIS2 · XDG MIME/desktop activation

> **Status: it plays and it has a library.** The
> library and two-surface direction were settled by the author on 2026-07-30, and
> so was the decode backend: [`spikes/`](spikes/) measured the candidates and the author
> chose **libmpv**. `fluorita-core` and `fluorita-engine` exist and are tested
> against real media — probing, artwork publication, bounded trailers and
> truthful playback sessions. The application builds, is covered by both suite
> guards, and hosts a libmpv render surface with a truthful transport: video and
> audio were played end to end in a real Wayland session on 2026-07-31. Stills
> are decoded by the toolkit under a measured budget, never by the media
> backend. A bare launch opens the library — Gallery and Music over the
> configured XDG roots — and activating an item is the same path as being handed
> it on the command line. The catalogue survives between launches, so tags
> already read are not read again; the thumbnails the shared cache is missing are
> produced only when asked for (`Ctrl+G`), bounded and cancellable; and playback
> is published over MPRIS2, so the shell, the media keys and the phone link read
> one source of truth. `scripts/run.sh` installs it into `~/.local` — binary,
> desktop entry and icon — with `--prefix` and `--uninstall` for exercising the
> layout somewhere disposable first.
> [ROADMAP.md](ROADMAP.md) records the numbers, the reasoning and what is still
> missing.

## Product contract

| User action | Surface | Contract |
|---|---|---|
| `Space` on image, video or audio in Siderita | Embedded Fluorita player | A minimal modal for viewing or playback without leaving Siderita |
| Double-click or `Enter` on media in Siderita | Standalone Fluorita | Open the complete app, start that item and keep Gallery/Music available |
| Direct app launch | Standalone Fluorita | Browse the persistent local library and play a selected item |

The standalone app is not a file manager. Its library indexes only configured
local media roots, initially seeded from the user's XDG Pictures, Videos and
Music directories when they exist. Gallery groups images and video; Music uses
media tags for albums, artists and tracks. Siderita remains the general browser.

## Shared media resources

Fluorita exposes one shared contract with two different output classes:

- **Static interoperable artwork:** image thumbnails, video poster frames and
  embedded audio covers are written as PNGs into the freedesktop thumbnail
  cache. Siderita and other desktop applications can reuse them without loading
  the decoder.
- **Live previews:** a video may expose a short, bounded trailer loop generated
  on demand. This is not a freedesktop thumbnail — that standard stores one
  static PNG — and it is never generated for every visible row. Selection,
  hover or `Space` may request one; moving away cancels it.

Music without embedded artwork uses a generic themed cover. A failed extraction
is a normal result, not a broken file.

## Shared architecture

| Path | Responsibility |
|---|---|
| `../celestina-rs/crates/fluorita-core` | media identity/kind, library and playback state, thumbnail/preview requests and cache contracts; no Qt or decoding |
| `../celestina-rs/crates/fluorita-engine` | bounded library scanning, the persisted catalogue file, metadata probing, freedesktop poster/cover publication and playback sessions over libmpv, behind a narrow replaceable contract (backend chosen by measurement, 2026-07-30), plus bounded live trailers in Fluorita's own pruned cache |
| `src/`, `qml/` | standalone library + player adapter and UI: activation, a scan and a session both owned off the GUI thread, the Gallery and Music surfaces, and a Qt Quick surface libmpv renders into |
| `cpp/` | the app's own hand-written C++: the header-only image probe (`QImageReader`), which cxx-qt-lib does not expose |
| `../celestina-rs/crates/fluorita-qt` | the shared render seam: the `QQuickFramebufferObject` that drives libmpv's render API, compiled by both this app and Siderita's embedded modal |
| `../siderita/src/`, `../siderita/qml/` | separate thin adapter and minimal embedded player consuming the same contracts |

The engine is loaded lazily: browsing normal files in Siderita continues to read
only cached PNGs. Media decoding starts only when a preview/player requests it.
Like Magnetita, shared domain/engine contracts support distinct UIs; Fluorita and
Siderita do not import each other's QML or duplicate playback rules.

## Library boundaries

The library is local and source-based. It may persist paths, stable media
identity, metadata and cache state, then update incrementally as configured
roots change. It does not crawl the whole filesystem, manage arbitrary files,
edit tags or fetch streaming catalogues. Removing an item from the library never
deletes the source file.

## Non-goals

No streaming service, social catalogue, tag editor, general file browser or
codec reimplementation. Subtitle, track-selection and playback-speed features
remain later comforts; the initial product must first prove library truth,
playback and bounded resource use.
