# Cuprita — design

- **Date:** 2026-10-08
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Product:** the suite's control centre for network, Bluetooth and audio,
  replacing nm-applet, Blueman and pavucontrol
- **Identifiers:** project `cuprita`, crate `cuprita-core`, commit prefix
  `cuprita`, desktop id `org.celestina.Cuprita`, unit prefix `CUP`

## 1. Goal and scope

Cuprita is the one window where the author connects the machine to the
world: which network it is on, which Bluetooth devices it talks to, and where
sound goes in and out. It replaces the three foreign applets that hold those
roles today (`HOST-HYGIENE.md`, "Blueman, nm-applet and Pavucontrol"), in this
order:

1. **Red** — Ethernet and Wi-Fi state; the Wi-Fi list with signal and
   security; connect (with a password when the network asks for one),
   disconnect, forget; airplane mode (every radio off); the VPNs already
   defined in NetworkManager, activated and deactivated from the same list.
2. **Bluetooth** — adapter on/off; discovery; the device list with kind,
   paired/connected state and battery when the device reports it; pair (with
   Cuprita as the pairing agent: PIN entry and passkey confirmation), connect,
   disconnect, forget.
3. **Audio** — default output and input; volume and mute per device and per
   application stream; the card profile (for example analogue versus HDMI).

Out of scope for every phase: editing connection profiles (IP settings,
certificates, enterprise Wi-Fi beyond a password), creating a hotspot or
sharing a connection, mobile broadband, Bluetooth file transfer (Magnetita
already moves files to the phone), audio routing of the PipeWire graph and
latency tuning, a tray icon or indicator (the shell is halted; Cuprita is a
window, opened from the launcher or a key binding), notifications, and any
work on the Celestina shell.

The audit's companion finding — removing `network-manager-applet`, `blueman`
and `pavucontrol` from the host — is the author's action and no phase
performs it; CUP-1-F records the evidence that they can go.

## 2. Constraints the design honours

- Root `AGENTS.md`: pure domain in `celestina-rs/`, Qt adaptation in
  `cuprita/src`, presentation in `cuprita/qml`, tokens from `celestina-style`,
  every QML file registered in `build.rs`, no blocking IO on the Qt thread,
  typed errors, no production `unwrap`, each dependency justified.
- Style contract (`celestina-style/DESIGN.md`) and the desktop-glass program:
  a transparent canvas with `CelestinaBackdrop`, grouped rounded cards, one
  accent, rows at the shared row height, `CelestinaWheelScroll` on every
  list, `CelestinaModalLayer` + `GlassCard` for the two dialogs, popup menus
  beside their button, motion within 100–500 ms honouring `reducedMotion`.
- Author rules: icon-first actions with the shared hover circle, no tooltips,
  Spanish product copy only through `qsTr()`, English development text.
- Privilege: Cuprita never runs as root and never sees a password it does not
  need. Wi-Fi secrets go to NetworkManager inside the activation call and are
  stored by NetworkManager; Bluetooth pairing codes are typed into the agent
  dialog and handed to BlueZ; polkit prompts in its own window when a change
  needs authorisation. No daemon: every operation is a D-Bus call from the
  running window.
- Template: Hematita is the structural model (crate + `src` + `qml` +
  `build.rs` + production scripts + ledger + three-section strip); its layout
  is copied, never its code.

## 3. Data sources (verified on the author's machine, 2026-10-08)

| Role | Source | Verified |
|---|---|---|
| Network | NetworkManager 1.58.1 on the system bus, `org.freedesktop.NetworkManager` (`Device`, `Device.Wireless`, `AccessPoint`, `Settings`, `Settings.Connection`, `Connection.Active`, `VPN.Connection`) | service active, 51 members on the manager object |
| Bluetooth | BlueZ 5.87 on the system bus, `org.bluez` (`Adapter1`, `Device1`, `Battery1`, `AgentManager1`; Cuprita exports `org.bluez.Agent1`) | service active, name owned |
| Audio | PipeWire 1.6.9 with WirePlumber 0.5.18 (user session); `libpipewire-0.3` headers installed at `/usr/include/pipewire-0.3` | both user units active, `wpctl status` lists sinks and sources |
| Bus access | `zbus` 5, already used by Siderita (UDisks2, portal) and Magnetita | in the workspace |

Audio has two candidate bindings and the choice is made by a spike inside
CUP-1-E, recorded in that unit's evidence:

- preferred: the `pipewire` crate (Rust bindings over `libpipewire-0.3`,
  registry + metadata + node params for volumes) in `cuprita-core`, if it
  links and runs cleanly beside cxx-qt in the release build;
- fallback: WirePlumber's `wpctl` driven as a child process with a parsed
  `status` and `set-volume`/`set-mute`/`set-default` calls, behind the same
  trait; slower to react, zero link risk.

## 4. `celestina-rs/crates/cuprita-core`

The domain, testable without D-Bus or Qt.

- **Models:** `Network { id, kind: Ethernet|Wifi|Vpn, name, state, signal: Option<u8>, security: Open|Psk|Enterprise, known: bool }`, `BluetoothDevice { address, name, kind, paired, connected, trusted, battery: Option<u8> }`, `AudioEndpoint { id, kind: Sink|Source, name, description, volume: f32 (0..1.5), muted, default }`, `AudioStream { id, app_name, app_icon, volume, muted, endpoint }`, `CardProfile { card_id, id, description, active }`.
- **Traits:** `Network`, `Bluetooth`, `Audio`, each with a synchronous query
  surface (`snapshot() -> Result<…>`) and commands (`connect`, `forget`,
  `set_volume`, …) returning typed errors (`NetworkError`,
  `BluetoothError`, `AudioError`, each with a `message_es()` for the notice the
  window shows). Events arrive through a `Watcher` that yields
  `Change::{Network, Bluetooth, Audio}` so the adapter re-reads one snapshot
  instead of tracking properties one by one.
- **Fakes:** `FakeNetwork`, `FakeBluetooth`, `FakeAudio` implementing the
  traits with scripted state, used by every adapter and QML test.
- **Pure logic with tests:** ordering of the Wi-Fi list (connected first, then
  known, then by signal), signal → bars (0–4), security labels, the
  pairing-agent state machine (request PIN, confirm passkey, cancel), volume
  clamping and the dB-free percentage shown to the person, and the
  "can the three applets be retired" checklist that CUP-1-F evaluates.
- **Real clients** (feature-gated so tests build without them): `nm.rs`
  (zbus proxies generated from introspection, blocking API on a worker
  thread), `bluez.rs` (proxies plus the exported `Agent1` object), `pw.rs`
  (the binding chosen in §3).

Dependencies: `zbus = "5"` (already pinned), `pipewire` only if the spike
chooses it; nothing else new.

## 5. `cuprita/src` — the CXX-Qt adapter

- `main.rs`: application identity (`org.celestina.Cuprita`), single instance
  via the name `org.celestina.Cuprita` (a second launch activates the first
  window), QML engine, the three clients started on worker threads.
- `controller/`: one `QObject` per section (`NetworkController`,
  `BluetoothController`, `AudioController`) exposing list models
  (`QAbstractListModel` through cxx-qt, one row per network/device/endpoint/
  stream), state properties (`wifiEnabled`, `airplane`, `adapterPowered`,
  `discovering`, `busy`) and invokables for every command. Commands run on
  the worker thread and report through a shared `notice(kind, text)` signal
  that the window turns into the suite's notice pill.
- `agent.rs`: the Bluetooth pairing agent; its requests become a modal in QML
  (`PairingDialog`) and the answer flows back to BlueZ.
- `secrets.rs`: the Wi-Fi password dialog's answer is passed straight into
  `AddAndActivateConnection2`; it is never written by Cuprita.
- Snapshot diffing keeps the models stable (rows update in place; no
  flicker), as Hematita's process model does.

## 6. `cuprita/qml` — the surface

- `Main.qml`: transparent canvas, `CelestinaBackdrop`, the three-section
  strip (Red, Bluetooth, Audio) in the Hematita layout, a `StackLayout` of
  pages, the notice pill, the two modal dialogs.
- `pages/NetworkPage.qml`: top card with the active connection (name, kind,
  IP shown read-only, disconnect); the Wi-Fi switch and the airplane switch;
  the network list (`CelestinaUsageList`-style rows: bars icon, name, lock,
  state); a trailing menu per row (olvidar); the VPN card with a switch per
  VPN. `WifiPasswordDialog.qml`.
- `pages/BluetoothPage.qml`: the adapter switch and a «Buscar» button with a
  discovery spinner; the device list (kind icon, name, state, battery);
  trailing actions (conectar/desconectar, olvidar). `PairingDialog.qml`
  (PIN field or passkey with «Coincide»/«No coincide»).
- `pages/AudioPage.qml`: an output card and an input card («Salida»,
  «Entrada»), each with the default endpoint selector (popup menu beside the
  button), a volume slider and a mute icon; an applications card with one
  row per stream; a profile card for sound cards with more than one profile.
- Keyboard: the strip is one Tab stop, pages are reachable with Ctrl+1/2/3,
  lists walk with the arrows, Enter activates the primary action, Space
  toggles switches, Esc closes a dialog; every control has an
  `Accessible.name`.

## 7. Delivery phases

Each unit is a ledger row in `cuprita/docs/plans/active/2026-10-08-cup-1-foundation.md`
with inventory and evidence, landed one at a time; the project's own
`ROADMAP.md` carries the same table.

| Unit | Outcome |
|---|---|
| AUD-1-P (`suite`) | Registration: `docs/projects.toml` entry (`cxx-qt-application`, version source, mirrors, agents/readme/status/roadmap), ratchet rows, workflow, this spec committed |
| CUP-1-A | Skeleton: crate stub, window with the three-section strip and empty pages, `build.rs`, style links, `.desktop`, icon, production scripts, smoke, document set, 0.1.0 |
| CUP-1-B | `cuprita-core`: models, the three traits, fakes, the pure logic and its tests; the adapter's controllers and models over the fakes; QML tests of the three pages over the fakes |
| CUP-1-C | Red: the NetworkManager client, the page wired to it, connect with password, forget, airplane, VPN switches |
| CUP-1-D | Bluetooth: the BlueZ client, the pairing agent and dialog, the page wired to it |
| CUP-1-E | Audio: the binding spike (decision recorded), the PipeWire client, the page wired to it |
| CUP-1-F | Exit: glass, keyboard and screen-reader pass; 1.0.0; STATUS/ROADMAP; `HOST-HYGIENE.md` evidence that nm-applet, Blueman and pavucontrol hold no remaining role |

## 8. Verification

- Crate: unit tests of every pure function and of the agent state machine;
  the real clients are exercised by a small `cuprita-core` example binary
  (`examples/snapshot.rs`) that prints the three snapshots on the author's
  machine, recorded in each unit's evidence.
- Adapter: model diffing tests over the fakes.
- QML: `cuprita/tests/qml` over the fakes — each page renders its rows, the
  dialogs answer and cancel, switches call the controller.
- `cuprita/scripts/smoke.sh`: launches the release binary on the offscreen
  platform with the fakes (`CUPRITA_FAKE=1`), constructs every page and
  exits.
- Live checks per unit, by the author on the real session: join a Wi-Fi with
  a password and forget it (C); pair the author's headphones or phone and
  unpair (D); switch output to HDMI and back, mute one application (E).
- Guards before every commit: architecture, qmllint, language, documentation
  contracts, radius and glass-canvas ratchets, `cargo fmt`, `clippy -D
  warnings`.

## 9. Decisions recorded here

- Three real backends behind traits with fakes, so the whole surface is
  testable in the session and the author's live check is only the last mile.
- NetworkManager, BlueZ and PipeWire are talked to directly; no helper
  library (`nmcli`, `bluetoothctl`) is spawned except `wpctl` as the audio
  fallback decided in CUP-1-E.
- Secrets never persist in Cuprita; NetworkManager and BlueZ keep them.
- No tray, indicator or shell integration: the shell is halted and this spec
  does not pre-empt its return.
- The strip is copied from Hematita structurally; extraction into
  `celestina-style` happens only if a third application needs it, per the
  extraction rule.
