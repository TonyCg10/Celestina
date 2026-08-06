# Evidence: 2026-08-05 hostile network input at Magnetita's boundaries

- **Date:** 2026-08-05
- **Scope:** `MAG-S1-A` (the checkpoint's five build-order steps) of
  [`../plans/active/2026-08-05-network-input-hardening.md`](../plans/active/2026-08-05-network-input-hardening.md),
  covering `magnetita/qml/components/` and
  `celestina-rs/crates/{magnetita-core,magnetita-net,magnetitad}`
- **Environment:** `rustc 1.97.1`, `cargo 1.97.1`, `qmllint 1.0`, Linux
  7.1.5-1-cachyos. Source, unit tests and static guards only. No production
  build, no deployment, no phone, and the live `magnetitad` was neither
  inspected nor restarted
- **Artifact:** none. No production artifact was built and no version moved

## What was corrected

### MAG-C1 — argument injection into `sshfs` (critical)

`magnetita-core/src/sftp.rs::read_sftp` now validates at the decode boundary
and returns `None` — no mount — for anything that does not pass:

- `user` must be non-empty, at most 64 characters and drawn from
  `[A-Za-z0-9._-]`, and may not start with `-`. This is what stops a `user` of
  `-oProxyCommand=… #` from making `argv[1]` an option `sshfs` forwards to
  `ssh`, which would run it through `/bin/sh`.
- `path` and every entry of `multiPaths` must be absolute, control-byte free,
  NUL-free and bounded.
- `password` may not be empty and may not contain `\n`, `\r` or NUL, because it
  is written to `sshfs` as a single stdin line.

`SftpMount::ip` was **removed**, not merely ignored: it had exactly one
consumer, `magnetitad/src/main.rs`, which preferred it over the link address.
That branch now always mounts against `link_host`, the address of the TLS
connection already authenticated, so a peer can no longer redirect the mount —
and its one-session password — to a host it chooses. This also closes MAG-M6.

### MAG-A1 — the handshake had no absolute deadline (high)

`magnetita-net/src/deadline.rs` is new and is now the single owner of the
`remaining_before` / `is_retryable_timeout` recipe. `payload.rs` had the only
correct implementation and now imports it instead of holding its own copy;
`link.rs` uses it through a `HandshakeDeadline` fixed before the first byte.

`Link::connect` and `Link::accept` set the socket read timeout to a 250 ms poll
cadence and check the absolute deadline on every iteration of the TLS handshake
(`complete_tls`) and of the byte-at-a-time identity read
(`read_delimited_line`). A peer sending one byte per socket timeout previously
held a connection, and an admission `Permit`, indefinitely.

`magnetitad/src/runtime.rs::log_admission_exhausted` makes the refusal
diagnosable: both `try_acquire` call sites in `main.rs` logged nothing.

### MAG-A2, MAG-M2, MAG-M5 — peer-chosen protocol and identity (high)

- `magnetita-core/src/identity.rs::MIN_PROTOCOL_VERSION` is the single owner of
  the floor (8). `magnetita-net` and `magnetita-core` both reference it.
- `Link::connect` refuses a below-floor announcement **before dialling**;
  `Link::accept` refuses it after parsing the plaintext identity and before any
  TLS work; `exchange_identity` re-checks the encrypted identity's version.
- `exchange_identity` no longer has a `< 8` branch returning the pre-TLS
  identity. The identity the link keeps always comes from the encrypted
  channel; the pre-TLS or announced identity now only *constrains* the answer.
- `magnetita-core/src/pair.rs::receive_new_request` requires the timestamp and
  the clock-drift check unconditionally, and refuses a below-floor version with
  the new `PairError::UnsupportedProtocol`.
- `magnetita-net/src/cert.rs::verification_key` binds the timestamp
  unconditionally and yields no code at all below the floor.
- `magnetitad/src/admission.rs::is_dialable` restricts dialling to port 1716 on
  a private, link-local or loopback address. A forged datagram previously made
  the daemon open TCP connections to any address on any port.
- `magnetitad/src/main.rs::serve` resolves trust **before** inserting the
  device into the registry. A peer whose certificate does not match the one
  pinned for the id it claims no longer occupies the real phone's slot for 60 s
  with a name of its choosing.

### MAG-A3 and the mount half of MAG-M1 — unbounded subprocesses (high)

`magnetitad/src/subprocess.rs` is new and now owns the bounded-child discipline
that only `media.rs` implemented: deadline, cancellation flag, non-blocking
output drain with a capture bound, and process-group teardown. `media.rs` was
reduced to its `playerctl` specifics and imports the rest.

`clipboard.rs::read`/`write` and `mount.rs::Mount::open`/`unmount` no longer
use `Command::output()`, `child.wait()` or `wait_with_output()` — all four ran
unbounded on the thread pumping the phone link. `sshfs` also gained
`-o ConnectTimeout=10`.

One correction was needed while extracting: `wait_with_output` terminated the
process group on *success*, which is right for `playerctl` but would have
killed the background child `wl-copy` and `sshfs` deliberately fork — the one
that owns the selection, or the mount. The policy is now explicit
(`GroupPolicy`) and covered by a test.

### MAG-M7 and MAG-M6 — key permissions, bounded text, plain rendering

- `magnetita-net/src/cert.rs::write_private` creates the key with
  `mode(0o600)` and `create_new(true)` on a sibling, syncs, then renames; the
  directory is created `0o700`. There is no longer a window in which
  `privateKey.pem` is world-readable, and an interrupted write can no longer
  leave a truncated PEM. `certificate.pem` uses
  `celestina_core::atomic_file::replace`; the key cannot, because that helper
  creates its temporary at the process umask, which is the very window this
  closes. The reason is recorded at the call site and in the plan.
- `magnetita-core/src/text.rs` is new and owns bounding peer text.
  `read_notification` refuses an over-long id (it is a map key, so it cannot be
  truncated) and bounds `appName`, `title` and `text`; `Identity::from_packet`
  refuses an empty or over-long `deviceId` and bounds `deviceName`.
- The notification map moved into `magnetitad/src/notify.rs::Mirror`, which
  owns its per-device bound (128) and a `forget_device` that
  `SessionRegistration::drop` now calls. Nothing purged it before.
- Every `Text` in `magnetita/qml/components/` that renders a remote string —
  `ActivityLog`, `ConnectedDeviceCard` (state, name, type, mount path,
  verification code, battery), `MediaCard` (title, secondary line),
  `PairedDeviceRow` (name, fingerprint) — declares
  `textFormat: Text.PlainText`. They were `Text.AutoText`, so Qt rendered
  peer-controlled strings that "looked like" HTML as rich text.

## Procedure

```sh
cd celestina-rs
cargo fmt -p magnetita-core -p magnetita-net -p magnetitad -- --check
cargo clippy -p magnetita-core -p magnetita-net -p magnetitad --all-targets
cargo test -p magnetita-core -p magnetita-net -p magnetitad
cargo check --workspace
cd .. && qmllint magnetita/qml/components/*.qml
bash scripts/check-architecture-contract.sh
cd magnetita && cargo check
```

## Result

- **Exit:** every command above exited 0.
- **`cargo fmt … -- --check`:** no output; nothing to reformat.
- **`cargo clippy … --all-targets`:** no warning and no error on any of the
  three crates.
- **`cargo test …`:** `98 passed; 0 failed` (`magnetita-core`),
  `45 passed; 0 failed` (`magnetita-net`), `64 passed; 0 failed`
  (`magnetitad`), plus two empty doc-test runs. 207 tests, 0 failures.
- **`cargo check --workspace`:** finished with no error, so no other crate in
  the monorepo depended on `SftpMount::ip` or on the removed `< 8` branches.
- **`qmllint magnetita/qml/components/*.qml`:** no output.
- **`bash scripts/check-architecture-contract.sh`:** no Magnetita row. The
  daemon's `main.rs` reached 998 lines mid-work and the ratchet refused the
  growth; the notification mirror was moved to its real owner (`notify.rs`) and
  the admission log line to `runtime.rs`, returning `main.rs` to its 927-line
  baseline without raising it. The remaining errors in that run
  (`siderita/src/controller.rs`, `siderita/qml/views/FolderView.qml`,
  `siderita/qml/components/picker/PickerOverwriteDialog.qml`) belong to
  concurrent Siderita work present in the same worktree and were not touched.
- **`cargo check` in `magnetita/`:** the Qt/QML client still builds against the
  changed `magnetita-core`; only pre-existing Qt 6 header warnings appeared.

### Tests added

| Unit | Test | Where |
|---|---|---|
| A | `a_user_that_would_become_an_sshfs_option_is_refused` | `magnetita-core/src/sftp.rs` |
| A | `a_user_longer_than_any_account_name_is_refused` | `magnetita-core/src/sftp.rs` |
| A | `a_path_that_is_not_an_ordinary_absolute_path_is_refused` | `magnetita-core/src/sftp.rs` |
| A | `a_hostile_volume_path_refuses_the_whole_reply` | `magnetita-core/src/sftp.rs` |
| A | `a_password_that_could_end_its_stdin_line_early_is_refused` | `magnetita-core/src/sftp.rs` |
| A | `an_ordinary_reply_still_decodes_after_the_checks` | `magnetita-core/src/sftp.rs` |
| B | `a_dribbling_peer_cannot_hold_the_handshake_past_its_budget` | `magnetita-net/src/link.rs` |
| B | `a_future_deadline_reports_what_is_left`, `a_passed_deadline_is_a_timeout_carrying_its_reason`, `only_window_expiry_and_interruption_are_retryable` | `magnetita-net/src/deadline.rs` |
| C | `a_peer_below_the_protocol_floor_is_refused_before_dialing` | `magnetita-net/src/link.rs` |
| C | `an_accepted_peer_below_the_protocol_floor_never_reaches_tls` | `magnetita-net/src/link.rs` |
| C | `a_request_below_the_protocol_floor_is_refused_with_or_without_a_timestamp` | `magnetita-core/src/pair.rs` |
| C | `a_protocol_below_the_floor_has_no_code_at_all` | `magnetita-net/src/cert.rs` |
| C | `only_the_standard_port_on_a_local_address_may_be_dialed` | `magnetitad/src/admission.rs` |
| D | `input_reaches_the_command_and_its_diagnostics_come_back` | `magnetitad/src/subprocess.rs` |
| D | `a_command_that_never_reads_its_input_ends_at_the_deadline` | `magnetitad/src/subprocess.rs` |
| D | `a_background_child_survives_a_successful_command` | `magnetitad/src/subprocess.rs` |
| D | `an_already_cancelled_run_spawns_nothing` | `magnetitad/src/subprocess.rs` |
| E | `the_private_key_is_never_readable_by_another_local_user` | `magnetita-net/src/cert.rs` |
| E | `peer_chosen_text_is_bounded_and_an_unbounded_id_is_refused` | `magnetita-core/src/notification.rs` |
| E | `an_identity_bounds_its_name_and_refuses_an_unbounded_id` | `magnetita-core/src/identity.rs` |
| E | `a_short_value_is_returned_unchanged`, `truncation_counts_characters_and_never_splits_one`, `an_identifier_is_bounded_but_never_shortened` | `magnetita-core/src/text.rs` |

`the_skew_boundary_is_accepted_and_v7_remains_compatible` was replaced by
`the_skew_boundary_is_accepted`: the v7 half asserted the behaviour MAG-A2
identifies as the defect, so keeping it would have asserted the bug.

Diffstat across the four owned trees at the time of writing:
**21 files, +941/-372**.

## Limits

- Static and unit-level only. No packet from a real phone, no real `sshfs`, no
  real `wl-copy`, no compositor, no session bus, no live daemon. The
  subprocess-bounding tests use `sh`, not the tools themselves.
- The sshfs `-o` forwarding behaviour that makes MAG-C1 exploitable is sshfs's
  documented one and was **not** executed, before or after the fix. What is
  proven is that the hostile packet bodies no longer decode into a mount.
- The protocol floor is asserted against synthetic peers. Whether the author's
  real phone still pairs and reconnects is `VAL-MAG-06`, and nothing here
  substitutes for it. Every current KDE Connect client announces version 8, so
  the floor is expected to be invisible — but "expected" is not "observed".
- The dial restriction assumes the phone is on a private, link-local or
  loopback address. A LAN using publicly routable addresses would no longer be
  dialled; that is a deliberate trade recorded for author confirmation.
- `qmllint` parses; it does not render. That `textFormat: Text.PlainText`
  changes what is drawn is `VAL-MAG-06`.
- No production build, verification, deployment or version transition ran, by
  the author's explicit instruction. The installed daemon still carries the
  uncorrected bytes.

## Not corrected

`magnetitad/src/payload_handlers.rs::with_live_paired_device` still holds the
`Revocations` mutex and the registry mutex across the callback, which in
`receive_file` performs up to 10 000 `hard_link` calls. Shortening that
critical section changes the order in which the revocation barrier and the
registry are taken, and that ordering is the mechanism `Forget` relies on to
guarantee a revoked source cannot publish. The author authorized this
correction only if it could be made without reordering the locking policy; it
cannot, so it was left and is recorded here and in the plan's exclusions.

## Follow-up

- `VAL-MAG-06` and `VAL-MAG-07` in
  [`../../VALIDATION.md`](../../VALIDATION.md).
- A mode-restricted variant of `celestina_core::atomic_file::replace` would let
  `cert.rs` drop its local atomic write. That belongs to `celestina-core`'s
  owner, outside this plan's authorized paths.
