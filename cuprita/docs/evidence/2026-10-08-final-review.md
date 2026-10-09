# The final review's fix wave — CUP-1-G

- **Date:** 2026-10-08
- **Scope:** `CUP-1-G` of
  [`../plans/active/2026-10-08-cup-1-foundation.md`](../plans/active/2026-10-08-cup-1-foundation.md):
  `celestina-rs/crates/cuprita-core/src/` (`bus.rs`, `nm/mod.rs`,
  `bluez/mod.rs`, `wpctl.rs`, `error.rs`, `model.rs`, `order.rs`),
  `celestina-rs/crates/cuprita-core/tests/order.rs`,
  `cuprita/qml/dialogs/WifiPasswordDialog.qml`,
  `cuprita/qml/dialogs/PairingDialog.qml`,
  `cuprita/qml/components/NetworkRow.qml`,
  `cuprita/tests/qml/tst_network_page.qml`, `cuprita/Cargo.toml`,
  `cuprita/STATUS.md`, `cuprita/ROADMAP.md`, the audio evidence, the ledger
- **Environment:** Qt 6.11.2, cxx-qt 0.9.1, zbus 5, `qmltestrunner`
  offscreen
- **Artifact:** the release binary in the shared Cargo target, not installed
- **Change kind:** maintenance (1.0.0 stays; no history row)

## Procedure

The final whole-program review of Cuprita 1.0 (main at `4e99ccd4`) listed
the items below. Each is fixed here or deferred to the roadmap's "Later"
with its reason.

| # | Review item | What this unit does |
| --- | --- | --- |
| 1 | The NetworkManager connection has no method timeout (BlueZ has 90 s): a hung NetworkManager or an open polkit prompt blocks the network worker and every queued command | Fixed: `nm` connects through `bus::system_connection(CALL_TIMEOUT)` with 120 s, room for a polkit prompt. `bus::proxy` and `bus::map_fdo` (generic over `E: From<Fault>`) replace the two copies in `nm/mod.rs` and `bluez/mod.rs`; each client keeps a one-line `proxy` that fixes its service and error type |
| 2 | `STATUS.md` still said CUP-1-F was open and would publish 1.0.0 | Fixed: rewritten for 1.0.0 as the current version, what is live, the author's `VAL-C`/`VAL-D`/`VAL-E`, and what this unit fixes |
| 3 | The audio watcher re-ran `wpctl status` every 2 s even while `pw-mon` ran, against the audio evidence and the roadmap | Fixed as ruled: the documents win. `next_wait(monitor_alive, pipewire_present)` returns `None` (wait for `pw-mon` only) while `pw-mon` runs and the last read worked, and 2 s otherwise; the 300 ms settle per burst is kept. Test `the_watcher_polls_only_without_pw_mon_or_pipewire`. The audio evidence and `STATUS.md` say the same now |
| 4 | Notices mixed languages: the generic failure sentence carried the service's raw English detail | Fixed: `bus::fault_of_name` maps `UnknownConnection`, `UnknownDevice`, `DoesNotExist`, `UnknownObject` to `NotFound`, `NotReady` and `ConnectionNotAvailable` to `Unavailable`, `PermissionDenied`, `AccessDenied`, `NotAuthorized` to `Denied`, `InProgress` and `Busy` to the new `Busy` (an operation-in-progress sentence). Any other detail is logged with `eprintln!` and the notice says only that the service answered with an error (`service_failed()`); `wpctl` failures do the same. Tests: `error_names_map_to_faults`, `a_service_detail_never_reaches_the_notice`, `bluez_errors_map_to_the_section_errors`, `errors_classify` |
| 5 | The passphrase dialog accepted any length; no input-method hints | Fixed: the connect button stays disabled and a one-line length hint shows unless the passphrase has 8–63 characters or is a 64-digit hexadecimal key (the raw WPA key, which NetworkManager also accepts). Both fields set `Qt.ImhSensitiveData \| Qt.ImhNoPredictiveText`. Test `test_the_passphrase_length_gates_connect`; `test_unknown_protected_network_asks_for_the_password` now types 6 then 8 characters |
| 5b | PIN field: `Qt.ImhDigitsOnly` where the request is numeric | Deferred: BlueZ's numeric `RequestPasskey` shares the `pin` token with the text `RequestPinCode`, so the dialog cannot tell them apart; a token of its own needs `cuprita-core`, the controller and `PairingDialog` |
| 6 | A WEP-only network read as protected (like WPA) and was refused on joining | Fixed: `Security::Wep` (label key `wep`) reads as WEP and unsupported; `connect` still refuses it, now by its security instead of a separate route flag |
| 7 | `pw-mon` not tied to Cuprita's death (`PR_SET_PDEATHSIG`) | Deferred: it needs `unsafe` `prctl` in `pre_exec`, which the root contract forbids without a recorded exception; a killed Cuprita's `pw-mon` ends on its next write |
| 8 | A stream whose link `pw-link` cannot name falls back to the nickname guess | Deferred: needs the port-to-node graph from `pw-dump`, a new parser |
| 9 | A BlueZ call can hold the section up to 90 s, also while airplane mode powers the adapter off | Deferred: zbus sets the timeout per connection; shorter calls need a second connection for `Pair()` |
| 10 | Airplane mode lives only in memory | Deferred: persisting it, or reading rfkill, is a design decision |
| 11 | `RequestAuthorization` is answered by policy | Deferred: a dialog for an unexpected request is a design decision |
| 12 | A failed `Trusted` write is only logged | Deferred: the pairing itself succeeded; a notice for it needs wording the design lacks |
| 13 | An access point's signal refreshes only on NetworkManager's signals | Deferred: a periodic scan read costs a wake-up for a small gain |
| 14 | The Tab path into a row's buttons and sliders has no test | Deferred: test-only work, recorded in the exit evidence already |
| 15 | `cuprita_core::agent::Agent` duplicates the BlueZ agent's live slot | Kept and deferred: it is not dead code; `cuprita/src/controller/bluetooth.rs` holds it as the controller's pending request, so removing it is a merge of the two slots, not a deletion |
| 16 | `cxx`, `cxx-qt`, `cxx-qt-lib` and the build dependencies had no justification in `cuprita/Cargo.toml` | Fixed: a comment above each |

## Result

| Command | Where | Exit |
| --- | --- | --- |
| `cargo test -p cuprita-core` (66 passed: 39 unit, 27 integration) | `celestina-rs/` | 0 |
| `cargo clippy -p cuprita-core --all-targets -- -D warnings` | `celestina-rs/` | 0 |
| `cargo fmt --all --check` | `cuprita/` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | `cuprita/` | 0 |
| `cargo test` (9 passed) | `cuprita/` | 0 |
| `cargo build --release --locked` | `cuprita/` | 0 |
| `sh cuprita/scripts/qml-tests.sh` (41 passed) | root | 0 |
| `sh cuprita/scripts/smoke.sh --binary <shared target>/release/cuprita` (`networks=4 devices=2 endpoints=3 streams=2 sink=40 source=50`) | root | 0 |
| `bash scripts/qmllint-cxxqt.sh cuprita` (0 warnings) | root | 0 |
| `bash scripts/check-architecture-contract.sh` | root | 0 |
| `python3 scripts/check-language-contract.py` | root | 0 |
| `bash scripts/check-documentation-contract.sh` | root | 0 |

## Limits

- The 120 s timeout, the Spanish notices for real NetworkManager and BlueZ
  errors, and the watcher without polling were not exercised against the
  live services; the mappings and the wait decision are unit-tested.
- A polkit prompt left open longer than 120 s now ends the call with an
  error notice; the person can try again.

## Follow-up

Author checks pending: `VAL-C`, `VAL-D`, `VAL-E` in
[`../../VALIDATION.md`](../../VALIDATION.md). The deferred items are in the
"Later" list of [`../../ROADMAP.md`](../../ROADMAP.md).

## Landing

- **Base revision:** `4e99ccd4c79e9fab6b2c6bd34b55f273ff21b8da`
- **Check:** `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7adf05e27452365011df4ac8664480c40a38983ef87a025573e81a5588aac661, verification_fingerprint sha256:9500c47fabc5aac3296cf82356ae3d29ca92d30a9d259c202ca80886e768e032; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d75bd20dd7875dfb9459347c1e6fc198865142789230cf81ce8599dfffe451d3, verification_fingerprint sha256:3b76ad6fff93639836638f8c746c5616891b52069963d74a512f4bbc3fe73467
- **Deploy:** after the push: cuprita: deploy-production.sh, status-production.sh
