# Evidence: 2026-09-02 apps performance audit

- **Date:** 2026-09-02
- **Scope:** read-only performance audit of the four applications —
  `siderita`, `fluorita`, `grafita`, `magnetita` — across their QML surfaces,
  their Rust application crates and the suite crates they consume
  (`celestina-rs/crates/*`), plus the shared controls in `celestina-style/`.
  The Celestina shell (`celestina/`) is explicitly out of scope by the
  author's instruction
- **Environment:** static review of the checkout at `0b97060`, with the day's
  uncommitted verification fixes present in the tree (test sampling fixes,
  `CelestinaShadow` module registration, the bookmark rename right-click fix,
  qmllint pragmas). None of those files carries a finding below. Nothing was
  profiled at runtime: every finding is an argued cost read from the source,
  not a measured one, and the estimates say so where they appear
- **Artifact:** none. The output is this record. Nothing was corrected: an
  audit does not authorize a fix

## Procedure

One pass per application plus one over the shared module, each preferring a
few substantiated findings over many speculative ones. "Performance" was
defined as: work on the GUI thread that scales with data size (per-keystroke,
per-frame, per-event); wholesale model republication that rebuilds delegates;
per-frame or short-interval timers and unbounded animations; image decodes
without `sourceSize` or off the async path; effect layers (`ShaderEffectSource`,
`MultiEffect`, `layer.enabled`) in delegates or always on; and Rust-side
blocking I/O, polling loops, and unthrottled progress publication. Every High
and Medium finding was re-verified against the source before being recorded,
including the threading of each incriminated call path.

```sh
rg -n 'Timer \{|interval:|Animation.Infinite|layer.enabled|MultiEffect|ShaderEffect' \
  -g '*.qml' siderita fluorita grafita magnetita celestina-style
rg -n 'GridView \{|ListView \{|reuseItems|cacheBuffer|sourceSize|asynchronous' \
  -g '*.qml' siderita fluorita grafita magnetita
rg -n 'std::thread::spawn|qt_thread|sleep|is_file|read_dir' \
  {siderita,fluorita,grafita,magnetita}/src celestina-rs/crates
```

The cxx-qt 0.9.1 property setter was read in the vendored generator source
(`cxx-qt-gen-0.9.1/src/generator/rust/property/setter.rs:70-77`) to settle a
question the findings depend on: setters carry an equality guard, so
re-assigning an unchanged `QStringList` emits nothing and rebuilds nothing.

## Result

- **Exit:** manual outcome. 9 findings: 0 Critical, 2 High, 2 Medium, 5 Low.
  Both Highs are per-keystroke work on the GUI thread that scales with the
  size of the data being edited or searched
- **Observed:** the two mature browsing surfaces — Siderita's folder view and
  Magnetita's device page — are in genuinely good shape: digest-gated
  publication, debounced watchers, off-thread snapshots, setter equality
  guards. The costs live where data is *searched* (Fluorita re-projects its
  whole library, twice, with a `stat()` per item, on the GUI thread, per
  keystroke pause) and where data is *edited* (Grafita makes at least five
  full passes over the document per keystroke). The common root is the same
  in both: a projection designed to run once per scan being re-run on the
  GUI thread inside an interactive loop

### Fluorita

**FLU-P1 — High — Searching re-projects the entire library twice on the GUI
thread, with a `stat()` per item, per keystroke pause.**
`fluorita/src/library.rs:593-608`: `search()` is a `#[qinvokable]` (GUI
thread). It calls `project(...)` once *unfiltered* — only to count the total
for `hidden_by_query` — and then `project_matching(...)` again with the query.
Each projection calls `cached_thumbnail` for every gallery item and every
music track (`fluorita/src/library/project.rs:113,155`), and `cached_thumbnail`
performs a synchronous `entry.is_file()` per item
(`fluorita/src/library/project.rs:234-245`). The scan budget allows 50 000
records (`celestina-rs/crates/fluorita-engine/src/library.rs:42-48`), so one
keystroke pause can issue on the order of 100 000 `stat()` calls on the GUI
thread. The search field debounces at 150 ms
(`fluorita/qml/components/LibraryView.qml:76-81`), which bounds the *rate*,
not the cost. On a slow or network-backed folder each `stat()` is a round
trip. The scan path itself proves the projection can run off-thread: it
already does (`fluorita/src/library/work.rs:271-272`). The unfiltered
projection computed only for its count is pure waste besides — the count
needs no thumbnail resolution at all.

**FLU-P2 — Medium — The artwork pass finishes by re-projecting the library on
the GUI thread.** `fluorita/src/library.rs:679-691`: `artwork_finished` calls
`project(...)` — the same per-item `stat()` cost as FLU-P1 — synchronously in
the Qt callback, once per completed artwork pass. One occurrence per pass
rather than per keystroke, hence Medium.

**FLU-P3 — Medium — Every library revision destroys and recreates every
instantiated gallery cell.** `fluorita/qml/components/GalleryGrid.qml:47-58`:
the grid's model is a plain JS array rebuilt wholesale by `weave()` on each
`revisionChanged`. Reassigning an array model diffs nothing: all delegates are
torn down and recreated, and neither `reuseItems` nor `cacheBuffer` applies to
a model replacement. Together with FLU-P1 this means each search projection
also rebuilds every visible card. The columns-plus-revision publication
protocol is sound (it exists to avoid half-published rows); the cost is in
using an array model where an object model with granular change signals, or a
keyed merge, would let unchanged rows live.

**FLU-P4 — Low — Each editor revision rebuilds every annotation delegate,
including the `Shape` curves.** `fluorita/qml/components/EditObjectLayer.qml:34-49`
rebuilds `rows` on every `editor.revision` bump, and both Repeaters recreate
all their delegates from it — the second one through a `.filter()` model
(`:146`) that recreates the stroke `Shape`s (CurveRenderer) even when only a
box moved. A held arrow key auto-repeats `moveObject`
(`fluorita/qml/components/EditSurface.qml:575`), and every repeat republishes
(`fluorita/src/editor.rs:892-899`) and rebuilds the whole layer. Bounded by
annotation count, hence Low — but it scales linearly with marks placed.

**FLU-P5 — Low — The ambient light re-decodes on every window resize step.**
`fluorita/qml/components/AmbientLight.qml:53-54` binds `sourceSize` to the
surface size ÷ 4; each interactive resize step changes `sourceSize` and forces
a re-decode. The source is a small cached thumbnail, so each decode is cheap;
the quarter-res sampling itself is the right call.

### Grafita

**GRA-P1 — High — At least five full passes over the document per keystroke,
on the GUI thread.** The chain, per typed character:
1. `grafita/qml/components/DocumentView.qml:235` — `onTextChanged:
   root.session.applyText(text)` reads `TextEdit.text`, a full
   QTextDocument→QString extraction, O(document);
2. `grafita/src/session.rs:415-424` — `text.to_string()` converts the whole
   QString UTF-16→UTF-8, another full copy;
3. `celestina-rs/crates/grafita-core/src/display.rs:149-179` — `reconcile`
   compares `current == proposed` (full scan) plus prefix/suffix scans;
4. `celestina-rs/crates/grafita-core/src/document.rs:291` — after the edit,
   `display::project(&self.buffer)` rebuilds the *entire* projection String;
5. `celestina-style/CelestinaLineGutter.qml:111-131` — the gutter's
   `onTextChanged` handler calls `reindex()`, which reads `surface.text`
   *again* (a second full QTextDocument→QString extraction) and scans every
   newline in JavaScript.
The gutter's own header (`CelestinaLineGutter.qml:7-9`) is designed for
documents of tens of megabytes — the one size at which five O(n) passes and
four O(n) allocations per keypress turn typing into tens of milliseconds of
latency per character, all on the GUI thread. The gutter could take its line
starts from the core (which just rebuilt the projection anyway), and the
reconcile path could take the widget's change signals (position, added,
removed) instead of the whole text.

### Siderita

No finding above Low. The folder pipeline is the counter-example the other
apps could copy: the projection digest drops watcher ticks that changed
nothing visible — the comment records 124 ms of CPU saved per tick in a
2 000-entry folder (`siderita/src/controller/scan.rs:262-277`); the
filesystem watcher is one shared, debounced instance
(`siderita/src/controller/watchreg.rs`); recursive search runs on a worker
thread with a 500-hit cap (`siderita/src/controller/find.rs:64-75`);
thumbnails go through a `QQuickAsyncImageProvider`
(`siderita/cpp/thumbnailprovider.cpp:302`) into delegates that set
`sourceSize` and `asynchronous` and are recycled (`reuseItems: true`,
`cacheBuffer` — `siderita/qml/components/folder/FolderListView.qml:54-55`).
The 16 ms drag-autoscroll timer runs only while a drag is over the edge
(`siderita/qml/components/entry/DragScrollEdge.qml:21-24`) and the search
field debounces at 220 ms (`siderita/qml/components/chrome/TopBar.qml:478-489`).

### Magnetita

**MAG-P1 — Low — A new OS thread per daemon signal.**
`magnetita/src/controller.rs:327-340` (`request_device_reload`) and
`:667-680` (`request_log_reload`) spawn a fresh `std::thread` per coalesced
`Changed`/`Event` signal. During media playback the daemon emits `Changed` at
1 Hz for position updates, so a listening app spawns thousands of threads per
hour. Each spawn is microseconds and the in-flight flag prevents pile-up, so
this is waste rather than jank; a long-lived worker would retire it.

**MAG-P2 — Low — MediaCard holds a mask layer alive unconditionally.**
`magnetita/qml/components/MediaCard.qml:82-89`: `roundedCardMask` keeps
`layer.enabled: true` even when no artwork is shown, costing one card-sized
texture per card permanently. The blur layer itself is correctly gated on
`visible` (`:103`).

The 1 Hz update path was checked and holds: the snapshot is fetched
off-thread with in-flight coalescing (`controller.rs:327-340`), and because
cxx-qt setters compare before emitting, the twenty-odd `QStringList`
re-assignments in `apply_devices` (`controller.rs:465-493`) only emit for the
columns that actually changed — a position tick does not rebuild the device
Repeaters.

### celestina-style (shared)

**STY-P1 — Low — A dialog's live glass re-renders its backdrop per damaged
frame.** `celestina-style/GlassCard.qml:17-20` sets `liveCapture: true`, so
while a dialog is open, any change under it (scrolling, video) re-renders the
`backdropSource` subtree into the capture texture and re-runs the blur, per
damaged frame. This is damage-driven — a static backdrop costs nothing — and
the per-frame *tracking* alternative was already tried and reverted for CPU
cost (the file's own header cites `a8c0084`). Recorded as the one standing
per-frame cost in the shared module, accepted by design. Siderita's floating
pills (`GlassPill`, `InfoPill`) carry the same property over scrolling
content, bounded by their small capture rectangles.

Also checked and sound: the glass capture re-arms on events, not per frame
(`GlassSurface.qml:112-136`); the shadow is an analytic SDF, not an effect
pass (`CelestinaShadow.qml`); the ambient light samples at quarter resolution;
no infinite animations and no sub-second repeating timers exist outside the
drag-edge case above; every content `Image` in the four apps sets
`sourceSize` and `asynchronous`.

## Limits

- Static reading only: nothing ran and nothing was profiled, so every cost
  above is argued from the source — call paths, thread boundaries, data
  bounds — not measured. The one number quoted (124 ms per watcher tick) is
  the code's own recorded measurement, not this audit's.
- The per-keystroke estimates for GRA-P1 and FLU-P1 scale with data size; on
  a small document or library both paths are imperceptible. The findings
  stand because both features advertise the large case (a 50 000-record scan
  budget, a gutter designed for tens of megabytes).
- Qt behaviour relied on (damage-driven `ShaderEffectSource` updates, array
  models diffing nothing, `TextEdit.text` extraction cost, auto-repeat of
  held keys) is from Qt 6 source and documentation, not from a probe.
- The cxx-qt setter equality guard was verified in the vendored 0.9.1
  generator; a future cxx-qt upgrade that drops it would change Magnetita's
  1 Hz analysis.
- Memory footprint, startup time, and the daemon's network path
  (`magnetita-net`, reconnect behaviour) were not assessed. The Celestina
  shell was not read, by instruction; nothing here says anything about it.

## Follow-up

**2026-09-02, later the same day.** The author authorized fixing the two High
findings; the Mediums and Lows stand open.

- **FLU-P1** — closed by `fluorita-bug` (PERF-1-FLU, 1.3.3). `search()` now
  hands the projection to a named worker thread and publishes through a
  revision guard: a result computed against a catalogue that was republished
  meanwhile, or superseded by newer typing, is re-run rather than applied.
  Bursts coalesce — one in flight, only the newest text pending. The
  unfiltered projection that existed only to be counted is gone: a new
  `census()` counts the scope with no thumbnail resolution and no `stat()`.
  The catalogue is now held behind an `Arc`, so the per-search and
  per-artwork-pass copies of every record went with it (`forget` pays a
  `make_mut` clone only while a worker holds the previous snapshot). Tests:
  `the_census_counts_what_the_projection_would_show`.
- **GRA-P1** — closed by `grafita-core` work delivered under `grafita-bug`
  (PERF-1-GRA, 1.2.2) and `celestina-style-bug` (PERF-1-STY, 1.8.2). The
  document now keeps a `LineMap` — per-line byte and UTF-16 lengths with
  prefix sums — maintained by the same single choke point every mutation
  passes through (`Document::apply_to_buffer`: edits, undo, redo). The
  projection is spliced line-aligned instead of rebuilt, and every caret and
  offset question (`caret_utf16`, `caret_location`, which also ran O(document)
  on **every caret move**) became a binary search plus one line's walk. The
  gutter grew an optional `lineSource` contract — `lineCount`,
  `lineStartUtf16(line)`, and a `lineRevision` beat that arrives *after* the
  core has absorbed the edit — and Grafita hands it its session, so the
  second full text extraction and the JavaScript newline scan per keystroke
  are gone; a host without a core (Siderita's preview) keeps the
  self-scanning path unchanged. What remains per keystroke is the protocol's
  price: one `TextEdit.text` extraction, one UTF-8 conversion and the
  reconcile scans. Tests: the `LineMap` oracle battery in `display.rs`, the
  spliced-projection invariant through edit/undo/redo in `documents.rs`, and
  `tst_linegutter.qml` covering both gutter modes and the revision beat.
- Verified after the fixes: grafita-core 174 tests, fluorita 61, grafita 7,
  celestina-style 66 QML tests (4 new), style and contrast contracts OK,
  qmllint ratchets unchanged (grafita 47, siderita 274).
