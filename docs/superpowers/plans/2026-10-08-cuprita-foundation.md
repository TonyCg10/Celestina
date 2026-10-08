# Cuprita Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Cuprita 1.0, the suite's control centre for network, Bluetooth and audio, so nm-applet, Blueman and pavucontrol hold no remaining role.

**Architecture:** A pure domain crate `celestina-rs/crates/cuprita-core` exposes three traits (`Network`, `Bluetooth`, `Audio`) with fake and real implementations (NetworkManager and BlueZ over zbus on the system bus, PipeWire through the binding chosen by a spike); a CXX-Qt adapter in `cuprita/src` holds one controller per section with list models fed by snapshot diffing on worker threads; the QML surface in `cuprita/qml` is a three-section strip in Hematita's layout over the shared style.

**Tech Stack:** Rust 2021 (workspace toolchain `celestina-rs/rust-toolchain.toml`), cxx-qt 0.9.1 (pinned like Hematita), Qt 6.11 QML, zbus 5, celestina-style, `pipewire` crate or `wpctl` (decided in Task 6).

**Spec:** `docs/superpowers/specs/2026-10-08-cuprita-design.md`

## Global Constraints

- Identifiers: project `cuprita`, crate `cuprita-core`, commit prefix `cuprita` (session commits `cuprita-maintenance:`), desktop id `org.celestina.Cuprita`, D-Bus single-instance name `org.celestina.Cuprita`, unit prefix `CUP`.
- Pure domain in `celestina-rs/crates/cuprita-core` (no Qt, no D-Bus in the trait layer); Qt adaptation only in `cuprita/src`; presentation only in `cuprita/qml`; every QML file registered in `cuprita/build.rs`.
- No blocking IO on the Qt thread; every D-Bus or PipeWire call runs on a worker thread and reports back through cxx-qt `Threading` queues.
- Typed errors (`NetworkError`, `BluetoothError`, `AudioError`), each with `message_es(&self) -> String`; no production `unwrap`/`expect`; each dependency justified in a `Cargo.toml` comment. New dependencies allowed: `zbus = "5"` (already in the workspace) and `pipewire` only if Task 6's spike chooses it.
- Secrets never persist in Cuprita: a Wi-Fi password is passed inside `AddAndActivateConnection2` and dropped; Bluetooth codes go straight to BlueZ through the agent.
- No daemon, no tray, no indicator, no shell work (`celestina/` is halted; never touch it).
- Style: transparent window `color: CelestinaTheme.clear` with a `CelestinaBackdrop`; every value from `CelestinaTheme` tokens; lists use `CelestinaWheelScroll`; dialogs use `CelestinaModalLayer` + `GlassCard`; popup menus use `GlassContextMenu` with `popupBeside`; icon-first actions with the shared hover circle; no tooltips; motion 100–500 ms honouring `reducedMotion`.
- Copy: Spanish product copy only inside `qsTr()`; English development text everywhere else (`python3 scripts/check-language-contract.py`).
- Ratchets never grow: qmllint (`scripts/qmllint-baseline.tsv`), radius, glass-canvas, architecture, language.
- Before every commit, from the project: `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test`, `cargo build --release --locked`; then from the root: `sh cuprita/scripts/smoke.sh --binary <bin>`, `bash scripts/qmllint-cxxqt.sh cuprita`, `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`; scope with `python3 scripts/commit_scope.py --check "<subject>"` (paths on stdin). Never `git stash`.
- Each unit: a ledger row (`active`) in `cuprita/docs/plans/active/2026-10-08-cup-1-foundation.md` (created in Task 2), evidence `cuprita/docs/evidence/<date>-<topic>.md` in the layout of `hematita/docs/evidence/`, landed with `scripts/land-unit.py` by the controller.
- Structural model: Hematita (`hematita/`): copy its layout (crate + `src` + `qml` + `build.rs` + scripts + document set + strip), never its code verbatim.

---

### Task 1: AUD-1-P — register the project (suite unit)

**Files:**
- Modify: `docs/projects.toml` (append a `[[projects]]` entry after `hematita`)
- Modify: `celestina-rs/Cargo.toml` (workspace member), `celestina-rs/Cargo.lock`
- Create: `celestina-rs/crates/cuprita-core/Cargo.toml`, `celestina-rs/crates/cuprita-core/src/lib.rs`
- Modify: `scripts/qmllint-baseline.tsv`, `scripts/glass-canvas-baseline.tsv`, `scripts/radius-baseline.tsv` (a `0\tcuprita` row each, in the file's order)
- Modify: `README.md` (one line in the project list, after Hematita)
- Modify: `docs/plans/active/2026-09-26-monorepo-hardening.md` (ledger row `AUD-1-P`, `active`, after AUD-1-O)
- Create: `docs/evidence/2026-10-08-cuprita-registration.md`
- Commit (already written, uncommitted): `docs/superpowers/specs/2026-10-08-cuprita-design.md` and this plan

**Interfaces:**
- Produces: registry id `cuprita`, prefix `cuprita`, roots `cuprita/` and `celestina-rs/crates/cuprita-core/`; the crate `cuprita_core` (empty) in the workspace.

- [ ] **Step 1: Read the model** — `git show 28d1b263` (Hematita's registration) and the `hematita` entry in `docs/projects.toml:367-402`.

- [ ] **Step 2: Append the registry entry** to `docs/projects.toml`, after the `hematita` entry:

```toml

[[projects]]
id = "cuprita"
name = "Cuprita"
path = "cuprita"
kind = "cxx-qt-application"
commit_prefix = "cuprita"
# Registered unversioned first so the registry commit carries no history row;
# the baseline commit that follows the skeleton declares the Cargo source.
versioned = false
agents = "cuprita/AGENTS.md"
readme = "cuprita/README.md"
status = "cuprita/STATUS.md"
roadmap = "cuprita/ROADMAP.md"
validation = "cuprita/VALIDATION.md"
active_plans = "cuprita/docs/plans/active"
context_documents = ["celestina-style/DESIGN.md", "docs/superpowers/specs/2026-10-08-cuprita-design.md"]
source_roots = ["cuprita/src", "cuprita/qml", "celestina-rs/crates/cuprita-core"]
commit_roots = ["cuprita/", "celestina-rs/crates/cuprita-core/"]
include_workspace_manifests = true
production_role = "desktop-application"
deployable = true
build_script = "cuprita/scripts/build-production.sh"
toolchains = ["rust", "cxx", "qt"]
complete_script = "cuprita/scripts/complete-production.sh"
verify_script = "cuprita/scripts/verify-production.sh"
deploy_script = "cuprita/scripts/deploy-production.sh"
status_script = "cuprita/scripts/status-production.sh"
artifact_manifest = "cuprita/target/production-artifact.toml"
artifact_paths = ["cuprita/target/release/cuprita"]
cargo_manifests = ["cuprita/Cargo.toml"]
production_inputs = ["cuprita/Cargo.toml", "cuprita/Cargo.lock", "cuprita/build.rs", "cuprita/src", "cuprita/qml", "cuprita/org.celestina.Cuprita.desktop", "celestina-style/icons/apps/org.celestina.Cuprita.svg", "celestina-rs/crates/celestina-core", "celestina-rs/crates/cuprita-core"]
# cuprita-core gains tests/ in CUP-1-B; this list gains that path then.
verification_inputs = ["cuprita/scripts/smoke.sh", "celestina-rs/rust-toolchain.toml"]
component_commit_scopes = [
  { prefix = "cuprita-core", roots = ["celestina-rs/crates/cuprita-core/"], include_workspace_manifests = true },
]
```

- [ ] **Step 3: Crate stub.** `celestina-rs/crates/cuprita-core/Cargo.toml`:

```toml
[package]
name = "cuprita-core"
version = "0.1.0"
edition = "2021"
description = "Cuprita's domain: networks, Bluetooth devices and audio endpoints, without Qt"
license = "MIT"

[dependencies]
```

`src/lib.rs`:

```rust
//! Cuprita's domain. The traits, models and pure logic arrive in CUP-1-B;
//! this stub registers the crate in the workspace.
```

Add `"crates/cuprita-core",` to `members` in `celestina-rs/Cargo.toml` (alphabetical position after `celestina-shell-core`'s neighbours as the file orders them). Run `cd celestina-rs && cargo check -p cuprita-core --locked` — if `--locked` refuses, run `cargo check -p cuprita-core` once to update `Cargo.lock` and commit the lock change.

- [ ] **Step 4: Ratchet rows.** Add `0\tcuprita` to `scripts/qmllint-baseline.tsv`, `scripts/glass-canvas-baseline.tsv`, `scripts/radius-baseline.tsv`, keeping each file's row order (alphabetical where the file is alphabetical).

- [ ] **Step 5: README line** after Hematita's: `- **Cuprita** — control centre: network, Bluetooth and audio (\`cuprita/\`).` in the README's existing phrasing.

- [ ] **Step 6: Ledger row and evidence.** Row `AUD-1-P` after AUD-1-O in `docs/plans/active/2026-09-26-monorepo-hardening.md` (same columns; inventory `—`; intended change "Register the Cuprita project and its core crate, commit its design spec and plan"; evidence link). Evidence `docs/evidence/2026-10-08-cuprita-registration.md` in the layout of `docs/evidence/2026-10-08-qmllint-order.md`.

- [ ] **Step 7: Checks.** `python3 scripts/project_registry.py` (or the test that reads the registry: `python3 scripts/test-land-unit.py`), `bash scripts/check-documentation-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-architecture-contract.sh`, `bash scripts/test-production-common.sh`. Expected: all exit 0.

- [ ] **Step 8: Commit** `suite-maintenance: Register the Cuprita project and its core crate` (scope-check the paths first, including the spec and the plan).

---

### Task 2: CUP-1-A — application skeleton

**Files:**
- Create: `cuprita/Cargo.toml`, `cuprita/Cargo.lock`, `cuprita/build.rs`, `cuprita/rust-toolchain.toml` (copy Hematita's), `cuprita/src/main.rs`, `cuprita/src/controller.rs` (stub `CupritaController` with `reducedMotion`), `cuprita/qml/Main.qml`, `cuprita/qml/components/NavStrip.qml`, `cuprita/qml/components/NavItem.qml`, `cuprita/qml/pages/NetworkPage.qml`, `cuprita/qml/pages/BluetoothPage.qml`, `cuprita/qml/pages/AudioPage.qml` (each a titled empty `CelestinaSurface`), the style symlinks under `cuprita/qml/` exactly as Hematita has them (`ls -l hematita/qml | grep '^l'`), `cuprita/qml/fonts.qrc`, `cuprita/qml/icons.qrc` (symlinks as Hematita), `cuprita/org.celestina.Cuprita.desktop`, `celestina-style/icons/apps/org.celestina.Cuprita.svg` (a copper-coloured glyph in the family of the other app icons — note: this file is in celestina-style; the unit commits it under the `cuprita` prefix only if `commit_scope.py` allows; otherwise it is a one-file `celestina-style-maintenance:` commit in the same unit), `cuprita/scripts/{build,complete,verify,deploy,status}-production.sh`, `cuprita/scripts/smoke.sh`, `cuprita/scripts/qml-tests.sh` (copy Siderita's shape), `cuprita/AGENTS.md`, `README.md`, `STATUS.md`, `ROADMAP.md`, `VALIDATION.md`, `cuprita/docs/plans/active/2026-10-08-cup-1-foundation.md` (ledger with rows CUP-1-A…F, A `active`, the rest `planned`), `cuprita/docs/evidence/README.md`, `cuprita/docs/evidence/2026-10-08-skeleton.md`, `cuprita/tests/qml/tst_strip.qml`
- Modify: `docs/projects.toml` (`versioned = false` → `version_source`/`version_mirrors` as Hematita's), `docs/version-history.tsv` (the 0.1.0 row, as Hematita's baseline commit did — read `git log --format=%h -S'hematita' -- docs/version-history.tsv | tail -1` and mirror it)

**Interfaces:**
- Produces: `cuprita` binary (release at the shared target), `Main.qml` with `window.sections = [{id: "network", title: qsTr("Red"), icon: …}, {id: "bluetooth", …}, {id: "audio", …}]`, a `StackLayout` whose `currentIndex` follows the strip, `Ctrl+1/2/3` shortcuts; `CupritaController` registered as the QML singleton the pages read `reducedMotion` from.

- [ ] **Step 1: Copy Hematita's layout** (`hematita/Cargo.toml`, `build.rs`, `src/main.rs`, `qml/Main.qml`, `qml/components/Nav*.qml`, `scripts/*`, documents) renaming every identifier; replace the four Hematita pages with the three empty pages; keep `main.rs` to window creation plus the single-instance claim of `org.celestina.Cuprita` (zbus `request_name` with `DoNotQueue`; on `NameTaken`, call `org.celestina.Cuprita /org/celestina/Cuprita org.celestina.Cuprita Activate` on the owner and exit 0 — implement the tiny served interface with one method `Activate()` that raises the window via a cxx-qt queue).

- [ ] **Step 2: Write the failing QML test** `cuprita/tests/qml/tst_strip.qml`: load `Main.qml`'s strip component, assert `sections.length === 3`, that clicking the second item sets `currentIndex` to 1, and that `Ctrl+2` does the same. Run `sh cuprita/scripts/qml-tests.sh` → expect FAIL (component missing).

- [ ] **Step 3: Implement** the strip and pages until the test passes; `cargo build --release --locked` from `cuprita/`.

- [ ] **Step 4: Smoke** — `sh cuprita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/cuprita` constructs the three pages offscreen (`QT_QPA_PLATFORM=offscreen`) and exits 0.

- [ ] **Step 5: Documents.** `AGENTS.md` (Hematita's with Cuprita's roots and the privilege rule from the spec §2), `README.md` (what it is, how to run, the three backends it talks to), `STATUS.md` (0.1.0, skeleton), `ROADMAP.md` (the CUP-1 table from the spec §7), `VALIDATION.md` (the three live checks of spec §8), the ledger and the evidence.

- [ ] **Step 6: All checks** from Global Constraints; commit `cuprita-maintenance: Add the application skeleton with the crate stub, the strip and the document set`.

---

### Task 3: CUP-1-B — domain crate, fakes, controllers and page tests

**Files:**
- Create: `celestina-rs/crates/cuprita-core/src/{model.rs, network.rs, bluetooth.rs, audio.rs, error.rs, fake.rs, order.rs, agent.rs, volume.rs}`, `celestina-rs/crates/cuprita-core/tests/{order.rs, agent.rs, volume.rs, fakes.rs}`
- Create: `cuprita/src/controller/{mod.rs, network.rs, bluetooth.rs, audio.rs, notice.rs}`, `cuprita/src/models/{network_model.rs, device_model.rs, endpoint_model.rs, stream_model.rs}`, `cuprita/src/backend.rs` (chooses fakes when `CUPRITA_FAKE=1`, real clients otherwise — real clients are `todo!`-free stubs returning `Err(…Unavailable)` until Tasks 4–6)
- Modify: `cuprita/qml/pages/*.qml` (rows, switches, buttons bound to the controllers), `cuprita/qml/Main.qml` (notice pill), `cuprita/build.rs`, `cuprita/tests/qml/{tst_network_page.qml, tst_bluetooth_page.qml, tst_audio_page.qml}`, `docs/projects.toml` (`verification_inputs` gains `celestina-rs/crates/cuprita-core/tests`)

**Interfaces:**
- Produces (crate, `pub`):

```rust
// model.rs
pub enum NetworkKind { Ethernet, Wifi, Vpn }
pub enum NetworkState { Disconnected, Connecting, Connected, Failed }
pub enum Security { Open, Psk, Enterprise }
pub struct Network { pub id: String, pub kind: NetworkKind, pub name: String, pub state: NetworkState, pub signal: Option<u8>, pub security: Security, pub known: bool }
pub struct NetworkSnapshot { pub wifi_enabled: bool, pub airplane: bool, pub networks: Vec<Network> }
pub enum DeviceKind { Audio, Input, Phone, Computer, Other }
pub struct BluetoothDevice { pub address: String, pub name: String, pub kind: DeviceKind, pub paired: bool, pub connected: bool, pub trusted: bool, pub battery: Option<u8> }
pub struct BluetoothSnapshot { pub powered: bool, pub discovering: bool, pub devices: Vec<BluetoothDevice> }
pub enum EndpointKind { Sink, Source }
pub struct AudioEndpoint { pub id: u32, pub kind: EndpointKind, pub name: String, pub description: String, pub volume: f32, pub muted: bool, pub default: bool }
pub struct AudioStream { pub id: u32, pub app_name: String, pub app_icon: String, pub volume: f32, pub muted: bool, pub endpoint: u32 }
pub struct CardProfile { pub card_id: u32, pub id: String, pub description: String, pub active: bool }
pub struct AudioSnapshot { pub endpoints: Vec<AudioEndpoint>, pub streams: Vec<AudioStream>, pub profiles: Vec<CardProfile> }

// error.rs — each: pub enum with Unavailable, Denied, NotFound(String), Failed(String); impl message_es(&self) -> String
pub enum NetworkError { … }  pub enum BluetoothError { … }  pub enum AudioError { … }

// network.rs
pub trait Network: Send {
    fn snapshot(&mut self) -> Result<NetworkSnapshot, NetworkError>;
    fn set_wifi_enabled(&mut self, on: bool) -> Result<(), NetworkError>;
    fn set_airplane(&mut self, on: bool) -> Result<(), NetworkError>;
    fn connect(&mut self, id: &str, password: Option<&str>) -> Result<(), NetworkError>;
    fn disconnect(&mut self, id: &str) -> Result<(), NetworkError>;
    fn forget(&mut self, id: &str) -> Result<(), NetworkError>;
    fn set_vpn_active(&mut self, id: &str, on: bool) -> Result<(), NetworkError>;
}
// bluetooth.rs
pub trait Bluetooth: Send {
    fn snapshot(&mut self) -> Result<BluetoothSnapshot, BluetoothError>;
    fn set_powered(&mut self, on: bool) -> Result<(), BluetoothError>;
    fn set_discovering(&mut self, on: bool) -> Result<(), BluetoothError>;
    fn pair(&mut self, address: &str) -> Result<(), BluetoothError>;
    fn connect(&mut self, address: &str) -> Result<(), BluetoothError>;
    fn disconnect(&mut self, address: &str) -> Result<(), BluetoothError>;
    fn forget(&mut self, address: &str) -> Result<(), BluetoothError>;
}
// audio.rs
pub trait Audio: Send {
    fn snapshot(&mut self) -> Result<AudioSnapshot, AudioError>;
    fn set_default(&mut self, id: u32) -> Result<(), AudioError>;
    fn set_volume(&mut self, id: u32, volume: f32) -> Result<(), AudioError>;   // endpoint or stream id
    fn set_muted(&mut self, id: u32, muted: bool) -> Result<(), AudioError>;
    fn set_profile(&mut self, card_id: u32, profile: &str) -> Result<(), AudioError>;
}
// agent.rs — the pairing agent's state machine, pure
pub enum AgentRequest { Pin { device: String }, Confirm { device: String, passkey: u32 }, DisplayPasskey { device: String, passkey: u32 } }
pub enum AgentAnswer { Pin(String), Confirmed, Rejected, Cancelled }
pub struct Agent { … } impl Agent { pub fn new() -> Self; pub fn request(&mut self, r: AgentRequest) -> Result<(), AgentBusy>; pub fn answer(&mut self, a: AgentAnswer) -> Option<(AgentRequest, AgentAnswer)>; pub fn pending(&self) -> Option<&AgentRequest> }
// order.rs
pub fn order_networks(networks: &mut Vec<Network>);     // connected, then known, then by signal desc, then name
pub fn signal_bars(signal: u8) -> u8;                    // 0..=4: 0 for <5, 1 for <30, 2 for <55, 3 for <80, else 4
pub fn security_label_key(s: Security) -> &'static str; // "open" | "psk" | "enterprise" (QML translates)
// volume.rs
pub const VOLUME_MAX: f32 = 1.5;
pub fn clamp_volume(v: f32) -> f32;                      // 0.0..=VOLUME_MAX, NaN → 0.0
pub fn percent(v: f32) -> u8;                            // round(clamp(v)*100)
// fake.rs
pub struct FakeNetwork, FakeBluetooth, FakeAudio — `::scripted()` constructors with 3 networks (one connected Ethernet, one known PSK Wi-Fi at 72, one open Wi-Fi at 20, plus one VPN), 2 devices (paired headphones with battery 80, unpaired phone), 2 sinks + 1 source + 2 streams + a card with 2 profiles; every command mutates the scripted state so tests observe it.
```

- Produces (adapter): `NetworkController { wifiEnabled, airplane, busy; models: networks; invokables setWifiEnabled(bool), setAirplane(bool), connect(id, password), disconnect(id), forget(id), setVpnActive(id,bool) }`, `BluetoothController { powered, discovering, busy; devices; setPowered, setDiscovering, pair, connect, disconnect, forget; signal agentRequest(kind, device, passkey) ; invokable answerAgent(kind, value) }`, `AudioController { endpoints, streams, profiles; setDefault, setVolume, setMuted, setProfile }`, shared signal `notice(kind: QString, text: QString)` on each controller, model roles `id, name, kind, state, signal, bars, security, known` / `address, name, kind, paired, connected, battery` / `id, kind, name, description, volume, percent, muted, isDefault` / `id, appName, appIcon, volume, percent, muted`.

- [ ] **Step 1: Failing crate tests.** `tests/order.rs`:

```rust
use cuprita_core::model::*;
use cuprita_core::order::{order_networks, signal_bars};

fn net(name: &str, state: NetworkState, known: bool, signal: Option<u8>) -> Network {
    Network { id: name.into(), kind: NetworkKind::Wifi, name: name.into(), state, signal, security: Security::Psk, known }
}

#[test]
fn connected_then_known_then_signal() {
    let mut v = vec![
        net("weak-known", NetworkState::Disconnected, true, Some(20)),
        net("strong-unknown", NetworkState::Disconnected, false, Some(90)),
        net("home", NetworkState::Connected, true, Some(60)),
        net("mid-unknown", NetworkState::Disconnected, false, Some(50)),
    ];
    order_networks(&mut v);
    let names: Vec<_> = v.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["home", "weak-known", "strong-unknown", "mid-unknown"]);
}

#[test]
fn bars_thresholds() {
    assert_eq!(signal_bars(0), 0);
    assert_eq!(signal_bars(29), 1);
    assert_eq!(signal_bars(54), 2);
    assert_eq!(signal_bars(79), 3);
    assert_eq!(signal_bars(100), 4);
}
```

`tests/agent.rs`: a `Pin` request answered with `Pin("1234")` yields that pair and clears `pending`; a second request while one is pending returns `Err(AgentBusy)`; `Cancelled` clears. `tests/volume.rs`: `clamp_volume(2.0) == 1.5`, `clamp_volume(-1.0) == 0.0`, `clamp_volume(f32::NAN) == 0.0`, `percent(0.5) == 50`. `tests/fakes.rs`: `FakeNetwork::scripted().connect("open-cafe", None)` makes that network `Connected` in the next snapshot; `FakeBluetooth::scripted().pair("AA:…")` sets `paired`; `FakeAudio::scripted().set_default(sink_b)` moves `default`. Run `cd celestina-rs && cargo test -p cuprita-core` → FAIL (modules missing).

- [ ] **Step 2: Implement the crate** until the tests pass. `order_networks` uses `sort_by` with a key tuple `(state != Connected, !known, Reverse(signal.unwrap_or(0)), name)`.

- [ ] **Step 3: Adapter.** Controllers in `cuprita/src/controller/`, each owning a worker thread that calls the trait object (`Box<dyn Network>` from `backend.rs`), re-reads `snapshot()` after every command and every 5 s (Task 4 replaces the poll with change signals), and applies the snapshot to its model by diffing on `id`/`address` (update rows in place, insert/remove the rest) — follow `hematita/src/controller` for the cxx-qt `QAbstractListModel` pattern. `notice.rs`: a small enum `NoticeKind { Info, Error }` turned into the `notice` signal with the error's `message_es()`.

- [ ] **Step 4: Failing QML tests** over the fakes (`CUPRITA_FAKE=1` is set by `qml-tests.sh`): `tst_network_page.qml` — the list shows 4 rows, the first is the connected Ethernet, toggling the Wi-Fi switch calls `setWifiEnabled(false)` and the Wi-Fi rows disappear; `tst_bluetooth_page.qml` — 2 rows, the headphones row shows «80 %», pressing its «Conectar» calls `connect` and the row reads «Conectado»; `tst_audio_page.qml` — the output selector lists 2 sinks, choosing the second calls `setDefault`, the mute icon toggles `muted`. Run → FAIL.

- [ ] **Step 5: Implement the pages**: rows at `CelestinaTheme.rowHeight` with `CelestinaWheelScroll`, a `CelestinaSwitch` (or the existing toggle control of celestina-style — check `ls celestina-style/*.qml`) for Wi-Fi/airplane/adapter, trailing `CelestinaIconButton`s, the selector as a `GlassContextMenu` opened with `popupBeside`, the notice pill in `Main.qml` fed by the three `notice` signals. Tests pass.

- [ ] **Step 6: Checks, ledger (CUP-1-B `active`), evidence, commit** `cuprita-maintenance: Add the domain crate with its fakes and the three pages over them`.

---

### Task 4: CUP-1-C — NetworkManager client and the Network page live

**Files:**
- Create: `celestina-rs/crates/cuprita-core/src/nm.rs` (feature `nm`, default on), `celestina-rs/crates/cuprita-core/examples/snapshot.rs` (prints the three snapshots; network part now)
- Create: `cuprita/qml/dialogs/WifiPasswordDialog.qml`
- Modify: `cuprita/src/backend.rs` (real `NmNetwork` unless `CUPRITA_FAKE=1`), `cuprita/src/controller/network.rs` (replace the 5 s poll with the watcher), `cuprita/qml/pages/NetworkPage.qml`, `cuprita/build.rs`, `cuprita/tests/qml/tst_network_page.qml` (the password dialog path), crate `Cargo.toml` (`zbus = { version = "5", default-features = false, features = ["blocking-api"] }` with its justification comment)

**Interfaces:**
- Consumes: `trait Network`, `NetworkSnapshot`, `order_networks`.
- Produces: `pub struct NmNetwork; impl NmNetwork { pub fn connect_system() -> Result<Self, NetworkError> }`, `impl Network for NmNetwork`, `pub fn watch(on_change: impl Fn() + Send + 'static) -> Result<WatchHandle, NetworkError>` (subscribes to `PropertiesChanged` on the manager and every device and to `DeviceAdded/Removed`, `AccessPointAdded/Removed`, coalescing with a 300 ms debounce).

- [ ] **Step 1: Introspect** the live bus to confirm names: `busctl --system introspect org.freedesktop.NetworkManager /org/freedesktop/NetworkManager`, a wireless device object (`GetDevices` → the one with `DeviceType == 2`), its `GetAllAccessPoints`, `org.freedesktop.NetworkManager.Settings.ListConnections`, and `/org/freedesktop/NetworkManager/ActiveConnection/*`. Record the exact paths in the evidence.

- [ ] **Step 2: Failing tests.** Pure mapping functions in `nm.rs` get unit tests without the bus: `ap_to_network(ssid: &[u8], strength: u8, flags: u32, wpa_flags: u32, rsn_flags: u32, known: bool, active: bool) -> Network` (SSID bytes → lossy UTF-8; `rsn_flags & 0x200 /*KEY_MGMT_802_1X*/ != 0` → Enterprise; `rsn_flags|wpa_flags != 0` → Psk; else Open), `device_state_to_state(u32) -> NetworkState` (NM states: 100 → Connected, 40..=90 → Connecting, 120 → Failed, else Disconnected). Tests: a WPA2-PSK AP maps to `Psk`, an 802.1X AP to `Enterprise`, an open AP to `Open`; state 100 → Connected.

- [ ] **Step 3: Implement `NmNetwork`** with `zbus::blocking::Proxy` calls:
  - `snapshot`: `GetDevices`; for each device read `DeviceType`, `State`, `Interface`, `ActiveConnection`; Ethernet devices (type 1) become one `Network` each (`kind: Ethernet`, name = the active connection's `Id` or the interface); the wireless device's `GetAllAccessPoints` become Wi-Fi rows deduplicated by SSID keeping the strongest, `known` = an entry in `Settings.ListConnections` whose `GetSettings()["802-11-wireless"]["ssid"]` matches; VPN rows from connections whose `connection.type` is `vpn` or `wireguard`, `state` from the matching `ActiveConnection`; `wifi_enabled` = manager `WirelessEnabled`, `airplane` = `!WirelessEnabled && !BluetoothEnabled` is **not** used — airplane is Cuprita's own flag that sets `WirelessEnabled=false` and, through the Bluetooth trait, powers the adapter off (the controller coordinates); `NmNetwork` only reports `wifi_enabled`.
  - `connect(id, password)`: Wi-Fi with a known connection → `ActivateConnection(connection_path, device_path, ap_path)`; unknown PSK → `AddAndActivateConnection2({ "connection": {"id": ssid, "type": "802-11-wireless"}, "802-11-wireless": {"ssid": bytes, "mode": "infrastructure"}, "802-11-wireless-security": {"key-mgmt": "wpa-psk", "psk": password} }, device, ap, {"persist": "disk"})`; unknown Open → the same without the security block; Enterprise → `Err(NetworkError::Failed(…))` with a message saying the profile must be created elsewhere (spec: out of scope). `password` is dropped after the call. VPN ids → `ActivateConnection(connection, "/", "/")`.
  - `disconnect`: `DeactivateConnection(active_path)`. `forget`: `Settings.Connection.Delete()`. `set_wifi_enabled`: write `WirelessEnabled`. `set_vpn_active(id,on)`: activate/deactivate.
  - Errors: zbus errors map to `Denied` when the D-Bus error name ends in `PermissionDenied`, else `Failed(message)`.

- [ ] **Step 4: Watcher** — one thread with `zbus::blocking::Connection::system()`, `receive_signal` streams on `PropertiesChanged` for `/org/freedesktop/NetworkManager` and the wireless device, and `DeviceAdded`/`DeviceRemoved`; debounce 300 ms; call `on_change`. The controller then re-reads `snapshot()` on its worker.

- [ ] **Step 5: `WifiPasswordDialog.qml`** (`CelestinaModalLayer` + `GlassCard`, a `CelestinaTextField` with `echoMode: TextInput.Password` and a show/hide icon button, «Conectar»/«Cancelar»); the page opens it when `connect` is pressed on a `Psk` row that is not `known`. QML test: pressing «Conectar» on the open row calls `connect(id, "")` directly; on the PSK unknown row the dialog appears and its «Conectar» calls `connect(id, "secret")`.

- [ ] **Step 6: Example** `examples/snapshot.rs` prints `NmNetwork::connect_system()?.snapshot()?` as debug; run it on the author's machine and paste the (SSID-redacted) output into the evidence.

- [ ] **Step 7: Checks, ledger, evidence (`2026-10-xx-network.md`, including the live NM paths and the author's pending live check), commit** `cuprita-maintenance: Add the NetworkManager client and bring the network page live`.

---

### Task 5: CUP-1-D — BlueZ client, pairing agent and the Bluetooth page live

**Files:**
- Create: `celestina-rs/crates/cuprita-core/src/bluez.rs` (feature `bluez`, default on), `cuprita/src/agent.rs`, `cuprita/qml/dialogs/PairingDialog.qml`
- Modify: `cuprita/src/backend.rs`, `cuprita/src/controller/bluetooth.rs`, `cuprita/qml/pages/BluetoothPage.qml`, `cuprita/build.rs`, `cuprita/tests/qml/tst_bluetooth_page.qml`, `examples/snapshot.rs` (Bluetooth part)

**Interfaces:**
- Consumes: `trait Bluetooth`, `BluetoothSnapshot`, `Agent`, `AgentRequest`, `AgentAnswer`.
- Produces: `pub struct BluezBluetooth; impl BluezBluetooth { pub fn connect_system() -> Result<Self, BluetoothError> }`, `impl Bluetooth for BluezBluetooth`, `pub fn serve_agent(requests: Sender<AgentRequest>, answers: Receiver<AgentAnswer>) -> Result<AgentHandle, BluetoothError>` (exports `org.bluez.Agent1` at `/org/celestina/cuprita/agent` with capability `KeyboardDisplay`, registers with `AgentManager1.RegisterAgent` + `RequestDefaultAgent`), `pub fn watch(on_change) -> Result<WatchHandle, BluetoothError>` (ObjectManager `InterfacesAdded/Removed` + `PropertiesChanged` on `org.bluez.Device1`/`Adapter1`).

- [ ] **Step 1: Introspect** `busctl --system tree org.bluez` and `introspect org.bluez /org/bluez/hci0` (`Adapter1`: `Powered`, `Discovering`, `StartDiscovery`, `StopDiscovery`, `RemoveDevice`; `Device1`: `Address`, `Name`/`Alias`, `Icon`, `Class`, `Paired`, `Connected`, `Trusted`, `Pair`, `Connect`, `Disconnect`; `Battery1.Percentage`). Record in the evidence.

- [ ] **Step 2: Failing tests** for pure mapping in `bluez.rs`: `icon_to_kind(icon: &str) -> DeviceKind` (`audio-headset`/`audio-headphones`/`audio-card` → Audio; `input-*` → Input; `phone` → Phone; `computer` → Computer; else Other) and `device_from_props(…) -> BluetoothDevice`. Agent tests already exist in the crate (Task 3).

- [ ] **Step 3: Implement `BluezBluetooth`** over `org.freedesktop.DBus.ObjectManager.GetManagedObjects` on `/`: the first adapter's `Powered`/`Discovering`, every `Device1` → `BluetoothDevice` (battery from `Battery1` when present). Commands: `set_powered` writes `Powered`; `set_discovering` → `StartDiscovery`/`StopDiscovery`; `pair` → `Device1.Pair()` (BlueZ calls the agent as needed) then sets `Trusted = true` so reconnects are automatic; `connect`/`disconnect`; `forget` → `Adapter1.RemoveDevice(path)`. Error mapping: `org.bluez.Error.AuthenticationCanceled/Rejected` → `Failed(message_es "Emparejamiento cancelado")` (message built in `message_es`, the English variant name stays), `AlreadyConnected` → `Ok(())`.

- [ ] **Step 4: Agent** — `serve_agent` implements `RequestPinCode`, `RequestPasskey`, `DisplayPasskey`, `RequestConfirmation`, `RequestAuthorization`, `AuthorizeService` (the last two answer `Ok` when the device is paired or pairing), `Cancel`, `Release`; each request is sent through `requests`, the method blocks on `answers` (BlueZ allows ~60 s), `Rejected` returns `org.bluez.Error.Rejected`, `Cancelled` returns `org.bluez.Error.Canceled`. `cuprita/src/agent.rs` bridges the channels to `BluetoothController.agentRequest(kind, device, passkey)` and `answerAgent(kind, value)`.

- [ ] **Step 5: `PairingDialog.qml`** — three looks by `kind`: `pin` (text field, «Aceptar»), `confirm` (the six-digit passkey large, «Coincide» / «No coincide»), `display` (the passkey to type on the other device, «Cancelar»). QML tests: the controller's `agentRequest("confirm", "Auriculares", 123456)` shows the dialog with «123456»; pressing «Coincide» calls `answerAgent("confirm", "yes")`; Esc calls `answerAgent("confirm", "cancel")`.

- [ ] **Step 6: Page live** — adapter switch, «Buscar» with a spinner while `discovering`, rows with kind icon / name / state / battery, trailing actions; unknown devices show «Emparejar», paired ones «Conectar»/«Desconectar» and a menu with «Olvidar».

- [ ] **Step 7: Example, checks, ledger, evidence (with the author's live check: pair and unpair the headphones), commit** `cuprita-maintenance: Add the BlueZ client with the pairing agent and bring the Bluetooth page live`.

---

### Task 6: CUP-1-E — audio binding spike, PipeWire client and the Audio page live

**Files:**
- Create: `celestina-rs/crates/cuprita-core/src/pw.rs` (feature `pipewire`) **or** `src/wpctl.rs` (feature `wpctl`) — the spike decides; the other file is not created
- Modify: crate `Cargo.toml`, `cuprita/Cargo.toml` (link flags only if the `pipewire` crate is chosen), `cuprita/src/backend.rs`, `cuprita/src/controller/audio.rs`, `cuprita/qml/pages/AudioPage.qml`, `cuprita/tests/qml/tst_audio_page.qml`, `examples/snapshot.rs` (audio part), `cuprita/README.md` (runtime dependency: `pipewire`, `wireplumber`)

**Interfaces:**
- Consumes: `trait Audio`, `AudioSnapshot`, `clamp_volume`, `percent`.
- Produces: `pub struct PwAudio` (or `WpctlAudio`) with `pub fn connect_session() -> Result<Self, AudioError>`, `impl Audio`, `pub fn watch(on_change) -> Result<WatchHandle, AudioError>`.

- [ ] **Step 1: Spike (throwaway, ≤ 1 hour, recorded in the evidence).** In a scratch example inside the crate, add `pipewire = "0.8"` (check the latest 0.8.x on crates.io with `cargo search pipewire`), build a `MainLoop` + `Context` + `Core`, register a `registry` listener, list nodes with `media.class` `Audio/Sink`, `Audio/Source`, `Stream/Output/Audio`, read `Props` (`volume`, `mute`, `channelVolumes`) via `node.enum_params` and the default sink from the `metadata` object (`default.audio.sink`). Then build `cuprita` release with cxx-qt and run the smoke. Decision rule: choose `pipewire` if (a) the crate builds with the workspace toolchain `--locked`, (b) the release binary links and the smoke exits 0, (c) a sink's volume can be set with `node.set_param(ParamType::Props, …)` and WirePlumber reflects it in `wpctl status`. Otherwise choose `wpctl`: parse `wpctl status` (sections `Sinks:`, `Sources:`, `Streams:` with `*` marking defaults, `[vol: 0.65 MUTED]`), `wpctl inspect <id>` for `node.description`/`application.name`, `wpctl set-default/set-volume/set-mute`, `wpctl status` polled every 2 s plus `pw-mon` as the watcher if present. Write the decision and its three measurements into the evidence before implementing.

- [ ] **Step 2: Failing tests** for the pure parts: with `pipewire` — `props_to_endpoint(media_class, name, description, volume, mute, is_default)`; with `wpctl` — `parse_status(&str) -> AudioSnapshot` over a captured `wpctl status` fixture in `tests/fixtures/wpctl-status.txt` (two sinks, one marked default, one source, two streams, with `MUTED` on one). Also `profiles`: with `pipewire`, from the `Device` object's `EnumProfile`/`Profile` params; with `wpctl`, from `wpctl inspect` of the device (`device.profile.*`) — if `wpctl` cannot change profiles, implement `set_profile` through `pw-cli set-param <device> Profile '{ index: N }'` and record that in the evidence.

- [ ] **Step 3: Implement** the chosen client and the watcher (PipeWire: the registry's global added/removed and node param changes; `wpctl`: the poll).

- [ ] **Step 4: Page live** — output and input cards with the selector menu, slider (`CelestinaTheme` tokens, step 1 %, range 0–150 % with a tick at 100 %), mute icon; the applications card; the profile card only when a device has more than one profile. QML tests over the fakes already exist; add one for the profile card visibility.

- [ ] **Step 5: Example, checks, ledger, evidence (with the author's live check: switch output to HDMI and back, mute one application), commit** `cuprita-maintenance: Add the PipeWire client and bring the audio page live`.

---

### Task 7: CUP-1-F — exit: polish, 1.0.0, hygiene evidence

**Files:**
- Modify: `cuprita/qml/**` (keyboard and accessibility pass), `cuprita/Cargo.toml` + `Cargo.lock` (1.0.0), `docs/version-history.tsv`, `cuprita/STATUS.md`, `cuprita/ROADMAP.md`, `cuprita/VALIDATION.md`, `cuprita/docs/plans/active/2026-10-08-cup-1-foundation.md` (every row `done` except F `active`), `HOST-HYGIENE.md` (three findings: `network-manager-applet`, `blueman`, `pavucontrol` — "Cuprita 1.0 holds the role; removal is the author's action", Decision line), `docs/evidence/…` or `cuprita/docs/evidence/2026-10-xx-exit.md`

**Interfaces:** none new.

- [ ] **Step 1: Keyboard pass** — Tab order: strip → page content; `Ctrl+1/2/3`; arrows in every list; Enter = primary action of the focused row; Space toggles the focused switch; Esc closes dialogs and menus. Add `tst_keyboard.qml` asserting the strip is one Tab stop and that Enter on the first Bluetooth row calls its primary action.

- [ ] **Step 2: Accessibility** — every row `Accessible.role: Accessible.ListItem` with a name that reads kind, name and state (the Spanish sentence built from the row's `qsTr()` parts: kind, name, state, bars of four); switches `Accessible.role: Accessible.CheckBox`; the dialogs announce their title.

- [ ] **Step 3: Glass and motion check** — `bash scripts/glass_canvas_contract.py`-backed guard passes; dialog fade within the shared timings; `reducedMotion` disables the spinner's rotation.

- [ ] **Step 4: Version** 1.0.0 in `cuprita/Cargo.toml` and lock; the history row as the landing expects (`scripts/version_tool.py check`).

- [ ] **Step 5: Hygiene** — in `HOST-HYGIENE.md`, under the existing Blueman / nm-applet / Pavucontrol findings, append `Decision: superseded on <date> by Cuprita 1.0 (<evidence path>); removing the packages is the author's action.`

- [ ] **Step 6: Checks, evidence, commit** `cuprita-maintenance: Close the foundation at 1.0 with the keyboard and accessibility pass`; the landing uses `--kind milestone`.

---

## Self-review

- Spec coverage: §1 scope → Tasks 4–6 (each section), out-of-scope items are not planned; §2 constraints → Global Constraints; §3 sources → Tasks 4, 5, 6 Step 1 (introspection/spike); §4 crate → Task 3 (+ real clients in 4–6); §5 adapter → Task 3 Step 3, agent in Task 5 Step 4, secrets handling in Task 4 Step 3; §6 surface → Tasks 2, 3, 4 (dialog), 5 (dialog), 6, 7 (keyboard); §7 phases → one task each, AUD-1-P first; §8 verification → crate tests (3–6), QML tests (2–7), smoke (2), example binary (4–6), author live checks recorded in evidence (4–6), guards in Global Constraints; §9 decisions → honoured (no helper CLIs except the `wpctl` fallback; no tray; strip local).
- Placeholders: none; the only open decision (audio binding) has an explicit rule and both branches planned.
- Type consistency: trait method names in Task 3 are the ones Tasks 4–6 implement; controller invokables match the QML tests' calls; model roles listed once in Task 3.
