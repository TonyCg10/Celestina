# Evidence: session liveness, released input, bounds and hygiene

- **Date:** 2026-09-27
- **Scope:** `MAG-D1-E` (program unit P-12) of the
  [app-design plan](../plans/active/2026-09-13-app-design.md): MAG-4, MAG-5,
  MAG-6, MAG-7, MAG-11, MAG-13, MAG-14, MAG-15, MAG-17, MAG-20, MAG-21,
  MAG-22, MAG-24, MAG-25, MAG-26, MAG-27 and MAG-30 of the
  [Magnetita audit](../../../docs/evidence/2026-09-26-monorepo-audit-magnetita.md),
  RS-5 and RS-13 of the
  [shared-crates audit](../../../docs/evidence/2026-09-26-monorepo-audit-shared-crates.md),
  and the two items carried from `MAG-D1-D`'s review (the `commands.json`
  write under the entries lock; tests touching the real `commands.json`)
- **Environment:** session worktree `magnetita-MAG-D1-E`, branch
  `unit/magnetita/MAG-D1-E` stacked on `unit/magnetita/MAG-D1-D` (base
  `d57c5e8`, which carries `RS-H1-A`); Linux container as uid 0; the
  toolchain `celestina-rs/rust-toolchain.toml` pins, rustc and cargo 1.97.1,
  offline; Python 3.11; `dbus-daemon` 1.x for a private session bus. No Qt 6
  SDK or CXX-Qt build, no `qmllint`, no libmpv, no `/dev/fuse`, no Wayland
  session, no AT-SPI bus, no Android SDK and no phone. Cargo ran with a
  private `CARGO_TARGET_DIR` in the session scratchpad
- **Artifact:** the landing builds it

## Procedure

```sh
cd celestina-rs
export CARGO_TARGET_DIR=<scratchpad>/mag-d1-e-target
cargo test --offline --locked -p celestina-core -p magnetita-core -p magnetitad \
    -p magnetita-link -p magnetita-net -p magnetita-proto -p magnetita-mobile -p magnetita-peer
cargo clippy --offline --locked -p celestina-core -p magnetita-core -p magnetitad \
    -p magnetita-link -p magnetita-net -p magnetita-proto -p magnetita-mobile -p magnetita-peer \
    --all-targets -- -D warnings
cargo fmt --all --check
cargo tree --offline -p magnetitad -i async-io
cargo tree --offline -p magnetita-mobile -e normal | grep -c uniffi_bindgen
for i in 1 2 3 4 5 6; do cargo test --offline --locked -p magnetitad; done

# RED: the new loopback tests copied onto the pre-change tree (a detached
# worktree at f2e7343) and run there.
cargo test --offline --locked -p magnetitad session_tests

# The daemon on a private session bus (scratchpad script): pair
# magnetita-peer from StartPairing's QR, connect it with --hold, call Forget
# while ListDevices runs, count Changed/Event with busctl monitor, then
# SIGTERM.
dbus-run-session -- sh run2.sh

# The Qt application, type-checked without Qt: every CXX-Qt bridge in
# magnetita/src replaced by a stub of the same shape (the FLU-H1-B recipe),
# qrcode stubbed (not in the offline registry).
cargo check --offline --tests && cargo clippy --offline --tests && cargo test --offline
rustc --edition 2021 --test -D warnings magnetita/src/mirror_input.rs

bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Exit:** every command above exited 0 on the final tree, except the
  expected: `cargo tree -i async-io` answers "did not match any packages",
  and the stubbed app crate's one QR test panics in the qrcode stub.
- **Tests:** celestina-core 82, magnetita-core 43, magnetita-link 17 (1
  new), magnetita-mobile 3 (1 new), magnetita-net 15 (1 new),
  magnetita-peer 1, magnetita-proto 66 (the golden vectors unchanged),
  magnetitad 121 passed and 2 ignored (19 new), six consecutive runs green.
  The app's pure input queue: 3 new tests, run standalone; the stubbed app
  crate: 21 of 22 pass.
- **RED, then GREEN:**
  - `a_slow_frame_survives_the_callers_timeouts` (MAG-4): RED "the control
    stream fell out of step: a frame of 1633771873 bytes exceeds the message
    limit"; GREEN with the reader task.
  - `a_phone_that_stops_reading_is_closed_at_the_send_deadline` (MAG-5):
    RED on the pre-change tree (the session never left the registry); GREEN.
  - `what_a_phone_holds_is_released_when_its_session_ends` (MAG-6): RED
    `["key 29 true", "button Left true"]` with no release; GREEN.
  - `forget_waits_without_holding_the_executor` (MAG-15): RED by mutation
    (the wait run inline) "another call waited 2.104944779s behind the
    Forget"; GREEN.
  - Compile-RED (new API written test first): `SessionInput`'s release test,
    the feed's drop-to-key-frame test (MAG-7), `wants_discovery` (MAG-11),
    the writer's and the desktop worker's tests, the signal coalescer, the
    SIGTERM test (MAG-27), the listing cap, inode forget and cache tests
    (MAG-17), the keep-alive test (MAG-20), the queue-merge tests (MAG-13).
  - Assertion-RED by construction, not run on the old tree: the new
    certificate's name (MAG-26, the old DN held `KDE`), and the tests'
    registry path (carried item, the old one was the real config).
  - GREEN only: `a_stopping_wire_closes_its_sessions_before_it_ends`. On the
    old tree the phone also heard "daemon stopping" in the run tried, so the
    test does not separate the two; the drain is kept for the unmount, which
    needs `/dev/fuse` to observe.
- **Private bus:** ListDevices answered in 116 ms while a Forget waited
  (the Forget returned in 987 ms, at the session's tick); 3 `Changed` and 3
  `Event` signals reached the bus from the emitter thread with zbus on
  tokio and no panic; `SIGTERM` logged `[stop] SIGTERM` and the daemon
  exited 0.

## Observed facts

- **MAG-4:** `PhoneSession` spawns one reader per session that feeds a
  bounded channel (64); `next()` races only channel receives. The reader is
  aborted with the session.
- **MAG-5:** `link_wire/writer.rs` owns a session's control-stream writes:
  a bounded outbox (256) drained by one task, each write under a 10 s
  deadline (2 s in tests); past it, or on a write error, the connection is
  closed and the session ends. The loop, shares, storage, the mirror's input
  and command results only queue. The tick that applies Forget, stop and
  supersede never waits on a send. The pairing reply before the session has
  the same deadline.
- **MAG-6:** `input::SessionInput` tracks held keys and buttons per session;
  a release of something held passes the governor and the input setting;
  dropping it (session end, supersede, Forget, stop) releases the rest.
- **MAG-7:** the FIFO feed is a `sync_channel` of 120 units; on overflow it
  drops to the next key frame that fits and asks the phone for one. The
  sound queue is bounded at 64 chunks and drops late sound.
- **MAG-11:** the dial loop and its `avahi-browse` are removed (the phone
  never advertises); the adb worker browses only while a screen off is
  wanted or held. ADR 0001's discovery paragraph carries an amendment.
- **MAG-13:** the mirror window's input worker and link watcher each hold
  one bus connection (`devices::MirrorBus`), reopened only after a failure;
  a drag's waiting move is replaced by the next one in a bounded queue
  (`mirror_input.rs`).
- **MAG-14:** the clipboard sink and notification post, withdraw and
  cleanup run on `link_wire/desktop_worker.rs`, one owned thread with a
  bounded ordered queue, joined when the wire stops.
- **MAG-15:** `Forget` and `Unpair` are `async`; the trust write and the
  acknowledgement wait run on `spawn_blocking`.
- **MAG-17:** listings stop at 65,536 entries; `forget` drops an inode the
  kernel released and its caches; inode numbers the kernel never held are
  pruned at 65,536; attributes, listings and read windows are capped at
  16,384, 64 and 8. `create` asks the phone afresh, truncates only on
  `O_TRUNC` and refuses on `O_EXCL` (`EEXIST`), with no protocol change.
- **MAG-20:** the link's keep-alive is 10 s against its 30 s idle timeout.
- **MAG-21:** `ConversationRow` is an `AbstractButton`: Tab focus, Space,
  Enter and Return, `Accessible.Button` with a name joined from the label,
  the snippet and "%n sin leer", a `visualFocus` ring, `unread` an `int`.
- **MAG-22 (partly):** `Changed` bursts are coalesced to one per 100 ms by
  `signals.rs`; the app's lists are still replaced whole.
- **MAG-24:** STATUS's header and "Current checkout truth" are rewritten;
  the local AGENTS boundary names proto, link, mobile and the own-wire
  invariants; the service unit's header describes the own wire; the
  protocol document's version and negotiation lines match the code.
- **MAG-25:** STATUS records the versioning practice and points to the
  plan's waiver.
- **MAG-26:** `magnetita-net` drops rustls's `tls12` and `logging` (the lock
  loses `log` under rustls); new certificates are `O=Celestina,
  OU=Magnetita`; the docs describe QR pinning.
- **MAG-27:** `shutdown.rs` waits for `SIGTERM` or `SIGINT`; `main` returns
  and the wire drains its sessions (up to 5 s) before closing the endpoint.
  The unit's `ExecStopPost` sweeps a killed daemon's mounts.
- **MAG-30:** the dead `GroupPolicy` statement is gone; the protocol's
  encoders open every infallible step through `codec::wrote`, which states
  the invariant once; `xdg-open` is reaped.
- **RS-5:** `zbus` runs on its tokio backend (`default-features = false,
  features = ["tokio", "blocking-api"]`); `async-io`, `async-executor`,
  `async-process`, `blocking` and their tree leave the lock (153 lines).
  zbus's blocking API then drives zbus's own tokio runtime, so no blocking
  zbus call runs on a tokio thread: signals go through `signals.rs`, the
  interface's waits are async, and the mirror window's launch runs on
  `spawn_blocking`.
- **Carried items:** `CommandStore::set` and `remove` hold a writers' lock
  across the synced write and take the entries' lock only to copy and swap;
  the tests' process-wide registry is in memory.
- **Also fixed:** `the_worker_starts_and_joins_deterministically` raced the
  worker's "tool missing" publication in a container without adb (1 in 4
  runs); it now accepts either state.
- `scripts/architecture-baseline.tsv`: `magnetitad/src/main.rs` lowered
  from 247 to 237.

## Limits

- The Magnetita application was not built: no Qt, CXX-Qt, libmpv or
  `qmllint` here. Its Rust was type-checked, linted and tested against
  stubs of the CXX-Qt bridges and `qrcode`, which proves the adapters
  against the stubs' shapes only. `ConversationRow.qml` and the
  `MessagesPage.qml` binding were checked by reading.
- No FUSE (`/dev/fuse` absent): the mount's create, forget and unmount on
  stop are proven by unit tests of their parts, not by a mounted tree.
- No phone, Wayland, compositor, AT-SPI or battery measurement:
  `VAL-MAG-17` carries the real-session checks (held keys after a Wi-Fi
  drop, the mirror feed's memory, Forget while Siderita browses, the stop's
  unmount, the conversation row by keyboard and screen reader, the idle
  battery).
- The deployed unit's `ExecStopPost` was not run under systemd.

## Fix round 1

A review of `81e61e2` found one Critical regression and three Important
defects; every item below is fixed, with its test.

- **Critical, the mirror watcher (MAG-13 regression):** the long-lived
  `Mirror1` proxy cached properties, and the daemon emits no
  `PropertiesChanged` for the link mirror, so from its second read the
  watcher saw the first `LinkState` forever. `Mirror1` proxies are now
  built with `CacheProperties::No`. Test
  `a_long_lived_mirror_bus_reads_the_current_link_state` serves a stand-in
  `Mirror1` on a private `dbus-daemon` the test starts: RED with caching
  (`left: "idle||"`, `right: "starting||"`), GREEN without. It runs in the
  stubbed app crate here; it reports and returns where no `dbus-daemon`
  exists.
- **Important, stop order (MAG-27):** a stopping session now closes its
  `StorageClient` first (requests in flight fail at once, later ones fail
  before they start a timer, and the file-system thread checks the close
  before entering the runtime), unmounts, then waits up to 1 s for its queued
  sends to be written through a flush mark the writer answers, and only then
  closes the connection and stops its tasks. Test
  `a_closed_client_frees_the_file_system_thread_before_the_runtime_stops`
  follows the reviewer's probe: a thread waiting on the phone returns within
  200 ms of the close, and a request after the runtime stopped answers
  "link closed" without panicking. RED by compilation (the close is new).
- **Important, a stop test that could not fail:** the drain is its own
  function. `the_drain_lets_a_slow_cleanup_finish` (a task with a 300 ms
  cleanup) and `the_drain_cuts_what_outlives_its_limit` test it directly,
  and `a_stopping_wire_closes_its_sessions_before_it_ends` now asserts the
  session's asynchronous cleanup: its storage client removed, its phone
  book forgotten and its notifications forgotten by the desktop worker.
  Mutation RED: with the drain replaced by an immediate `shutdown`, all
  three fail, 3 of 3 runs.
- **Important, a forgotten phone registering again (pre-existing):** a
  session admitted by its pin that waited for its earlier session to leave
  re-checks the pin under the registry lock before registering; the Forget
  that landed meanwhile had removed it, and the earlier session's teardown
  had cleared the tombstone. Test
  `a_session_forgotten_while_it_waits_for_its_slot_is_refused`: RED with the
  check disabled ("a forgotten phone registered again"), GREEN.
- **RS-13:** the opt-out gate above; `cargo tree -p magnetita-peer -e normal`
  and `cargo tree -p magnetita-mobile -e normal --no-default-features` show no
  `uniffi_bindgen`, and the library builds without the feature.
- **Minor:** the sound queue holds about half a second (6 chunks); the inode
  table's prune waits for a quarter of the bound more when the kernel holds
  most numbers (`a_table_the_kernel_holds_is_not_scanned_on_every_insert`);
  the link's idle timeout is built from a `VarInt` in milliseconds, which
  cannot fail (the old fallback would have meant no idle timeout, not the
  default); the daemon's own calls on its bus connection (the notification
  server's) time out after 3 s instead of never; the app's input queue
  keeps a reserve for touch-ups, key-ups and the stop, so a full queue never
  drops a release. A release can still be lost on the daemon side when the
  phone has stopped reading and the outbox is full; the session then ends
  at its send deadline (recorded in STATUS as debt).
- **Checks after the round:** the 8 crates' tests (magnetitad 126 passed, 2
  ignored, four runs green; link 17; the rest unchanged), clippy
  `-D warnings`, fmt, the stubbed app crate (clippy clean, 22 of 23 tests,
  the one failure inside the qrcode stub), and the three guards.

## Fix round 2

A re-review of `3059646` found items 1, 3, 4, 5 and 6 fixed and raised two
Important defects and three minor ones; each is fixed.

- **Important, sends lost at the stop (MAG-27):** the flush mark only
  proved the bytes were handed to QUIC, and the close let the phone drop
  what it had not yet received (the reviewer's probe: 0 or 5 of 21
  envelopes). A stopping session now flushes its writer, then finishes the
  control stream and waits for the phone to acknowledge receiving all of it
  (`Session::finish_control` in `magnetita-link`, within 2 s), and only then
  closes. Test `a_queued_send_reaches_the_phone_before_the_stop_closes`
  (the probe): RED with the flush alone, `(5, 0)`, `(0, 0)`, `(0, 0)` of
  `(20, 1)` in three runs; GREEN five of five. Delivered: every envelope
  queued on the control stream before the stop. Not delivered: a file
  transfer in flight on its own stream (it resumes on the next session),
  and anything a phone has not acknowledged within the 2 s.
- **Important, dead code in the real build:** `MirrorBus::on` was used only
  by a test, which `verify-production.sh`'s `clippy --all-targets -D
  warnings` refuses; `MirrorBus::open` now goes through it. The stubbed app
  crate was linted without its `allow(dead_code, unused_*)` and with its
  modules public, so the entry points the CXX-Qt glue calls count as used:
  what remains is the stubs' own (property fields read by generated
  getters, and the `CxxQtType`/`Threading` traits the real bridges use),
  none from this unit.
- **Minor:** a registration guard (`storage::register`) closes a session's
  storage client and removes it from the registry however the session
  ends, including a session the stop's drain cuts, and never removes a
  newer session's client (test
  `a_dropped_registration_closes_its_client_and_spares_a_newer_one`); the
  mirror watcher's private-bus test fails with a clear message where
  `dbus-daemon` is missing instead of returning early; the app's input
  worker tries a touch-up, key-up or stop up to three times on a fresh
  connection, 100 ms apart (test `a_release_survives_one_failed_send`), so
  end-to-end release delivery depends on the bus only past that.
- **Checks after the round:** the 8 crates' tests (magnetitad 128 passed, 2
  ignored, three runs green), clippy `-D warnings`, fmt, the stubbed app
  crate (23 of 24 tests, the one failure inside the qrcode stub; the input
  queue's 4 tests standalone), and the three guards.

## Follow-up

- **RS-13 in part (fix round 1).** `uniffi/cli` is behind a `bindgen`
  feature, on by default so `magnetita-android/scripts/build-native.sh`
  keeps working unchanged; `magnetita-peer` and `magnetitad`'s tests opt
  out. Making it opt-in, with `--features bindgen` in the Android build
  script, is `AUD-1-E`'s, which touches both.
- **MAG-22 not fully closed:** row-level `QAbstractListModel`s for the
  device and conversation lists, and position-only media updates kept apart
  from `Changed`, need a real Qt build and interaction checks.
- **MAG-13 in part:** the link watcher still polls `LinkState` every
  400 ms, over its one connection; watching a signal needs the daemon to
  emit `PropertiesChanged` for the link mirror.
- **MAG-24 in part:** Magnetita Android's STATUS version line and README
  install step belong to `magnetita-android:`; `MessagesPage.qml`'s
  polling comment goes with the timer in `AUD-1-E` (MAG-12).
- **MAG-30 in part:** `magnetita-peer`'s `avahi-browse` stays: with the
  daemon's own browse removed it is the only one for `_magnetita._udp`,
  beside the adb worker's for adb services.

## Landing

- **Base revision:** `12c54cef1ddddd10e105ce336ac52633954f6112`
- **Check:** `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4447ebc3dbcc3d830383d836399a0d2858588d7b03c127a67b8761ea1500f050, verification_fingerprint sha256:8f64ef73a9f8332f14d995690ecca4f8d6bea3f0ed79c2d4440bac62265f92cf; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:97929361329538a09f441efadd9b01f593c421a9a067dbed97b08e2732c75001, verification_fingerprint sha256:94bcc082968f0877b58932966e68620cd41a54a965ca64a7c9045539d91418a2; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8bda07470ebc4966a47c48c4d007a163b2d4329f62dfba0a5709601d92a94ea6, verification_fingerprint sha256:fa82ca7246fe59062a048c43f9f92a2260f78c5b8953058902516ad8ae7e1246; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:099caae3fad43776ddf8fb601256e5e54f0fee413bf7f80eda239d4d714a7486, verification_fingerprint sha256:6c736567be5b2567f4211b43a42ae1508f59dcf9a6ad4f0d6135e70e1aea4782; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4f3d55b9e4a4b8cf627513babbc4ffd8c9022e3740490fec2769f02391cfdcbf, verification_fingerprint sha256:bba7b031d56aff5f9a02d69293fadbcbb1634161371788c3a990f144705f2015; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9e5b150dd93bcdc9206b1e7d2f8d57bb05a610b535f76201ef8684b6c08fd034, verification_fingerprint sha256:52856bae821097c06c36c76d1661d0c1963bf33a27e685af9e1aadae638278d2; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:9f02dee0271129b02f8b9715c891c0ef9151ba6213e291f05fba537786f1654e, verification_fingerprint sha256:bf933973a88a62a9110f42d5cab15ba9c469487d46317d7c23231d021035f024
- **Deploy:** after the push: magnetita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
