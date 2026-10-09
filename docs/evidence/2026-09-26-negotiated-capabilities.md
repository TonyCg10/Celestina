# Evidence: negotiated capabilities and one owner for the phone's rules

- **Date:** 2026-09-28
- **Scope:** `AUD-1-E` (program `P-13`) of the
  [monorepo hardening plan](../plans/archive/2026-09-26-monorepo-hardening.md):
  `MAG-8`, `MAG-12`, `MAG-19`, `MAG-28`, `AND-2`, `AND-3`, `AND-5`, `AND-7`,
  `AND-9`, plus ruling `R-A16` (the Kotlin `PairPreview` and `LanAddress`
  owners `AND-6-D` left) and `RS-13` (the bindgen gate `MAG-D1-E` left).
  Branch `unit/suite/AUD-1-E`, based on `2454d5e` (`MAG-D1-E` merged with
  `AND-6-D`), with `MAG-D1-E`'s final review round (`149ffe6`) merged in
  at `239eaa9`: its outbox flush was kept together with this unit's
  negotiated gate in `link_wire/writer.rs`, and its `drain` helper next to
  `offered` in `link_wire/mod.rs`.
- **Environment:** worktree `Celestina.worktrees/suite-AUD-1-E`; Rust
  1.97.1 from `celestina-rs/rust-toolchain.toml`, offline registry, a
  private target directory; the Kotlin 2.0.21 compiler shipped inside Gradle
  8.14.3, JUnit 4.13.2, `kotlinx-coroutines` 1.10.2 and JNA 5.15.0 fetched
  from Maven Central into the session scratchpad. No Android SDK, NDK, Qt,
  `qmllint`, device, phone or session bus.
- **Artifact:** the landing builds it (Magnetita and Magnetita Android).

## What changed, per finding

- **MAG-8.** `magnetita_proto::Negotiated` is the per-session set: what
  both hellos offer at the lower version; the hello (0) and pairing (14)
  always pass. The daemon declares its thirteen capabilities once
  (`link_wire::offered`), computes the set per session, refuses outbound
  envelopes of anything else in the `Outbox` (one gate for every sender:
  loop, shares, storage, mirror, commands) and refuses inbound envelopes,
  datagrams and bulk streams of anything else with a log line. Greetings
  and tick-driven sends (clipboard request, media, contacts, SMS,
  commands) are sent only when negotiated. The phone now advertises the
  thirteen capabilities it uses (`phone::offered`), keeps the set on its
  `PhoneSession`, refuses sends (`LinkError::Declined`) and hands a
  refused envelope to the caller as `Incoming::Refused`, which reaches
  Kotlin as `DesktopSignal.Declined`.
- **MAG-12.** `SmsConversations` asks the phone only until the phone has
  sent a list this session (`DeviceBook::conversations_held`); a new,
  additive `RefreshSms` method asks explicitly. The Messages page has no
  timer: it calls `pull` (one `RefreshSms`, then a read) when it opens,
  and the model re-reads the daemon's cache on `Changed` while the page is
  active, skipping an equal snapshot so the list keeps its scroll.
- **MAG-19.** The rule and its one bound live in
  `magnetita_proto::daily::clipboard::syncable` (256 KiB, the wire's
  `MAX_CLIPBOARD`). `magnetita-core`'s 64 KiB copy is deleted with its
  module; the daemon's watcher, its `wl-paste` read and its handler use the
  proto rule. The phone refuses a text the rule refuses before sending
  (`LinkError::Refused`), and the app shows its refusal message (the new
  `clipboard_refused` string) instead of the sent note.
- **MAG-28.** Unknown FFI codes are refused with `MobileError::UnknownCode`
  (call state, pointer button, codec, media button), never mapped to the
  nearest value; `MobileError` gains `Declined` and `Refused`. Tests cover
  every desktop message's signal, what is never guessed (unknown kind,
  undecodable body, non-empty conversation list, unknown capability), the
  storage records, the error mapping, and a loopback run through the
  `MobilePhone`/`MobileSession` objects themselves.
- **AND-2.** An accepted upload runs in a coroutine of the session scope,
  tracked per transfer, cancelled when the desktop ends the transfer
  incomplete and when the session ends; the copy loop checks cancellation.
- **AND-3.** The conversation list reads `Telephony.Threads` (`simple=true`,
  one row per thread, newest 256), resolves the canonical addresses those
  threads name, and counts unread messages from the unread inbox rows only.
- **AND-5.** The desktop's messages reach Kotlin as the UniFFI enum
  `DesktopSignal` (`magnetita-mobile/src/signal.rs`); `CoreConnector`
  only carries it into the app's sealed interface. Deleted from Kotlin:
  the capability and kind constants and `DesktopSignal.of`, the `limit ?:
  50` default (the wire's limit is required and bounded), `DocumentPaths
  .valid` (the core checks paths at decode), the listing `PAGE` (a list
  request carries the wire's page size in `len`), `NsdDiscovery`'s id,
  address and IPv4-first rules (now `rank_advertised` over
  `magnetita_link::discovery::rank_peers`, which `parse_peers` also uses),
  `Backoff.kt` (now `ReconnectSchedule` over `magnetita_link::Backoff`),
  `ClipboardPolicy.MAX_BYTES`, and `PairLink` (now `pair_link_accepts` over
  `QrPayload::is_link`, one scheme and length bound).
- **R-A16.** `PairPreview.of` and `LanAddress` are deleted.
  `magnetita-mobile/src/pairing.rs` reads a link with
  `QrPayload::parse_uri`, the parse that pairs, and applies the LAN rule
  (RFC 1918, `fc00::/7`, a port, no scope) and this phone's own addresses
  (fail closed when none reads). `QrPayload::parse_uri` now refuses a
  repeated `v`, `id`, `fp` or `secret`, the property the Kotlin preview
  added. `PairingConsent` keeps the consent flow and takes the preview as
  a function; the app passes `CorePairing.preview`. The Kotlin rule tests
  moved to Rust (`pairing::tests`), the flow tests stay on the JVM.
- **AND-7.** The sender suspends on `select` over the forget request, the
  battery changes and the outbound queue (conflated channels for the
  first two); no timer. Input goes through `InputQueue`: bounded at 256,
  drained in order by one worker with at most one drain waiting, waiting
  motion merged into the move before it.
- **RS-13.** `MAG-D1-E` put the generator behind a `bindgen` feature, on by
  default; this unit makes it opt-in (no default) and runs the generator
  with `--features bindgen` in `magnetita-android/scripts/build-native.sh`,
  in the same landing. `cargo tree -p magnetita-mobile -e normal` lists 198
  lines and no `uniffi_bindgen` (1 with `--features bindgen`); the
  generator without the feature is refused by Cargo, with it it generates
  the Kotlin bindings.
- **AND-9.** The phone declines an offer over the payload limit the
  desktop uses (`magnetita_net::MAX_PAYLOAD_SIZE`), holds at most eight
  unanswered offers, and declines at accept time an offer whose remaining
  bytes would leave less than 64 MiB free (`accept_file` takes the free
  space). The file-name rule is `magnetita_proto::daily::share
  ::safe_filename`, used by the phone and the daemon. `Downloads.publish`
  deletes its pending MediaStore row when the copy fails.

The capability ids, kinds and wire bodies are unchanged; nothing moves on
the wire except that each side now refuses what the other did not offer.

## Procedure

```sh
cd celestina-rs
export CARGO_TARGET_DIR=<scratchpad>/aud-1-e-target
cargo test --offline --locked -p magnetita-proto -p magnetita-mobile -p magnetitad \
  -p magnetita-link -p magnetita-net -p magnetita-peer -p magnetita-core -p celestina-core
cargo clippy --offline --locked -p magnetita-proto -p magnetita-mobile -p magnetitad \
  -p magnetita-link -p magnetita-net -p magnetita-peer -p magnetita-core -p celestina-core \
  --all-targets -- -D warnings
cargo fmt --all --check
cargo tree --offline -p magnetita-mobile -e normal | grep -c uniffi_bindgen   # 0
cargo run --offline -p magnetita-mobile --features bindgen --bin uniffi-bindgen -- generate \
  --library $CARGO_TARGET_DIR/debug/libmagnetita_mobile.so --language kotlin --no-format --out-dir <kt>
for i in 1 2 3; do cargo test --offline -q -p magnetitad; done
# Kotlin: host bindings from the debug library, then kotlinc 2.0.21 over the
# generated bindings, the pure sources and CoreConnector/CorePairing, and
# JUnit over the six changed test classes
cargo run --offline -p magnetita-mobile --features bindgen --bin uniffi-bindgen -- generate \
  --library $CARGO_TARGET_DIR/debug/libmagnetita_mobile.so --language kotlin --no-format --out-dir <kt>
java -cp <kotlin-compiler> K2JVMCompiler -cp <stdlib:coroutines:jna> ... 
java -cp ... org.junit.runner.JUnitCore org.celestina.magnetita.link.LinkControllerTest \
  org.celestina.magnetita.link.ClipboardPolicyTest org.celestina.magnetita.link.InputQueueTest \
  org.celestina.magnetita.link.PairingConsentTest org.celestina.magnetita.storage.DocumentPathsTest \
  org.celestina.magnetita.storage.DeleteRuleTest
# A Kotlin probe calling the real library through JNA on the host
java -Djna.library.path=$CARGO_TARGET_DIR/debug ... ProbeKt
# The Magnetita app against CXX-Qt stubs (MAG-D1-E's recipe)
cargo build --tests --offline; cargo clippy --tests --offline -- -D warnings
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Exit:** every command above exited 0.
- **Rust tests:** celestina-core 82, magnetita-core 39 (its four clipboard
  tests moved to proto), magnetita-link 18, magnetita-mobile 23,
  magnetita-net 15, magnetita-peer 1, magnetita-proto 72, magnetitad 125
  plus 2 ignored before the merge (three consecutive runs) and 132 plus 2
  ignored on the merged tree, where the whole command set above was run
  again.
- **Compatibility tests:** `a_desktop_that_declines_a_capability_is_refused
  _both_ways` (phone crate, desktop offering battery and find),
  `the_ffi_session_gates_on_what_the_desktop_offered` (through the UniFFI
  objects), `an_older_phone_keeps_what_both_offer_and_is_refused_the_rest`
  (daemon, a phone hello as the pre-`AUD-1-E` APK sent it: battery works,
  its clipboard text is refused, the desktop's clipboard request and
  clipboard never reach it, the ring does), and
  `the_phone_crate_and_the_daemon_agree_on_every_capability` (the real
  `magnetita_mobile::Phone` paired with the daemon agrees on all thirteen
  and syncs the clipboard). Unknown FFI codes: `unknown_ffi_codes_are_refused`.
- **RED before GREEN:**
  - `qr_payload_refuses_a_repeated_field` failed with the duplicate guards
    removed (`pair.rs:613`).
  - With `Negotiated::allows` forced to `true`: the phone's decline test
    failed (`phone.rs:1078`, the clipboard send was not refused), and the
    daemon's outbox test (`writer.rs:167`) and older-phone test
    (`negotiation_tests.rs:125`, the clipboard reached the phone) failed.
  - With the daemon's SMS read forwarding every call, as before, the cache
    test failed ("a held list is read, not fetched again", 3 requests).
  - With the upload pump inlined and a poll delay put back in the sender,
    `anUploadDoesNotHoldTheReceiveLoopAndStopsWhenAbandoned` (the ring was
    taken after 300000 bytes) and `aQueuedMessageGoesOutWithoutWaitingForAPoll`
    failed.
  - The other new units (signal mapping, codes, pairing, discovery,
    offers, clipboard, input queue) are compile-RED: their API did not
    exist before.
- **JVM:** `OK (29 tests)`. The host probe called `previewPairing`,
  `pairLinkAccepts`, `rankAdvertised`, `ReconnectSchedule` and
  `clipboardSyncable` through `CorePairing`/`CoreSchedule` and the
  generated bindings: all ten checks passed.
- **Qt app:** built and clippy-clean against the CXX-Qt stubs.
- **Guards:** architecture OK; language OK (147 ratcheted, the deleted
  `magnetita-core/src/clipboard.rs` row removed); documentation OK.
- **Ownership:** the canonical owners are `magnetita-proto` (negotiation,
  clipboard rule, file-name rule, pairing-link rule), `magnetita-link`
  (discovery ranking, backoff) and `magnetita-mobile` (typed signals, FFI
  codes, pairing preview and LAN rule, offer bounds). Searched: every
  Kotlin file under `magnetita-android/app/src` for ids, kinds, bounds and
  address rules; the peer's CLI moved from the deleted `*_fields` helpers
  onto `DesktopSignal`, so no second mapping of desktop messages remains.

## Review round 1

- **Refusal is not the session's end.** The app mapped every core error to
  `false` and its sender stopped at the first `false`, so one message of a
  capability the desktop declined ended battery reports, clipboard and the
  forget. `LiveSession` now exposes `gone` (the core's
  `MobileSession::is_closed`, over `Session::is_closed` in the link); the
  sender stops only when the session has ended and drops a refused message
  with a log line naming its kind, never its content. A forget always
  closes.
- **A failing upload ends alone.** An exception from the file's source
  escaped the upload coroutine, cancelled the session and left
  `LinkController.run` for the service's scope. The upload now catches it,
  and a source that cannot be opened or whose bytes the core refuses is
  handled the same way: the core's new `abandon_transfer` resets the
  stream and tells the desktop `ShareDone { complete: false }`, and the
  session goes on.
- The daemon logs a refused capability once per session; the Messages page
  queues one pull when a device change also makes it shown; the evidence's
  second generator command names `--features bindgen`.
- **RED before GREEN:** the reviewer's two probes, kept as
  `aDeclinedSendDoesNotStopTheSender` and
  `anUploadReadErrorEndsOnlyThatUpload`, fail against the previous
  `LinkController` (no clipboard after the refusal; `IOException: provider
  gone` out of `run`) and pass now: `OK (31 tests)`. Rust:
  `an_abandoned_upload_tells_the_desktop_and_keeps_the_session` and the FFI
  test's liveness checks; magnetita-mobile 24 tests, magnetitad 132 plus 2
  ignored, the other crates unchanged; clippy, fmt and the three guards clean.

## Review round 2

- **An abandoned stream no longer stops the bulk streams after it.** Round
  1's `abandon_transfer` reset the stream. A reset that reaches the desktop
  before the 4-byte transfer id made `Transfers::accept` fail, and the
  daemon's acceptor ended: no later upload and no mirror stream was
  received on that session. The same was true of deployed daemons.
- Two fixes, one on each side:
  - The phone now ends an abandoned stream with `finish()`. The desktop
    reads a short stream, marks the transfer incomplete and keeps its
    partial. This is safe with older daemons.
  - In the link, `Transfers::accept_or_skip` returns `Accepted::Skipped`
    for a stream reset or ended before its id, or silent past
    `STREAM_HEADER_BUDGET` (10 s). Only a connection-level error is an
    error. `accept` skips such streams, and the daemon's acceptor logs the
    skip and goes on.
- **RED before GREEN:**
  - `a_later_upload_arrives_after_one_reset_before_its_id` (daemon) failed
    with the link's old error propagation: no `ShareDone` in time.
  - `an_abandoned_upload_tells_the_desktop_and_keeps_the_session` (phone)
    failed with `reset` in `abandon_transfer`: "the abandoned stream was
    reset".
  - `a_later_upload_arrives_after_one_abandoned_short` and
    `an_abandoned_bulk_stream_does_not_stop_the_next` (link) are new
    guards.
- **Checks:** magnetita-link 19, magnetita-mobile 24, magnetitad 134 plus
  2 ignored, the other crates unchanged. Clippy, fmt and the three guards
  pass.

## Review round 3

- **Silent streams delay no other stream (N2).** Bulk streams are now
  demultiplexed in `magnetita-link/src/demux.rs`, the owner for both ends.
  - One task per connection only accepts streams.
  - Each stream's 4-byte id is read by a task of its own, at most 8 at a
    time, each within `STREAM_HEADER_BUDGET`, now 2 s (the id is written
    as the stream opens).
  - The transport admits at most 16 concurrent unidirectional streams.
- **Each receive gets its own stream.** A receive reserves its transfer's
  stream (`Transfers::expect`) before the phone sends `ShareAccept`.
  - Before, the phone's `receive_into` took the next stream of any id and
    dropped any that was not its own, so one of several concurrent
    downloads could lose another's stream.
  - The reserved stream reaches its waiter in any order.
  - A stream nobody reserved goes to a bounded queue that
    `accept`/`accept_or_skip` read (16); a stream past it is stopped.
  - The connection's end answers every waiter.
- **RED before GREEN:**
  - `silent_bulk_streams_delay_no_other_stream` failed with one header
    read at a time: the good stream waited 4.0 s behind two silent ones.
  - `concurrent_downloads_each_get_their_own_stream` failed with the old
    take-and-drop receive: nothing arrived in time.
  - `expected_streams_reach_their_own_waiters` guards the routing, the
    single waiter per transfer and the connection's end.
- **Checks:** magnetita-link 21, magnetita-mobile 25, magnetitad 134 plus
  2 ignored, the other crates unchanged. Clippy, fmt and the three guards
  pass.

## Review round 4

- **Unread streams never fill the stream limit (N3).** A queued stream
  nobody reads keeps one of the peer's 16 stream slots, and the phone never
  reads the unreserved queue.
  - The phone's sessions now take only reserved streams
    (`Transfers::reserve_only`): an unreserved stream is stopped at once.
  - The desktop's queue holds at most 8 streams, under the limit.
  - The module document says both.
- **An abandoned or silent download ends (N4).**
  - A `ShareReject`, or a `ShareDone { complete: false }`, that names a
    download still waiting for its stream now ends that download at once.
    The desktop sends these when its file cannot be read.
  - A download whose stream does not come within `STREAM_WAIT` (30 s)
    ends incomplete.
  - Either way the partial stays for a resume, the reservation is freed,
    and the desktop is told `ShareDone { complete: false }`.
- **RED before GREEN:**
  - With the reservation-only mode a no-op,
    `unreserved_streams_never_hold_the_stream_limit` failed: opening the
    reserved stream waited for a slot.
  - Before the fix, `a_download_the_desktop_abandons_ends_at_once` and
    `a_download_whose_stream_never_comes_ends_after_its_budget` failed:
    the downloads never ended.
- **Checks:** magnetita-link 22, magnetita-mobile 27, magnetitad 134 plus
  2 ignored, the other crates unchanged. Clippy, fmt and the three guards
  pass, and `cargo tree -p magnetita-mobile -e normal` still has no
  `uniffi_bindgen`. The FFI is unchanged.

## Limits

- Gradle, the Android SDK and lint did not run. `LinkService`,
  `NsdDiscovery`, `Messages`, `Downloads`, `PhoneStorage`, `MainActivity`
  and `ScanScreen` use Android APIs and were checked only by reading. The
  pure sources and `CoreConnector`/`CorePairing` compiled with Kotlin
  2.0.21, not the project's 2.2, against bindings generated on the host.
- The `Telephony.Threads` query, the canonical addresses provider and the
  `LIMIT` in its sort order are unverified on the S25U's provider.
- The Magnetita app was type-checked against stubs; QML was neither linted
  nor run, so the Messages page's `Binding` and `pull` on showing are
  unverified in a real session.
- An APK older than `AUD-1-E` offered only battery and find, so against
  this daemon it keeps only those two. Daemon and APK must be installed
  from the same landing; that is the compatibility the plan names.
- `VAL-AND-2` in [Magnetita Android's lane](../../magnetita-android/VALIDATION.md)
  lists the device checks.

## Follow-up

- `VAL-AND-2`, pending.
- The storage request's operation is still a small integer code from Rust
  to Kotlin (`kind`); it is produced from a typed Rust enum and never
  guessed, but an enum on the FFI would type it on both sides.

## Landing

- **Base revision:** `962510f939262c811884d6a138be5c7bddb4e8ed`
- **Check:** `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:059c45a5cd6a6cf5c9938c6245402e4dc656553d83c9f0fe0bd3ab12f5ce3e30, verification_fingerprint sha256:94bcc082968f0877b58932966e68620cd41a54a965ca64a7c9045539d91418a2; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:55a2dea04c95d8c039136d287eafe4b53bc5cc57d4304c8b269af7c10065ca3c, verification_fingerprint sha256:8f64ef73a9f8332f14d995690ecca4f8d6bea3f0ed79c2d4440bac62265f92cf; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5b55c4f62b23679f8584a626b5fece8b9b67c9f8a3e6778efeff4a4895fbb271, verification_fingerprint sha256:003159ebbca1fe92ae7f79cc5d72a2a4345320177b7dae829cb380cd8ce79079
- **Deploy:** after the push: magnetita: deploy-production.sh, status-production.sh
