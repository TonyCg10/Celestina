# Suite Conventions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the five applications behave as one system: a shared activation interface, a shared appearance source edited from Cuprita, open-with inside the suite, and drag-and-drop between the applications.

**Architecture:** A `celestina-core::activation` module (feature `activation`, zbus 5 blocking) owns the claim-first hand-off and serves `org.celestina.Application1` (`Activate()`, `Open(as paths)`) for every application; a `celestina-settings` crate owns `~/.config/celestina/appearance.toml` (reduced motion, text scale) with atomic writes and an inotify watcher that every window follows; Siderita's context menu and Magnetita's `--send` action give open-with inside the suite; three `DropArea`s give drag-and-drop.

**Tech Stack:** Rust 2021 (workspace toolchain), zbus 5, `notify-debouncer-full` 0.5, `toml` (check the workspace for the crate already used by `magnetitad`'s settings or the TOML reader of `scripts/`), cxx-qt 0.9.1, Qt 6.11 QML, celestina-style.

**Spec:** `docs/superpowers/specs/2026-10-09-suite-conventions-design.md`

## Global Constraints

- Program `CONV-1`; ledger `docs/plans/active/2026-10-09-suite-conventions.md` (rows CONV-1-A…F, created by Task 1; one row `active` at a time, the rest `planned`/`done`); evidence `docs/evidence/2026-10-09-<topic>.md` in the layout of `docs/evidence/2026-10-08-qmllint-order.md`; application units use their own ledgers only for CONV-1-C (`cuprita/docs/plans/active/…`).
- Prefixes: `suite:` for multi-application units (session commits `suite-maintenance:`), `cuprita:` for CONV-1-C; `celestina-settings:` registered as a component prefix of `celestina-rs` in `docs/projects.toml` by Task 2.
- Byte-exact paths (ADR 0008): `Open(as paths)` elements are `celestina_core::pathkey` encodings; `text/uri-list` decoded with `celestina_core::file_uri::to_path`; remote URIs ignored with a notice.
- No blocking IO on the Qt thread: every bus call and file read runs on a worker and reaches Qt through cxx-qt queues; typed errors; no production `unwrap`/`expect`; each dependency justified in `Cargo.toml`.
- Style: tokens only; text scale multiplies the nine font tokens only (`fontMini`…`fontDisplay`); layout tokens unchanged; `reducedMotion` semantics unchanged.
- The halted shell (`celestina/`, `celestina-shell-core`) is never touched; `org.celestina.Shell1` and the portal `Settings` backend are left alone.
- Spanish only in `qsTr()`/`message_es`; English development text; ratchets never grow (qmllint, radius, glass-canvas, architecture, language).
- Before every commit, in each touched application: `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test`, `cargo build --release --locked`; in `celestina-rs/`: `cargo test -p <crate>`; from the root: each touched application's `scripts/qml-tests.sh` and `scripts/smoke.sh`, `bash scripts/qmllint-cxxqt.sh <app>`, `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`; scope with `python3 scripts/commit_scope.py --check "<subject>"` (paths on stdin). Never `git stash`.
- A unit that changes a shared crate completes every application that consumes it (the landing rebuilds and verifies all of them).

---

### Task 1: CONV-1-A — shared activation

**Files:**
- Create: `celestina-rs/crates/celestina-core/src/activation.rs`, `celestina-rs/crates/celestina-core/tests/activation.rs`
- Modify: `celestina-rs/crates/celestina-core/Cargo.toml` (feature `activation = ["dep:zbus"]`, `zbus = { version = "5", default-features = false, features = ["async-io","blocking-api"], optional = true }` with a justification comment), `celestina-rs/crates/celestina-core/src/lib.rs`
- Modify (adoptions): `grafita/src/activation.rs` (delete; callers in `grafita/src/main.rs` and the controller that served `OpenDocument`), `hematita/src/activation.rs` (delete; `hematita/src/main.rs`), `cuprita/src/controller/activation.rs` (delete; `cuprita/src/main.rs`, `cuprita/qml/Main.qml` `activation.start()`), `siderita/src/main.rs` + a new `siderita/src/activation.rs` adapter (`Open` → `window.openTab` per folder, parent folder with the file selected for a file), `fluorita/src/main.rs` + `fluorita/src/activation.rs` (reworked: single instance; `Open` → play first, queue rest; folder → library's current folder), each application's `Cargo.toml` (`celestina-core` gains `features = ["activation"]`)
- Modify: `siderita/src/editor.rs:249`, `siderita/src/media.rs:264`, `siderita/src/usage.rs:313` (open through the bus first), `siderita/src/apps.rs` (helper `open_in`)
- Create: `scripts/activation_contract.py` scanner + a line in `scripts/check-architecture-contract.sh` + `scripts/test-activation-contract.py`
- Docs: `docs/plans/active/2026-10-09-suite-conventions.md` (new ledger with rows A active, B–F planned), `docs/evidence/2026-10-09-shared-activation.md`, each application's `STATUS.md`/`README.md` line on activation, `hematita/docs/plans/active/2026-09-26-hardening.md` HEM-16 row → done by reference

**Interfaces:**
- Produces (crate, feature `activation`):

```rust
pub struct ActivationName(pub &'static str);                 // "org.celestina.Grafita"
pub enum ActivationError { Bus(String), Timeout, Refused(String) }
pub enum Claim { Owner(Owner), HandedOff, Unsettled(ActivationError) }
pub trait Activatable: Send + 'static {
    fn activate(&self);                  // queue a raise to the Qt thread
    fn open(&self, paths: Vec<PathBuf>); // queue "open these" to the Qt thread
}
pub fn claim(name: ActivationName, served: Box<dyn Activatable>, argv_paths: &[PathBuf]) -> Claim;
pub struct Owner { /* keeps the connection; */ }
impl Owner { pub fn attach(&self); }     // replays the bounded inbox (16) once Qt is ready
pub fn open_in(name: ActivationName, paths: &[PathBuf], timeout: Duration) -> Result<bool, ActivationError>;
    // true = an owner answered; false = nobody owns the name (caller spawns)
pub const INTERFACE: &str = "org.celestina.Application1";
pub fn object_path(name: &ActivationName) -> String;          // "/org/celestina/Grafita"
```

  D-Bus: `Activate()`, `Open(as paths)` (pathkey strings); the served object replies immediately and queues.

- [ ] **Step 1: Failing crate tests** (`tests/activation.rs`, pure parts): `object_path(&ActivationName("org.celestina.Grafita")) == "/org/celestina/Grafita"`; the inbox replays in order and caps at 16 (oldest dropped, counted); `decide(argv_paths_empty, owner_present) -> HandOff::Activate | HandOff::Open | HandOff::Serve`. Run `cd celestina-rs && cargo test -p celestina-core --features activation` → FAIL.
- [ ] **Step 2: Implement** `activation.rs` from Hematita's claim-first version (`hematita/src/activation.rs`, HEM-H1-F) in the crate's own words: connection builder with `method_timeout(3 s)` for the hand-off call, `request_name_with_flags(DoNotQueue)`, the served object, the inbox, `attach()`. Tests pass. A bus-level test runs under `dbus-run-session` when available (skip with a message otherwise): claim twice from two threads → one Owner, one HandedOff, the owner's `open` receives the paths.
- [ ] **Step 3: Adopt** in Grafita, Hematita, Cuprita (delete the copies; `Open` wired to their existing open paths; Cuprita ignores paths), Siderita (new name; `Open` semantics above; `FileManager1` untouched), Fluorita (single instance + `Open`). Each application's QML tests gain one test that an `Open` queued through the adapter reaches the controller (over the existing stubs). `%U`/`%F` in `.desktop` unchanged.
- [ ] **Step 4: Siderita's activator** calls `open_in` on its worker (3 s) and spawns only on `Ok(false)`; the spawn passes the paths. Unit test of the decision with a fake `open_in` result.
- [ ] **Step 5: Scanner** `scripts/activation_contract.py`: for every registered `cxx-qt-application`, refuse a `request_name` / `request_name_with_flags` call outside `celestina-rs/crates/celestina-core/src/activation.rs` except Siderita's `dbus.rs` (`FileManager1`) and `portal.rs` (allowlist in the script with the reason); wire into `check-architecture-contract.sh`; `test-activation-contract.py` with a fixture tree.
- [ ] **Step 6: Checks** per Global Constraints for all five applications; ledger, evidence (with the `dbus-run-session` transcript), STATUS lines; commit `suite-maintenance: Add the shared activation interface and adopt it in every application`.

---

### Task 2: CONV-1-B — shared appearance

**Files:**
- Create: `celestina-rs/crates/celestina-settings/{Cargo.toml, src/lib.rs, src/appearance.rs, src/watch.rs, tests/appearance.rs, tests/watch.rs}`
- Modify: `celestina-rs/Cargo.toml` (member), `docs/projects.toml` (component prefix `celestina-settings` under `celestina-rs`, and `production_inputs` of the six applications gain the crate), `celestina-style/CelestinaTheme.qml` (`textScale`, scaled font tokens), each application's controller + `Main.qml` (feed `reducedMotion`/`textScale` from the watcher; delete the six `CELESTINA_REDUCED_MOTION` reads in favour of the crate), each application's `Cargo.toml`
- Docs: ledger row B, evidence `docs/evidence/2026-10-09-shared-appearance.md`, `celestina-style/DESIGN.md` paragraph on text scale

**Interfaces:**
- Produces:

```rust
pub enum TextScale { Compact, Normal, Large, Larger }   // factors 0.9, 1.0, 1.15, 1.3; toml strings "compact" | "normal" | "large" | "larger"
impl TextScale { pub fn factor(self) -> f64; }
pub struct Appearance { pub reduced_motion: bool, pub text_scale: TextScale }   // Default: false, Normal
pub fn path() -> PathBuf;                                 // config_home/celestina/appearance.toml
pub fn load() -> Appearance;                              // defaults on missing/malformed (logged), env override applied
pub fn save(value: &Appearance) -> Result<(), SettingsError>;   // atomic, keeps unknown lines
pub fn env_forces_reduced_motion() -> bool;               // CELESTINA_REDUCED_MOTION present
pub fn watch(on_change: impl Fn(Appearance) + Send + 'static) -> Result<WatchHandle, SettingsError>;
```

  QML: `CelestinaTheme.textScale: real` (default 1.0); font tokens `fontX = Math.round(fontXBase * textScale)`; controllers expose `appearanceReducedMotion: bool`, `appearanceTextScale: real`.

- [ ] **Step 1: Failing tests** `tests/appearance.rs`: defaults when missing; round trip of both keys; an unknown key and a comment survive `save`; a malformed file yields defaults and is not overwritten by `load`; env override forces `reduced_motion = true`; factor table. `tests/watch.rs`: writing the file in a temp config home calls back once with the new value within 2 s; an unchanged rewrite does not call back. Run → FAIL.
- [ ] **Step 2: Implement** the crate (line-based TOML rewrite of the two keys; `notify-debouncer-full` 300 ms on the directory; a `CELESTINA_CONFIG_HOME`-style override only if `celestina_core::xdg` already offers one for tests — otherwise use `XDG_CONFIG_HOME` in the tests).
- [ ] **Step 3: Theme**: `textScale` + scaled tokens; `celestina-style`'s own tests (`tst_*`) gain a check that `fontBody` at 1.3 is `Math.round(13 * 1.3)`.
- [ ] **Step 4: Applications**: one worker per application starts `watch` and queues both values; `Main.qml` binds `CelestinaTheme.reducedMotion`/`textScale`; a QML test per application that a queued appearance change reaches the theme (over the existing stubs).
- [ ] **Step 5: Checks** for the six applications and celestina-style; ledger, evidence; commit `suite-maintenance: Add the shared appearance file with the text scale and feed every window from it`.

---

### Task 3: CONV-1-C — Cuprita's appearance section

**Files:**
- Create: `cuprita/qml/pages/AppearancePage.qml`, `cuprita/src/controller/appearance.rs`, `cuprita/tests/qml/tst_appearance_page.qml`, `cuprita/tests/qml/fakes/FakeAppearanceController.qml`
- Modify: `cuprita/qml/Sections.qml` (fourth entry), `cuprita/qml/SectionShortcuts.qml` (Ctrl+4), `cuprita/qml/Main.qml`, `cuprita/build.rs`, `cuprita/src/backend.rs` (a fake appearance store under `CUPRITA_FAKE=1`), `cuprita/README.md`, `cuprita/STATUS.md`, `cuprita/VALIDATION.md` (VAL-F), ledger `cuprita/docs/plans/active/2026-10-08-cup-1-foundation.md` row CUP-1-I (this unit is also CONV-1-C in the suite ledger: both rows, each saying so), evidence `cuprita/docs/evidence/2026-10-09-appearance.md`

**Interfaces:**
- Consumes `celestina_settings::{load, save, watch, env_forces_reduced_motion, TextScale}`.
- Produces `AppearanceController { reducedMotion: bool, textScale: string ("compact"…), forcedByEnvironment: bool, loaded: bool; invokables setReducedMotion(bool), setTextScale(string); signal notice(kind, text) }`.

- [ ] **Step 1: Failing QML tests**: the page shows the switch and four choices; choosing "large" calls `setTextScale("large")`; with `forcedByEnvironment` the switch is on and disabled and the hint is shown; the loading row shows until `loaded`.
- [ ] **Step 2: Implement** the page in the CUP-1-H grammar (`SectionCard`, `SettingRow` with `accessibleName`, a segmented choice: `CelestinaSegmentedControl` if present in celestina-style, else four `CelestinaButton`s with the selected emphasis), the controller over the crate on the worker (save on change; the watcher feeds the properties back), keyboard: Space toggles, Left/Right move the choice.
- [ ] **Step 3: Checks** (Cuprita's full set), ledger rows, evidence, VAL-F steps; commit `cuprita-maintenance: Add the appearance section with reduced motion and the text size`.

---

### Task 4: CONV-1-D — open-with inside the suite

**Files:**
- Modify: Siderita's context menus (`siderita/qml/views/*ContextMenu*.qml` or `SidebarContextMenus.qml` — read where the folder-view menu is built), `siderita/src/apps.rs` (`suite_targets(selection) -> Vec<Target>`; `send_to_phone(device_id, paths)` through `org.celestina.Magnetita` `Devices1.SendFile`, listing devices with the existing proxy in `siderita/src/devices.rs`), `siderita/tests/qml/` (menu test over a stub), `siderita/README.md`
- Modify: `magnetita/org.celestina.Magnetita.desktop` (`[Desktop Action send]`, `MimeType`), `magnetita/src/main.rs` (`--send PATH…`: one device → `SendFile` each and exit; several → the device chooser window with the paths preselected), `magnetita/README.md`, `magnetita/tests/` (a unit test of the argv parsing and the one-device decision)
- Docs: ledger row D, evidence `docs/evidence/2026-10-09-open-with.md`

**Interfaces:**
- Consumes Task 1's `open_in`; Magnetita's `Devices1.ListDevices`/`SendFile` (read the daemon's interface in `celestina-rs/crates/magnetitad/src/devices.rs:564-770` for exact signatures).
- Produces: Siderita menu section «Abrir en» with entries Grafita (files), Fluorita (media/images/folders), Hematita (folders), send-to-phone (one entry, or one per device), each hidden when it does not apply or the target is not installed (`desktop_entry::find`).

- [ ] **Step 1: Failing tests**: `suite_targets` for a text file, an image, a folder, a mixed selection; `--send` parsing; QML: the section lists the expected entries for a stubbed selection.
- [ ] **Step 2: Implement** both sides.
- [ ] **Step 3: Checks** for Siderita and Magnetita; ledger, evidence; commit `suite-maintenance: Add the open-in entries of the suite to Siderita and the send action to Magnetita`.

---

### Task 5: CONV-1-E — drag-and-drop between applications

**Files:**
- Modify: `grafita/qml/Main.qml` or the document view (a `DropArea` with `keys: ["text/uri-list"]` → `controller.openDropped(uris)` → the same open path as `Open`), `grafita/tests/qml/`; `fluorita/qml/Main.qml` (`DropArea` → play/queue, folder → source dialog prefilled, image → viewer), `fluorita/tests/qml/`; `magnetita/qml/` device rows and page (`DropArea` → `SendFile`), `magnetita/tests/qml/`; each application's controller gains `openDropped(uris: QStringList)` decoding through `celestina_core::file_uri::to_path` on the worker and ignoring remote URIs with a notice
- Docs: ledger row E, evidence `docs/evidence/2026-10-09-drag-and-drop.md`

- [ ] **Step 1: Failing tests**: per application, a QML test that a simulated drop (`TestCase.drop`-style helper or calling the `DropArea`'s handler with a fake `drop` object holding `urls`) calls `openDropped` with the URIs; a Rust unit test that `file:///tmp/a%20b` decodes to the byte path and `smb://x/y` is ignored.
- [ ] **Step 2: Implement** the three drop areas with the suite's drop highlight (reuse Siderita's `DropArea` look: read `siderita/qml/views/FolderView.qml:473-492` and give the style a `CelestinaDropHighlight` only if Siderita's can be lifted unchanged; otherwise keep it local).
- [ ] **Step 3: Checks** for the three applications; ledger, evidence; commit `suite-maintenance: Add drag-and-drop from Siderita into Grafita, Fluorita and Magnetita`.

---

### Task 6: CONV-1-F — exit

**Files:**
- Create: `docs/decisions/0012-suite-conventions.md` (activation interface, appearance file, open-with, drag-and-drop; the decisions of spec §11)
- Modify: every application's `STATUS.md`/`ROADMAP.md` (the conventions as adopted; VAL entries for the author's live checks of spec §10), `docs/plans/active/2026-10-09-suite-conventions.md` (rows A–E done, F active), `README.md` (one paragraph "how the applications work together")
- Docs: evidence `docs/evidence/2026-10-09-suite-conventions-exit.md`

- [ ] **Step 1: ADR** in the layout of `docs/decisions/0011-seal-at-landing.md`.
- [ ] **Step 2: Documents** as listed; the author's live checks written with exact steps and marked pending.
- [ ] **Step 3: Checks** (documentation, language, architecture); commit `suite-maintenance: Record the suite conventions and close the program`.

---

## Self-review

- Spec coverage: §4 → Task 1; §5 → Task 2; §6 → Task 3; §7 → Task 4; §8 → Task 5; §9/§11 → Task 6; §10 verification → each task's tests plus the author's VAL entries in Task 6; §2 constraints → Global Constraints.
- Placeholders: none; where a file name depends on reading the tree (Siderita's menu file, Magnetita's QML), the task says what to read.
- Type consistency: `open_in` (Task 1) is what Task 4 calls; `openDropped` (Task 5) reuses the `Open` path of Task 1; `Appearance`/`TextScale` (Task 2) are what Task 3 consumes; `SettingRow.accessibleName` exists since CUP-1-H.
