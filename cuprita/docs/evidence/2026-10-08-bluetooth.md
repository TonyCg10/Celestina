# The BlueZ client, the pairing agent and the Bluetooth page live — CUP-1-D

- **Date:** 2026-10-08
- **Scope:** `CUP-1-D` of
  [`../plans/active/2026-10-08-cup-1-foundation.md`](../plans/active/2026-10-08-cup-1-foundation.md):
  `celestina-rs/crates/cuprita-core/` (`bluez/`, `bus.rs`, `agent.rs`,
  `order.rs`, `error.rs`, `nm/`, `examples/snapshot.rs`, the `bluez`
  feature), `cuprita/src/` (`agent.rs`, backend, worker, Bluetooth and
  network controllers), `cuprita/qml/` (`dialogs/PairingDialog.qml`,
  `pages/BluetoothPage.qml`, `components/DeviceRow.qml`, `Main.qml`),
  `cuprita/tests/qml/`, `cuprita/build.rs`
- **Environment:** Rust 1.97.1 (`celestina-rs/`) and 1.98.1 (`cuprita/`),
  cxx-qt 0.9.1, Qt 6.11.2, zbus 5, BlueZ 5.87 on the system bus
- **Artifact:** the release binary in the shared Cargo target, not installed

## Live bus (Step 1)

Read unprivileged with `busctl --system` (tree, introspect, get-property),
exit 0 each. Device addresses are redacted as `dev_A`, `dev_B`, `dev_C`.

| Object | Path | Read |
| --- | --- | --- |
| Object manager | `/` | `org.freedesktop.DBus.ObjectManager`: `GetManagedObjects` → `a{oa{sa{sv}}}`, signals `InterfacesAdded(oa{sa{sv}})`, `InterfacesRemoved(oas)` |
| Agent manager | `/org/bluez` | `org.bluez.AgentManager1`: `RegisterAgent(os)`, `RequestDefaultAgent(o)`, `UnregisterAgent(o)` |
| Adapter | `/org/bluez/hci0` | `org.bluez.Adapter1`: `Powered` (b, writable) true, `Discovering` (b) false, `PowerState` "on", `StartDiscovery()`, `StopDiscovery()`, `RemoveDevice(o)`; `Properties.PropertiesChanged` |
| Game controller | `/org/bluez/hci0/dev_A` | `org.bluez.Device1`: `Address`, `Name`, `Alias` (writable), `Icon` "input-gaming", `Class`, `Paired` true, `Connected` false, `Trusted` (writable) true, `Pair()`, `CancelPairing()`, `Connect()`, `Disconnect()`; `org.bluez.Input1` |
| Earbuds | `/org/bluez/hci0/dev_B` | `Device1`: `Icon` "audio-headset", `Paired` true, `Connected` false |
| Phone | `/org/bluez/hci0/dev_C` | `Device1`: `Icon` "phone", `Paired` true, `Connected` true; `org.bluez.Battery1.Percentage` (y) |

## Procedure

| Command | Where | Exit |
| --- | --- | --- |
| `busctl --system tree org.bluez` | any | 0 |
| `busctl --system introspect org.bluez /org/bluez/hci0` | any | 0 |
| `busctl --system introspect org.bluez /org/bluez` | any | 0 |
| `busctl --system introspect org.bluez /org/bluez/hci0/dev_A` | any | 0 |
| `busctl --system get-property org.bluez <device> org.bluez.Device1 Icon Paired Connected` (three devices) | any | 0 |
| `cargo test -p cuprita-core` (43 passed) | `celestina-rs/` | 0 |
| `cargo clippy -p cuprita-core --all-targets --locked -- -D warnings` | `celestina-rs/` | 0 |
| `cargo build -p cuprita-core --no-default-features` | `celestina-rs/` | 0 |
| `cargo build -p cuprita-core --no-default-features --features nm` | `celestina-rs/` | 0 |
| `timeout 30 cargo run -p cuprita-core --example snapshot -- --discover` | `celestina-rs/` | 0 |
| `timeout 20 cargo run -p cuprita-core --example snapshot -- --agent` | `celestina-rs/` | 0 |
| `timeout 14 <debug>/examples/snapshot --watch`, with `snapshot --discover` run from a second process meanwhile | `celestina-rs/` | 124 (the timeout, expected) and 0 |
| `cargo fmt --all --check` | `cuprita/` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | `cuprita/` | 0 |
| `cargo test` (7 passed) | `cuprita/` | 0 |
| `cargo build --release --locked` | `cuprita/` | 0 |
| `sh cuprita/scripts/qml-tests.sh` (29 passed) | root | 0 |
| `sh cuprita/scripts/smoke.sh --binary <shared target>/release/cuprita` | root | 0 |
| `bash scripts/qmllint-cxxqt.sh cuprita` (0 warnings, baseline 0) | root | 0 |
| `bash scripts/check-architecture-contract.sh` | root | 0 |
| `python3 scripts/check-language-contract.py` | root | 0 |
| `bash scripts/check-documentation-contract.sh` | root | 0 |

## Result

- **Exit:** 0 for every check.
- **Example output** (the Bluetooth part; addresses redacted as above, the
  owner's device names as `<phone name>` and `<earbuds name>`):

  ```text
  BluetoothSnapshot { powered: true, discovering: false, devices: [
  address: "dev_C", name: "<phone name>", kind: Phone, paired: true, connected: true, trusted: true, battery: Some(40),
  address: "dev_B", name: "<earbuds name>", kind: Audio, paired: true, connected: false, trusted: false, battery: None,
  address: "dev_A", name: "DualSense Wireless Controller", kind: Input, paired: true, connected: false, trusted: true, battery: None ] }
  searching for five seconds
  during the search: discovering=true devices=5
  after: discovering=false
  ```

  The connected phone comes first, then the paired devices by name; the
  phone's battery comes from `Battery1`. The five-second search added two
  named strangers; nameless advertisers are not listed.
- **Agent:** `--agent` printed
  `agent registered at /org/celestina/cuprita/agent as the default` and
  `agent unregistered`: `RegisterAgent(…, "KeyboardDisplay")` and
  `RequestDefaultAgent` succeeded unprivileged, with no polkit prompt, and
  the handle's drop unregistered it. It was exercised this once; no pairing
  was started.
- **Watcher:** while a second process searched, the watch printed
  `bluetooth change: powered=true discovering=true devices=5` twice, then
  `bluetooth change: powered=true discovering=false devices=5`: the
  adapter's `Discovering` and the devices found arrive as coalesced changes;
  signal-strength updates during the search do not trigger re-reads.
- **Window over the fakes:** the smoke printed
  `cuprita-smoke: networks=4 devices=2 endpoints=3 streams=2 sink=40 source=50`.
- **Page and dialog:** the page tests check the two scripted rows and the
  battery; «Conectar» connects the headphones; the row menu's «Olvidar»
  forgets them; «Buscar» shows the spinner and turns into `"Detener búsqueda"`;
  the controller's `agentRequest("confirm", "Auriculares", 123456)` opens
  the dialog showing «123456», «Coincide» calls
  `answerAgent("confirm", "yes")` and Escape calls
  `answerAgent("confirm", "cancel")`; a PIN typed and entered calls
  `answerAgent("pin", "0427")` and empties the field; a shown passkey
  (`display`) closes on `agentClosed`.
- **Airplane mode:** `airplane_mode_powers_the_adapter_off` (in
  `cuprita/src/controller/bluetooth.rs`) runs the Bluetooth worker over
  `FakeBluetooth`, calls `airplane_on()` — what the network controller's
  `setAirplane(true)` calls — and reads the adapter powered off.

## Design notes

- **Shared bus mechanics:** `map_error_name` and the debounced watcher moved
  from `nm/` to `bus.rs` (`Fault`, `fault_of_name`, `watch_signals`); `nm`
  and `bluez` keep only their own names and signal filters. `Fault` turns
  into either section's error; the tests moved with the code.
- **Errors:** `org.bluez.Error.AuthenticationCanceled` and
  `…AuthenticationRejected` read as `BluetoothError::pairing_cancelled()`
  (`Failed`, its Spanish text in `error.rs`); `NotReady` (adapter off) reads
  as `Unavailable`; `AlreadyConnected`, `AlreadyExists` (pair),
  `NotConnected` (disconnect), `InProgress` (start a search) and stopping a
  search Cuprita did not start are success; `DoesNotExist` on forget is
  `NotFound`.
- **Agent:** `serve_agent` exports `org.bluez.Agent1` on a connection of its
  own. Its methods are async (zbus runs each call in its own task), so a
  `Cancel` arrives while a `RequestConfirmation` waits; the answer reaches
  the waiting call through a small slot with a waker, without new
  dependencies. `RequestAuthorization` and `AuthorizeService` answer yes for
  a paired device or the one Cuprita is pairing. The pure state machine
  gained `AgentRequest::Cancel` (withdraws the waiting request),
  `AgentRequest::presentation()` and `AgentAnswer::from_tokens()`.
- **Discovery** belongs to the client's connection: BlueZ ends it when
  Cuprita exits.

## Limits

- Pairing, connecting, disconnecting and forgetting were not exercised live:
  they change the author's bonds and need the author's devices. That is
  `VAL-D` below.
- Cuprita's `Pair` goes out on the client's connection, the agent on its
  own, so pairing reaches Cuprita's agent only as BlueZ's default agent.
  When another agent (a desktop's) claims the default after Cuprita
  started, its dialog answers instead.
- A pairing the device starts with no code at all ("just works",
  `RequestAuthorization`) is refused unless the device is already paired or
  is the one Cuprita is pairing; pair from Cuprita instead.
- A failed `Trusted = true` after a successful `Pair()` is only logged
  (`eprintln!`); the device then asks the agent again on its next
  reconnect.
- The client's 90 s call timeout against BlueZ's ~60 s wait for each agent
  answer is untested live: a slow PIN entry is expected to end in BlueZ's
  own `Cancel`, not in the client's timeout.
- The live window was not run against BlueZ in this unit, to register the
  agent only once; the example covers the same client.
- If the watcher cannot subscribe, the page still re-reads after every
  command but not on outside changes.

## Review fixes (round 1)

- **One waiting request:** `ask()` claims the agent's slot for the
  requesting device and replies `org.bluez.Error.Rejected` at once while
  another request waits, so a request is never both waiting and unshown
  and «Coincide» always answers the device the dialog names.
  `cancel_waiting` cancels only the waiting device's request (BlueZ's
  `Cancel` names none: it withdraws the one request outstanding with the
  agent, which is the waiting one); an abandoned call frees the slot.
  Unit tests: `a_second_request_while_one_waits_is_refused`,
  `a_cancel_for_another_device_is_ignored`,
  `an_abandoned_call_frees_the_slot`.
- **Airplane keeps the adapter off:** the network controller reports its
  flag to `bluetooth::set_airplane` on a toggle and on every snapshot whose
  flag changes (so a launch that already reads airplane powers the adapter
  off). While it is on, `setPowered(true)` is refused with an info notice
  (`airplane_mode_message()` in `error.rs`) and the page's adapter switch is
  disabled (`airplane` property); the worker re-checks, so a queued switch-on
  cannot undo it. Airplane off does not power the adapter back on.
  `airplane_mode_powers_the_adapter_off_and_keeps_it_off` covers off, a
  refused switch-on, and switch-on after airplane ends;
  `test_airplane_mode_disables_the_adapter_switch` covers the page.
- **Shown passkey closes precisely:** commands are tagged in order; only the
  `Done` of a `pair` closes a shown passkey. A device-initiated one closes
  on BlueZ's `Cancel` or when a snapshot shows that device paired.
- **«Cancelar» on a shown passkey** calls `Device1.CancelPairing()` for that
  device (the address of the pair under way, else the snapshot's device of
  that name) on a thread of its own, since the worker is blocked in
  `Pair()`; failures are logged.
- **No notice without BlueZ:** the airplane power-off treats `Unavailable`
  as nothing to do and logs it; the network controller logs when the
  Bluetooth worker is not running (the returned `bool`).
- **Call timeout:** the client's connection is built with
  `zbus::blocking::connection::Builder::method_timeout(90 s)`; zbus sets the
  timeout per connection, so every client call shares it.
- **STATUS:** the active list matches the ledger (A, C, D active; B done).

| Command | Where | Exit |
| --- | --- | --- |
| `cargo test -p cuprita-core` (46 passed) | `celestina-rs/` | 0 |
| `cargo clippy -p cuprita-core --all-targets --locked -- -D warnings` | `celestina-rs/` | 0 |
| `cargo fmt --all --check` | `cuprita/` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | `cuprita/` | 0 |
| `cargo test --locked` (7 passed) | `cuprita/` | 0 |
| `cargo build --release --locked` | `cuprita/` | 0 |
| `sh cuprita/scripts/qml-tests.sh` (30 passed) | root | 0 |
| `sh cuprita/scripts/smoke.sh --binary <shared target>/release/cuprita` (`devices=2`) | root | 0 |
| `bash scripts/qmllint-cxxqt.sh cuprita` (0 warnings) | root | 0 |
| `bash scripts/check-architecture-contract.sh` | root | 0 |
| `python3 scripts/check-language-contract.py` | root | 0 |
| `bash scripts/check-documentation-contract.sh` | root | 0 |

## Author check (pending)

`VAL-D` in [`../../VALIDATION.md`](../../VALIDATION.md): with the deployed
Cuprita, search, pair the headphones through the pairing dialog (confirm the
passkey or type the PIN), connect them, then unpair them with «Olvidar».

## Landing

- **Base revision:** `b0f0fcdcd53d794f17706f1239d7137c54982575`
- **Check:** `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8b702307c4df7349566d2379c9c4aae683384a82a3a124c6a292bd5ddc3a89de, verification_fingerprint sha256:5d0d07b8b0a93ac32c185a43e64112cfb1f2374099f9d7b1f44690e896d0ac4b; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:333ccbeef52d3ac4e3c7eb3d41abfccf981ff725634273eb444410ebc946b3e1, verification_fingerprint sha256:44df1240f44cecca7eace4ca7d97ad8ea31cb82e199593228429a6b6ec8665da
- **Deploy:** after the push: cuprita: deploy-production.sh, status-production.sh
