# CEL-2-F — Connectivity and power pieces — Implementation Plan

- **Date:** 2026-10-17
- **Status:** approved by the author in chat on 2026-10-17 (model table and decision D1) and executed; kept as the unit's plan record. Where it differs from what was built, [the evidence](2026-10-17-connectivity.md) is authoritative
- **Artifact:** none; a plan record, not a build
- **Scope:** unit CEL-2-F as planned before implementation
- **Environment:** none; nothing in this record was run


> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver CEL-2-F of the parity program: four pieces for the bar — **network**, **Bluetooth**, **battery** and **power profile** — that show read-only state of NetworkManager, BlueZ, UPower and power-profiles-daemon and offer a short, closed list of actions from the Island page each one opens. `nm-applet` and `blueman` stay the editing UIs, and no secret ever passes through the shell.

**Architecture:** One new module `celestina/src/connectivity/` holds three provider workers behind the `Worker` guard: the **network** and **Bluetooth** workers wrap the `NmNetwork` and `BluezBluetooth` clients of the existing `cuprita-core` crate (they already read NM and BlueZ over D-Bus, with signal watchers and scripted fakes) behind a narrower, shell-owned action type; the **power** worker is new (UPower's display device and the profile daemon over `zbus`). Each publishes a bounded, shell-owned state into the loop (`Message::Connectivity`). A pure module in `celestina-shell` owns those state types, their bounds, the action validation against the last confirmed inventory and the profile cycling (salvaged by copy from the core's `power.rs`). The four pieces plug into the piece table of CEL-2-A; their popups are Island pages (decision D1). No duty switch: a piece appears when the author places it.

**Tech Stack:** Rust (workspace toolchain), `zbus` blocking (already a dependency), `cuprita-core` as a path dependency with features `nm` and `bluez` only, the toolkit crates. One new dependency edge, no new crate in the lockfile beyond `cuprita-core` itself.

**Spec:** `docs/superpowers/specs/2026-10-16-celestina-parity-program-design.md` §4 (principles), §5 row F, §6.3 (piece contract), §7 row F, §8. Ledger: `celestina/docs/plans/active/2026-10-16-cel-2-parity.md`.

## Global constraints

- Identifiers: project `celestina`, unit `CEL-2-F`, plan ID `cel-2-parity` (the program's), checkpoint `CEL-2`, commit prefix `celestina:` (session commits `celestina-maintenance: <imperative>`; the author chooses the kind at landing). A session never bumps a version, never writes an inventory, never runs `build-production.sh`, never commits or pushes without the author's request. **This unit does not land and does not push** until the author says so in chat.
- The halt: never edit `celestina-qt/` or `celestina-rs/crates/celestina-shell-core`; never touch `/home/toni/CODIGO/isla-spike`. Never edit `cuprita-core` or `cuprita/` either: it is Cuprita's, consumed read-only; a gap is adapted on this side or recorded as a finding.
- **Salvage gate (AGENTS.md "Halted projects").** The session "Salvamento CEL-2 restante" adds this unit's line. Before the first commit, check that `main` contains it. The copies this unit needs: `celestina-shell-core/src/power.rs` into `celestina-shell` (`next_profile` and its tests) and `connectivity.rs` as reference only (its narrowness rules and `Expected` idea; its types are not copied because they are tied to `nmcli`/`bluetoothctl` output). `network.rs` and `bluetooth.rs` are **not** copied: `cuprita-core`'s D-Bus clients replace them. Every copy records its source path and commit in its head comment.
- Language: development truth English; Spanish product copy only in `celestina/src/copy.rs`. Service-supplied text (network names, device aliases, error detail) is **data**: bounded, shown only as a name in a row, never interpreted, never put in a sentence the shell builds from the service's words. The error sentences come from `copy.rs` by error kind.
- Root `AGENTS.md` invariants: no production `unwrap`/`expect`/`panic!`, no `#[allow]`/TODO/FIXME, typed errors with sources, blocking IO never on the loop's thread, bursts coalesced, workers joined at shutdown (every blocking wait carries a timeout), best-effort D-Bus failures degrade a feature instead of blocking or crashing.
- Style: sizes and colours from `Theme` tokens; focus with `visualFocus`; reduced motion honoured; the pieces and pages are keyboard-operable where the page is (the bar itself takes no keyboard yet: every piece action is also reachable from its page, as the piece contract requires).
- **Safety of tests (never negotiable):** nothing runs against the author's real session or network. Tests and the smoke use a private `dbus-daemon` (the recipe of `src/tray/test_bus.rs`), `DBUS_SYSTEM_BUS_ADDRESS` pointed at the private bus (fake NM/BlueZ/UPower/PPD) or at a socket that does not exist, and a throwaway `XDG_CONFIG_HOME`. No test calls the real NetworkManager, BlueZ, UPower or PPD; no test issues a real `Connect`, `Disconnect`, `Set` or `RequestScan`.
- **Parallel units** (C, D, G, J, K, L run beside this one): shared files get minimal additive edits so the later `git rebase --onto origin/main <previous tip>` is trivial: `app.rs` (fields and arms only), `message.rs` (one variant), `command.rs` (none expected), `pieces/mod.rs` (table entries, one `BarCx` slice, `PieceAction` variants), `island/pages/mod.rs` (`PageCx` slice), `settings.rs` (default end zone only), `Cargo.toml`/`Cargo.lock` (one dependency line), the ledger and `STATUS.md` (own row/line). Never reformat code this unit does not change. Landings are one at a time.
- SDD workspaces (briefs and reports) live in the session notes folder, never in the tree.
- Before every session commit, from the worktree root: `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/version_tool.py check`; from `celestina/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`; from `celestina-rs/`: the same for `-p celestina-shell`; `python3 scripts/commit_scope.py --check "<subject>"`. Never `git stash`, never `cargo clean`.

## Decisions this unit closes (spec §7 row F)

**D1 — The popup of a piece is an Island page, not an `xdg_popup`.** The network piece opens the `network` page, Bluetooth the `bluetooth` page, battery the `power` page; the power-profile piece opens the same `power` page. Reasons: the bar takes no keyboard yet and the contract demands every action be reachable from a page or verb, so an `xdg_popup` beside a page would be a second path with no keyboard; the page path already has focus, Escape, click-outside, reduced motion and the tab strip; no new popup shell, so no second grab/dismiss policy and little overlap with the slider popup CEL-2-C builds. Cost: the Island grows instead of a small card hanging under the icon. A popup card is a later change inside `pieces/` and `island/` with no effect on the state, workers or validation (they are the part of the unit that does not change). **The author confirms or overrules this before Task 6.**

**D2 — Read-only state (published, never written by the shell).**
- NetworkManager: Wi-Fi switch (`WirelessEnabled`) and its hardware kill, per row: kind (Wi-Fi, Ethernet, VPN), name, connection state, signal 0..=100, security class (open, passphrase, enterprise, WEP), whether a saved profile exists, the first IPv4 address of the active link. Connectivity check state is not read (it can reach the network).
- BlueZ: adapter `Powered` and `Discovering` (shown, never started), per device: address, alias (bounded), icon class, `Paired`, `Connected`, `Trusted`, battery percentage when `Battery1` reports one.
- UPower display device: `IsPresent`, `State`, `Percentage`, `TimeToEmpty`, `TimeToFull`.
- Power profile daemon: `ActiveProfile`, the list `Profiles`, `PerformanceDegraded`.
- Rows are bounded: at most 24 networks and 24 devices, names cut at 48 grapheme clusters, addresses validated as `XX:XX:XX:XX:XX:XX`, ids at 128 bytes.

**D3 — Actions the pages may send (closed list, typed; no string verbs).**

| Action | Target | Check before the call |
|---|---|---|
| `Refresh` | a section | none: re-read |
| `SetWifiEnabled(bool)` | NM manager | the NM state is known |
| `JoinSaved(id)` | a Wi-Fi or Ethernet row | `id` is in the last inventory, `known`, kind not VPN, security not enterprise/WEP |
| `Leave(id)` | the active Wi-Fi or Ethernet row | `id` is in the inventory and active |
| `SetBluetoothPowered(bool)` | BlueZ adapter | adapter present |
| `ConnectDevice(addr)` / `DisconnectDevice(addr)` | a device | `addr` in the last inventory and `paired` |
| `SetProfile(name)` / `CycleProfile` | PPD | `name` is in the daemon's `Profiles` |
| `OpenEditor(Network\|Bluetooth)` | the editing UI | fixed program (`nm-connection-editor`, `blueman-manager`), no arguments from data, only if found on `PATH` |

**Never, by construction (no variant exists):** reading or sending a passphrase or any secret; creating, editing, deleting (forgetting) or exporting a profile; `AddAndActivateConnection`; activating or deactivating a VPN; airplane mode; starting a scan or discovery; pairing, trusting or removing a device; registering a BlueZ agent; holding a power profile; touching UPower's settings; suspending anything (that is the session verbs of CEL-2-B). A saved Wi-Fi that needs a secret NM does not hold fails with the typed error `NeedsSecret` and the page says so and offers `OpenEditor(Network)`.

**D4 — Secrets never pass through the shell.** `cuprita-core`'s `Network::connect(id, password)` is only ever called with `None`; the wrapper's action type has no field that could carry a secret, and a unit test pins that the wrapper never forwards `Some`. No `nm-applet` secret agent is registered; NM asks the user's agent (nm-applet, polkit, the keyring) as it does today. The shell holds no D-Bus method that returns secrets and logs no connection settings.

**D5 — Identity and confirmation.** A request names a row by the stable `id` of the inventory (NM connection UUID or access-point identity; BlueZ address). It must be in the *last confirmed* inventory when the worker takes it, else it is dropped with a journal line (rows move under an open page). The page shows the row *pending* until a later observation shows the requested state or a 20 s window ends (then the row shows the failure by kind). No optimistic state.

**D6 — `cuprita-core` as the reader.** Reuse over rewrite: `NmNetwork`, `BluezBluetooth`, their `watch` functions (signal-driven, coalesced) and the `FakeNetwork`/`FakeBluetooth` fakes. The workers take the clients as `Box<dyn Network>` / `Box<dyn Bluetooth>` so the logic is tested without a bus. Types are converted at the boundary into the shell's bounded types; cuprita's model never reaches pieces. Features `nm` and `bluez` only (no `wpctl`). The network and Bluetooth clients' longest call (120 s for a polkit prompt) runs on the worker, never the loop; the command queue holds 4 and refuses the rest.

**D7 — Pieces and layout.** Ids `network`, `bluetooth`, `battery`, `power-profile`. `network` and `bluetooth` join the default end zone after `tray`, in Noctalia's order (bubbles, sysmon, tray, **network, bluetooth**, volume, brightness, clipboard). `battery` and `power-profile` are registered but not in the default layout: the author's Noctalia bar shows neither, and on a machine without a battery `battery` is absent anyway. A piece whose service or hardware is absent (`bar()` returns `None`) draws nothing and its zone draws no capsule if it was the only one.

**D8 — Power workers.** UPower's `DisplayDevice` and PPD are read by one worker every 5 s and published only when the value changed (so rest stays at zero frames). PPD is tried under `org.freedesktop.UPower.PowerProfiles` first and `net.hadess.PowerProfiles` second. `SetProfile` is `org.freedesktop.DBus.Properties.Set` on `ActiveProfile`, validated against `Profiles`.

**D9 — No duty, no owner detection.** Reading does not conflict with `nm-applet`, `blueman` or Noctalia's widgets; an action is idempotent and the observation after it is the truth, so two actors on the same row converge. The coexistence rule of §4.1 holds trivially: the author places the pieces in `bar.end` when they want them.

**D10 — Absent services degrade.** With no system bus, or a service not running, a worker reports `Unavailable` once, retries with backoff 5 s doubling to 60 s, and the piece is absent. A service that appears later is picked up by the retry.

**D11 — Editing UIs.** `OpenEditor` spawns the fixed program detached through the launcher's spawn path (`launcher::spawn`), with no environment from data. If the program is missing the row says so (copy.rs), nothing else.

## File map

Created:
- `celestina-rs/crates/celestina-shell/src/connectivity.rs` — pure state types, bounds, `Action` validation against an inventory, `Expected`/pending window, `PowerProfiles::cycle` (the salvaged `next_profile`).
- `celestina/src/connectivity/mod.rs` (the `ConnectivityHandle`, `ConnectivityEvent`, start/stop), `network.rs`, `bluetooth.rs`, `power.rs`, `editor.rs`.
- `celestina/src/pieces/{network,bluetooth,battery,power_profile}.rs` — the four pieces.
- `celestina/src/island/pages/{network,bluetooth,power}.rs` — the three pages.
- `celestina/crates/celestina-fake-system/` (a dev-only crate: `celestina` names it only under `[dev-dependencies]`; first written as `celestina/src/connectivity/fakes.rs` and moved after review) and `celestina/tests/connectivity_bus.rs`, `celestina/examples/fake_system_bus.rs` — fake NM, BlueZ, UPower and PPD on a private bus.
- `celestina/docs/evidence/2026-10-17-connectivity.md`, `celestina/docs/decisions/00NN-…` for D1–D4 (numbers after the highest one landed).

Modified (additive): `celestina-shell/src/{lib,settings}.rs`, `celestina/Cargo.toml` and `Cargo.lock`, `celestina/src/{lib,app,message,copy}.rs`, `celestina/src/pieces/mod.rs`, `celestina/src/island/{mod,pages/mod}.rs`, `celestina/scripts/smoke.sh`, `celestina/{STATUS,VALIDATION}.md`, the CEL-2 ledger row.

## Tasks

Each task: write the failing test, make it pass, run the unit's fast checks, report. Models and reasoning are the table the author approves before execution (the table is in the session, not here).

### Task 1 — Pure domain in `celestina-shell`
- [ ] `connectivity.rs`: `NetSection`, `BtSection`, `BatterySection`, `ProfileSection` (bounded, shell-owned), `NetRow`/`BtRow`, `Action` enum (D3), `fn validate(&Action, &Inventory) -> Result<Checked, Refusal>` with typed `Refusal` (`UnknownId`, `NotKnown`, `KindNotAllowed`, `NeedsSecretClass`, `NoSuchProfile`), `Expected` and `PendingWindow` (20 s), bound helpers (names at 48 clusters, address grammar, 24 rows, 128-byte ids).
- [ ] `PowerProfiles::cycle`: copy of `next_profile` and its tests from `power.rs`, head comment with source path and commit; adapt to the typed list.
- [ ] Tests: every refusal; a hostile inventory (huge, duplicate ids, control characters, non-UTF-8-lossy names); a pin that no `Action` variant carries a string that could be a secret (an exhaustive match in a test, so a new variant must be classified).
- [ ] `settings.rs`: `network` and `bluetooth` appended to the default end zone after `tray`; a missing `bar` key keeps today's value for old files except for those two new pieces (the existing default-follows tests extended).

### Task 2 — Network and Bluetooth workers
- [ ] `connectivity/network.rs`: worker on `NmNetwork::connect_system()` behind `Box<dyn Network>`; first snapshot, then event-driven by `cuprita_core::nm::watch` plus a 30 s safety re-read; diff before publishing; bounded `sync_channel(4)` for commands; each command validated by Task 1 against the last snapshot; `connect(id, None)` only; result by the next observation (D5); typed error by kind, detail to the journal only; reconnect backoff (D10).
- [ ] `connectivity/bluetooth.rs`: same over `BluezBluetooth`; never `set_discovering`, `pair`, `forget`; `connect_known`/`disconnect_known` only for paired devices.
- [ ] Tests over `FakeNetwork`/`FakeBluetooth`: only allowed calls reach the backend (a recording wrapper), `connect` always gets `None`, an unknown id is dropped, queue full is refused, shutdown joins within a bound, a backend that returns each error kind surfaces the right typed state, a vanished service degrades and recovers.

### Task 3 — Power worker
- [ ] `connectivity/power.rs`: one worker, system bus, UPower `DisplayDevice` and PPD (both names), poll 5 s, publish on change, `SetProfile`/`CycleProfile` validated against `Profiles`, `Properties.Set` only on `ActiveProfile`.
- [ ] Tests against fake UPower/PPD services on a private bus (Task 4's fixtures): reads, change publishing, set accepted, set of an unlisted profile refused before any call, the other PPD name, absent services.

### Task 4 — Fake system services on a private bus
- [ ] The fake crate `celestina-fake-system` and the integration test: a `dbus-daemon` of the test's own (the `test_bus.rs` recipe), in-process `zbus` services for UPower, PPD, a minimal NetworkManager (manager, one Wi-Fi device, two access points, one saved profile, one active connection, `Settings`) and a minimal BlueZ (`ObjectManager`, one adapter, two devices, one with `Battery1`); calls recorded so a test asserts what was and was not called. `examples/fake_system_bus.rs` starts the same services for the nested smoke.
- [ ] `tests/connectivity_bus.rs`: the real `NmNetwork` and `BluezBluetooth` read the fakes through `DBUS_SYSTEM_BUS_ADDRESS` (set once, in a test binary of its own).

### Task 5 — Pieces and pages
- [ ] The four pieces (D7): icon from state (signal bars, adapter off, battery level and charging, profile), accessible name from `copy.rs`; click opens the page; for `power-profile` a middle click cycles; `bar()` is `None` when absent.
- [ ] The three pages: rows are keyed and focusable, one action per row, pending and failure shown per row by kind, an `Editar…` row (D11), the explanation line when the service is unavailable, empty states.
- [ ] `BarCx`/`PageCx` gain one read-only `connectivity` slice; `PieceAction` gains `Connectivity(Action)`; `copy.rs` gains the product copy (Spanish).
- [ ] Pure tests: piece views for each state, keys unique, page `wants`, no page shows more than the bounds, characterisation of the existing three pages unchanged.

### Task 6 — Island and app integration (design-critical)
- [ ] `Message::Connectivity(ConnectivityEvent)` (one variant), `App` field and arm, handle started at launch (no duty) and joined at exit; the page opens from the piece table; row activation becomes `IslandAction::Connectivity(Action)` and the app forwards it to the handle; `OpenEditor` through `launcher::spawn`.
- [ ] Verbs: `island open network|bluetooth|power` come from the table (no code); no new verb.
- [ ] Tests: input on a row yields the typed action; a refused/unknown action never reaches a worker; shutdown joins every worker; a page for an absent service shows its explanation.

### Task 7 — Smoke, evidence and documents
- [ ] `smoke.sh`: a run with the fake system bus (`examples/fake_system_bus.rs` on a private `dbus-daemon`) and a layout placing the four pieces; requires exit 0, frames on every output, the pieces present, rest frames 0 after the first settle, `connectivity: network ok bluetooth ok power ok`; a second run with `DBUS_SYSTEM_BUS_ADDRESS` at a socket that does not exist requires `unavailable` for all three and no panic. No run touches the real system bus or network.
- [ ] `celestina/docs/evidence/2026-10-17-connectivity.md`, VALIDATION rows `VAL-CEL2-NETWORK`, `VAL-CEL2-BLUETOOTH`, `VAL-CEL2-POWER` (queued, `pending`), `STATUS.md` line, the ledger row `active` with the unit's intent, decision records.

### Task 8 — Review and exit
- [ ] Final review by a model other than the one that implemented each task, at high reasoning, on the whole diff against this plan, the root contract and the safety rules (no secret path, no unlisted call, bounded input, no blocking on the loop, no real-system access in any test).
- [ ] Fix pass; the unit's fast checks green; `celestina --check`; `smoke.sh` green; `verify-production` for the owner as the landing will run it (never `build-production.sh` in the worktree: if the contract requires evidence of verify, run it only by the registered entry and record it).
- [ ] Hand-over: report to the author with the diffstat, what is proven and what is not, and wait for the author's word to land.

## Procedure

The tasks above, executed one by one by separate agents, each followed by two cross-reviews and three fix passes; the commands each task ran are in [the evidence](2026-10-17-connectivity.md).

## Result

See [the evidence](2026-10-17-connectivity.md): this record plans and does not report results.

## Limits

A plan record: it may differ from what was built, and it proves nothing by itself.
