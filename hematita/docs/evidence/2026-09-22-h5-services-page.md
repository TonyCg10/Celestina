# The Services page and the polkit-mediated paths — H5-B

- **Date:** 2026-09-22
- **Scope:** `H5-B` of
  [`../plans/active/2026-09-22-h5-services.md`](../plans/active/2026-09-22-h5-services.md):
  the sampler's service section over both buses, `HematitaServices`,
  `privilege.rs`, the foreign-process path in `HematitaProcesses`, the shared
  `ConfirmDialog`, `ServicesPage`/`ServiceRow`, the fifth section, and the
  three items `H4-D` deferred
- **Environment:** the author's checkout, an AMD Ryzen 7 9800X3D session
  under niri; `polkitd` running, **no authentication agent**
- **Artifact:** `hematita/target/production-artifact.toml` (verified, not
  deployed)

## What changed

1. **`src/privilege.rs`** is the one place Hematita runs anything as root
   (ADR 0010): `Signal::{Terminate, Kill}` maps to `-TERM`/`-KILL`, and
   `signal_as_root(pid, signal, report)` spawns the named thread
   `hematita-pkexec`, runs `/usr/bin/pkexec /usr/bin/kill -SIG <pid>` with a
   fixed argument shape and no shell, and hands
   `services::outcome_of_pkexec(status.code())` — or `Failed` when the spawn
   itself failed — to `report`, which queues it onto the Qt thread. `pkexec`
   and `Command::new` are spelled nowhere else in the crate.
2. **`src/sampler.rs`** gained the service section: `SERVICE_TICKS = 5`,
   `ServiceSnapshot { system, user }` (a `Section` per bus),
   `Snapshot.services: Option<ServiceSnapshot>` — `Some` on generation 1 and
   every fifth generation, so the page fills at once and then stays live —
   and `list_units`/`sample_services`, which open
   `zbus::blocking::Connection::{system, session}` lazily, keep them in
   `run`'s locals, retry a bus that would not open on the next service tick,
   and call `ListUnits` through a `zbus::blocking::Proxy` on the sampler
   thread. A bus that is not there is `Unreadable` with the label
   `system bus`/`session bus`; a reply that does not match the declared
   `a(ssssssouso)` row shape is `Malformed`.
3. **`src/services.rs`** is the `HematitaServices` hub: the contract's
   index-aligned lists (`unitNames`, `unitDescriptions`, `unitScopes`,
   `unitActives`, `unitSubs`, `unitKinds`, `unitActionable`), the state QML
   sets (`filterText`, `showSystem`, `showUser`, `servicesOnly`, default
   `true`/`true`/`true`), the counts, a per-bus availability triple, the
   action triple (`actionOutcome`, `actionUnit`, `actionKind`) and
   `startFailed`. `apply` stores both sections and re-projects;
   `refresh` concatenates the available units (user first), calls
   `services::project` and publishes `revision` last. `start_unit`,
   `stop_unit` and `restart_unit` share `act`, which refuses a
   non-actionable kind without a thread and without a bus, otherwise sets
   `pending`, spawns the named thread `hematita-systemd`, opens the bus the
   scope names, calls the method with `(name, "replace")`, discards the job
   path and queues `outcome_of_call` back through `qt_thread()`.
4. **`src/processes.rs`**: `owned_pid` now answers
   `Result<Pid, Refusal>` — `Refusal::Foreign { start_ticks }` for another
   user's listed process, `Refusal::NotAllowed` for PID ≤ 1, this process, an
   unlisted PID or a number that is not a PID. `send_signal` re-validates
   identity against `/proc` exactly as before and then either signals
   directly (own process), asks `privilege::signal_as_root` and reports
   `pending` (foreign), or answers `refused`.
5. **QML**: `ConfirmDialog.qml` generalises `KillDialog.qml` (deleted) —
   `ask(question, confirmText, payload)` and `confirmed(var payload)` over
   the same `CelestinaModalLayer` + `GlassCard` body — and serves both the
   kill in `ProcessTable` and stop/restart in `ServicesPage`.
   `ServiceRow.qml` is a non-focusable `AbstractButton` with a state dot, the
   name, the description and a scope chip. `ServicesPage.qml` is the search
   with its 150 ms debounce, the three filter toggles, the three actions, the
   sentence line (per-bus availability, then the action outcome, `pending`
   and `no-agent` included) and one `ListView` that is the page's single Tab
   stop, with `ProcessTable`'s `anchorCursor` discipline. `Main.qml` gained
   the fifth section, the hub, the page and the smoke line.
6. **The `H4` deferrals**: `SENSOR_FACTS_TICKS = 30` re-enumerates every
   chip's labels and limits every thirtieth tick, so a chip that gains a
   channel is seen within half a minute; `sensors.rs` retains a channel's
   session extremes while its chip is still listed (keys matched by the
   chip's `chipKey/chipName/` prefix) rather than only while this tick read
   the value file; `publish::power_load(value, cap)` grades a power reading
   against the chip's own cap with the same two thresholds as
   `thermal_load`, and `ChannelKind::Power` now uses it.

## The ADR's invariants, checked against the code

```sh
grep -rn "pkexec" hematita/src hematita/qml
grep -rn "Command::new" hematita/src
```

- `Command::new` appears on exactly one line of the crate,
  `src/privilege.rs:42`, inside `signal_as_root`.
- The only executable use of `pkexec` is `src/privilege.rs` (the `PKEXEC`
  constant and the `hematita-pkexec` thread name); every other hit is a doc
  comment in `src/processes.rs` explaining which path a foreign PID takes.
  No QML file mentions it.
- The argument shape is fixed in code: `PKEXEC`, then `KILL`, then
  `-TERM`/`-KILL` from `Signal::flag`, then one `pid.to_string()`. No shell,
  no user-supplied string, no environment passed.
- Nothing is cached: each action is one `pkexec` process or one bus call on a
  freshly opened connection, and no authorised connection is kept.
- Every outcome is typed: `done`, `pending`, `no-agent`, `denied`, `failed`,
  `refused`, and the page has a sentence for each.

## Procedure

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
cd hematita && cargo test --release --locked --all-targets \
  && cargo clippy --release --all-targets --locked -- -D warnings \
  && cargo fmt --all --check
hematita/scripts/verify-production.sh
# 12 s offscreen, isolated XDG dirs and a session-bus address pointing nowhere
QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 \
  timeout 12 ./target/release/hematita
```

## Result

- **Exit:** 0 for the build, the three release-profile checks and the
  verification; `124` (still alive) for the 12-second run.
- **Observed:** `cargo test --release --all-targets` — `19 passed; 0 failed`
  in the binary's own tests (the four new `services` tests, the new
  `privilege` test, the new `power_load` and `chip_prefix` tests, and the
  rewritten PID-classification test). `clippy -D warnings` and
  `fmt --all --check` clean on the first run. `verify-production.sh`:
  `hematita-core` `67 passed` plus captures `11 passed`,
  `qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline
  warning(s))`, and `smoke: OK — binary alive for 10 s, every section was
  shown, the first row published the CPU contract, the Sensors page published
  chips, the Services page listed system units, no QML errors, no
  auto-bindings`. The 12-second run printed exactly
  `hematita-shape cpu 3 60`, `hematita-sensors 9 44` and
  `hematita-services 143 true false`, and
  `grep -E 'TypeError|ReferenceError|Unable to assign|Cannot read property'`
  found nothing.
- The `143 true false` line is the state this run is meant to prove: the
  system bus answered with 143 shown service units, the session bus
  deliberately pointed nowhere, and the page rendered the missing-bus
  sentence without a single QML error.

## Limits

- **No action was performed.** No `StartUnit`, `StopUnit` or `RestartUnit`
  was called and no signal — direct or through `pkexec` — was sent during
  any of these runs; the smoke walks the sections and never touches a button.
  The action paths are proven by their unit tests and by reading the code,
  not by execution.
- **This session has no authentication agent**, by the author's order
  (the shell that will provide one is in standby). Every privileged path can
  therefore only answer `no-agent` here today; that this is what polkit and
  `pkexec` actually return on the author's machine, and that an agent makes
  the prompts work, is `VAL-H5`.
- Keyboard, focus, the confirmation's focus containment, hover and the row
  colours are not proven offscreen: `VAL-H5` walks them on the real session.
- The service listing was read from the real system bus in the verification
  runs, but no unit's state was changed, so the page's live state transition
  after an action is unproven here.
- `scripts/complete-production.sh` was not run and nothing was installed:
  that is `H5-Z`.

## Follow-up

`H5-Z` (implementation exit and 0.6.0). `VAL-H5` is recorded pending in
`VALIDATION.md` and does not block this closure.
