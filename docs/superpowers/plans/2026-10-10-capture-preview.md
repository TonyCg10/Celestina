# Capture Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After every Selenita capture or recording, show a corner preview that can be dragged out as a file or clicked into a floating Fluorita editor, which saves both versions or only the edited one and can trim a video frame-accurately.

**Architecture:** Selenita owns the preview (a second, frameless top-level window placed by a niri window rule) because it holds the file the moment it exists; Fluorita owns editing (its picture editor already exists) and gains its own interface `org.celestina.Fluorita1.Edit(path)`, a floating edit window on any path, and a video trim run by an `ffmpeg` child. The two applications meet only at `Fluorita1.Edit` and `Selenita1.Adopt`, both fixed by the first suite unit.

**Tech Stack:** Rust (workspace toolchain), cxx-qt 0.9.1, Qt 6.12 QML, celestina-style, celestina-core (activation, pathkey, file_uri, atomic_file), selenita-core (`History`, runner, tools), fluorita-core/engine (editor, libmpv player), zbus 5, `gst-launch-1.0` (Selenita's poster frame), `/usr/bin/ffmpeg` from the `ffmpeg` package libmpv already needs.

**Spec:** `docs/superpowers/specs/2026-10-10-capture-preview-design.md`

## Global Constraints

- Program `PRV-1`; suite ledger `docs/plans/active/2026-10-10-capture-preview.md` (rows PRV-1-A, PRV-1-E; created by Task 1, which also opens the root ROADMAP on `PRV-1` as EXT-1-A opened `EXT-1`); application ledgers `selenita/docs/plans/active/2026-10-10-sel-2-preview.md` (row SEL-2-A) and `fluorita/docs/plans/active/2026-10-10-flu-p1-preview.md` (rows FLU-P1-A, FLU-P1-B), each created by its first unit.
- Before Task 2 the author archives `selenita/docs/plans/active/2026-10-09-sel-1-foundation.md` by hand; before Task 3, `fluorita/docs/plans/active/2026-09-26-hardening.md` (every row of both is done). A unit never moves its own plan.
- Prefixes: `suite:` for PRV-1-A and PRV-1-E (session commits `suite-maintenance:`); `selenita:`/`fluorita:` for application units (session commits `<app>-maintenance:`). Commit subjects start with a verb `scripts/commit_scope.py` accepts.
- Zero new external dependencies: no layer-shell-qt, no new crate that links a media library; the preview is placed by a niri window rule; the trim runs `/usr/bin/ffmpeg` as a child (never linked).
- ADR 0012: activation through `celestina_core::activation`; paths cross D-Bus as pathkeys, byte-exact; new literals `org.celestina.Fluorita1` and `Adopt` on `org.celestina.Selenita1` join `scripts/activation_contract.py`'s `ALLOWED_LITERALS` with their reasons.
- ADR 0009 amended (Task 4) for one operation only: a video's duration trim, raster-class, by an `ffmpeg` child.
- Window titles niri matches: Selenita's preview `qsTr("Vista previa")` exactly; Fluorita's editor `qsTr("Editar — %1")` (em dash, file name). App ids `org.celestina.Selenita` and `org.celestina.Fluorita`.
- Save outcome labels: «Guardar ambas» (copy beside, default) and «Guardar solo la editada» (replace; the original to the Trash). Close-with-changes asks those two plus «Descartar».
- Preview timings: visible 5 s, the timer stops while hovered and restarts on leave; 200 ms fade and slide, immediate with `reducedMotion`; about 240 px wide, height clamped 140–240 px; drag offers `Qt.CopyAction` only.
- Style: tokens only, glass canvas, motion 100–500 ms honouring `reducedMotion`, icon-first actions, Spanish only in `qsTr()`, English development text; no blocking IO on the Qt thread; typed errors with `message_es`; no production `unwrap`/`expect`; each dependency justified in `Cargo.toml`.
- Ratchets never grow (qmllint per application: selenita 1, fluorita 56; radius; glass-canvas; architecture; language). Host Qt 6.12.0. A `for` loop inside a QML handler silences qmllint: never use it to pass the ratchet.
- Before every commit: in the application `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test`, `cargo build --release --locked`; `cargo test -p <app>-core` in `celestina-rs/`; from the root `sh <app>/scripts/qml-tests.sh` (×3), `sh <app>/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/<app>`, `bash scripts/qmllint-cxxqt.sh <app>`, `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/test-activation-contract.py`, `python3 scripts/commit_scope.py --check "<subject>"` (paths on stdin). Never `git stash`.
- Subagents verify offscreen with fixtures and fakes (`SELENITA_FAKE=1`, Fluorita's fixtures); live checks (the corner, a real drag into WhatsApp or Slack, a real edit) are VAL entries for the author, never performed by a subagent; no real screenshot or recording.

---

### Task 1: PRV-1-A — the program's interfaces (suite)

**Files:**
- Modify: `docs/decisions/0012-suite-conventions.md` (a `PRV-1 (2026-10-10)` bullet under `## Follow-ups`: Fluorita serves `org.celestina.Fluorita1` with `Edit(path)` beside the shared interface on its activation connection; Selenita's `org.celestina.Selenita1` gains `Adopt(path)`; both take one pathkey string), `scripts/activation_contract.py` (`ALLOWED_LITERALS`: `("fluorita/src/activation.rs", "org.celestina.Fluorita1")` with its reason, `("selenita/src/preview.rs", "org.celestina.Fluorita1")` as a client of that interface, and `("fluorita/src/adopt.rs", "org.celestina.Selenita1")` as a client of `Adopt`; extend the Selenita1 reason with `Adopt`), `scripts/test-activation-contract.py` if it pins the allowlist's size, root `ROADMAP.md` (`Status: active`, `Active implementation checkpoint: PRV-1`, a `## PRV-1 — Capture preview` section with the five units as a checklist), root `STATUS.md`, `docs/plans/active/README.md`
- Create: suite ledger `docs/plans/active/2026-10-10-capture-preview.md` (rows PRV-1-A `active` with its evidence link, PRV-1-E `planned`; the application units named in prose with their ledgers), evidence `docs/evidence/2026-10-10-capture-preview-interfaces.md`; the spec and this plan are already committed in the worktree and land with this unit.

**Interfaces:**
- Produces (the contract the two application units implement, written in the ADR):
  - `org.celestina.Fluorita1` at Fluorita's activation object path (`celestina_core::activation::object_path(&FLUORITA)`), method `Edit(s key)` → `()`; `key` is `pathkey::encode(path)`. Errors: `org.freedesktop.DBus.Error.InvalidArgs` for a key that does not decode or is not a regular file.
  - `org.celestina.Selenita1.Adopt(s key)` → `()`; a regular file inside Selenita's pictures `Capturas` or videos `Recordings` folder joins the history (kind from the extension: `.png` screenshot, `.mp4` recording); anything else is ignored without error.

- [ ] **Step 1: Read the models** — `git show 4a118b0a --stat` and its ROADMAP/STATUS/active-plans hunks (how EXT-1-A opened `EXT-1`), `docs/decisions/0012-suite-conventions.md` `## Follow-ups`, `scripts/activation_contract.py` (`ALLOWED_LITERALS`), `scripts/test-activation-contract.py`.
- [ ] **Step 2: Failing scanner test** — add to `scripts/test-activation-contract.py` a case that a file `fluorita/src/activation.rs` containing the literal `"org.celestina.Fluorita1"` passes and the same literal in `grafita/src/x.rs` is reported. Run `python3 scripts/test-activation-contract.py` → the first case FAILS.
- [ ] **Step 3: Allowlist entries and reasons**; run the test → PASS; `bash scripts/check-architecture-contract.sh` → OK.
- [ ] **Step 4: ADR follow-up, ROADMAP, STATUS, active-plans README, suite ledger, evidence** (every command with its exit code).
- [ ] **Step 5: Checks** (`check-architecture-contract.sh`, `check-language-contract.py`, `check-documentation-contract.sh`, `test-activation-contract.py`, `commit_scope.py`), commit `suite-maintenance: Fix the capture preview's interfaces between Selenita and Fluorita`.

---

### Task 2: SEL-2-A — the corner preview (Selenita)

**Files:**
- Create: `selenita/src/preview.rs` (the `Edit` hand-off to Fluorita and the poster-frame extraction, both on the capture worker), `selenita/qml/PreviewWindow.qml`, `selenita/tests/qml/tst_preview.qml`, `celestina-rs/crates/selenita-core/src/poster.rs` + `tests/poster.rs` (the `gst-launch-1.0` argv for one frame), `selenita/docs/plans/active/2026-10-10-sel-2-preview.md` (row SEL-2-A), `selenita/docs/evidence/2026-10-10-preview.md`
- Modify: `selenita/src/controller.rs` (the preview state and invokables below; a launch that captures from a key binding no longer reveals the main window), `selenita/src/activation.rs` (`Adopt`), `selenita/src/capture.rs` (a `Report::Preview` after every publish, screenshot or recording), `selenita/qml/Main.qml` (instantiates `PreviewWindow`), `selenita/build.rs` (register the QML), `celestina-rs/crates/selenita-core/src/lib.rs`, `selenita/scripts/smoke.sh` (report `preview=shown` after the fake capture), README («La vista previa», the niri rule of spec §7), STATUS, VALIDATION (VAL-SEL-PREVIEW: the corner without focus; drag a capture and a recording into WhatsApp and Slack; click opens Fluorita once Task 3 is installed)

**Interfaces:**
- Consumes: `org.celestina.Fluorita1.Edit(s key)` and `Selenita1.Adopt(s key)` (Task 1); `selenita_core::history::{History, Entry, EntryKind}`; `celestina_core::activation::{FLUORITA, object_path, HAND_OFF_TIMEOUT}`.
- Produces:
  - crate `selenita_core::poster::poster_argv(video: &Path, out_png: &Path) -> Vec<OsString>` = `gst-launch-1.0 -q filesrc location=<video> ! qtdemux ! decodebin ! videoconvert ! pngenc snapshot=true ! filesink location=<out_png>`.
  - controller properties `previewVisible: bool`, `previewKey: QString` (pathkey), `previewKind: QString` (`"screenshot"`/`"recording"`), `previewSource: QUrl` (the picture, or the poster PNG for a recording), `previewDuration: QString` (`m:ss`, empty for a picture); invokables `dismissPreview()`, `editPreview()`; signal `previewShown()`.
  - `Selenita1.Adopt(key)` pushes an `Entry` and saves through `History`; with Selenita not running, nobody serves it (Fluorita writes the file itself in Task 3).

- [ ] **Step 1: Failing crate test** — `tests/poster.rs`: the argv for `/tmp/a b.mp4` and `/run/x.png` is exactly the tokens above with the paths as single `OsString`s (spaces kept). Run `cargo test -p selenita-core poster` → FAIL. Implement `poster.rs`. PASS.
- [ ] **Step 2: Failing QML tests** (`tst_preview.qml`, fake controller): appears on `previewShown`; closes 5 s later; `containsMouse` stops the timer and leaving restarts it; the × calls `dismissPreview()`; a click calls `editPreview()` and closes; a second `previewShown` replaces the content; the `Drag` has `mimeData["text/uri-list"]` equal to the file's URI and `supportedActions === Qt.CopyAction`; with `reducedMotion` the opacity is 1 at once; window `title === "Vista previa"` and `flags` frameless. Run `sh selenita/scripts/qml-tests.sh` → FAIL.
- [ ] **Step 3: Implement** `PreviewWindow.qml` (glass grammar, picture or poster with duration and film glyph, × icon button, `DragHandler` + `Drag.dragType: Drag.Automatic`, `HoverHandler`, `Timer` 5000 ms) and the controller side: `Report::Preview { path, kind }` after publish → for a recording, run `poster_argv` into `$XDG_RUNTIME_DIR/selenita/poster-<n>.png` through the runner (deadline 3 s; on failure show the film glyph alone) → set the properties → `previewShown`. A trashed entry whose path equals `previewKey` sets `previewVisible` false. Tests → PASS.
- [ ] **Step 4: Hand-off** — `editPreview()` sends `Job::Edit(path)` to the capture worker: `Fluorita1.Edit(pathkey)` on a session connection if `org.celestina.Fluorita` has an owner (`DBusProxy::name_has_owner`), else spawn `fluorita --edit <path>` detached (Selenita's runner with null stdio); failure → `notice("error", qsTr("No se ha podido abrir Fluorita"))`. Unit test over a fake hand-off records `Edit(key)` versus spawn.
- [ ] **Step 5: `Adopt`** on `Selenita1` (activation.rs + controller): a file under the `Capturas`/`Recordings` folders joins the history once; other paths ignored; test with a scratch home.
- [ ] **Step 6: No main window after a key-binding capture** — `reveal` no longer shows the window when the launch was `--screenshot`/`--record`; update `tst_main.qml` and smoke (`shown=false` stays after the capture; `preview=shown`).
- [ ] **Step 7: Docs, ledger, evidence; checks; commit** `selenita-maintenance: Add the corner preview with drag, edit and adopt`.

---

### Task 3: FLU-P1-A — the floating editor on any path (Fluorita)

**Files:**
- Create: `fluorita/qml/EditWindow.qml` (a top-level window per file), `fluorita/tests/qml/tst_edit_window.qml`, `fluorita/docs/plans/active/2026-10-10-flu-p1-preview.md` (rows FLU-P1-A, FLU-P1-B), `fluorita/docs/evidence/2026-10-10-floating-editor.md`
- Modify: `fluorita/src/activation.rs` (serve `org.celestina.Fluorita1` with `Edit` on the activation connection, as `selenita/src/activation.rs` serves `Selenita1`; a new `edit_requested(key)` signal), `fluorita/src/main.rs` (`--edit <path>`: claim, start without the library window, open one edit window; a second `--edit` while an instance runs calls `Fluorita1.Edit` and exits), `fluorita/src/editor.rs` (opening a plain path that the catalogue does not hold: the worker's stat/header read decides, the recipe store keyed by path as now), `fluorita/qml/Main.qml` (an `Instantiator` of `EditWindow` over the open edit keys; the library window stays as it is), `fluorita/qml/components/EditToolbar.qml` (the two save labels; the close-with-changes question), `fluorita/src/adopt.rs` (new): Selenita adoption after «Guardar ambas» (`Selenita1.Adopt(copy)` when `org.celestina.Selenita` is owned, else append through `selenita_core::history::History` — add `selenita-core` as a path dependency, justified), `fluorita/build.rs`, README (the editor's window and the niri rule of spec §7), STATUS, VALIDATION (VAL-FLU-EDIT-WINDOW)

**Interfaces:**
- Consumes: `Fluorita1.Edit(s key)` and `Selenita1.Adopt(s key)` (Task 1); `FluoritaEditor::open_item(key)` and the editor's existing save paths.
- Produces: `FluoritaActivation.edit_requested(key: QString)`; `EditWindow { property string key }` with `title: qsTr("Editar — %1").arg(fileName)`; editor invokables unchanged in name; save labels exactly «Guardar ambas» / «Guardar solo la editada»; `EditWindow` exposes `property bool video` that Task 4 fills.

- [ ] **Step 1: Read** `fluorita/src/editor.rs` (`open_item`, how a key resolves to a file, the worker's open measurement), `fluorita/src/activation.rs`, `selenita/src/activation.rs` (how `Selenita1` is served beside the shared interface), `fluorita/qml/Main.qml`'s editor wiring.
- [ ] **Step 2: Failing Rust tests** — `Edit` with a key outside every library root resolves to an openable picture; a key that is not a regular file is `InvalidArgs`; `--edit` parses one path. FAIL → implement → PASS.
- [ ] **Step 3: Failing QML tests** (`tst_edit_window.qml` over a fixture PNG in a scratch folder): the window's title; the editor surface loads the picture; unchanged close closes without a question; changed close shows the three choices; «Guardar ambas» writes the copy beside and keeps the original; «Guardar solo la editada» replaces and moves the original to a scratch Trash; after «Guardar ambas» the adoption hook is called with the copy's key. FAIL → implement → PASS.
- [ ] **Step 4: Drag out of the edit window** after a save (`text/uri-list`, copy only); test.
- [ ] **Step 5: Docs, ledger, evidence; checks; commit** `fluorita-maintenance: Add the floating editor that opens any path`.

---

### Task 4: FLU-P1-B — trimming a video (Fluorita)

**Files:**
- Create: `celestina-rs/crates/fluorita-core/src/trim.rs` + `tests/trim.rs` (the span model and the `ffmpeg` argv), `fluorita/src/trim.rs` (the worker: run, progress, cancel, publish), `fluorita/qml/components/TrimBar.qml`, `fluorita/tests/qml/tst_trim.qml`, `fluorita/tests/fixtures/three-seconds.mp4` (generated once by a checked-in script `fluorita/tests/fixtures/make-three-seconds.sh` with `ffmpeg -f lavfi -i testsrc=duration=3:size=320x240:rate=30 -f lavfi -i sine=duration=3 -c:v libx264 -crf 30 -c:a aac`), evidence `fluorita/docs/evidence/2026-10-10-video-trim.md`
- Modify: `fluorita/qml/EditWindow.qml` (`video: true` shows the player + `TrimBar` instead of the picture editor), `fluorita/src/editor.rs` or a sibling controller (`trimStart`, `trimEnd`, `saveTrim(outcome)`), `docs/decisions/0009-editing-without-an-encoder.md` (the amendment: video duration trim only, raster-class, by an `ffmpeg` child never linked; pictures unchanged), README, STATUS, VALIDATION (VAL-FLU-TRIM), ledger row FLU-P1-B

**Interfaces:**
- Consumes: `EditWindow.video` and the save outcomes (Task 3).
- Produces: crate `pub struct Span { pub start: Duration, pub end: Duration }` with `Span::new(start, end, length) -> Result<Span, TrimError>` (`start < end <= length`, at least one frame), `Span::is_whole(length) -> bool`; `pub enum VideoEncoder { Vaapi { device: PathBuf }, X264 }` with `choose(render_node: Option<PathBuf>, has_h264_vaapi: bool)`; `pub fn trim_argv(input: &Path, output: &Path, span: Span, encoder: &VideoEncoder) -> Vec<OsString>` = `ffmpeg -hide_banner -nostdin -y -ss <start> -to <end> -i <input>` + video (`-vaapi_device <dev> -vf format=nv12,hwupload -c:v h264_vaapi -qp 20` or `-c:v libx264 -crf 21 -preset veryfast`) + `-c:a aac -b:a 160k -movflags +faststart -progress pipe:1 <output>`; `TrimError` with `message_es`.

- [ ] **Step 1: Failing crate tests** — `Span::new(1s, 1s, 3s)` is an error; `Span::new(0, 3s, 3s).is_whole(3s)`; `trim_argv` for x264 equals the exact token list above with `-ss 1.000 -to 2.000`; VA variant includes `-vaapi_device /dev/dri/renderD128`. FAIL → implement → PASS.
- [ ] **Step 2: Fixture script and fixture**, committed.
- [ ] **Step 3: Failing worker test** — trimming the fixture to [1.0 s, 2.0 s] with the x264 encoder writes a hidden file, publishes it, and `ffprobe -v error -show_entries format=duration` reads 1.0 ± 0.034; cancel mid-run leaves no file; a missing `ffmpeg` (PATH emptied) is a `TrimError` and the original is untouched. FAIL → implement (`Command` with null stdin, `-progress` parsed on a thread, hidden output `.<name>.trim-<pid>.mp4`, rename on exit 0) → PASS.
- [ ] **Step 4: Failing QML tests** (`tst_trim.qml`): two handles, the start cannot pass the end; moving a handle seeks; whole span disables save; saving calls `saveTrim` with the chosen outcome. FAIL → implement → PASS.
- [ ] **Step 5: ADR 0009 amendment, docs, ledger, evidence; checks; commit** `fluorita-maintenance: Add the frame-accurate video trim through ffmpeg`.

---

### Task 5: PRV-1-E — the program's exit (suite)

**Files:**
- Modify: the author's `~/.config/niri/config.kdl` — only after the author approves, outside the repository — with the two window rules of spec §7 (backup to the scratchpad first; `niri validate` before and after must report the same pre-existing errors); `HOST-HYGIENE.md` (the preview and edit roles; the parked SEL-1-F addendum from the scratchpad `sdd-ext/host-hygiene-sel1f.patch`; ffmpeg's role recorded as already present through libmpv), root `README.md`, root `ROADMAP.md` checklist (PRV-1 units ticked except PRV-1-E), suite ledger row PRV-1-E, evidence `docs/evidence/2026-10-10-capture-preview-exit.md`

**Interfaces:**
- Consumes: everything above, landed.

- [ ] **Step 1: Window rules** — print the two rules for the author; after approval append them, `niri validate`, and record the diff in the evidence.
- [ ] **Step 2: HOST-HYGIENE, README, ROADMAP, ledger, evidence.**
- [ ] **Step 3: Checks; commit** `suite-maintenance: Close the capture preview program`.
- After the landing (controller): the author archives the PRV-1 suite plan and the two application plans by hand.
