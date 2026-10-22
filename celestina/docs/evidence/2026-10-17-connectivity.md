# Evidence: CEL-2-F — the connectivity and power pieces

- **Date:** 2026-10-17
- **Scope:** unit CEL-2-F in the session worktree, not yet landed:
  `celestina-rs/crates/celestina-shell/src/{connectivity,settings,lib}.rs`,
  `celestina/src/connectivity/` (`mod`, `network`, `bluetooth`, `power`,
  `state`, `editor`), `celestina/crates/celestina-fake-system/`,
  `celestina/src/pieces/{network,bluetooth,battery,power_profile,conn_glyph}.rs`,
  `celestina/src/island/pages/{network,bluetooth,power,rows}.rs`, additive hunks
  in `celestina/src/{app,main,message,copy,command,lib}.rs`,
  `celestina/src/island/{mod,view,bar_tree}.rs`, `celestina/src/pieces/mod.rs`,
  `celestina/src/launcher/spawn.rs`, `celestina/examples/fake_system_bus.rs`,
  `celestina/tests/connectivity_bus.rs`, `celestina/Cargo.toml` and `Cargo.lock`,
  `celestina/scripts/smoke.sh`, `docs/projects.toml` (the production input
  `cuprita-core`; the verification inputs `celestina/examples` and
  `celestina/tests`), and the documents of this unit.
- **Environment:** Linux 7.2.10-1-cachyos; the toolchain
  `celestina/rust-toolchain.toml` pins; cargo with `--locked --offline`.
  The build used a private target directory (`CARGO_TARGET_DIR` outside the
  tree), not the `.cargo-target` directory the sibling worktrees share: a
  binary without this unit's code was found in the shared directory during the
  smoke (see Finding). Nothing is committed; the landing computes the
  inventory and seals the artifact.
- **Artifact:** none sealed by this record; the landing builds and seals its own.
- **Plan:** `celestina/docs/evidence/2026-10-17-connectivity-plan.md`; the
  ledger row is `celestina/docs/plans/active/2026-10-16-cel-2-parity.md`
  (status `active`; diffstat and inventory are the landing's).

## What was built

- **Pure domain (`celestina-shell`, `connectivity.rs`).** Bounded shell-owned
  states (`NetSection`, `BtSection`, `BatterySection`, `ProfileSection`),
  validated newtypes (`RowId`, `BtAddress`, `ProfileName`, `clean_text`), the
  closed `Action` enum, `validate` against the last confirmed inventory with
  typed `Refusal`s, `Expected` and a `PendingSet` with a 20 s window and an
  injected clock. `PowerProfiles::cycle` is copied by salvage from the halted
  core's `power.rs`; its head comment records the source and commit.
  `settings.rs` appends `network` and `bluetooth` to the default end zone after
  `tray`.
- **Workers (`celestina/src/connectivity/`).**
  - `network.rs` and `bluetooth.rs`: a generic lane (one loop, `Provider` trait)
    over `cuprita-core`'s `NmNetwork` and `BluezBluetooth` clients behind
    `Box<dyn …>`. One cuprita signal watcher per lane, started after the
    lane's first good read and kept for the lane's life (a failed retry starts
    none); a 30 s safety re-read, a 2 s re-read while a request is pending or
    a Wi-Fi join is followed (up to 180 s, so a failure that arrives after the
    link drops is reported: `NeedsSecret` is observable), and backoff 5 s
    doubling to 60 s. Only a Wi-Fi join is followed, because cuprita marks a
    failure only for Wi-Fi rows. A row that already read `Failed` when its
    join began keeps that mark as the previous join's while the join is
    younger than the 20 s pending window; a mark that still stands after the
    window is taken as this join's failure. Every clock a read or an open sets
    counts from when the call returned, so a snapshot slower than the 2 s
    re-read cannot make the lane re-read with no sleep. At rest a lane wakes
    once a second (not ten times); a re-read asked for while open (after a
    request, or after the watcher started) is honoured without that sleep.
    Names are cleaned by `clean_text`, which also drops zero-width and other
    invisible characters and a joining character with no base, so a name made
    only of them is empty and the page shows its placeholder. A network's
    signal is kept to its bar step (25, 50, 75, 100), so jitter inside a step
    publishes nothing and redraws nothing.
    Only snapshot, `set_wifi_enabled`, `connect(id, None)`, `disconnect`,
    `set_powered`, `connect` and `disconnect` of paired devices are ever called.
  - `power.rs`: one worker for UPower's display device and power-profiles-daemon
    (`org.freedesktop.UPower.PowerProfiles` first, `net.hadess.PowerProfiles`
    second), polled every 5 s, published on change only; `Properties.Set` on
    `ActiveProfile` only, after validation against `Profiles`. Every call
    carries `NoAutoStart`, so reading never starts a service through D-Bus
    activation. The connection is dropped, and reopened by the next attempt,
    only when a call finds the connection itself gone (an I/O error, which
    includes a call that outlived the 3 s timeout, or a disconnect). A reply
    from the bus proves the link alive whatever it says, so an absent UPower
    (`ServiceUnknown`, `NameHasNoOwner`) or UPower reporting no battery never
    drops it, and a profile change that finds no link opens one and reads the
    daemon's name before its one `Set`. A first version also dropped the link
    when no due read answered; cross-review found that on a machine without a
    battery the battery's retries then dropped it between two profile reads,
    and a profile change failed as unreachable without any call. The commands of one queued batch are each
    validated after a fresh poll, so two quick cycles advance two profiles.
  - `editor.rs`: the editing UIs found on `PATH` (no arguments from data),
    started through `launcher::spawn`'s new `spawn_program` (`setsid -f`).
  - `mod.rs`: `ConnectivityHandle` (start, `send`, `stop` with one shared 2 s
    bound for every worker's join), the `forward` function that validates before any send, and the
    journal lines the smoke reads: `network: ready`, `bluetooth: ready`,
    `power: battery ready`, `power: profiles ready` (and the matching
    `unavailable` lines). The first answer of each power service is announced
    once; later changes are silent. These lines were added in this unit's last
    pass (`announce_power`, `first_power_answer`, and one line in `Lane::read`).
- **Pieces and pages.** Four pieces (`network`, `bluetooth`, `battery`,
  `power-profile`) with the page each opens (network, bluetooth, power; the
  profile piece also cycles on a middle click). The three pages and their
  shared row list (`rows.rs`): one action per row, pending and failure per row,
  `Editar…` for the editors, the absent-service explanation. Product copy is in
  `copy.rs`; the icons are drawn (see Limits).
- **Fake system services (`celestina/crates/celestina-fake-system`).** Fake UPower, power-profiles-daemon, a minimal
  NetworkManager and a minimal BlueZ on a `dbus-daemon` of their own.
  `examples/fake_system_bus.rs` serves them for the nested smoke; its
  `DBUS_SYSTEM_BUS_ADDRESS=` line is read by the smoke wrapper, and closing its
  input ends its `dbus-daemon`. Journal lines of the connectivity code carry
  fixed tokens only (an error, step or refusal kind), never service text, a
  name, an id or an address.

## Decisions as they ended

- **D1 — popups are Island pages.** Built as planned (ADR 0017). The plan
  asked for the author's confirmation before the Island integration; the
  author approved D1 in chat on 2026-10-17, and the session recorded that
  approval in ADR 0017's context. It is not the author's own edit of the
  repository, so ADR 0017, like 0018 and 0019, stays `proposed` until the
  author sets it: the decisions README names no case where a session accepts
  a record. The six-tab
  strip is changed: each pill takes its title's width plus an equal share of
  the rest, so it fits the default body. This is a change of the shared tab
  strip, measured with PIL, not in a real render. Rebased onto the landed
  CEL-2-K and CEL-2-G units, the strip holds nine pages (Activity,
  Notifications, Tray, System, Clock, Clipboard, Network, Bluetooth, Power):
  this unit's sizing (each pill its label plus an equal share, shrinking with
  the label clipped) is kept, with CEL-2-G's caption-size labels and short
  forms; the PIL measurement of six titles below predates that and was not
  repeated for nine.
- **D2 — read-only state.** As planned, with one gap: `cuprita-core` has no
  Wi-Fi hardware kill switch, so `wifi_hardware_blocked` is always false and
  `Refusal::HardwareBlocked` never fires. A NetworkManager refusal to switch
  Wi-Fi shows as a timeout. BlueZ's icon is coarse in cuprita, so every input
  device shows the pointer glyph.
- **D3 — closed action list.** As planned (ADR 0018, `proposed`). `CycleProfile` is resolved
  to `SetProfile` before the call; `Refresh` re-reads and is not a call to the
  service. No variant exists for the forbidden actions.
- **D4 — no secret through the shell.** As planned (ADR 0019, `proposed`): `connect(id,
  None)` only, pinned by a unit test; no secret agent; the saved network that
  needs a secret fails with `NeedsSecret` and offers `Editar…`.
- **D5 — identity and confirmation.** As planned: rows are named by their
  stable id; a request must be in the last confirmed inventory, else it is
  refused; a pending row is confirmed by an observation or fails after 20 s.
  Finding: a join that cuprita marks `Failed` is shown as `NeedsSecret` for a
  passphrase row and as `Unreachable` otherwise; NetworkManager's real reason
  is not visible through cuprita.
- **D6 — `cuprita-core` as the reader (NetworkManager and BlueZ).** As
  planned. UPower and power-profiles-daemon are read by the shell's own `zbus`
  power worker, not by cuprita. The fakes live in their own crate,
  `celestina-fake-system` (depends on `zbus` only), which `celestina` names
  only under `[dev-dependencies]`; there is no `fake-system` feature and no
  self dev-dependency. A first version used a feature plus a self
  dev-dependency; cross-review found that it made `cargo test` leave a
  different binary (with the fakes inside) at `target/<profile>/celestina`,
  which the landing's verify step would have smoked or flagged as changed. The
  crate sits under `celestina/crates`, so it is a production input in
  `docs/projects.toml`, but the binary does not link it: see Result. The lock
  gains `cuprita-core` (with `zbus` only) and `celestina-fake-system`.
- **D7 — pieces and layout.** As planned. `network` and `bluetooth` are in the
  default end zone after `tray`; `battery` and `power-profile` are registered
  but not in the default layout. An absent service draws nothing.
- **D8 — power worker.** As planned, with its own loop (not the lane's): one
  `GetAll` per service per poll, the answering name remembered until it fails,
  backoff per service, a 3 s call timeout, a 20 s pending window.
- **D9 — no duty, no owner detection.** As planned: the workers start with the
  shell and only read; the author places the pieces. The `connectivity` slice of
  the state is read-only and does not change the Island's frames at rest
  (`set_connectivity` returns false for an unchanged state).
- **D10 — absent services degrade.** As planned: `Unavailable` published once,
  retry with backoff. Recovery is tested for the pure `Watch` (it publishes
  `Ready` again when a read answers) and end to end for the power worker: a
  private bus is killed under a live worker (both services go `Unavailable`),
  a new bus is started at the same socket, and battery and profiles come back
  and a profile change then works. The lanes' recovery is tested with
  scripted providers (a failed client and a recovered one), not on a restarted
  bus.
- **D11 — editing UIs.** As planned: `nm-connection-editor` and `blueman-manager`
  are started with no arguments from data, only when found on `PATH` (at most
  64 entries are examined, on the loop thread, once per deliberate `Editar…`);
  a missing program is said in the row.

## Procedure

From `celestina/`, with `CARGO_TARGET_DIR` set to a directory of the session's
own:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo build --locked --offline --bin celestina
sh scripts/smoke.sh --binary <target>/debug/celestina
```

The smoke builds `examples/fake_system_bus` into the directory of the
`--binary` it is given. It runs inside a nested niri
(`dev-session.sh --own-bus --monitor none`) and never on the live session; the
live `NIRI_SOCKET` was removed from its environment for the last run.

From `celestina-rs/`:

```sh
cargo test -p celestina-shell --locked --offline
```

From the worktree root:

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

## Result

- **`cargo fmt --all --check`:** clean.
- **`cargo clippy --workspace --all-targets --locked --offline -- -D warnings`:**
  clean.
- **`cargo test --workspace --locked --offline` in `celestina/`:** 881 passed,
  0 failed, 1 ignored, after the third review pass (the `celestina` library
  626, `connectivity_bus` 11, `wallpaper_decode` 26 with the 1 ignored, `cli`
  3, `notifications_bus` 1, `celestina-paint` 51 + 2, `celestina-ui` 81 + 7 +
  9, `celestina-wayland` 64; `celestina-fake-system` has no tests of its own).
- **`cargo test -p celestina-shell --locked --offline` in `celestina-rs/`:**
  200 passed in the library and 2 in the next binary, 0 failed.
- **The binary `cargo test` leaves, debug profile (private target
  directory).** On the final tree, `cargo build --bin celestina` had nothing
  to rebuild after the test run and the file at `target/debug/celestina` had
  sha256 `211e2134…f40b34` with 0 `strings` lines matching `fakes`,
  `FakeSystem` or `celestina_fake`; after `cargo test --workspace --no-run`
  the sha256 and the 0 count were the same. Review D measured the tree of the
  pass before (fix pass 2) the same way: `355e8fb6…216fe`, unchanged after
  `cargo test --workspace --no-run`, 0 fakes strings.
- **Release profile (measured by review D on the fix pass 2 tree, not
  re-measured on the final tree).** `cargo build --release --bin celestina`
  gave sha256 `361c536c…ad536` with 0 fakes strings; the hash stayed the same
  after `cargo test --workspace --release --no-run`, and after the verify
  sequence (`cargo clippy --workspace --release --all-targets -- -D warnings`
  clean, then `cargo test --workspace --release`: 875 passed, 0 failed, 1
  ignored). `cargo tree -p celestina -e normal` holds no
  `celestina-fake-system`.
- **Tests added for the review fixes, and their mutants.** Every test below
  was run once against a mutant of the code it pins, and failed:
  - power worker: a bus killed under a live worker and restarted at the same
    socket (both services `Unavailable`, then `Ready`, then a profile `Set`
    works); a bus whose three service names are activatable but unowned,
    where reads, retries and a refresh start nothing (a control call without
    `NoAutoStart` does activate, so the empty record is the flag's doing;
    mutant: the flag removed); `two_quick_cycles_advance_two_profiles`
    (mutant: one poll per batch instead of one before every command; the
    second cycle never resolved); `a_change_succeeds_on_a_machine_without_upower`
    (review D's repro: no UPower, profiles polled every 60 s, the battery
    retried every 100 ms, a `SetProfile` after 450 ms is confirmed with one
    `Set`) and `a_change_with_no_link_opens_one_and_finds_the_daemon_first`
    (mutant for both: a battery error drops the link and a change with no
    link fails at once, as before; both failed with
    `Failed(Profile("performance"), Unreachable)`). The restart test was not
    run against a mutant in this pass: it stays green under the new rule
    (the link is dropped only on `Lost`), which shows a killed bus is seen as
    lost;
  - lanes: `a_slow_read_or_open_schedules_from_when_it_returned` (mutant: the
    clock taken before the read) and `a_lane_with_a_reread_asked_for_does_not_sleep`
    (mutant: `wait` ignores the flag);
  - network: `only_a_wifi_join_is_followed` (mutant: every row's join
    followed) and `a_stale_mark_that_outlasts_the_pending_window_is_this_joins`
    (mutant: a stale mark never counts);
  - `celestina-shell`: `a_name_of_invisible_characters_only_cleans_to_empty`
    (not run against a mutant; it fails on the previous `clean_text`, whose
    output for `"  \u{200d}  "` was `"\u{200d}"`, as review C probed).
- **Smoke, the last run (exit 0, `smoke: OK`), on the fix pass 2 tree, not
  re-run after this pass's changes (power reconnect rule, lane clocks, join
  following, `clean_text`, two copy strings).** The nest's own socket, the
  live `NIRI_SOCKET` removed from the environment, a binary of this tree in
  a private target directory. The seven nested runs passed: notifications
  (`frames=17 rest_frames=2`, rss 97.8 MiB), launcher, bar (bar on:
  `frames=15 rest_frames=0`, rss 95.9 MiB), wallpaper (`frames=14
  rest_frames=0`), energy (`idle: armed`, `night light: not applied`,
  `suspend refused without a locker`), then the two connectivity runs:
  - **Fake system bus:** the journal says `network: ready`, `bluetooth: ready`,
    `power: battery ready` and `power: profiles ready`; the bar draws on the
    winit output with `frames=26 rest_frames=0 late=1 comp_late=1`, cpu p50
    2.4 ms, p99 39 ms. The host was loaded by other units' smokes at the time;
    the one late frame and the p99 are not attributed to this unit.
  - **No system bus (`DBUS_SYSTEM_BUS_ADDRESS` at a path that does not
    exist, set by the run's own wrapper):** every service unavailable
    (`network`, `bluetooth`, `power: battery`, `power: profiles`), no panic.
    Its frame counts are not checked by the smoke.
- **Repository checks, worktree root:** `check-architecture-contract.sh` OK;
  `check-language-contract.py` OK; `check-documentation-contract.sh` OK.

### Smoke on the final tree

The orchestrator re-ran `scripts/smoke.sh` on the tree after the last fix pass
(private target directory, nested niri): exit 0, `smoke: OK`. Bar on:
`frames=3 rest_frames=0 rss_mib=96.7`; connectivity on the fake bus:
`frames=3 rest_frames=0 late=0`; no system bus: every service unavailable, no
panic. The frame counts are lower than in the earlier run (26 and 15) on every
bar run, the one without connectivity included, so they track how long the
nest drew under host load and not this unit; the figure that matters, zero
frames at rest, held in both runs.

## Finding: the shared target directory

An earlier smoke pass (before the rename of one module) used the binary in
`.cargo-target`, which every worktree of the repository shares, and passed.
The pass after the rename used the same path and failed: the binary there had no
connectivity code (no `connectivity` string, and its bar warned that it did not
have `network`), and the journal had no `network: ready` line. Its timestamp
matched a build this unit ran at that moment, yet it had no connectivity code,
so the cause is not established (a sibling worktree's build of the same package name is the
suspicion, not a verified fact). The smoke was then run against a binary built
in a private target directory, which contains the code, and it passed.

## Limits

Nothing was checked against a real session, a real NetworkManager, BlueZ,
UPower or power-profiles-daemon. Not proven:

- Anything of the real services. The fakes implement only what the workers
  read and the calls the shell makes; NetworkManager's join, BlueZ's connect
  and the profile daemon's `Set` were never issued against the real services.
- Rendering at the author's scale. The tab strip was measured with PIL, not in
  a render; at "Larger" (1.3) the six titles are about 12 px wider than the
  body and clip slightly. Icons for Wi-Fi off, Bluetooth off and battery level
  are drawn, not taken from the icon set.
- The join's failure reason. cuprita does not expose NetworkManager's reason:
  a passphrase row's failure is shown as `NeedsSecret` by inference.
- Live behaviour beside `nm-applet`, `blueman` and the Noctalia widgets. The
  VAL-CEL2 rows are queued for the author.
- Wi-Fi hardware blocking (always false), the airplane switch (not read), VPN
  rows (shown, not actionable).
- Nothing in the smoke measures the absent-bus run's frames.
- Sizes from tokens (the plan's Global constraints): not met for a few
  literals, because `Theme` exposes no token for them. `battery.rs` draws its
  body at 20 x 11 with an inset of 1.5 and radii 2 and 3 (`tokens.toml` has
  `radius.xs = 3`, which `Theme` does not expose, and `celestina` does not
  depend on `celestina-tokens`); `rows.rs` draws signal bars 3 wide with a gap
  of 2; `conn_glyph.rs` draws icons at 18 (the token `icon.sm`, not exposed)
  with a hit padding of 5. The existing tray piece has local constants of the
  same kind. The hover radius uses `theme.radius_md`.
- Signal steps have no hysteresis: an active access point whose strength
  hovers at a step boundary (75/76) changes step on every scan, and each
  change republishes and redraws.
- A3 is met for strength jitter only: changes to the access-point list (an
  access point appearing or leaving) still publish a new section, and so
  redraw the bar while it is on.
- A Wi-Fi join is followed for at most 180 s; a join of a wired or VPN row is
  not followed, so a failure NetworkManager reports for one later (no DHCP,
  for example) is not shown on its row (cuprita marks none).

## Open risks

- The workers' blocking client calls can take 120 s (NetworkManager) or 90 s
  (BlueZ) under a polkit prompt. A lane blocked in one is left behind when
  the one shared 2 s bound of `stop` runs out.
- cuprita's watchers and the NetworkManager join follower are detached threads
  that the shell cannot join.
- A pending row has no clock in the state: if a worker died mid-request the row
  stays pending until the next event for that subject (bounded at 8 entries).
- D-Bus activation of UPower and of either power-profiles-daemon name is
  mitigated, not just noted: every call of the power worker carries
  `NoAutoStart`, and a test with activatable names shows no activation. What
  is not proven is the real system bus: the flag is honoured by dbus-daemon
  and dbus-broker as the specification says, but only dbus-daemon was tried.
- A lane's watcher is one per lane for life. If the system bus itself
  restarts, that watcher dies and the lane falls back to its 30 s re-read until
  the shell restarts (the lane's client reconnects; only the watcher is not
  restarted). The power worker has no watcher and reconnects on its own.
- Input-path changes repaint only the asking surface; other outputs show the
  change on their next frame.
- The pieces show no pending marks; the pages do.
- The pieces' accessible name is the static `Piece::name()` (`&'static str`),
  so it does not carry the state (charge, connected, profile). The per-state
  names that a first version built were not reachable from the contract and
  were removed.
- The shared `.cargo-target` directory can hand one worktree's binary to another
  (see Finding); the smoke's `--binary` must be a binary of this tree.

## Follow-up

The author's rows VAL-CEL2-NETWORK, VAL-CEL2-BLUETOOTH and VAL-CEL2-POWER in
[VALIDATION.md](../../VALIDATION.md). The author's confirmation of ADR 0017 (D1).
`build-production.sh`, `verify-production.sh` and the deploy were not run: the
landing seals and deploys.

## Landing

- **Base revision:** `4f0d02ab1e57f43ba6c67a86d1daf1a7dc992d64`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d9b763224678532d37b1e549333c26d321a4e7c4e3c240212e7f9ac84c7d858e, verification_fingerprint sha256:93d9483e10440c57a81961e37a1b6af3e84cce990778f56232d5d318d03fc968; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:bbe90a50d409e1c59a71f316ed1cb576040dd956271095b9f1ddf4362d5946b9, verification_fingerprint sha256:f3dd02f7200f196f178f563d50d05b49f20ace092eefe249f560837b65f661b4
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
