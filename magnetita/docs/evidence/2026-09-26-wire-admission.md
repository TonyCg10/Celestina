# Evidence: own-wire admission, identity and bounds

- **Date:** 2026-09-26
- **Scope:** `MAG-D1-D` (program unit P-11) of the
  [app-design plan](../plans/active/2026-09-13-app-design.md): MAG-1, MAG-2,
  MAG-3, MAG-10, MAG-16, MAG-18 and MAG-29 of the
  [Magnetita audit](../../../docs/evidence/2026-09-26-monorepo-audit-magnetita.md),
  the Magnetita halves of RS-2 and RS-3 of the
  [shared-crates audit](../../../docs/evidence/2026-09-26-monorepo-audit-shared-crates.md),
  and the daemon half of AND-4
- **Environment:** session worktree `magnetita-MAG-D1-D`, branch
  `unit/magnetita/MAG-D1-D` stacked on `unit/celestina-rs/RS-H1-A` (base
  `14b0e02`); Linux container running as uid 0 with umask 022; the
  toolchain `celestina-rs/rust-toolchain.toml` pins, rustc and cargo 1.97.1,
  offline (the host's default 1.94.1 did not run these checks); Python 3.11. No Qt 6 SDK or CXX-Qt build, no
  `/dev/fuse`, no session bus, no notification server, no Wayland session and
  no phone. Cargo ran with `CARGO_TARGET_DIR` in the session scratchpad (see
  Observed facts for why the shared target was not used)
- **Artifact:** the landing builds it

## Procedure

```sh
cd celestina-rs
export CARGO_TARGET_DIR=<scratchpad>/target
cargo test --offline --locked -p celestina-core -p magnetita-core -p magnetitad \
    -p magnetita-link -p magnetita-net -p magnetita-proto -p magnetita-mobile -p magnetita-peer
cargo clippy --offline -p celestina-core -p magnetita-core -p magnetita-net -p magnetitad \
    -p magnetita-proto -p magnetita-link -p magnetita-mobile -p magnetita-peer \
    --all-targets -- -D warnings
cargo fmt --all --check
cd ..
sh -n magnetita/scripts/verify-production.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

Each new test was written first and run against the unchanged code; the
RED results below are those runs.

## Result

- **Exit:** every command above exited 0. Tests: `celestina-core` 82,
  `magnetita-core` 43, `magnetita-link` 15 (11 before), `magnetita-mobile` 2,
  `magnetita-net` 14 (13 before), `magnetita-peer` 1, `magnetita-proto` 66,
  `magnetitad` 98 passed and 2 ignored (81 before). `magnetitad` was run eight
  times in a row after the last change, all green. The review rounds below
  bring `magnetita-link` to 16 and `magnetitad` to 102 passed and 2 ignored.
- **RED, then GREEN:**
  - MAG-1: `a_hello_naming_another_pinned_device_is_refused` failed with "a
    hello that names another device's id closes the session";
    `a_pairing_phone_must_name_the_id_its_certificate_gives` failed with "the
    desktop never answers the proof" (the old daemon pinned `chosen-id`).
  - MAG-2: `a_stalled_handshake_does_not_hold_the_door` failed with "the
    second peer waited behind the stalled handshake: Err(Elapsed(()))".
  - MAG-3: `an_unpinned_connection_does_not_burn_the_qr` failed: a scanner's
    connection took the secret and the next dial got `Connection("connection
    lost")`.
  - RS-2 and MAG-18: `a_persisted_store_is_readable_by_its_owner_only`
    (trust store), `saved_settings_are_readable_by_their_owner_only`,
    `a_new_identity_is_readable_by_its_owner_only` and
    `a_remembered_endpoint_round_trips_through_its_file` failed on 0644
    files; `a_failed_save_changes_nothing_and_a_save_is_owner_only` failed
    with "memory changes only once the file has".
  - RS-3: `a_mount_path_never_falls_back_to_tmp` failed with `Ok("/tmp/magnetita/abc123")`.
  - AND-4: `the_storage_client_browses_the_phone_tree` failed with
    `Err("Directory not empty (os error 39)")` against `Err("not empty")`.
  - MAG-16: `an_offer_larger_than_any_payload_is_refused_before_a_byte`
    failed: the daemon answered `ShareAccept` (kind 2), not `ShareReject`.
  - MAG-29, MAG-16's expiry and sweep, and the ENOTEMPTY errno mapping were
    new functions; their tests (`phone_text_reaches_a_markup_server_as_plain_text`,
    `an_offer_left_unanswered_expires_and_gives_its_permit_back`,
    `the_start_up_sweep_removes_only_left_over_partials`,
    `a_directory_that_is_not_empty_answers_enotempty`) failed to compile
    before them. All pass now.
- **Observed:**
  - MAG-1: a session runs under the id its certificate is pinned under
    (`link_wire/admission.rs`, `session_id`) and a hello naming another is
    closed; a phone pairs only under `magnetita_link::device_id_of` of its
    certificate, the id the phone already gives itself, and never under an id
    pinned to another certificate (`pairing_id`, which is the first caller of
    `TrustStore::check`). `magnetita-mobile` now derives its id through the
    same owner.
  - MAG-2: `Endpoint::accept` returns a `Pending` attempt at the first packet;
    TLS, the pin check and the hello run in the spawned task within the
    existing 10 s budget. Attempts hold a handshake slot, at most 16 in all and
    4 per address; one past either is refused on arrival, and under pressure
    an unvalidated source gets a Retry instead (fix round 1).
  - MAG-3: reading the QR window no longer closes it; a proof that verifies
    closes it; wrong proofs refuse their address for the rest of the window
    instead of closing it (fix round 1); at most four unpinned connections
    wait for their proof at once, each until it is pinned. `prove_again`
    follows the same rule.
  - RS-2 and MAG-18: the private key, certificate, trust store, settings,
    device id, adb mirror state and `commands.json` go through
    `atomic_file::replace_private` (0600, missing parents 0700);
    `magnetita-net`'s `write_private` copy is deleted; the configuration
    directory is created or tightened by `xdg::ensure_private_dir`. A command
    change reaches memory only after the file holds it; `commands.json` and
    `trust.json` are read through `read_bounded` (1 MiB). `main.rs` lost one
    line and its ratchet row is lowered to 247.
  - RS-3: mounts, mirror FIFOs and the artwork cache resolve under
    `xdg::runtime_dir()` through `runtime::runtime_base` with no `/tmp`
    fallback; the base is created 0700 before use. The app's libmpv log now
    goes beside the FIFO the daemon names instead of its own
    `XDG_RUNTIME_DIR`-or-`/tmp` lookup.
  - AND-4 (daemon half): `magnetita-proto` names the storage error
    `ERROR_NOT_EMPTY` (`"not empty"`, the token the Android branch
    `AND-6-D` already answers); the Rust phone stand-in answers it, and the
    FUSE `rmdir`/`unlink` map it to `ENOTEMPTY`. The wire document records it.
  - MAG-16: offers over `MAX_PAYLOAD_SIZE` are refused on both sides; offers
    unanswered or unstarted for five minutes are dropped with their permit on
    the session tick; leftover `.magnetita-receive-<pid>-<n>.part` regular
    files of other processes are swept once the wire has bound its port (fix
    round 1). A received file is published
    through `atomic_file::publish_without_replacing`.
  - MAG-29: the daemon asks `GetCapabilities` once it can and escapes `&<>`
    in the body when `body-markup` is announced, or when the server cannot be
    asked.
  - MAG-10: `magnetita/scripts/verify-production.sh` now clippies and tests
    `magnetita-proto`, `magnetita-link`, `magnetita-mobile` and
    `magnetita-peer`.
  - The shared `.cargo-target` of the session worktrees served a
    `celestina-core` build without RS-H1-A's functions (E0425 on
    `replace_private`), because the path crates of every worktree share
    artifact names there; a private target directory built the right one.
  - The new tests exposed an existing race: the link-mirror loopback test
    sends a screen request through the adb worker's process-wide inbox, which
    another test's worker could receive. The tests that share it now hold
    `mirror::TEST_INBOX`.

Canonical owners reused: `celestina_core::atomic_file::{replace_private,
read_bounded, publish_without_replacing}`, `celestina_core::xdg::{runtime_dir,
ensure_private_dir}`. Old paths removed: `magnetita-net`'s `write_private`,
`private_file` and `create_private_dir`; the three `XDG_RUNTIME_DIR` lookups
in `magnetitad` and the one in `magnetita/src/mirror_view.rs`'s engine log;
the phone's own copy of the id derivation. New owner: `link_wire/admission.rs`
(the pairing window and the identity rules, moved out of the session module
with their change).

## Fix round 1

The review of `62d4e6d` asked for seven fixes; each was written test first.

1. **Pairing attempt held by the paired session (Important).** The attempt
   permit lived across `pair_then_run(..).await`, so four phones that paired
   and stayed connected refused every later QR. It is now passed into
   `pair_then_run` and dropped once the phone is pinned, before
   `run_session`. `paired_sessions_do_not_hold_the_pairing_attempts` pairs
   five phones in turn, keeping each session; RED: the fifth `prove` failed
   with `Connection("connection lost")`; GREEN now.
2. **Wrong proofs closed the window (ruling R-A22).** Wrong proofs are counted
   per source address in the window; after 3 the address is refused for the
   rest of the window, which keeps its own expiry and is never closed by
   failures. `wrong_proofs_refuse_their_address_and_leave_the_qr_open` (8
   wrong proofs from 127.0.0.1, then a correct one from 127.0.0.2) was RED
   with "wrong proofs never close the window" and is GREEN; the unit test
   `wrong_proofs_refuse_their_address_and_never_close_the_window` covers the
   count, the other address and a new window.
3. **Spoofed sources filling the handshake slots.** Once half of the 16 slots
   are held, `Endpoint::accept` answers an attempt whose address is not
   validated with a QUIC Retry (refusing it if a Retry is not allowed) and
   takes no slot for it. `under_pressure_an_unvalidated_address_must_prove_it_can_answer`
   fills 8 slots with silent hosts, sends a silent Initial from 127.0.0.20
   and dials a real peer from 127.0.0.3: RED returned the silent source; GREEN
   returns the validated peer, and the silent one holds no slot.
4. **Sweep before bind.** `install` now goes through `open_wire`, which
   sweeps the downloads directory only after `spawn` has bound the port, and
   `sweep_partials` never removes this process's own partials.
   `only_a_wire_that_bound_its_port_sweeps_the_partials` shows a daemon that
   cannot bind leaves another process's partial in place, and one that binds
   removes it (the old `install` had no seam to test, so this test is new).
5. **Author checks.** `VAL-MAG-16` in `VALIDATION.md`, linked from the ledger
   row.
6. **Toolchain.** The environment line now names the pinned 1.97.1.
7. **Environment in a shared test.** `runtime::runtime_base` and
   `private_runtime_base` delegate to `base_under` and `private_base_under`,
   which take the runtime directory lookup's answer; `mount::mountpoint_in`
   takes the base. `a_mount_path_never_falls_back_to_tmp` no longer touches
   `XDG_RUNTIME_DIR`.

After the round: `magnetitad` 101 passed and 2 ignored, `magnetita-link` 16,
the other six crates unchanged; clippy, `fmt --check` and the three guards
exit 0. Of 18 consecutive `magnetitad` runs, one failed:
`a_registered_command_runs_by_id_and_input_reaches_the_sink_by_stream_and_datagram`
missed its 5 s envelope wait in a run that took 115 s while the container's
load average was about 8 on 4 cores from other sessions; the next ten runs
passed in 6 to 7 s each.

## Fix round 2

- The per-window book of failing addresses holds at most 256
  (`MAX_FAILURE_ADDRESSES`); once full it records no new address and
  refuses every further unpinned attempt until the window ends, so the map
  stays bounded and no refused host is forgotten to make room.
  `a_full_failure_book_refuses_every_unpinned_attempt_until_the_window_ends`
  failed to compile before the bound existed and passes now.
- After the round: `magnetitad` 102 passed and 2 ignored, `magnetita-link`
  16; clippy on the eight crates, `fmt --check` and the three guards exit 0.

## Limits

- The Magnetita Qt application was not built: `magnetita/src/mirror_view.rs`
  (`engine_options` now takes the FIFO path) was checked by reading only, and
  neither `qmllint` nor the smoke ran. The landing's `verify-production.sh`
  is the first build.
- The FUSE mount test stays ignored here (no `/dev/fuse`); its runtime
  directory was made 0700 so it still runs where FUSE exists.
- `GetCapabilities` and the escaped `Notify` were not exercised against a
  real notification server; only the pure escaping is tested.
- The Retry under pressure was exercised on loopback only, with a quinn
  client; the Android client's handling of a Retry is not observed here.
- The checks only a real session can make are `VAL-MAG-16` in
  [VALIDATION.md](../../VALIDATION.md).

## Follow-up

- `celestina-core`'s adoption notes (`atomic_file.rs`, `xdg.rs`) still say no
  consumer uses the new owners; they belong to `RS-H1-A`'s files and were not
  edited here.
- Session commits used `magnetita-maintenance:` because the hook requires a
  version bump for `magnetita-bug`; the landing records the unit as
  `magnetita-bug`.
- For `MAG-D1-E`, found in review and not changed here:
  `CommandStore::set` and `remove` hold the entries mutex across the file's
  fsync, so a concurrent `list` or `run` waits on the disk; and the
  command-registry loopback test goes through the process-global
  `commands::store()`, which reads and writes the real
  `$XDG_CONFIG_HOME/magnetita/commands.json` while tests run.
- `MAG-D1-E` (P-12) follows: the session writer, the other `STATUS.md`
  corrections and the `magnetita-net` leftovers (MAG-26).

## Landing

- **Base revision:** `e2f96faa4dbc638424be987329531a60d0db3d6f`
- **Check:** `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5563c76bf2b51dab6b128860f21101e64285eba260b79edfbe49424906d9d9c1, verification_fingerprint sha256:b9eae1f377c797835520fc71e026e35f8cc078e51727dd418bd7b9b94633bc38; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:b814b44745804c891aab4b0d6516350a2ef4dd41773691eb1bf504b7c051f197, verification_fingerprint sha256:2f4990023012645b99b986f06577ee5e480a6d11ccbade346e205927d58398c9; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a46e2fb478cb7b5747cd766007317aadabfcc7ae179995ac98d9b2382c463737, verification_fingerprint sha256:62cb14ced6b4ac56184653adabaf43e7587fc989f5d271b3905c482cdf159546
- **Deploy:** after the push: magnetita: deploy-production.sh, status-production.sh
