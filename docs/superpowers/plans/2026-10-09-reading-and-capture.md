# Reading and Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Calcita (the suite's PDF viewer) and Selenita (screen capture and recording), both 1.0, joined to the suite through the CONV-1 conventions.

**Architecture:** Each application follows Cuprita's shape — a pure crate in `celestina-rs/crates/<app>-core`, a CXX-Qt adapter in `<app>/src`, QML in `<app>/qml` over celestina-style — registered by a `suite` unit, versioned by the author's baseline hand commit, closed at 1.0 with `--kind release`. Calcita renders through Qt's own `QtQuick.Pdf`; Selenita orchestrates `grim`, `slurp`, niri's IPC and the ScreenCast portal with GStreamer.

**Tech Stack:** Rust (workspace toolchain), cxx-qt 0.9.1, Qt 6.12 QML + QtQuick.Pdf, celestina-style, celestina-core (activation, file_uri, atomic_file, xdg), celestina-settings, zbus 5 (portal), `grim`/`slurp`/`wl-clipboard`, GStreamer 1.28 (+ `gst-plugins-good` runtime dependency), optionally the `gstreamer` Rust crate.

**Spec:** `docs/superpowers/specs/2026-10-09-reading-and-capture-design.md`

## Global Constraints

- Program `EXT-1`; suite ledger `docs/plans/active/2026-10-09-reading-and-capture.md` (rows EXT-1-A, EXT-1-B, EXT-1-C; created by Task 1 with the root ROADMAP opened on `EXT-1` as CONV-1-A did for CONV-1); application ledgers `calcita/docs/plans/active/2026-10-09-cal-1-foundation.md` and `selenita/docs/plans/active/2026-10-09-sel-1-foundation.md` (rows A–C each, created by their skeleton units; the skeleton row itself is described in prose, as CUP-1-A ended up).
- Prefixes: `suite:` for registration and exit units; `calcita:`/`selenita:` for application units (session commits `<app>-maintenance:`); component prefixes `calcita-core`/`selenita-core` registered in `docs/projects.toml` by the registration units.
- Registration rule (learned with Cuprita): the pre-commit hook runs the documentation contract on the index, so registration + skeleton + icon are ONE suite commit; the project is registered `versioned = false`; after the landing the author hand-commits the baseline (`version_source`/`version_mirrors` + the `0.1.0 baseline` row in `docs/version-history.tsv`). 1.0 lands with `--kind release`.
- Suite conventions (ADR 0012): activation through `celestina_core::activation` (the two new names join the crate's name list and the scanner's knowledge), `Open(as paths)` byte-exact, a `DropArea` for `text/uri-list` decoded with `file_uri::to_path`, the shared appearance follower (`celestina-settings::follow`, seeded `Default`), `DBusActivatable` unset.
- Style: transparent canvas + `CelestinaBackdrop`; CUP-1-H card grammar (`SectionCard`-like local, `rowInset` 16, `SettingRow`, `RowDivider`, `LoadingLine`); tokens only; `CelestinaWheelScroll` on lists; dialogs via `CelestinaModalLayer`; popup menus `popupBeside`; motion 100–500 ms honouring `reducedMotion`; Spanish only in `qsTr()`; English development text.
- No blocking IO on the Qt thread; typed errors with `message_es`; no production `unwrap`/`expect`; each dependency justified in `Cargo.toml`; no verbatim copies from other applications (structure yes, code no).
- Ratchets never grow (qmllint rows `0` for both new applications; radius; glass-canvas; architecture; language). Host Qt is 6.12.0.
- Before every commit: `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test`, `cargo build --release --locked` in the application; `cargo test -p <app>-core` in `celestina-rs/`; from the root: `sh <app>/scripts/qml-tests.sh`, `sh <app>/scripts/smoke.sh --binary …/.cargo-target/release/<app>`, `bash scripts/qmllint-cxxqt.sh <app>`, `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`; scope with `python3 scripts/commit_scope.py --check "<subject>"` (paths on stdin). Never `git stash`. Both applications get `tests/qml` harnesses and a `scripts/qml-tests.sh` from the skeleton (copy Cuprita's shape).
- Live checks that need the author (a real PDF, a real capture on the session, a recording) are VAL entries, never performed by a subagent; subagents verify offscreen with fixtures and fakes (`CALCITA_FAKE`/`SELENITA_FAKE`).

---

### Task 1: EXT-1-A — register Calcita with its skeleton (suite)

**Files:**
- Modify: `docs/projects.toml` (`calcita` entry modelled on `cuprita`'s, `versioned = false`, component scope `calcita-core`), `celestina-rs/Cargo.toml` (member), `celestina-rs/Cargo.lock`, `scripts/qmllint-baseline.tsv`/`glass-canvas-baseline.tsv`/`radius-baseline.tsv` (`0 calcita`), `README.md` (table row), root `ROADMAP.md` (`Status: active`, `Active implementation checkpoint: EXT-1`, a `## EXT-1 — Reading and capture` section with the nine units as a checklist), root `STATUS.md`, `docs/plans/active/README.md`, `celestina-rs/crates/celestina-core/src/activation.rs` (add `CALCITA` and `SELENITA` to the name constants and the allowlist knowledge of `scripts/activation_contract.py` if it enumerates names)
- Create: `celestina-rs/crates/calcita-core/{Cargo.toml, src/lib.rs}` (stub), `calcita/` skeleton: `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/main.rs` (identity `org.celestina.Calcita`, claim-first activation through the crate with a thin `src/activation.rs` adapter, the appearance follower with a thin `src/appearance.rs`, `CalcitaController` singleton), `qml/Main.qml` (transparent canvas, backdrop, `CelestinaAppearance`, an empty-state card «Sin documento» with an «Abrir…» button that is wired in Task 2), the style symlinks + `fonts.qrc`/`icons.qrc` as Cuprita has, `org.celestina.Calcita.desktop` (`Exec=calcita %U`, `MimeType` left for Task 4), `celestina-style/icons/apps/org.celestina.Calcita.svg` (a white page glyph in the family), `scripts/{build,complete,verify,deploy,status}-production.sh`, `scripts/smoke.sh`, `scripts/qml-tests.sh`, `tests/qml/tst_main.qml` (the window constructs and the empty state shows), `AGENTS.md`, `README.md`, `STATUS.md`, `ROADMAP.md`, `VALIDATION.md`, `docs/plans/active/2026-10-09-cal-1-foundation.md` (rows CAL-1-A/B/C `planned`; the skeleton described in prose), `docs/evidence/README.md`, `docs/evidence/2026-10-09-skeleton.md`; suite ledger `docs/plans/active/2026-10-09-reading-and-capture.md` (EXT-1-A `active` with evidence link; B, C `planned`); evidence `docs/evidence/2026-10-09-calcita-registration.md`; commit the spec and this plan (untracked in the worktree).

**Interfaces:**
- Produces: project `calcita`; `CalcitaController` singleton with `reducedMotion`/`textScale` plumbing through `CelestinaAppearance`; `ActivationName` constant `org.celestina.Calcita` in the crate; the two ledgers.

- [ ] **Step 1: Read the models** — `git show 720406e9 --stat` (Cuprita registration + skeleton), `cuprita/` as it is now (its `src/main.rs`, `src/activation.rs`, `src/appearance.rs`, `qml/Main.qml`, scripts, documents), `scripts/activation_contract.py`.
- [ ] **Step 2: Registry, crate stub, ratchet rows, README row, ROADMAP/STATUS/active-plans README** (opening EXT-1 as CONV-1-A opened CONV-1: `git show efc08f29 -- ROADMAP.md STATUS.md docs/plans/active/README.md`).
- [ ] **Step 3: Skeleton** — failing `tst_main.qml` first (window + empty state), then the files; `cargo build --release --locked`; smoke offscreen; qmllint 0.
- [ ] **Step 4: Documents, ledgers, evidence** (every command with its exit code).
- [ ] **Step 5: Checks** per Global Constraints (`calcita` set + the contracts + `python3 scripts/test-land-unit.py`, `test-production-artifacts.py`, `test-production-common.sh`), commit `suite-maintenance: Register the Calcita project with its skeleton and icon`.
- After the landing (controller): the author's baseline hand commit (`version_source`, `version_mirrors`, `calcita 0.1.0 baseline EXT-1-A` row).

---

### Task 2: CAL-1-A — open, pages, zoom, navigation, suite joins

**Files:**
- Create: `celestina-rs/crates/calcita-core/src/{recent.rs, zoom.rs, pagelabel.rs, reading.rs}`, `celestina-rs/crates/calcita-core/tests/{recent.rs, zoom.rs, pagelabel.rs}`
- Modify: `calcita/src/controller.rs` (document path as pathkey, `openPath(key)`, `openDropped(uris)`, `page`/`pageCount`, `goTo(text)`, `zoomMode`/`zoomTo(mode|factor)`, recents, notice), `calcita/src/activation.rs` (`Open` → first path opens; a second window per extra path: `CalcitaWindow` instantiated per document through a `Instantiator` in `Main.qml` over a `documents` list), `calcita/qml/Main.qml` + `qml/DocumentWindow.qml` (bar: name, page field «n / N», zoom controls, open; page area `PdfMultiPageView` over `PdfDocument`; `DropArea`), `calcita/build.rs` (`QtQuick.Pdf` depend), `calcita/tests/qml/{tst_document.qml, tst_bar.qml}` over `tests/fixtures/three-pages.pdf` (a tiny PDF generated with a checked-in script or hand-written minimal PDF syntax), `calcita/scripts/smoke.sh` (opens the fixture and asserts `pageCount == 3`), (Siderita's «Abrir en» target for PDFs is Task 9's, suite scope), docs: ledger row CAL-1-A `active`, evidence `calcita/docs/evidence/2026-10-09-open-and-pages.md`, STATUS, README, VALIDATION (VAL-CAL-OPEN pending)

**Interfaces:**
- Crate: `pub struct Recent { pub path: PathBuf, pub page: u32, pub zoom: ZoomMode, pub opened_at: SystemTime }`, `pub struct RecentStore` (`load()`, `touch(&mut self, recent)`, `save()`, max 50, `data_home/calcita/recent` TSV-like lines with pathkey), `pub enum ZoomMode { FitWidth, FitPage, Free(f32) }` with `LADDER: [f32; 12]` (0.5, 0.67, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 3.0, 4.0), `zoom_in(f32) -> f32`, `zoom_out(f32) -> f32`; `pub enum GoTo { Absolute(u32), Relative(i32), First, Last }` with `parse(text, current, count) -> Result<u32, GoToError>`; `pub struct Reading { page, zoom }`.
- Controller invokables: `openPath(key)`, `openDropped(uris)`, `goTo(text)`, `zoomIn()`, `zoomOut()`, `fitWidth()`, `fitPage()`, `setZoom(f)`; properties `documentKey`, `documentName`, `page`, `pageCount`, `zoomMode`, `zoomFactor`, `recents` (list model), `loaded`; signal `notice(kind, text)`.

- [ ] **Step 1: Failing crate tests** (`tests/zoom.rs`: `zoom_in(1.0) == 1.1`, `zoom_in(4.0) == 4.0`, `zoom_out(0.5) == 0.5`; `tests/pagelabel.rs`: `"12"`→12, `"+3"` from 5→8, `"-2"` from 1→Err, `"fin"`→count, `"inicio"`→1, `"0"`→Err; `tests/recent.rs`: touch keeps order, caps at 50, round-trips a non-UTF-8 path). Run → FAIL. Implement.
- [ ] **Step 2: Failing QML tests** over the fixture: the document loads with 3 pages; `goTo("3")` scrolls to page 3; zoom buttons change `zoomFactor` along the ladder; a dropped non-PDF URI yields a notice and no document; the page field shows «2 / 3» after a scroll. Implement the bar, the view, the drop area, the empty state with recents and «Abrir…» (FileChooser portal through Qt's `FileDialog`, which routes to Siderita's backend).
- [ ] **Step 3: Activation** — `Open` with one path replaces the empty state or raises the window holding it; with several, one window each; test over the fake queue.
- [ ] **Step 4: Keyboard** — PageDown/PageUp/Space/Shift+Space/Home/End, `Ctrl+G` focuses the page field, `Ctrl+Plus/Minus/0`, `Ctrl+1/2`, `Ctrl+O`; `tst_keyboard.qml`.
- [ ] **Step 5: Checks, smoke, ledger, evidence; commit** `calcita-maintenance: Add opening, continuous pages, zoom and navigation`.

---

### Task 3: CAL-1-B — search, outline, selection, links

**Files:**
- Create: `celestina-rs/crates/calcita-core/src/search.rs` + `tests/search.rs` (`SearchRequest { query, case_sensitive, direction, wrap }`, `next_hit(current, hits, direction, wrap) -> Option<usize>`), `calcita/qml/{SearchCard.qml, OutlinePanel.qml}`, `calcita/tests/qml/{tst_search.qml, tst_outline.qml}`, `calcita/tests/fixtures/outline.pdf` (a tiny PDF with two bookmarks and an internal link — hand-written or generated by a checked-in script)
- Modify: `DocumentWindow.qml` (`PdfSearchModel`, `PdfBookmarkModel`, `PdfLinkModel`, selection through the view's `selectedText`, Ctrl+C), `controller.rs` (`copySelection(text)` to the clipboard via Qt's `QClipboard` on the Qt thread — no IO), docs/ledger/evidence

- [ ] **Step 1: Failing crate test** for `next_hit` (wrap on/off, both directions, empty). Implement.
- [ ] **Step 2: Failing QML tests**: search «page» over the fixture yields hits; Enter/Shift+Enter walk them and the counter reads «n de N»; the outline shows the two bookmarks and Enter navigates; the internal link navigates; an external link shows the confirmation pill. Implement.
- [ ] **Step 3: Checks, ledger, evidence; commit** `calcita-maintenance: Add search, the outline, text copy and links`.

---

### Task 4: CAL-1-C — reading mode, recents, appearance, 1.0

**Files:**
- Modify: `calcita/qml/DocumentWindow.qml` (reading mode: inverted/hue-rotated layer effect toggled by a bar button and `Ctrl+I`, state remembered in the crate's `Reading`), the empty state's recents card (open on click, «Quitar de recientes» menu), `calcita/org.celestina.Calcita.desktop` (`MimeType=application/pdf;`), `calcita/Cargo.toml` (version left for the landing), `calcita/STATUS.md`, `ROADMAP.md`, `VALIDATION.md` (VAL-CAL-* with exact steps), `HOST-HYGIENE.md` — NOT here (suite scope; Task 9), the ledger rows (A, B done after their landings; C active), evidence `calcita/docs/evidence/2026-10-09-exit.md`
- Create: `calcita/tests/qml/tst_reading_mode.qml`, `tst_recents.qml`

- [ ] **Step 1: Failing QML tests**: toggling reading mode flips the effect's `enabled` and `Ctrl+I` toggles it; `reducedMotion` disables the toggle's fade; the recents card lists the fixture after opening it and removes it on the menu action. Implement.
- [ ] **Step 2: Keyboard/accessibility pass** (names on the bar's controls, the page field, the outline rows, the search card); glass/motion check.
- [ ] **Step 3: Checks, ledger, evidence; commit** `calcita-maintenance: Add the reading mode and the recents and close the foundation at 1.0`; the landing uses `--kind release`.

---

### Task 5: EXT-1-B — register Selenita with its skeleton (suite)

Same shape as Task 1 for `selenita` (crate stub `selenita-core`, skeleton with a one-window layout of three empty cards: capture, recording, history; `Exec=selenita`, no `%F`; icon a moon-glyph in the family; `SELENITA_FAKE=1` wiring prepared; ledger `selenita/docs/plans/active/2026-10-09-sel-1-foundation.md`; suite ledger row EXT-1-B active, EXT-1-A done). Commit `suite-maintenance: Register the Selenita project with its skeleton and icon`; then the author's baseline hand commit.

---

### Task 6: SEL-1-A — screenshots, delay, destinations, history

**Files:**
- Create: `celestina-rs/crates/selenita-core/src/{geometry.rs, target.rs, names.rs, history.rs, niri.rs, tools.rs, runner.rs}`, `tests/{geometry.rs, names.rs, history.rs, niri.rs, tools.rs}` with fixtures `tests/fixtures/niri-focused-window.json`, `niri-outputs.json` (captured from `niri msg --json`)
- Create: `selenita/src/{controller.rs, capture.rs (worker), backend.rs (fake under SELENITA_FAKE=1)}`, `selenita/qml/{CaptureCard.qml, HistoryCard.qml, HistoryRow.qml}`, `selenita/tests/qml/{tst_capture_card.qml, tst_history.qml}`, `selenita/src/activation.rs` serves a second small interface `org.celestina.Selenita1` on the same object path from its adapter, with `Capture(s)` and `ToggleRecording()` (the crate's `Activatable` interface stays fixed; the scanner allowlists the literal by (file, literal) with the reason), `selenita/src/main.rs` (`--screenshot <screen|window|region>` → `Capture` on the running instance or a fresh one; `--record`/`--stop` reserved for Task 7)
- Modify: docs/ledger/evidence, `selenita/README.md` (runtime dependencies `grim`, `slurp`, `wl-clipboard`, niri), `VALIDATION.md` (VAL-SEL-SHOT pending)

**Interfaces (crate):**
```rust
pub struct Geometry { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }   // Display "x,y wxh"; FromStr
pub enum Target { Screen { output: String }, Window { geometry: Geometry, app_id: String, title: String }, Region { geometry: Geometry } }
pub struct Capture { pub target: Target, pub delay: Duration, pub to_clipboard: bool, pub to_file: bool }
pub fn capture_file_name(stem: &str, now: SystemTime, extension: &str) -> String;  // "<stem> 2026-10-09 14.32.05.<ext>"; the stem is product copy passed from the Qt seam (qsTr), so the crate holds no Spanish
pub fn pictures_dir() -> Option<PathBuf>;  // xdg-user-dir PICTURES via the user-dirs.dirs file, parsed in the crate (no process)
pub struct History { entries: Vec<Entry> }  // load/save/push/prune_missing, max 100
pub mod niri { pub fn focused_window(socket: &Path) -> Result<Window, NiriError>; pub fn outputs(socket: &Path) -> Result<Vec<Output>, NiriError>; pub fn parse_focused_window(json: &str) -> …; pub fn parse_outputs(json: &str) -> … }
pub mod tools { pub fn grim_argv(target: &Target, out: &Path, output_name: Option<&str>) -> Vec<OsString>; pub fn slurp_argv(accent: &str, background: &str) -> Vec<OsString>; pub fn wl_copy_argv() -> Vec<OsString>; }
pub mod runner { pub fn run(argv, stdin: Option<&[u8]>, deadline: Duration) -> Result<Output, RunError> }  // deadline-killed child, as Cuprita's wpctl runner
```

- [ ] **Step 1: Failing crate tests** (geometry round trip; file-name rule with a fixed `now` and a collision suffix; history prune of a missing file and the cap; niri parsers over the fixtures; argv builders; the runner's deadline with `sh -c 'sleep 5'`). Implement.
- [ ] **Step 2: Worker plan** (`capture.rs`): close the window if the target would include it → wait the delay with a countdown signal to QML (`countdown(seconds)`) → `slurp` for a region (accent/background from `CelestinaTheme` passed by the controller) → `grim` → `wl-copy` when asked → file when asked → history push → `captured(entryId)` → reopen the window on the history. Errors → `notice`.
- [ ] **Step 3: Failing QML tests** over the fake backend: the three target buttons set the target; the delay choice; the switches; pressing capture over the fake yields a history row with a thumbnail (the fake writes a 1×1 PNG into a scratch dir under `XDG_DATA_HOME`); row actions call `openInFluorita`, `copy`, `showInSiderita`, `delete`. Implement the cards in the CUP-1-H grammar.
- [ ] **Step 4: Key-binding flags and `org.celestina.Selenita1.Capture(s)`**; test the argv → request mapping.
- [ ] **Step 5: Checks, smoke (fake), ledger, evidence (with a read-only live `niri msg --json focused-window` capture in the evidence, no screenshot taken by the subagent); commit** `selenita-maintenance: Add screenshots with delay, destinations and the history`.

---

### Task 7: SEL-1-B — recording

**Files:**
- Create: `celestina-rs/crates/selenita-core/src/record.rs` + `tests/record.rs` (pipeline builder for the four combinations video×{x264, vah264} × audio{off, on}, muxer `mp4mux`; state machine `Idle → Preparing → Recording → Stopping → Idle | Failed` with its transitions tested; the stop-file protocol path `runtime_dir/selenita/stop`), `selenita/src/portal.rs` (ScreenCast client: `CreateSession`, `SelectSources` types=1 (monitor), `Start`, the response signals through zbus on a worker; the PipeWire node id and fd), `selenita/src/record.rs` (worker; the spike's winner: the `gstreamer` crate if it builds and links with cxx-qt and `mp4mux` is found at runtime, else `gst-launch-1.0` as a child stopped with SIGINT — decision recorded BEFORE implementing, as Cuprita's audio spike was), `selenita/qml/RecordingCard.qml`, `selenita/tests/qml/tst_recording.qml` (fake portal + fake recorder), `selenita/src/main.rs` (`--record`/`--stop` → `ToggleRecording()`), README (runtime dependency `gst-plugins-good` and its check at start with a notice)

- [ ] **Step 1: Spike** (≤ 1 h, throwaway): the `gstreamer` crate in a scratch example; `gst-inspect-1.0 mp4mux` presence after `pacman -S gst-plugins-good` — do NOT install packages: if `mp4mux` is absent on the host, record that the author must install `gst-plugins-good` and build the pipeline builder against the element names anyway; test the pipeline with `videotestsrc` to a scratch MP4 only if the muxer exists. Decision in the evidence.
- [ ] **Step 2: Failing crate tests** (builders, state machine). Implement.
- [ ] **Step 3: Portal client and the worker**; QML card with elapsed time, red dot, the audio switch, the last recording's path and «Abrir en Fluorita»; `--record`/`--stop`.
- [ ] **Step 4: Checks, ledger, evidence (no recording performed by the subagent beyond `videotestsrc` to a scratch file; VAL-SEL-REC pending); commit** `selenita-maintenance: Add screen recording through the portal and GStreamer`.

---

### Task 8: SEL-1-C — exit at 1.0

Keyboard (`1/2/3`, Enter, `R`, Delete, arrows) and accessibility pass with `tst_keyboard.qml`; STATUS/ROADMAP/VALIDATION; evidence; commit `selenita-maintenance: Close the foundation at 1.0 with the keyboard and accessibility pass`; landing `--kind release`.

---

### Task 9: EXT-1-C — the program's exit (suite)

- `siderita/src/suite.rs` + tests: Calcita as an «Abrir en» target for `.pdf` files (MIME `application/pdf` through the existing MIME path); `HOST-HYGIENE.md`: the browser no longer holds the PDF role once the author pins `application/pdf` (Decision line, author's action), Selenita holds the capture role (`grim`/`slurp` stay as its tools; note niri's own screenshot UI remains available); ADR 0012 follow-up paragraph (two new activation names, Selenita's extra interface) — or ADR 0013 if a new decision arose in the spike; root README table rows; ROADMAP checklist; evidence; the plan archived by the author's hand commit after the landing.
- Commit `suite-maintenance: Add Calcita to the open-in targets and close the reading and capture program`.

---

## Self-review

- Spec coverage: §4 → Tasks 1–4; §5 → Tasks 5–8; §6 phases → Tasks 1–9 in order; §7 verification → crate/QML/smoke per task, VAL entries for the author; §8 decisions honoured (QtPdf, one document per window, tools orchestrated, portal + `gst-plugins-good`, no notifications, slurp with theme colours).
- Placeholders: none; the only open decision (GStreamer crate vs child) has a spike with a rule.
- Type consistency: `Geometry`/`Target`/`Capture` (Task 6) are what Task 7's pipeline and Task 8's keyboard use; `ZoomMode`/`GoTo` (Task 2) are what Task 4's reading state stores; the extra interface `org.celestina.Selenita1` is declared in Task 6 and extended in Task 7.
