# Fluorita roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the media app
> only. Checklist legend: `[x]` done · `[ ]` planned. "Implemented" is not
> "verified": decode correctness, frame pacing and cost must be proven on real
> files in a real Wayland session, tracked as its own goal. The build gate has
> been opened by the author, but **no code exists yet**: this roadmap now doubles
> as the start plan, written so a cold session can resume it without rediscovery.

## Overview

**Purpose.** Open and play whatever media the session hands it — a song, a clip,
an image — and produce the thumbnails the rest of the suite consumes. A
player/viewer, not a library: no recursive or persistent collection scan, no tag
database, no "recently played" store. Siderita browses; Fluorita plays what it is
given.

**What it replaces, and what it doesn't.** It replaces whatever plays media on
this session today (mpv/imv or a desktop's stock viewer) — *only* once that tool
proves a recurring daily gap, per suite discipline. It does not replace a codec:
Fluorita is a shell around a decode backend, not a reimplementation of one, and
it will never grow a media library, a tag editor or a streaming client.

**Why it exists at all.** Two suite contracts already point at it. Siderita
consumes video first-frames and audio covers from the shared freedesktop
thumbnail cache and generates none; and Siderita's quick-look hands video and
audio to an info card that names Fluorita. Both are deliberate: the media weight
is *located*, not avoided. PDF is outside Fluorita's current contract.

**Shape.** A windowed app, and later an **embeddable widget** — the same player
component hosted by its own window, by the `celestina` panel (a playing clip or
now-playing music, live) and by Siderita's quick-look. Playback is
published over MPRIS2 so the shell, the media keys and Magnetita all read one
source of truth.

**Key decisions.**
- **The decode backend is the one deliberately expensive dependency, and it is
  an open decision to be settled with numbers at CP0.** The suite's non-goals
  name Qt Multimedia specifically as a framework not to add without a measured
  need, which rules it out as a default rather than a choice. The leading
  candidate is **libmpv** — one library, hardware decode, Wayland-native
  rendering, an embeddable render API and a small integration surface — with
  GStreamer (heavier, more moving parts) and raw FFmpeg (more code we own, more
  correctness we must prove) as the alternatives. Whichever wins does so on a
  measured closure size and a measured decode cost, and it lives behind one
  narrow trait in `fluorita-decode` so the decision stays reversible.
- **Images do not go through the video path.** Still images are decoded with
  what Qt already has (plus the EXIF-aware, scaled-read approach Siderita's
  thumbnailer already uses), so viewing a photo costs nothing from the media
  stack. The heavy backend is loaded lazily, only for what needs it.
- **Thumbnail production is a contract, not a feature.** The cache path, the
  `md5(file-uri)` key, the 256 px "large" size and the mtime validity rule are
  fixed by what Siderita already reads. Siderita must not change by one line for
  its video frames to start appearing.
- **Truthful state.** Position, duration and playing/paused are what the backend
  reports, never what was requested — a click is a request. A file that cannot
  be decoded says so plainly instead of showing a stalled transport.
- **Never the browser.** Fluorita opens what it is handed. If a "play this
  folder" need appears, it is a queue built from that hand-off, not a library.

## Status — where to resume (2026-07-30)

The author opened the build gate on **2026-07-30** and asked for the core and the
app to be started. Nothing has been written yet: `fluorita/` still holds only
this roadmap and its README, and `celestina-rs` has no `fluorita-*` crate.

The work is broken into four steps, **F1 → F4**, detailed under Checkpoint 0.
Start at F1. The planning pass on 2026-07-30 settled every choice needed to
reach the F4 evidence gate; backend selection itself remains measurement-driven.
One thing is worth knowing before opening any file:

- **F2 is atomic guard onboarding + scaffold.** The suite's guards enumerate
  projects by name, but some also require the named `qml/` tree and `build.rs` to
  exist. Therefore the first minimal scaffold, every guard-list addition and
  persistent negative fixtures land in one change: no Fluorita QML exists
  outside guard coverage, and no intermediate commit points a guard at a missing
  project shape.

## Start decisions settled (2026-07-30)

1. **The product name is Fluorita.** F2/F3 may use
   `org.celestina.Fluorita`, `org.celestina.fluorita` and the matching desktop
   and icon names. Renaming is reopened only by an explicit author decision.
2. **MD5 is a small interoperability dependency, not code we maintain.** Add
   `md5 = "=0.8.1"` with an inline `Cargo.toml` justification: the freedesktop
   cache mandates MD5, it is not used for security, and this release has no
   transitive dependencies. Do not vendor a second hash implementation and do
   not pull the larger RustCrypto digest abstraction for one fixed key.
3. **The Qt-compatible path codec extends the existing canonical module.** Keep
   `celestina_core::percent::encode` byte-for-byte compatible and add a separately
   named `encode_qt_path` for RFC 3986 `pchar` plus `/`, uppercase escapes. F1's
   `file_uri` prepends `file://`; golden vectors prove the whole pipeline.
4. **The CXX-Qt bridge stays in the app through CP1.** Do not create
   `fluorita-qt` at F3. Reconsider a separate view-contract crate only at CP2,
   when the embeddable widget creates a second real host.
5. **The icon cannot stall CP0.** A dedicated
   `org.celestina.Fluorita.svg` remains a repository deliverable before polished
   installation, but build/run scripts warn and use a generic icon until it
   exists. Installation and MIME-handler changes still require explicit author
   authorization.

## Frozen contracts (the thumbnail key)

This is the half of Fluorita that another project already depends on, so it was
pinned down first — by measurement, not by reading the spec. **Siderita is the
authority**, because it is the consumer that must not change: the derivation
lives in [`thumbnailprovider.cpp`](../siderita/cpp/thumbnailprovider.cpp) and is
`md5(QUrl::fromLocalFile(absolutePath).toEncoded())`, hex, lowercase, written to
`$XDG_CACHE_HOME/thumbnails/large/<key>.png` at 256 px, temp-file + rename, with
`Thumb::URI`/`Thumb::MTime` text keys and owner-only permissions.

**The encoding rule, recovered from Qt 6 itself** (a throwaway `Qt6Core` probe
calling `QUrl::fromLocalFile().toEncoded()`, not from documentation): every byte
is percent-escaped with **uppercase** hex except

```
ALPHA DIGIT  -._~  !$&'()*+,;=  :@  /
```

which is RFC 3986's `pchar` plus `/`. The MD5 digest is then rendered as
**lowercase** hex. Golden vectors, to be frozen as `fluorita-core` tests:

```
/home/toni/clip.mp4	file:///home/toni/clip.mp4	053a0fcc87f42f4b9e33ebc076783935
/home/toni/a b.mp4	file:///home/toni/a%20b.mp4	2275fc454ce0dc91ae3cfe0fe70eebb0
/home/toni/Vídeos/canción ñ.mp3	file:///home/toni/V%C3%ADdeos/canci%C3%B3n%20%C3%B1.mp3	70e33e372a4c9c1f732b967ed9df9df2
/home/toni/emoji 🎬.mkv	file:///home/toni/emoji%20%F0%9F%8E%AC.mkv	cd28796eed0cf805feb69ccd90f44154
/home/toni/weird#hash?q.png	file:///home/toni/weird%23hash%3Fq.png	40cd8e865ec08f622a53eac35cc64ab7
/home/toni/quote'and"dq.jpg	file:///home/toni/quote'and%22dq.jpg	196259b0dd86fb879c3413eae4843ca5
/home/toni/paren(1)[2]{3}.webm	file:///home/toni/paren(1)%5B2%5D%7B3%7D.webm	db6e38d9f643b4cc7e62bd380294ee40
/home/toni/plus+amp&eq=semi;.flac	file:///home/toni/plus+amp&eq=semi;.flac	6708bf323aa2562b306e70f3ce85a20c
/home/toni/percent%20literal.avi	file:///home/toni/percent%2520literal.avi	d089d4131f54cb8a8e9624865b79052c
/home/toni/tilde~dash-under_dot..ogg	file:///home/toni/tilde~dash-under_dot..ogg	58e7b3568a80fc5afc683371f4f5657d
/home/toni/at@colon:comma,.opus	file:///home/toni/at@colon:comma,.opus	3e1260f33bf697b60462df77fcc4912a
/home/toni/star*bang!dollar$.wav	file:///home/toni/star*bang!dollar$.wav	c27427c121c671385717636ecbd22cf3
/home/toni/back\slash.mp4	file:///home/toni/back%5Cslash.mp4	2b7c730c72cba7ae2d4f0cd19908d2d4
/home/toni/pipe|caret^tick`.mov	file:///home/toni/pipe%7Ccaret%5Etick%60.mov	7fa069aa9fd9701a792495f9134953a3
/home/toni/less<greater>.m4a	file:///home/toni/less%3Cgreater%3E.m4a	a6eac6b1d52cc409d44e60afa31fc1bf
```

Those vectors prove compatibility with Qt for valid UTF-8 paths. On Unix, the
codec additionally accepts raw `OsStr` bytes and percent-escapes every non-ASCII
byte without a lossy Unicode conversion. That byte-safe extension has its own
golden vector (the path column is Rust byte-string notation, not text):

```
b"/home/toni/bad-\xFF.mp4"	file:///home/toni/bad-%FF.mp4	ff7e7879531a24532843de4e2ef3ead9
```

Do not claim that last vector came from Qt: it is Fluorita's explicit Unix
extension so a malformed filename remains addressable and deterministic.

Two consequences fall out of that rule, both load-bearing:

- **`celestina-core::percent::encode` cannot be reused as-is.** Its preserved set
  is `alnum` + `-_.~/`, so it escapes `!$&'()*+,;=:@` — bytes Qt leaves raw. It
  is correct for what it was written for (the Trash spec and the app's own URI
  glue) and wrong here; the two codecs are genuinely different sets, not a
  duplicated recipe. Settled start decision 3 names where the second set lives.
- **Qt and GLib disagree, and the disagreement is pre-existing.** GLib's
  `g_filename_to_uri` escapes `;` as `%3B` where Qt keeps it raw (checked
  directly against GLib); every other character in the sample above agrees.
  Files whose names contain `;` therefore land on two different keys, and their
  thumbnails are not shared between Celestina and GTK apps. **Fluorita follows
  Qt**, because Siderita is the consumer whose contract must hold. Record this
  as a known interop limit; do not "fix" it by diverging from Siderita.

## Checkpoint 0 — Play one file, and prove the backend
**Goal:** a window opens a file passed on argv or by `xdg-open`, decodes and
renders it in a real Wayland session, and the backend decision is settled by
measurement rather than preference.

- [ ] `fluorita-core` — MIME → capability mapping, transport state model,
      thumbnail cache keys and validity, all pure and unit-tested without I/O
- [ ] `fluorita-decode` — two bounded backend probes first; only after both have
      exposed control/events and their Qt Quick render sink is the narrow common
      contract extracted and the author has approved the winning heavy
      dependency, with the losing backend retained as a minimal test stub
- [ ] Qt/QML host — a single surface over `celestina-style`: video/image output,
      transport, position, volume, and a truthful error state
- [ ] Still images decoded without loading the media backend at all
- [ ] **Measured** — installed closure delta, memory, decode CPU and dropped
      frames for a 1080p clip, each inside a declared budget; the backend
      decision recorded with the numbers that settled it
- [ ] **Verified** — plays real audio, video and images on the author's Wayland
      session, launched both from argv and from Siderita via `xdg-open`

**Done when:** pressing Enter on a media file in Siderita opens it here and it
plays correctly — and the cost of the decode stack is a number, not a hope.

### F1 — `fluorita-core`, the pure domain

A new crate at `../celestina-rs/crates/fluorita-core`, registered in the
workspace `members` list. It inherits `edition`/`rust-version`/`license` from
the workspace and carries `[lints] workspace = true`, so `forbid(unsafe_code)`
and the shared Clippy level apply. Its dependencies are
`celestina-core.workspace = true` and the exact `md5` release justified above.
No Qt, no I/O, no filesystem calls: every function takes what it needs as an
argument and returns a decision.

- [ ] `capability.rs` — extension → MIME → capability. Two lookups rather than
      one, so the app can later feed a real system MIME string without the core
      changing: `mime_for_extension`, `capability_for_mime`, and a
      `classify_path` convenience over both. A capability says which of the two
      decode paths a file needs — the toolkit's own image decode, or the media
      backend — and whether Fluorita is the thumbnail producer for it (video and
      audio: the kinds Siderita deliberately cannot do). The image extension
      list must cover at least the set Siderita already generates for, in
      `thumbnailprovider.cpp`, so the two agree on what "an image" is.
- [ ] `thumbnail.rs` — the frozen contract above: `file_uri` (absolute paths
      only; return `None` for a relative one rather than emit a wrong key),
      `cache_key`, the `large/<key>.png` path from a cache root, the 256 px
      constant, and validity as a pure function over two timestamps
      (`cache_mtime >= source_mtime`, missing cache → regenerate). Operate on
      raw path bytes so a non-UTF-8 name does not panic or get mangled.
- [ ] `transport.rs` — the truthful playback model. Confirmed state
      (`Idle`/`Opening`/`Playing`/`Paused`/`Ended`/`Failed`) moves **only** on a
      backend report; a user action is recorded as a *pending request* beside
      it, never as state. Position and duration are last-reported values, and a
      pending seek is exposed as its own thing so the UI can show "seeking"
      instead of lying about where the playhead is. Reuse
      `celestina_core::Generation` to stamp requests, so a late report cannot
      overwrite a newer one — the same staleness discipline the rest of the
      suite already uses.
- [ ] MD5 wrapper around the dependency, tested against RFC 1321 vectors **and**
      the golden vectors above, which prove the whole pipeline
      (encode → digest → hex) matches Qt byte for byte for valid UTF-8 and keeps
      the separately documented byte-safe Unix extension stable.
- [ ] Same-change documentation: the crate table in
      [`../celestina-rs/README.md`](../celestina-rs/README.md), the crate count
      and dependency claims in its overview, and a line in
      [`../celestina-rs/ROADMAP.md`](../celestina-rs/ROADMAP.md). Both currently
      state that the workspace has eight crates and that the pure cores carry no
      third-party dependencies; both claims change when F1 lands.

**Evidence:** `bash scripts/check-architecture-contract.sh` from the suite root
first, then `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test --workspace`, all from
`celestina-rs/`. The golden-vector test is the one that matters: it is the proof
that Siderita will not need a line of change.

### F2 — guarded application scaffold

Create the smallest buildable Magnetita-shaped scaffold and teach the guards
about it in the **same change**. Both guard scripts enumerate targets by name,
while their scanners require the named app shape to exist; neither half lands
alone.

- [ ] [`../scripts/check-architecture-contract.sh`](../scripts/check-architecture-contract.sh),
      five places: the `for app in siderita magnetita` loop in
      `check_qml_registration`; the same loop plus the `shared-style-links`
      scanner inputs in `check_shared_style_links`; the `qml-auto-bindings`
      inputs in `check_top_level_auto_bindings`; the `local-controls` inputs in
      `check_local_control_ratchet`; and the `(siderita|magnetita)` alternation
      in `check_dependency_direction`, which is what stops `celestina-style`
      from importing an application module.
- [ ] [`../celestina-style/scripts/check-style-contract.sh`](../celestina-style/scripts/check-style-contract.sh),
      three places: the `find` that builds the QML manifest, and the input lists
      of the `qml-style-contract` and `style-copies` scanners.
- [ ] No baseline entries. `scripts/architecture-baseline.tsv` freezes existing
      debt; new code has none, and adding a line there to make F3 pass would be
      exactly the move the contract forbids.
- [ ] `Cargo.toml` depends on `fluorita-core` and pins the same bridge stack as
      the existing apps: `cxx = "=1.0.176"`, `cxx-qt = "=0.9.1"`,
      `cxx-qt-lib = "=0.9.1"` with `qt_gui`/`qt_qml`/`qt_quickcontrols`, plus
      build dependencies `cxx-qt-build = "=0.9.1"` and
      `cxx-gen = "=0.7.176"`; its release profile mirrors those apps. Generate
      and keep `Cargo.lock` in this slice so every `--locked` check is real.
- [ ] Complete minimal `build.rs` — one `QML_FILES` constant, the
      `org.celestina.fluorita` module, `CelestinaTheme` and `CelestinaIcons`
      registered as singletons outside that list, `icons.qrc` and `fonts.qrc`
      compiled in, and an explicit `rerun-if-changed` for every input.
- [ ] Minimal `src/main.rs`, `qml/Main.qml`, relative canonical style symlinks
      and `fluorita/AGENTS.md`. `main.rs` sets
      `APP_ID = "org.celestina.Fluorita"` as desktop/Wayland identity, selects
      the `Basic` Quick Controls style, reads `CELESTINA_REDUCED_MOTION` once and
      injects it as an initial property. It reads the path with
      `std::env::args_os` into `OsString`/`PathBuf`; only a separate display label
      may be lossy. The window identifies the requested path and says rendering
      is not implemented; it does not simulate image or transport state.
- [ ] Same-change documentation: this roadmap's status, `fluorita/README.md`
      and the suite [`README.md`](../README.md) / [`ROADMAP.md`](../ROADMAP.md)
      move from "no code" to a guarded scaffold. Authorization remains an event
      recorded here; do not copy dated project status into the root `AGENTS.md`.

**Evidence:** run `bash scripts/check-architecture-contract.sh` first, then
`bash scripts/test-architecture-scanners.sh` — and, per the suite's
evidence matrix for guard changes, **persistent negative fixtures** under
`scripts/fixtures` must prove a Fluorita raw colour, missing registration and
copied style file fail for the intended reason. Never place a temporary
violation in the product tree. A guard that passes because it inspects nothing
is worse than no guard. The scanner suite also has a positive integration
assertion that the complete guards' discovered input manifests contain
`fluorita/qml`, so a forgotten hard-coded list fails even when the app itself is
clean. Then run fmt, Clippy with `-D warnings`, locked tests, a locked release
build, `qmllint` over every registered QML file and an offscreen start with no
`TypeError`/`ReferenceError`. A Unix integration test launches the scaffold with
a non-UTF-8 filename and proves the raw `OsString` reaches the core unchanged.

### F3 — the real image slice, honest about what it cannot do yet

Extend F2's already buildable thin client; this slice adds only the real image
path and its packaging delta.

- [ ] A small bridge object in `src/` that projects `fluorita-core` decisions
      into QML: the opened path, its display name, its classified kind, a view
      state, an opaque image-generation token and a human error string.
      Classification/stat and opening the raw `PathBuf` run on an owned,
      deterministically joined Rust worker; Qt applies only the current
      generation. No decoder ever reconstructs a path from the lossy display
      label.
- [ ] A local `QQuickAsyncImageProvider` consumes `image://fluorita/<generation>`.
      Rust opens the raw path and passes a bounded immutable encoded-byte buffer
      through the safe bridge; the provider decodes it off the GUI thread with
      `QImageReader` over `QBuffer`, enables auto-transform and scales to the
      capped requested size before allocation. Keep at most current + previous
      generations, cap compressed input at 128 MiB and decoded dimensions at
      100 MP, and cancel stale responses. The hand-written C++ registration hook
      carries the required comment: CXX-Qt 0.9 exposes no image-provider
      registration hook.
      Compare Siderita's provider rather than copy it: Siderita owns a
      QString-path thumbnail cache, while this provider owns full-image bytes and
      deliberately supports Unix names that are not UTF-8.
- [ ] `qml/Main.qml` plus a component per surface. **Images work for real**
      through Qt's own decoder, the asynchronous provider and a capped requested
      size — no media backend loaded, which is CP0's "still images cost nothing"
      item. Video and audio are
      *recognised* and get an honest state
      naming the pending backend decision — **not** a transport bar that cannot
      move. A missing or undecodable file says so plainly. New components are
      added to F2's single `QML_FILES`/`rerun-if-changed` inventory. All colour,
      type, radius, spacing, motion and easing come from `CelestinaTheme`.
- [ ] Accessibility is part of the component, not a later pass: whatever
      controls exist expose role, name and state, are reachable and operable by
      keyboard, and show focus via `visualFocus`. Any `Behavior` honours
      `CelestinaTheme.reducedMotion`.
- [ ] `org.celestina.Fluorita.desktop` lists only the image MIME types F3 really
      renders and uses one `%f`; multi-file argv/queues are outside CP0.
      Installing the entry or changing the user's default handler is separate,
      explicitly authorized acceptance work.
- [ ] `scripts/run.sh` on Magnetita's model (build release, install binary +
      desktop entry and any available rasterised icon into `~/.local`, with
      `--uninstall`). A missing dedicated icon warns and keeps the generic
      fallback. **Do not run it** without the author asking: installation is a
      change outside the repository.
- [ ] Same-change documentation moves the recorded state from guarded scaffold
      to a verified image slice; it does not restate F2's authorization event.

**Evidence:** the architecture guard first; then `cargo fmt --check`, Clippy
`-D warnings`, `cargo test --locked` and `cargo build --release --locked` for
the package; `qmllint`; an offscreen start with no `TypeError`/`ReferenceError`;
and a real-session look at the image surface. The ratified F3 budget is under
300 ms from open request to first painted image for the fixed sample corpus;
record the corpus, cold/warm state and result. A real temporary image whose Unix
filename contains an invalid UTF-8 byte must load through the raw-path → bounded
bytes → provider path; the test proves Qt/QML never reconstructs its path from
the lossy display label. Oversized input/dimension fixtures fail truthfully.
State plainly what a build and an offscreen start do *not* prove — they do not
prove interaction, appearance, compositor behaviour or accessibility.

### F4 — the backend spike (what Checkpoint 0 actually turns on)

The decision is still open and must be settled by measurement. The bounded
probes may use candidates already installed on the machine, but they do not
adopt either candidate into the product. Start with a live preflight of binaries,
development headers/libraries and versions. If a probe prerequisite is missing,
stop and request explicit authorization to install only that measurement
dependency; that approval does not approve the production backend. Declare the
protocol **before** measuring, so the numbers cannot be chosen after the fact.

- [ ] Prototype the same 1080p clip through **libmpv** and through
      **GStreamer** using provisional adapters for control, events and the
      render sink, isolated from the production manifest. Both receive an owned
      file handle opened from the raw `PathBuf`, never a lossy path string; a
      candidate that cannot consume that bounded local-source contract fails the
      probe. Raw FFmpeg is
      recommended for elimination on the record rather than by measurement: it
      maximises code we own and correctness we must prove, which the non-goals
      already argue against.
- [ ] Measure, per candidate: installed closure in a clean disposable
      environment rather than inferring it from the current machine, PSS at idle
      / playing / paused, decode CPU, dropped frames, time to audible output for
      audio and time to first painted frame for video.
- [ ] **Budgets ratified before the spike:** audible audio under 300 ms; first
      video frame under 600 ms; PSS under 150 MB while playing 1080p; zero
      sustained dropped frames at 1080p30 and under 1 % at 1080p60. The fixed
      corpus, cold/warm state, measurement tools and repetition count are part of
      the protocol and are recorded before the first run.
- [ ] Record the recommendation **with the numbers that settled it** in this
      roadmap, then stop at the dependency gate. Incorporating a heavy backend,
      adding its Rust/system dependencies or installing packages requires the
      author's explicit approval after seeing those numbers; each adopted
      dependency gets an inline manifest justification and measured closure.
- [ ] After that approval, extract `fluorita-decode` only from the intersection
      exposed by both provisional adapters, integrate the winner and retain the
      loser as a minimal test stub so the trait is not shaped around one backend.
      Prove real audio/video first, then add only those proven MIME types to the
      desktop entry. An isolated temporary-XDG test verifies the handler and
      one-file `%f` contract for valid desktop paths; direct argv tests preserve
      non-UTF-8 audio/video names through the owned-handle source. Installing the
      entry, changing the live handler and accepting Siderita → `xdg-open` remain
      separately authorized real-session actions.

**Evidence after integration:** the common guard first; package/workspace fmt,
Clippy with `-D warnings`, tests and locked release build; `qmllint`; an offscreen
start free of QML runtime errors; contract tests that drive the extracted trait
through the winner and loser stub; and the frozen measurement protocol rerun on
the integrated build. CP0 closes only after real Wayland audio/video/image
acceptance and an explicitly authorized Siderita → `xdg-open` hand-off, with the
live handler restored afterward unless the author chooses to keep it.

**Two integration risks the spike exists to expose, and should attack first:**

- **Painting into the Qt Quick scene graph.** libmpv's render API is
  GL/FBO-shaped and CXX-Qt 0.9 does not expose that surface, so the seam will be
  hand-written C++ under `cpp/` — which the contract permits only with a comment
  naming the precise CXX-Qt limitation. Prove that seam can stay thin before
  committing to a backend; backend *control* stays in Rust behind the trait.
- **`unsafe` is forbidden.** A hand-rolled FFI binding to libmpv would need it,
  and the ban is repo-wide with only pre-approved, isolated exceptions. The two
  routes that do not require an exception are a vetted binding crate (with its
  justification in `Cargo.toml`) or letting the C++ seam own the library call
  while Rust owns the state contract. Decide this with the spike, not on paper.

## Checkpoint 1 — The producer half (make Siderita's thumbnails appear)
**Goal:** Fluorita starts paying its way to the rest of the suite, by generating
exactly what Siderita already knows how to consume.

- [ ] Video first-frame and audio cover extraction into the shared cache —
      256 px "large" PNGs, `md5(file-uri)` key, atomic write, mtime validity
- [ ] Generation is bounded and off the UI thread, and never blocks playback
- [ ] A `.thumbnailer` entry so *other* desktop apps get the same frames, since
      the cache is shared and the standard exists. Creating it in the repository
      is part of CP1; installing or registering it outside the repository needs
      explicit author authorization
- [ ] MPRIS2 — `org.mpris.MediaPlayer2` transport and now-playing metadata
- [ ] **Verified** — opening one media file in Fluorita queues its media siblings
      and, on returning to that folder in Siderita, real frames/covers appear
      with **no change to Siderita**; a second pass reuses the cache

**Done when:** after one standards-based hand-off to Fluorita, Siderita's video
and audio rows reuse real thumbnails without a private API or decode dependency,
and playback is controllable from outside the app.

> **Generation trigger settled.** When Fluorita opens a file, it enumerates only
> that directory (never recursively), considers at most 512 entries and queues at
> most 64 media siblings. The handed file has foreground priority; thumbnailing
> uses at most two workers, reads at most 256 MiB and spends at most 10 seconds
> per candidate, with a whole-handoff ceiling of 1 GiB read and 60 seconds wall
> time. It cancels the remaining queue when the directory/source generation
> changes or the app exits, then deterministically joins both workers before
> releasing their state. This bounded queue is derived from the hand-off, not a
> persistent library scan. The `.thumbnailer` entry serves
> desktops that invoke the standard directly; Siderita remains a cache-only
> consumer and needs no private D-Bus request.

## Checkpoint 2 — One suite (the embeddable widget)
**Goal:** the player stops being a separate window and becomes a component the
rest of the session hosts.

- [ ] The player surface extracted as an **embeddable widget** with a bounded,
      documented contract (size, lifecycle, what it costs while idle)
- [ ] Shell widget — a playing clip or now-playing music in the `celestina` panel
- [ ] Siderita live quick-look — the widget replaces the info card that names
      Fluorita today, so spacebar previews a clip instead of describing it
- [ ] One settings source shared with the suite (volume, defaults, per-type
      handlers), not a private store
- [ ] **Measured** — the idle cost of an embedded widget, since it lives in the
      panel and the panel lives forever

**Done when:** the same player runs in its own window, in the panel and inside
the file manager's preview, over one contract and one visual language.

## Later / someday
- [ ] Subtitles, audio-track selection, playback-speed control — each when a
      daily need appears, never for parity with a full media player
- [ ] A queue built from a multi-selection hand-off (still not a library)
- [ ] Hardware-decode tuning per format, once there is a measured reason

## Non-goals
- **No library.** No recursive or persistent collection scan, tag database or
  watch history. The bounded immediate-sibling CP1 queue is transient work
  derived from one hand-off; Siderita remains the browser.
- **No codec reimplementation.** The decode backend is a dependency, chosen and
  measured, not something this project writes.
- **No streaming or network sources** before a local player is complete and a
  daily gap is proven.
- **No feature parity** with mpv, VLC or a stock viewer. A settings page full of
  switches is not progress.
- **No leaking media weight.** Siderita must never gain a decode dependency
  because of anything decided here; the hand-off stays freedesktop-shaped.
- **Not a general product.** Like the rest of Celestina, this is for its author's
  session.
