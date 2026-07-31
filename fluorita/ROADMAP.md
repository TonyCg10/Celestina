# Fluorita roadmap

> Part of the [Celestina suite](../ROADMAP.md). Checklist legend: `[x]` done ·
> `[ ]` planned. Source presence is not playback evidence. The author authorized
> a standalone library/player and a minimal Siderita player on 2026-07-30. The
> shared core and the libmpv engine exist and are tested against real media, and
> the application is a library and a player, both exercised in a real session.

## Settled product decisions

- **The standalone app is a library and a player.** Gallery contains images and
  video; Music contains albums, artists and tracks. Opening a file directly
  starts it without hiding those library sections.
- **`Space` means play/view in place.** In Siderita it opens a minimal Fluorita
  modal for image, video or audio. Text remains Grafita's branch; directories,
  binaries and unsupported types retain ordinary Quick Look.
- **Double-click and `Enter` mean the full app.** Media activates standalone
  Fluorita and starts the selected item.
- **One contract, two UIs.** `fluorita-core` owns media/library/playback truth;
  `fluorita-engine` owns metadata, decoding and derived resources. Fluorita and
  Siderita keep separate thin adapters and QML compositions.
- **Static thumbnails and live trailers are different.** The freedesktop cache
  receives a PNG image thumbnail, video poster or audio cover. A short video
  trailer is an on-demand, bounded live preview and never masquerades as a
  standard thumbnail.
- **Decode weight is lazy.** Siderita may consume the shared engine for its
  explicit media modal, but normal browsing reads cached artwork and does not
  initialize the backend.

## Technical contract

### Library

The initial sources are configured local roots, seeded from existing XDG
Pictures, Videos and Music directories. Scans are bounded, cancellable and never
run on a GUI thread. The persistent catalogue records stable media identity,
path, kind, size/mtime and extracted metadata. It does not delete source files;
missing items become unavailable until reconciliation confirms removal.

Gallery presents images and video together with kind filters. Music projects
artists, albums and tracks from tags while retaining an honest “unknown” bucket.
Folder navigation and arbitrary filesystem operations remain Siderita's job.

### Artwork and preview

Static PNGs use the same freedesktop `large/<md5(file-uri)>.png` contract that
Siderita already reads: image thumbnail, representative video frame or embedded
audio cover. Writes are atomic and validity follows source identity/mtime.

A video trailer is requested only for the focused/hovered item or an open player,
has a short measured duration, a strict decode/memory budget and a cancellation
token. At most one interactive trailer per host may run. It is ephemeral or kept
in a bounded Fluorita cache; it is not published as a freedesktop thumbnail.

### Playback and host lifecycle

Confirmed play/pause/position/duration come only from engine reports. Requests
remain pending until confirmed. Sessions and derived-resource jobs carry
generations so late events cannot replace a newer selection. Workers and decoder
sessions are owned, bounded and deterministically closed.

In Siderita, the minimal modal shows the image or cover/video plus only the
controls the media supports: play/pause, seek, volume and close. It traps focus,
blocks folder actions and restores focus. The full app adds library navigation,
queue/now-playing context and richer chrome over the same engine state.

## Completed — F0: product and ownership contract

- [x] Ratify Fluorita as local library plus player with Gallery and Music.
- [x] Define `Space` as the embedded minimal player and double-click/`Enter` as
      standalone activation.
- [x] Separate static thumbnails/covers from bounded live video trailers.
- [x] Authorize `fluorita/`, `celestina-rs/crates/fluorita-*` and the bounded
      Siderita consumer surface.

**Exit:** scope, ownership and activation are canonical; no product source is
claimed as implemented.

## Completed — F1: shared media and library core

**Observable outcome.** A tested Rust API classifies local media, projects a
Gallery/Music catalogue, describes static artwork and live-preview requests and
models truthful playback without depending on Qt or a decode backend.

**In scope:** media identity/kind, configured sources, catalogue records and
projections, playback state, generation-stamped requests, freedesktop cache keys
and static/live preview descriptors.

**Out of scope:** filesystem scanning, database selection, decoding, Qt/QML,
desktop handlers and live installation.

- [x] `celestina-rs/crates/fluorita-core` — `celestina-core` plus a pinned `md5`
      justified inline; no Qt, runtime or decoder dependency.
- [x] `MediaId` (device+inode, or the raw path when nothing is stat'ed),
      `MediaKind` covering at least every extension Siderita thumbnails,
      non-nesting configured roots seeded from XDG Pictures/Videos/Music, and
      catalogue records whose reconciliation marks a file missing instead of
      deleting anything.
- [x] Deterministic Gallery (kind filters, two orders, unavailable items still
      visible) and Music (artist → album → track with an unknown bucket that
      sorts last) projections.
- [x] Playback whose confirmed state, position, duration and volume move only on
      generation-stamped engine reports, with pending transport/seek/volume
      exposed separately and per-kind capabilities.
- [x] Byte-safe `file_uri`, the freedesktop cache key/validity rule and the
      atomic publication descriptor, frozen against the Qt-measured golden
      vectors; the Qt-compatible codec landed as
      `celestina_core::percent::encode_qt_path` beside the Trash codec, whose
      preserved set genuinely differs.
- [x] Type-distinct `StaticArtworkRequest` and `TrailerRequest`: only the static
      one can name a publication, a trailer path never lands under
      `thumbnails/large/*.png`, and one host decodes at most one trailer, the
      previous one cancelled.

**Evidence (2026-07-30).** `bash scripts/check-architecture-contract.sh` from
the suite root: contraste, QML and architecture contracts OK. From
`celestina-rs/`: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings` clean, and `cargo test --workspace` green with the
new crate's 54 tests — including the Qt golden vectors, the non-UTF-8 Unix
extension, stale-generation rejection and trailer cancellation. Not proven:
anything that needs IO, a decoder or a session — no file was scanned, no artwork
written and nothing played.

## Now — F2: measured media engine

Probe the installed decode candidates against one narrow engine contract:
metadata extraction, image decode, audio/video playback, poster/cover extraction
and a cancelable short trailer. Choose the backend from measured closure size,
startup, PSS, CPU, hardware decode, frame pacing and Qt Quick render integration.
Only after author approval does the winning dependency enter
`fluorita-engine`; image work must not initialize the audio/video backend.

### The harness

[`spikes/`](spikes/) measures the stacks already installed on the machine and
adds no dependency to the workspace: an environment/integration probe, a package
closure comparison, bounded fixtures derived from the author's own media, a
decode sweep, a derived-resource pass and an opt-in presentation pass. Run it
with `python3 fluorita/spikes/run_all.py --source <clip> --cover <image>`; it
writes `report.md` plus raw JSON to its output directory.

Two measurement rules the harness enforces, because breaking either invents a
winner: hardware modes are named apart (`hw-copy` returns frames to RAM,
`hw-gpu` keeps them on the GPU), and sustained cost is expressed as core-seconds
per second of decoded content, since not every candidate honours 1× from a
command line.

### Measured on 2026-07-30

Machine: Radeon RX 9070 (radeonsi VA-API), niri/Wayland, Rust/Qt toolchain as
declared. mpv 0.41 · libmpv 2.5 · FFmpeg 8.1.2 · GStreamer 1.28.5 with the VA
plugin · Qt Multimedia 6.11.1. Fixtures: 20 s each of 1080p H.264, 1080p HEVC
and 2160p H.264 built from a real clip, plus a tagged MP3 with an embedded
cover. The source runs at **30 fps**, so nothing here stresses 60 Hz pacing.

**Installed closure.** Every candidate pulls FFmpeg into its closure, so the
separating number is what each adds *over* Qt Quick + FFmpeg: FFmpeg alone 0
packages, Qt Multimedia 4 (10.0 MiB), libmpv 14 (16.9 MiB), GStreamer 38
(101.4 MiB).

**Decode, 2160p H.264** (core-seconds per second of video · peak PSS): GStreamer
`hw-gpu` 0.0055 · 28.6 MiB; FFmpeg `hw-gpu` 0.0079 · 173.2 MiB; libmpv `hw-copy`
0.0355 · 212.7 MiB. Software costs ~0.24 for all three, which is expected — it
is the same libavcodec underneath. At 1080p the ordering holds (0.0032 / 0.0045
/ 0.0183 for H.264). First frame lands between 32 ms and 233 ms for every
candidate, so startup separates nobody. PSS excludes VRAM, which flatters every
GPU-resident path; and libmpv's headless figure is its floor, not its ceiling —
see the presentation pass.

**Derived resources** (1080p fixture): metadata 64 ms via ffprobe, mpv or
`gst-discoverer`; poster 114 ms with FFmpeg and 164 ms with mpv; embedded cover
32 ms with FFmpeg; a 5 s trailer 314 ms in software and 364 ms with VA-API
encode, both inside the budget `fluorita-core` froze (1280×720, 5.0 s, 492 KiB /
915 KiB). A trailer killed at 400 ms died in 416 ms and left 48 partial bytes —
the host must delete them, exactly as the preview contract requires. Poster and
cover extraction have no `gst-launch` one-liner: the library can do it, the
command line cannot, which is a code cost rather than a capability gap.

**Presentation, real niri/Wayland session** (15 s per clip, windows opened with
the author's authorization): both candidates played every fixture with **zero
dropped frames** at the clip's rate. libmpv ran `vaapi` on `gpu-next` — the
zero-copy path it cannot reach headless — at 0.025–0.040 cores and 171–249 MiB
PSS; GStreamer ran `vah264dec`/`vah265dec` into `glimagesink` at 0.016–0.019
cores and 36–41 MiB. mpv's own `estimated-vf-fps` disagreed with the container
on one clip (59 reported for 30 fps content), so the pacing verdict rests on the
drop counters, which are exact, and every row records whether its telemetry was
actually read — a zero with no telemetry would mean "did not observe", not
"perfect".

**Qt Quick integration.** libmpv ships its render API (`mpv/render_gl.h`).
Qt Multimedia has `VideoOutput` natively. Packaged GStreamer here has **no** Qt6
sink at all — no `qml6glsink` in `/usr/lib/gstreamer-1.0/` and no repository
package providing it — so its decode advantage would have to be paid for by
building that plugin or by going through `qt6-multimedia-gstreamer`.

### Decision — libmpv (author, 2026-07-30)

The author chose **libmpv** after the numbers above. What decided it was not raw
decode cost, where GStreamer wins (~2× less CPU, ~5× less PSS): it was that
libmpv is the only candidate whose Qt Quick render path exists on this machine
today, that it arrives as a complete engine — demux, decode, audio output, A/V
sync, seek, hardware — driven by commands and properties that map almost 1:1
onto the `PlaybackSession` this suite already froze, and that its closure is
small (14 packages · 16.9 MiB over Qt Quick + FFmpeg). GStreamer's advantage
would have to be bought with a Qt6 sink that no package here provides; FFmpeg
alone would mean writing and proving the clock, the sync and the seek model
ourselves; Qt Multimedia would hand its own state machine to a project that
already owns one.

The choice binds the engine, never the interface: mpv's on-screen controller and
key bindings stay off, every control is Fluorita's QML over `celestina-style`,
and the engine sits behind a narrow contract so replacing it later costs the
adapter and not the application.

### Built — `fluorita-engine` (2026-07-31)

`celestina-rs/crates/fluorita-engine` now turns that decision into code. It
depends on `libmpv2` (whose whole runtime closure is itself plus its `-sys`
crate, with pregenerated bindings, so no bindgen or clang enters the build) and
keeps everything behind `MediaEngine`/`EngineSession`, so hosts never name
libmpv.

- [x] **Baseline instances that draw nothing.** No user config, no scripts, no
      OSC, no key bindings, no playlist following. Fluorita owns every pixel of
      chrome, and reading `~/.config/mpv` would make one machine behave unlike
      another.
- [x] **Bounded probing** into `fluorita-core`'s own metadata: duration, tags,
      dimensions, seekability — with an untagged field staying absent instead of
      being invented, `3/12` read as track three, and an embedded cover
      reported as *not* a moving picture so a tagged MP3 never looks like video.
- [x] **Artwork publication** of video posters and embedded covers onto exactly
      the path `fluorita-core` computes: rendered into a staging directory
      inside the cache root so the last step is a rename, restricted to
      owner-only before it becomes visible, and leaving nothing temporary
      behind. An image request is refused by type — the toolkit already decodes
      those, and starting the media backend for one would be the waste the
      contract forbids.
- [x] **Playback sessions that never lie.** A request returns when the backend
      accepts it; `Playing` is only reported when the backend's own `pause`
      property moves, and a seek is only complete when it restarts playback at
      the new position. Every report carries the session's generation.
- [x] **A joinable worker** so no probe or extraction runs on a GUI thread, with
      cancellation of the job in flight and an explicit shutdown message. That
      last part is a fix, not a flourish: while shutdown relied on dropping the
      queue's sender, any surviving clone left the thread blocked in `recv` and
      `Drop` waited forever. Its own test caught it and now pins it.
- [x] **Bounded live trailers.** A short preview is encoded into Fluorita's own
      cache under its own extension — never the freedesktop `large/<key>.png`
      another application scans — inside a 16:9 box computed from the budget's
      pixel cap. The encode goes to a staging file that is only renamed after
      the result is **decoded back** and checked against the budget it was made
      for; an over-budget or truncated encode is discarded rather than cached,
      and `prune_cache` keeps the directory bounded by removing the oldest
      entries and nothing that is not a trailer.
- [x] **Byte-safe sources.** A non-UTF-8 filename travels as `fd://` rather than
      through a lossy conversion that would open a *different* file — the worst
      failure a media library can have.

**Evidence.** `bash scripts/check-architecture-contract.sh` OK; from
`celestina-rs/`: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings` clean, `cargo test --workspace` green — including
fourteen integration tests that drive **real libmpv** over two tiny synthetic
fixtures (`tests/fixtures/`, built from `lavfi` sources so nothing personal is
committed): probing, tag parsing, cover-versus-video, poster and cover landing
byte-for-byte on the core's path with mode `0600`, confirmed-only state, seek
completion, a non-UTF-8 name, trailer production inside its budget, an
over-budget encode being discarded, and the worker.

### Built — guarded application scaffold (2026-07-31)

`fluorita/` is now a real, buildable Qt Quick application, and it landed
**together with its guard coverage** because the suite's guards enumerate their
targets by name: QML outside those lists is QML nobody inspects.

- [x] Both guards learned about Fluorita in the same change: the five
      enumerations in
      [`scripts/check-architecture-contract.sh`](../scripts/check-architecture-contract.sh)
      (the QML-registration loop, the shared-style-link scanner and its loop,
      the auto-binding inputs, the local-control inputs and the
      dependency-direction alternation) and the three in
      [`celestina-style/scripts/check-style-contract.sh`](../celestina-style/scripts/check-style-contract.sh).
      The dependency-direction pattern also gained `grafita`, which was missing:
      the style could have imported an application module undetected.
- [x] **A test that no future list can be forgotten.**
      [`scripts/test-architecture-scanners.sh`](../scripts/test-architecture-scanners.sh)
      now discovers every project with a `qml/` tree and asserts it appears in
      **every** scanner input line of both guards — per line, not per file,
      because being in three lists of four leaves one scanner blind. Proven by
      removing Fluorita from a single list and from a single loop: both fail
      with a diagnostic naming the omission.
- [x] A minimal window over `celestina-style`: relative symlinks for the shared
      components, `CelestinaTheme`/`CelestinaIcons` registered as singletons,
      one `QML_FILES` list feeding both registration and `rerun-if-changed`,
      the `org.celestina.Fluorita` desktop/Wayland identity, the `Basic` Quick
      Controls style and `CELESTINA_REDUCED_MOTION` injected at startup.
- [x] Activation that keeps a filename's bytes: `args_os` into a raw `PathBuf`,
      a `file://` argument decoded with `celestina_core::percent`, a remote
      authority refused rather than read as local, and a lossy label used only
      for display. Its five tests include a non-UTF-8 name.
- [x] The window says what it was handed and what it cannot do. Classification
      comes from `fluorita-core` by name alone, so **no decoder starts**; there
      is no transport and no picture, because a control that does nothing is a
      lie.
- [x] `scripts/smoke.sh` asserts all of it offscreen — and asserts that libmpv
      is *not* mapped into the process, which is the lazy-engine contract
      turned into a gate rather than a promise.

**Evidence.** `bash scripts/check-architecture-contract.sh` OK (contraste, QML,
arquitectura); `bash scripts/test-architecture-scanners.sh` OK plus the two
negative probes above; from `fluorita/`: `cargo fmt --check`, `cargo clippy
--all-targets -- -D warnings` clean, `cargo test` green, `cargo build --release
--locked`, `qmllint` clean over the registered QML, and `scripts/smoke.sh` OK.
Not proven: anything needing a real Wayland session — appearance, keyboard,
focus and accessibility.

### Built — the Qt Quick render path (2026-07-31)

The app now hosts a real player: `MpvVideo` is a `QQuickFramebufferObject` that
drives libmpv's render API on Qt's render thread, and `FluoritaPlayer` is the
Rust QObject that owns the session off the GUI thread.

- [x] **Hand-written C++ where CXX-Qt cannot reach** — [`cpp/mpvvideoitem.cpp`](cpp/mpvvideoitem.cpp)
      subclasses `QQuickFramebufferObject` and overrides `createRenderer()`,
      neither of which CXX-Qt 0.9 can express, and it is the only place that
      touches the render API. It pins the scene graph to OpenGL, which that API
      requires, and registers in its own QML namespace because Qt 6 refuses
      `qmlRegisterType` into a namespace a generated module already owns.
- [x] **One opaque seam.** `fluorita-engine` grew `VideoOutput::Embedded`
      (`vo=libmpv`) and `EngineSession::render_handle()`, an *address* rather
      than a pointer: nothing in Rust can dereference it, and a closed session
      reports none at all.
- [x] **The surface releases before the backend does.** Closing clears the
      handle, the renderer frees its render context on the render thread and
      reports back, and only then may the session be dropped — the one
      use-after-free this seam can cause.
- [x] **A transport that never lies**: play/pause, a local seek bar that draws
      the confirmed playhead and a *pending* seek separately, keyboard seeking,
      a focus ring keyed to keyboard focus, and `Accessible` roles throughout.
      No shared slider exists yet, so it is local until a second app needs one.
- [x] **Two integration facts, found by running it**, both now permanent:
      libmpv refuses to start under a locale whose decimal separator is not a
      dot (the app sets `LC_NUMERIC=C`), and mpv ships *built-in* Lua overlays —
      a console, a stats overlay, a track selector, a context menu — that draw
      over the app and cost a thread each. `load-scripts=no` does not cover
      them; the engine now disables each by name.

**Evidence.** Architecture guard and scanner suite OK; from `fluorita/`:
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test`, `cargo build --release --locked`, `qmllint` clean, and
`scripts/smoke.sh` — which was rewritten because it had been passing on
nothing: it measured the wrong PID and never noticed a QML that failed to load
at all. It now proves the QML loads, that a video opens a session on a worker
thread, and that a non-media file starts no backend, with a negative probe
showing it fails when the QML breaks. Clippy also caught a `Default` that
recursed into itself before it could ever run.

**Verified in the author's Wayland session (2026-07-31, authorized).** A 1080p60
clip renders: two captures of the app's own window, five seconds apart, show
different decoded frames and the clock moving `0:05 → 0:10` of `0:20` with the
seek bar following. Audio plays too (`0:06 / 1:00`, state `reproduciendo`), and
closing the window mid-playback exits in about a second with no crash and no
orphan process. Qt reports `QRhi with backend OpenGL`, which is what the render
API requires.

That session found two defects the automated gates could not, both now fixed:

- **`ao=auto` is not an audio driver.** mpv autoprobes when the option is left
  alone; naming a driver that does not exist left the session with no audio, so
  a cover-art-only file reached end of file instantly and `keep-open` parked it
  paused at zero. It looked exactly like a file that would not play.
- **Loading before the surface exists ends in `NOTHING_TO_PLAY`.** With
  `vo=libmpv` there is no video output until the host's render context is
  created, so `open` and `start` are now two steps in the engine contract: the
  host opens, publishes the handle, waits for its surface to report a context,
  and only then loads. Audio, which has no surface to wait for, starts
  immediately.

### Built — the image path (2026-07-31)

A still never touches the media backend. That was the promise from F0 onward,
and it is now the code as well as the claim.

- [x] **The toolkit decodes it.** `MediaKind::Image` short-circuits before any
      session exists: no engine, no handle, no decoder thread. The picture is a
      QML `Image` with `autoTransform` (so a camera's orientation is honoured),
      `asynchronous` (so the window never freezes on a large file) and a
      `sourceSize` cap, which is what makes the reader do a scaled read instead
      of allocating the full surface first.
- [x] **Bounded before it is decoded.** [`src/image.rs`](src/image.rs) judges
      every still against two budgets — 256 MiB on disk, 100 megapixels decoded
      — from the file size and a header-only probe
      ([`cpp/imageprobe.cpp`](cpp/imageprobe.cpp), a `QImageReader` that never
      decodes, because cxx-qt-lib exposes none). A refusal names *which* budget
      it broke: "no se pudo abrir" for a file that is merely enormous is a lie.
- [x] **No transport for a still.** The player publishes whether time means
      anything for the item, and the transport binds to that instead of
      guessing from "has a picture".

**Verified in the author's session.** A 6300 × 3600 photograph displays with its
aspect kept and no transport, and the process has **no `core` and no
`fluorita-player` thread** — the lazy-engine promise, observed rather than
asserted. A hand-made 2.5 KB PNG declaring 40 000 × 40 000 (1600 megapixels) is
refused with "La imagen tiene 1600 megapíxeles y el límite son 100" while
resident memory stays at 216 MiB; decoding it would have needed about 6 GiB. The
smoke gate now covers the same ground headlessly.

**Still unproven:** frame pacing under load, seeking while playing, and keyboard
and focus with a real assistive stack.

### Built — the library (2026-07-31)

Gallery and Music are no longer a contract: they are the window a bare launch
opens.

- [x] **Walking the roots lives in the engine.**
      [`library.rs`](../celestina-rs/crates/fluorita-engine/src/library.rs) reads
      names and `stat` and stops — no file is opened and nothing is decoded, so
      a library of ten thousand photographs costs ten thousand `stat` calls.
      Four bounds keep it from becoming an incident (file ceiling, depth,
      deadline, cancellation), symlinks are not followed so one `ln -s` cannot
      walk the scan in a circle, and dotfiles are not library items. A
      truncated pass **says so**, because a caller that believed it would then
      mark every unvisited file as missing.
- [x] **Roots seeded from the XDG directories that exist**, with Spanish and
      English names both accepted, and a missing one simply not configured.
- [x] **The GUI thread never walks a directory:** the scan is a `Job::Scan` on
      the engine's worker, and the result arrives through the queue.
- [x] **Browsing starts no decoder.** Thumbnails come from the shared
      freedesktop cache *if something already produced them*; a missing one is a
      themed glyph, never a generated one, because generating during browsing
      would start a decoder per card.
- [x] **Two surfaces over the core's own projections:** a Gallery grid of images
      and video, and a Music list already in artist → album → track order, with
      the honest "Sin artista" bucket. Both are keyboard-operable and carry
      accessible roles and names.
- [x] **One way to start playing.** Activating an item from the library goes
      through the same door as a path on the command line, and `Escape` returns
      to the grid.

**Two defects this milestone caught in its own code**, both from gates rather
than from luck: the shared guard found an `x: x` auto-binding in `Main.qml`
(`library: library`), the exact shadowing bug it exists for; and the offscreen
smoke found that publishing four index-aligned lists one by one makes QML
rebuild its rows *between* them, half from the previous scan. The rows are now
woven once, when a single `revision` property says every column is in place.

**Verified in the author's session.** A bare launch scans the real library —
**94 items (86 images, 8 videos) in 251 µs** — and shows it with cached
thumbnails where they exist and the video glyph where no poster was ever made.
Activating an image opens it with **no `core` and no `fluorita-player` thread**;
`Escape` returns; activating a 4K video starts playback with the transport, and
only then do `core`, `demux`, `av:h264:df0-3` and `vo` appear. The same process
went from zero decoder threads to a playing video and back — the lazy-engine
promise, observed. The smoke gate now asserts the browsing half headlessly.

**Decided by measurement, not preference.** Rows travel to QML as parallel
`QStringList`s rather than a native model: CXX-Qt 0.9 cannot override
`QAbstractListModel`'s virtuals from Rust, so a native model means a second
hand-written C++ model beside Siderita's, and at 94 items found in 251 µs there
is nothing yet to justify one. Re-measure at a few thousand items.

### Built — the persisted catalogue (2026-07-31)

- [x] **Why it exists, measured first.** Not to save the walk — that is 251 µs —
      but to keep what is expensive: tag extraction. The library now reads tags
      once, bounded to 500 files per launch, and stores them.
- [x] **A file, not a database, and the threshold is written down.** The shape
      was measured before it was chosen: flat records, no relations, no queries,
      keyed by device+inode. So the store is one versioned tab-separated file
      rewritten through the suite's existing atomic replacement — no dependency,
      no schema, and the previous file intact until the new one is durable.
      Every field is percent-encoded with the canonical codec, so a non-UTF-8
      filename survives byte for byte and no tag can smuggle a tab or a newline
      into the next record. A real database earns its place when a library is
      big enough that rewriting is felt, or when something must update one
      record without loading the rest; it costs one file to change, behind
      `load`/`save`.
- [x] **The window opens on what it already knew.** The stored catalogue is
      published before the walk starts; the walk then refreshes it.
- [x] **Absorbing a scan is domain truth, so it lives in the core.**
      `Catalogue::absorb` keeps the metadata of a file whose identity *and*
      size/mtime are unchanged, drops it for one edited underneath, follows a
      rename without re-reading, restores an item that came back, and only lets
      a *complete* pass conclude that something disappeared.
- [x] Corrupt lines are skipped and counted, an unknown version is treated as
      "no catalogue" rather than guessed at, and a first run is not an error.

**The defect this milestone caught, and how.** Unit tests round-tripped the
catalogue happily — but a measured launch showed the first run and the second
costing the same CPU. The stored mtime was whole seconds while the filesystem
reports nanoseconds, so every record looked changed on reload and every tag was
read again, silently undoing the only thing the file is for. Timestamps now
carry their sub-second part (a bare second count still loads), and a test pins
the real property: store → load → absorb keeps the tags.

**Verified.** With 201 tagged tracks in a throwaway home: **0.55 s of CPU on the
first launch, 0.07 s on every launch after** — the same library, eight times
cheaper, which is the whole point. On the author's real library the catalogue is
94 records in 10 KB, rewritten with no temporary left behind, and the window
still opens with no decoder thread.

### Built — producing the missing artwork (2026-07-31)

The engine could already extract a poster or a cover; what was missing was a way
to ask for the ones a library lacks, without breaking the rule that browsing
costs nothing.

- [x] **It is a decision, not a side effect.** Producing a thumbnail *is*
      starting the backend, so it happens only when asked: a button in the
      library header, `Ctrl+G` to start and `Ctrl+Shift+G` to cancel. A launch
      still opens with no decoder thread, and the smoke gate still proves it.
- [x] **Deciding what is missing costs `stat`, not decodes.**
      `fluorita_engine::pending_artwork` walks the catalogue, asks the shared
      cache whether an entry exists and whether it is at least as new as its
      source — the core's frozen validity rule — and returns at most a bounded
      batch. Images are excluded by construction: the toolkit already makes
      those.
- [x] **Bounded and cancellable:** 200 items per pass, 30 s per extraction, a
      cancellation token checked between items and honoured on window close, so
      a large library finishes across several passes instead of holding the
      backend open. A file that will not give up a frame is a normal outcome —
      the card keeps its glyph and the pass moves on.
- [x] Progress is what was actually produced, and the control disappears when
      there is nothing left to do.

**Verified in the author's session.** The three 4K wallpapers and five other
clips showed the themed video glyph; one `Ctrl+G` and the shared cache went from
**330 to 338 entries**, every one `-rw-------` and keyed exactly where
`fluorita-core` computes it — independently recomputed from the file URI to be
sure. The grid refreshed to real posters and the button vanished. Because it is
the *shared* cache, Siderita gets those posters too.

**A defect this found in the library's own interface:** the header actions were
not in the tab chain, so nothing but the mouse could reach them. `Galería`,
`Música`, the generate button and both views now set `activeFocusOnTab`, and the
shortcut exists so the action never depends on focus order at all. The chain
still needs a real assistive-technology pass — pressing Tab did not visibly move
the focus ring in this session, which is exactly the kind of thing the roadmap
already lists as unproven and a screen reader would settle.

### Still open


- [ ] **A real-session pass**: picture, frame pacing, seeking under load,
      keyboard and focus, and closing while playing.
- [ ] The **desktop entry, the icon and installation**. No `.desktop` and no
      `org.celestina.Fluorita.svg` exist yet, and installing a handler for an
      app that plays nothing would be worse than not having one. Installation
      and MIME wiring still need explicit author authorization.
- [ ] Not measured, and worth knowing before the engine is called done: 60 fps
      pacing, tearing, long sessions, aggressive seeking and malformed files.
      Cancelling an encode *mid-flight* is only proven by the cleanup path a
      failed encode shares: the two-second fixture finishes too fast to lose
      the race reliably.

## Next — F3/F4: both player hosts

The first QML that lands must land already guarded: both architecture guards and
the style guard enumerate their target projects by name, and `fluorita` is in
none of those lists, so the first scaffold, every guard-list addition and the
negative fixtures belong to one change rather than a later cleanup.

Build the complete application with Gallery, Music and Now Playing over the
shared core/engine. Add configured roots, incremental catalogue reconciliation,
static artwork generation, MPRIS2, direct one-file activation and isolated
desktop/installer tests. Database choice is made from the measured catalogue
shape in this milestone, not preselected in F1.

Once standalone playback proves the engine contract, add the second host: route
`Space` on image/video/audio to a nearly full available-area minimal player and
route double-click/`Enter` to standalone Fluorita. Siderita continues reading
static cache entries while browsing and loads the engine only for an explicit
player/trailer request. Validate one-session-only playback, cancellation on
selection change, keyboard/focus, reduced motion and real Wayland rendering.

## Later

- On-demand trailer hover/focus in the standalone Gallery after resource budgets
  prove that it does not start a decoder per card.
- Subtitles, audio-track selection and playback speed.
- Queues and playlists derived from the local library.
- A compact shell now-playing surface over MPRIS2, not an always-live decoder in
  the panel.

## Non-goals

- No streaming or network catalogue before the local library is complete.
- No general file browser, arbitrary file operations or source deletion.
- No tag editor, codec reimplementation or automatic whole-filesystem crawl.
- No shared app-specific QML between Fluorita and Siderita; shared visual
  primitives remain in `celestina-style`, shared media truth in the Rust crates.
