# Evidence: the Magnetita audit

- **Date:** 2026-09-26
- **Scope:** `AUD-1-A` of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md): Magnetita (`magnetita/`), `magnetitad`, the Magnetita protocol and transport crates, and Magnetita Android (`magnetita-android/`), on `main` at `9d022dd`; one of the seven area records the [monorepo audit](2026-09-26-monorepo-audit.md) consolidates
- **Environment:** read-only audit in a Linux container (kernel 6.18) running as uid 0; rustc and cargo 1.94.1, Python 3.11, Git 2.43.0; no Qt 6 SDK or CXX-Qt build, no libmpv, no Android SDK, NDK or Gradle, no Wayland session, no AT-SPI bus and no real device; Cargo ran `--offline` with its target directory in the session scratchpad, so no production target or cache was touched
- **Artifact:** not applicable

The auditor's report was titled "Audit MAG: Magnetita (desktop app, daemon, protocol crates) and Magnetita Android".

## Procedure

The auditor worked read-only from a common brief: no tracked file was
edited, nothing was committed, no production entry ran and no
subagent was spawned. Every finding was verified by reading the code at
the cited path.

```sh
python3 scripts/agent-context.py magnetita
python3 scripts/agent-context.py magnetita-android
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
cargo test -p magnetita-proto --offline
cargo test -p magnetita-link -p magnetita-mobile -p magnetita-net -p magnetita-core --offline
cargo test -p magnetitad --offline
git log --oneline -- magnetita celestina-rs/crates/magnetita-* celestina-rs/crates/magnetitad
```

## Result

- **Exit:** the guards printed OK (148 ratcheted); `magnetita-proto`: 66 passed; `magnetita-link`, `magnetita-mobile`, `magnetita-net` and `magnetita-core`: all passed; `magnetitad`: 81 passed, 2 ignored.
- **Observed:** 39 findings: 1 Critical, 21 Important, 17 Minor. The auditor's summary, the findings table and every finding follow unchanged, with their original IDs; the consolidated record merges a few of them across areas without renaming them.

### Auditor's notes

Checkout: `/home/user/Celestina`, `main` at `9d022dd`. The audit changed nothing. Guards run: `check-architecture-contract.sh` OK, `check-language-contract.py` OK (148 ratcheted), `check-documentation-contract.sh` OK. Tests run offline: `cargo test -p magnetita-proto` (66 passed), `-p magnetita-link -p magnetita-mobile -p magnetita-net -p magnetita-core` (all passed), `-p magnetitad` (81 passed, 2 ignored). No Android toolchain was used: Gradle, lint and the JVM tests were not run.

### Executive summary

1. The pure wire (`magnetita-proto`) is in good shape. Every peer length is checked at the CBOR header, it has golden vectors, a strict pairing state machine, and 66 passing tests. The bound discipline from `MAG-S1` is real.
2. On the daemon, D-Bus actions are ordered through bounded queues. Revocation uses tombstones with generations, received files are published without overwriting, subprocesses are grouped and time-limited, and the desktop app's worker threads are owned and joined. These parts are healthy.
3. **Top risk (Critical, AND-1):** the exported `magnetita://pair` deep link pairs the phone with no confirmation. One tap on a web link, or no tap at all from any installed app, pins a desktop the attacker controls. That desktop then receives notifications, SMS, contacts, call state, the shared storage root, and key and navigation actions on the phone.
4. Identity and admission on the own wire are weaker than the old KDE Connect path. The session's device id is whatever the peer sends in its hello and is never checked against the pinned fingerprint (MAG-1). TLS handshakes run one at a time in the accept loop (MAG-2). Any unpinned connection uses up the one-time QR secret before a proof arrives (MAG-3).
5. Liveness bugs:
   - The phone's `PhoneSession::next` cancels a read that is not cancel-safe, so the control stream can fall out of sync (MAG-4).
   - The daemon's session loop awaits sends with no deadline, and the same loop enforces Forget (MAG-5).
   - The Kotlin loop uploads a whole file inline, so nothing from the desktop is handled until the upload ends (AND-2).
6. Unbounded or stuck state reaches the author's desktop and memory:
   - A key or button held when the link drops stays held on the desktop; the rate limiter can also drop the release (MAG-6).
   - The mirror FIFO queue grows without limit when nothing reads it (MAG-7).
   - The daemon starts `avahi-browse` about 1.4 times per second forever, for a dial loop that nothing can answer (MAG-11).
7. The protocol's promises about versioning and compatibility are not implemented. No consumer calls `negotiate`, and the phone's hello lists only battery and find (MAG-8). Kotlin re-declares wire ids, kinds, the path rule, the discovery rule, the backoff and the clipboard bound (AND-5), and the clipboard bound already differs between implementations (MAG-19).
8. The production registry leaves out `magnetita-proto`, `magnetita-link`, `fluorita-engine` and `fluorita-qt` from Magnetita's inputs, and `verify-production.sh` never tests proto, link or mobile. A protocol fix can land without the daemon being rebuilt or verified (MAG-9, MAG-10).
9. Performance and battery:
   - The Messages page polls every 3 s, and each poll makes the phone scan its entire SMS provider (MAG-12, AND-3).
   - Every mirror touch opens a new session-bus connection (MAG-13).
   - QUIC keep-alive runs every 1 s on the phone for the life of the session (MAG-20).
10. Documentation has drifted. STATUS contradicts itself, the local AGENTS.md still requires KDE Connect invariants, and the service file describes the retired KDE daemon. 37 product fixes since 2026-09-01 landed as `maintenance`, with no version bump.

### Findings

| ID | Severity | Category | path:line | Summary | Effort | Prefix |
|---|---|---|---|---|---|---|
| AND-1 | Critical | Security | magnetita-android/app/src/main/java/org/celestina/magnetita/MainActivity.kt:186 | The exported BROWSABLE `magnetita://pair` link pairs with no confirmation | S | `magnetita-android:` |
| MAG-1 | Important | Security | celestina-rs/crates/magnetitad/src/link_wire/mod.rs:591 | The session's device id comes from the peer's hello and is never tied to the pinned fingerprint | S | `magnetita:` |
| MAG-2 | Important | Security | celestina-rs/crates/magnetita-link/src/endpoint.rs:123 | The accept loop awaits each TLS handshake (up to 10 s) before accepting the next one | M | `magnetita:` |
| MAG-3 | Important | Security | celestina-rs/crates/magnetitad/src/link_wire/mod.rs:444 | Any unpinned connection uses up the QR secret before any proof | S | `magnetita:` |
| MAG-4 | Important | Correctness | celestina-rs/crates/magnetita-mobile/src/phone.rs:301 | The phone's `next()` cancels a `recv()` that is not cancel-safe; the control stream falls out of sync | S | `magnetita:` |
| MAG-5 | Important | Correctness | celestina-rs/crates/magnetitad/src/link_wire/mod.rs:877 | The session loop awaits sends with no deadline; Forget, stop and supersede are checked in that same loop | M | `magnetita:` |
| MAG-6 | Important | Correctness | celestina-rs/crates/magnetitad/src/link_wire/mod.rs:991 | Held keys and buttons are never released when a session ends; the rate limiter can drop releases | S | `magnetita:` |
| MAG-7 | Important | Correctness | celestina-rs/crates/magnetitad/src/link_wire/mirror.rs:106 | The mirror FIFO feed queue is unbounded while no one reads it, or the reader is slow | S | `magnetita:` |
| MAG-8 | Important | Architecture | celestina-rs/crates/magnetita-mobile/src/phone.rs:42 | Capability negotiation is never done; the phone advertises 2 of the 12 capabilities it uses | M | `magnetita:` |
| MAG-9 | Important | Architecture | docs/projects.toml (magnetita `production_inputs`) | The artifact fingerprint omits proto, link, fluorita-engine and fluorita-qt | S | `suite:` |
| MAG-10 | Important | Tests | magnetita/scripts/verify-production.sh:25 | Verification never tests or lints proto, link, mobile or peer | S | `magnetita:` |
| MAG-11 | Important | Performance | celestina-rs/crates/magnetitad/src/link_wire/mod.rs:528 | A dead dial loop and the retired adb poller start `avahi-browse` about 1.4 times per second forever | S | `magnetita:` |
| MAG-12 | Important | Performance | magnetita/qml/pages/MessagesPage.qml:26 | A 3 s poll makes every `SmsConversations` call trigger a phone resync | S | `magnetita:` |
| MAG-13 | Important | Performance | magnetita/src/devices.rs:550 | Every mirror touch opens a new session-bus connection, through an unbounded queue that never merges moves | S | `magnetita:` |
| MAG-14 | Important | Correctness | celestina-rs/crates/magnetitad/src/link_wire/mod.rs:1190 | Blocking `wl-copy` and D-Bus Notify calls run on the 2-worker async runtime | S | `magnetita:` |
| MAG-20 | Important | Performance | celestina-rs/crates/magnetita-link/src/tls.rs:103 | 1 s QUIC keep-alive on both ends wakes the phone's radio every second | S | `magnetita:` |
| MAG-21 | Important | QML/a11y | magnetita/qml/components/ConversationRow.qml:74 | The conversation row is a bare MouseArea: no role, name, action or keyboard path | S | `magnetita:` |
| AND-2 | Important | Correctness | magnetita-android/app/src/main/java/org/celestina/magnetita/link/LinkController.kt:236 | A file upload runs inline in the receive loop; nothing from the desktop is handled until it ends | S | `magnetita-android:` |
| AND-3 | Important | Performance | magnetita-android/app/src/main/java/org/celestina/magnetita/phone/Messages.kt:28 | The conversation list scans every SMS row on each request | S | `magnetita-android:` |
| AND-4 | Important | Correctness | magnetita-android/app/src/main/java/org/celestina/magnetita/storage/PhoneStorage.kt:94 | SAF `deleteDocument` deletes a whole directory tree; FUSE `rmdir` of a non-empty directory therefore loses data | S | `magnetita-android:` |
| AND-5 | Important | Architecture | magnetita-android/app/src/main/java/org/celestina/magnetita/link/DesktopSignal.kt:115 | Kotlin re-implements wire ids and kinds, the path rule, the discovery rule, the backoff and the bounds | M | `magnetita-android:` |
| AND-6 | Important | Security | magnetita-android/app/src/main/java/org/celestina/magnetita/link/LinkService.kt:114 | Desktop keys and global actions reach the phone with no mirror session or consent | S | `magnetita-android:` |
| MAG-15 | Minor | Correctness | celestina-rs/crates/magnetitad/src/devices.rs:518 | Forget and Unpair block zbus's shared executor for up to 2 s | S | `magnetita:` |
| MAG-16 | Minor | Security | celestina-rs/crates/magnetitad/src/link_wire/share.rs:195 | No size cap on offers; payload permits are held forever; partial files are never swept | S | `magnetita:` |
| MAG-17 | Minor | Correctness | celestina-rs/crates/magnetitad/src/link_wire/storage.rs:118 | FUSE client: pagination with no end, caches with no limit, `create` truncates when the listing cache is stale | S | `magnetita:` |
| MAG-18 | Minor | Correctness | celestina-rs/crates/magnetitad/src/link_wire/commands.rs:118 | `commands.json` is written non-atomically, and memory changes before the write succeeds | S | `magnetita:` |
| MAG-19 | Minor | Architecture | celestina-rs/crates/magnetita-core/src/clipboard.rs:7 | Clipboard bound: 64 KiB in core, 256 KiB in proto and Kotlin; the daemon drops text silently | S | `magnetita:` |
| MAG-22 | Minor | Performance | magnetita/src/controller.rs:421 | Models are parallel string lists replaced wholesale; `Changed` fires on every media tick | M | `magnetita:` |
| MAG-23 | Minor | Architecture | magnetita/src/mirror_view.rs:111 | A second niri IPC client, via `niri msg` subprocesses with no timeout and the author's gap hard-coded | M | `magnetita:` |
| MAG-24 | Minor | Documentation | magnetita/STATUS.md:66 | Contradictory STATUS, stale AGENTS/service/protocol/ADR/Android docs | S | `magnetita:` / `magnetita-android:` |
| MAG-25 | Minor | Documentation | git log (e.g. `43f6f19`, `c56605b`) | 37 product fixes landed as `maintenance`, avoiding the PATCH bump | S | `magnetita:` |
| MAG-26 | Minor | Architecture | celestina-rs/crates/magnetita-net/Cargo.toml:17 | KDE Connect leftovers: unused rustls features, `TrustCheck`, `MAX_PAYLOAD_SIZE`, "KDE" in the certificate DN | S | `magnetita:` |
| MAG-27 | Minor | Quick win | celestina-rs/crates/magnetitad/src/main.rs:170 | No SIGTERM path: the daemon is killed and leaves a stale FUSE mount | S | `magnetita:` |
| MAG-28 | Minor | Tests | celestina-rs/crates/magnetita-mobile/src/mobile.rs:790 | The mobile FFI layer (about 1,900 lines) has no tests; unknown codes are coerced silently | M | `magnetita:` |
| MAG-29 | Minor | Security | celestina-rs/crates/magnetitad/src/notify.rs:46 | Phone text is sent as the Notify body without escaping; safe only because the suite's server omits `body-markup` | S | `magnetita:` |
| MAG-30 | Minor | Quick win | celestina-rs/crates/magnetitad/src/link_wire/commands.rs:154 | Dead `let _ = GroupPolicy::Terminate;`; encoder `unwrap`s without a stated invariant; `xdg-open` child never reaped | S | `magnetita:` |
| AND-7 | Minor | Performance | magnetita-android/app/src/main/java/org/celestina/magnetita/link/LinkController.kt:184 | The outbound reporter polls every 1 s; the input executor queue is unbounded | S | `magnetita-android:` |
| AND-8 | Minor | Security | magnetita-android/app/src/main/res/xml/data_extraction_rules.xml:6 | Template backup rules (with a TODO); the private key and pins are not excluded from device transfer | S | `magnetita-android:` |
| AND-9 | Minor | Correctness | celestina-rs/crates/magnetita-mobile/src/phone.rs:453 / share/Downloads.kt:23 | No size cap on received offers; pending MediaStore rows leak; name sanitising is duplicated | S | `magnetita:` / `magnetita-android:` |

---

### AND-1: `magnetita://pair` deep link pairs with no confirmation (Critical, Security)

**Evidence.** In `AndroidManifest.xml:152-158`, `MainActivity` is `exported="true"` with a `VIEW` + `BROWSABLE` filter for `magnetita://pair`. At `MainActivity.kt:186-190`:
```kotlin
if (intent.action == Intent.ACTION_VIEW && data.scheme == "magnetita" && data.host == "pair") {
    LinkService.pair(this, data.toString())
```
`Phone::pair` (`magnetita-mobile/src/phone.rs:160`) dials whatever `addr` the URI names, which can be any IP including a public one. It proves a secret the attacker chose, pins the attacker's fingerprint, and keeps the session. `LinkService` then serves any connected desktop with no per-desktop check:
- `Storage` requests (`LinkService.kt:120`), covering the whole phone when all-files access is granted;
- SMS lists and threads, and contacts;
- every notification, through `PhoneNotifications`;
- call events;
- `MirrorKey`/`MirrorGlobal` (AND-6).

The scanner's `PairLink.accepts` check is not applied on this path either.

**Why it matters.** Any installed app can pair with no user action (`startActivity` with that URI). A web page or message link can do it with one tap. Either way the attacker's desktop becomes a trusted sink for the phone's most private data: 2FA codes in notifications, SMS, contacts, files. It keeps receiving until the author notices the extra pinned desktop.

**Fix.** Never pair from an intent without an explicit confirmation screen that shows the desktop's id, fingerprint and address, and refuse non-LAN addresses. Alternatively, drop the VIEW filter and accept pairing only from `ScanScreen`. Have `magnetita-mobile` expose `QrPayload::parse_uri` as a preview so Kotlin does not parse the URI itself.

### MAG-1: Hello `device_id` is not bound to the pinned certificate (Important, Security)

**Evidence.** `admit` checks only that the fingerprint is pinned (`mod.rs:427-432`). `run_session` then keys everything on the peer's own claim: `let device_id = hello.device_id.clone();` (`mod.rs:591`). "Everything" here means the registry, commands, supersede, revocation, mount path, storage client and SMS store. Pairing pins under the claimed id as well (`device_id: hello.device_id.clone()`, `mod.rs:502`). `TrustStore::pin` replaces any existing entry with that id (`magnetita-net/src/trust.rs:109`).

**Why it matters.** A pinned device can claim another pinned device's id. It then supersedes that session (`Command::Superseded`, `mod.rs:603-607`), takes over its `Devices1` entry, its mount path and the SendFile and clipboard traffic meant for it. A phone pairing with the QR can overwrite another phone's pin. The KDE path had identity binding (STATUS: "identity binding"); the own wire lost it.

**Fix.** Derive the session id from `Trust::peer_by_fingerprint(fp).device_id` and close the session if the hello disagrees. At pairing, require `device_id == hex(fp[..8])`, which is how the phone derives it (`phone.rs:118`), and refuse an id already pinned under another fingerprint.

### MAG-2: TLS handshakes are serialised in the accept loop (Important, Security)

**Evidence.** `Endpoint::accept` (`magnetita-link/src/endpoint.rs:118-131`) awaits `incoming.accept()?` and then `deadline(started, connecting)` (up to `HANDSHAKE_BUDGET` = 10 s) before it returns. `accept_loop` (`magnetitad/src/link_wire/mod.rs:407-424`) spawns `admit` only after that.

**Why it matters.** One LAN host that sends an Initial packet and stalls holds the loop for 10 s. Repeating it keeps the author's phone locked out indefinitely. This is a cheap denial of service, and it also slows legitimate reconnects when two arrive together.

**Fix.** Make `accept()` return the pending `Incoming` or `Connecting` straight away and run the handshake inside the spawned task. Put a semaphore on concurrent handshakes, with a per-address limit.

### MAG-3: The QR secret is taken by any unpinned connection before a proof (Important, Security)

**Evidence.** `admit` calls `self.pairing.take_live()` (`mod.rs:444`) as soon as an unpinned fingerprint finishes TLS. That is before the hello and before any proof, and `take_live` empties the window. `prove_again` (`mod.rs:1147`) also takes it for any pinned session that sends a proof.

**Why it matters.** Any device on the LAN that connects during the 2-minute window burns the one-time QR: a port scanner, the author's second phone, or a deliberate racer. The author's scan then fails with "unpinned and no pairing armed". Handshake or hello failures also burn it.

**Fix.** Keep the window open until `accept_proof` succeeds (`peek` rather than `take`), limit failed proofs per window, and close the window only on a successful pin.

### MAG-4: Phone `next()` desynchronises the control stream (Important, Correctness)

**Evidence.** In `magnetita-mobile/src/phone.rs:299-306`:
```rust
let got = tokio::select! {
    env = tokio::time::timeout(timeout, self.session.recv()) => ...
    Some(done) = received.recv() => done,
};
```
`Session::recv` (`magnetita-link/src/session.rs:114-120`) uses quinn's `read_exact`, which quinn documents as "*not* cancel-safe" (`quinn-0.11.11/src/recv_stream.rs:86`). Kotlin calls `next(1000)` in a loop (`LinkController.kt:232`). The daemon avoids this exact hazard deliberately (`mod.rs:585-588`: "a control-stream read dropped mid-frame desynchronises the stream").

**Why it matters.** Any frame still arriving when the 1 s timeout fires, or when a file completion wins the `select!`, loses bytes it has already read. The next read then takes body bytes as a length and gets `FrameTooLarge` or garbage, and the session drops. The frames most at risk are the large ones: 1 MiB storage writes from the FUSE mount, and 256 KiB clipboard text, over a Wi-Fi link that stalls.

**Fix.** Move the phone to the daemon's pattern: one reader task per session feeding a channel, with `next()` receiving from the channel under the timeout. Add a loopback test with a slow sender.

### MAG-5: The session loop can wedge on a send, and Forget waits on the same loop (Important, Correctness)

**Evidence.** Every `select!` arm in `run_session` awaits `session.send_message(...)` inline, with no timeout. That covers `outbox_rx` (`mod.rs:877-881`), the tick's mirror, media and clipboard sends and `self.command(...)`, and replies to share. The revocation check, stop check and supersede check live only in the tick arm of the same loop (`mod.rs:914-938`). `outbox` is an `unbounded_channel` (`mod.rs:669`). `Devices::revoke` waits 2 s and then fails with a Spanish error saying the link did not apply the unlink within the limit (`devices.rs:518-529`).

**Why it matters.** When the phone stops reading its control stream (AND-2 makes this routine during uploads), QUIC flow control blocks `write_all`. The loop then stops processing Forget, stop and ticks, and `outbox` grows without limit (FUSE requests, share replies). Forget fails from the UI while the session stays open.

**Fix.** Give the control stream a dedicated writer task with a bounded queue and a per-send deadline. Enforce the revocation, stop and supersede checks in the reader/tick path independently of sends, and close the connection when a send exceeds its deadline.

### MAG-6: Stuck keys and buttons on the author's desktop (Important, Correctness)

**Evidence.**
- `Governor::admit` (`link_wire/input.rs:44-56`) drops any event past 4,000 per second, including `pressed: false` transitions.
- `handle_input` applies it to every input kind (`mod.rs:991`).
- The virtual device is global and outlives sessions (`LazyUinput`, `input.rs:295`).
- Session cleanup (`mod.rs:940-953`) releases nothing.
- There is no tracking of held keys (grep for `held|release` finds nothing).

**Why it matters.** A phone that drops off Wi-Fi mid-drag, or mid-Ctrl in the key screen, leaves BTN_LEFT or a modifier held on the author's live session until something presses and releases it again. A burst that trips the governor has the same effect.

**Fix.** Track the pressed keys and buttons per session. Release them all on session end and on supersede. Let release transitions bypass the governor.

### MAG-7: Unbounded mirror FIFO queue (Important, Correctness)

**Evidence.** `let (tx, rx) = channel::<(Vec<u8>, bool)>();` (`link_wire/mirror.rs:106`). The feed thread blocks in `OpenOptions::new().write(true).open(&feed_path)` until a reader appears (`:116`), and blocks in `fifo.write_all` while the reader is slow. `FifoSink::write` keeps sending units the whole time; the only visibility is a `QUEUED` counter in the log.

**Why it matters.** If the app is not running, its window failed to open, or libmpv stalls, the whole HEVC stream (about 1 MB/s) builds up in the daemon's memory until the mirror stops.

**Fix.** Use a bounded `sync_channel`. On overflow drop to the next key frame and request a key frame, which the reader-open path already does.

### MAG-8: Capability negotiation is never performed (Important, Architecture)

**Evidence.** `magnetita-mobile/src/phone.rs:42-53`: the phone's `capabilities()` lists only `BATTERY` and `FIND`, yet it sends and handles clipboard, notifications, share, media, SMS, contacts, telephony, commands, input, mirror and storage. No crate calls `negotiate` (`rg "negotiate\("` outside `hello.rs` finds nothing), and the daemon never reads `hello.capabilities`. `protocol.md:57` says "Both sides keep the capabilities both offer, each at the lower version", and its versioning section relies on it.

**Why it matters.** The protocol's only way to evolve a capability without bumping the envelope version does not work. Every change becomes all-or-nothing between the app and the daemon, and the documented compatibility story is false.

**Fix.** Advertise the phone's real set. Store the result of `negotiate` on each session on both ends and gate each capability's sends and handlers on it. Add a loopback test with a peer that declines one capability.

### MAG-9: Production fingerprint omits Magnetita's own dependencies (Important, Architecture)

**Evidence.** The magnetita entry's `production_inputs` in `docs/projects.toml` lists `celestina-core`, `magnetita-core`, `magnetita-net` and `magnetitad` only. `magnetitad/Cargo.toml:17-18` depends on `magnetita-proto` and `magnetita-link`. `magnetita/Cargo.toml:28,42` depends on `fluorita-engine` and `fluorita-qt`. `production_input_patterns` (`scripts/production_artifact.py:254-262`) hashes only the declared list plus the workspace manifests. `magnetita-android` declares no `production_inputs`, and its Gradle `buildNative` inputs omit `magnetita-net` and the Cargo manifests and lock (`app/build.gradle.kts:20-23`).

**Why it matters.** A security fix in the link or the wire (for example MAG-1, MAG-2 or MAG-4) leaves Magnetita's seal "current". The landing then skips build, verify and deploy of the daemon, and the author keeps running the old bytes. The same applies to engine changes that reach the mirror window. The APK can ship a stale native library.

**Fix.** Add `celestina-rs/crates/magnetita-proto`, `magnetita-link`, `fluorita-engine` and `fluorita-qt` to magnetita's `production_inputs`. Declare Android inputs, including the Rust crates and `celestina-rs/Cargo.lock`, and add them to `buildNative.inputs`. Consider deriving path dependencies from `cargo metadata` in the guard.

### MAG-10: Verification skips proto, link, mobile and peer (Important, Tests)

**Evidence.** `magnetita/scripts/verify-production.sh:25-29` runs clippy and tests only for `-p celestina-core -p magnetita-core -p magnetita-net -p magnetitad`. The 66 proto tests, 11 link tests and the mobile tests exist, but no registered verification runs them. I ran them by hand and they pass.

**Why it matters.** The golden vectors that pin the wire ("a change to the wire is a change to a test") are not part of the production gate. Clippy's `-D warnings` never sees the phone-side crate.

**Fix.** Add `-p magnetita-proto -p magnetita-link -p magnetita-mobile -p magnetita-peer` to both commands.

### MAG-11: Constant `avahi-browse` subprocesses; the dial loop is dead code (Important, Performance)

**Evidence.**
- `dial_loop` (`mod.rs:528-575`) runs `discovery::browse` (an `avahi-browse -rpt` subprocess with a budget of up to 4 s) every 5 s, forever.
- The phone never registers `_magnetita._udp`: `NsdDiscovery.kt` only calls `discoverServices`, and `rg registerService` finds nothing. So the loop can only ever find the daemon's own advertisement.
- On a connect failure it sleeps `backoff.next_delay()` (up to 60 s, `mod.rs:567`) without watching `stopped()`, which delays daemon shutdown.
- Separately, the retired adb mirror worker calls `poll_discovery` for two adb services every 2 s (`mirror.rs:212-231, 343-346`), even when nobody asked for screen-off.
- ADR 0001:65-66 says discovery runs "over the `zbus`" and that "either side may dial".

**Why it matters.** About 1.4 process spawns per second plus multicast queries, around the clock, for no function. Shutdown can hang for up to a minute.

**Fix.** Either remove the dial loop or have the phone advertise, and choose one in the ADR. Poll adb discovery only while a screen-off is requested. Make the backoff sleep a `select!` with `stopped()`.

### MAG-12: The Messages page polls, and each poll makes the phone resync (Important, Performance)

**Evidence.** `MessagesPage.qml:26-32` runs a `Timer { interval: 3000; repeat: true }` that calls `refresh()`. `devices.rs:598-606` (`sms_conversations`) calls `self.forward(&device_id, Command::SmsList)` on every call. The phone answers each request with a full scan (AND-3), and the daemon calls `notify_change()` on each answer (`mod.rs:1026`), which makes Siderita and the shell re-read `ListDevices`. The page's comment says "the daemon signals no SMS change of its own" (`:11`), but `notify_change` is emitted.

**Why it matters.** With the Messages page open, the phone reads its whole SMS database every 3 s and sends up to 256 conversations. Every `Devices1` consumer re-reads the device list at the same rate.

**Fix.** Refresh from the phone only on the first read or an explicit pull, and let received SMS drive updates. Replace the timer with the `Changed` signal the app already watches.

### MAG-13: Mirror input opens a D-Bus connection per touch, with no coalescing (Important, Performance)

**Evidence.** `magnetita/src/devices.rs:550-553`:
```rust
fn mirror_proxy() -> Result<Proxy<'static>, String> {
    let connection = Connection::session()...
```
It is called for every `mirror_link_touch` and `mirror_link_key` (`:530-541`). There are 18 `Connection::session()` call sites in the file. The input worker's queue is an unbounded `channel::<Outbound>()` (`mirror_view.rs:340`), and move events are not merged. The link-state watcher opens a connection every 400 ms (`mirror_view.rs:366-378`).

**Why it matters.** Each drag sample pays a bus connect, authentication and Hello. Under load the queue fills with stale moves and the phone lags behind the hand. That contradicts the rule "bound or coalesce bursts".

**Fix.** Keep one connection and proxy per worker thread. Merge queued moves into the latest one, so a drag sends at most one move per frame. Watch a signal instead of polling every 400 ms.

### MAG-14: Blocking adapters on the async link runtime (Important, Correctness)

**Evidence.** `handle()` is a sync function called from `run_session`'s `select!` loop. It runs `(self.adapters.clipboard_sink)(&clip.text)` (`mod.rs:1190`), which is `clipboard::write`: a `wl-copy` subprocess waited for up to 2 s (`clipboard.rs:101-110`). Notification posts are blocking zbus `Notify` calls (`notify.rs:46-60`) through `handle` and `handle_phone`. The runtime has two worker threads (`mod.rs:326`).

**Why it matters.** Two slow adapters stall every session, the accept loop and all timers, including the 1 s revocation tick.

**Fix.** Run the adapters on `spawn_blocking`, or on dedicated threads fed by bounded channels.

### MAG-20: 1 s keep-alive on the phone for the life of the session (Important, Performance)

**Evidence.** `magnetita-link/src/tls.rs:103`: `tp.keep_alive_interval(Some(Duration::from_secs(1)))`, with a 30 s idle timeout. The same `transport()` is used by the phone's endpoint (`Phone::open`, via `Endpoint::bind`).

**Why it matters.** Both directions send a packet every second, 24/7, whenever the phone is linked. That keeps the Wi-Fi radio out of power save, a large and unmeasured battery cost; VALIDATION records no battery observation. A 30 s idle timeout needs roughly one keep-alive every 10 s.

**Fix.** Use about a 10 s keep-alive, raised to 1 s only while the mirror streams. Measure the effect in the Android VALIDATION lane.

### MAG-21: ConversationRow cannot be reached by keyboard or assistive technology (Important, QML/a11y)

**Evidence.** In `magnetita/qml/components/ConversationRow.qml`, the root is an `Item` with a `MouseArea { onClicked: root.openRequested() }` (`:74-79`). It has no `Accessible.role`, name, press action or `activeFocusOnTab`, and no `Keys` handling. `unread` is a `string` property compared with `"0"`.

**Why it matters.** A conversation cannot be opened from the keyboard or with a screen reader. The AGENTS rule requires role, name, state and action on custom controls, and `visualFocus`.

**Fix.** Use a shared row or button control from `celestina-style`, or add `Accessible.role: Accessible.Button`, a name built from label, snippet and unread count, `activeFocusOnTab`, Enter/Space handling and a `visualFocus` highlight. Make `unread` an `int`.

### AND-2: A file upload blocks the phone's receive loop (Important, Correctness)

**Evidence.** `LinkController.kt:236`: `offered.remove(signal.transfer)?.let { file -> pump(live, signal.transfer, file, signal.offset) }`. It runs inside the `while (isActive) { val event = live.next(pollMs) ... }` loop, and `pump` copies the whole file before it returns.

**Why it matters.** For the length of any upload from the phone:
- Ring and mirror stop are ignored.
- FUSE requests from the desktop time out after 30 s, and the mount returns `EIO`.
- The daemon's control writes back up (MAG-5).

**Fix.** Launch `pump` in its own coroutine owned by the `hold` scope, and track it so that close cancels it.

### AND-3: Every conversation-list request scans all SMS rows (Important, Performance)

**Evidence.** `phone/Messages.kt:28-39` queries `Telephony.Sms.CONTENT_URI` with no selection and no limit, sorted by date, and walks every row to find the latest message and unread count per thread. Only then does it `take(256)`.

**Why it matters.** With tens of thousands of SMS this is seconds of I/O and CPU per request, and MAG-12 issues a request every 3 s.

**Fix.** Query `Telephony.Threads` or the `mms-sms/conversations` provider, which returns one row per thread, with `LIMIT 256`.

### AND-4: `rmdir` through the mount deletes whole trees in document-tree mode (Important, Correctness)

**Evidence.** The wire's `Delete` removes "a file or an empty directory" (`magnetita-proto/src/storage.rs:115`). The daemon maps both `unlink` and `rmdir` to `delete` (`link_wire/storage.rs:559-561`). The phone's document-tree path calls `DocumentsContract.deleteDocument(...)` (`PhoneStorage.kt:94-95`), and the external storage provider deletes directories recursively. The all-files path uses `File.delete()`, which fails on a non-empty directory, so the two modes behave differently.

**Why it matters.** A plain `rmdir dir`, a file manager's "remove empty folders", or any tool that relies on ENOTEMPTY silently deletes a folder's contents on the phone.

**Fix.** In document-tree mode, refuse to delete a directory that has children: query the child documents first and answer `ENOTEMPTY`. On the desktop, map that error to `ENOTEMPTY` rather than `EIO`.

### AND-5: Kotlin re-implements protocol rules that `magnetita-mobile` owns (Important, Architecture)

**Evidence.** `magnetita-android/AGENTS.md`: "Every protocol rule, bound, pairing step and message lives in `magnetita-mobile` ... A rule implemented in Kotlin is a defect." Instances:

| Kotlin | Rust owner |
|---|---|
| `DesktopSignal.kt:114-172`: capability ids and kinds as constants and literals (`CAPABILITY_SMS to 4`, `CAPABILITY_MIRROR to 7`); the default `event.limit ?: 50` | `magnetita_proto::capability`, the kind constants, `THREAD_PAGE` |
| `storage/DocumentPaths.kt:17-21` `valid()` | `magnetita_proto::storage::check_path`, applied already at decode; the listing `PAGE = 256` duplicates `storage::list_dir` |
| `link/NsdDiscovery.kt:40-44`: id of at most 64 alphanumeric characters, loopback and link-local exclusion, IPv4 first | `magnetita_link::discovery::parse_peers` and `reachability_rank` |
| `link/Backoff.kt` ("the same numbers the desktop's link uses") | `magnetita_link::Backoff` |
| `ClipboardPolicy.kt` `MAX_BYTES = 256 * 1024` | proto `MAX_CLIPBOARD`; see also MAG-19 |
| `PairLink.accepts` length ≤ 512 | `QrPayload::parse_uri` ≤ 4096 |

**Why it matters.** These are exactly the duplicate owners the contract forbids. The first divergence has already shipped (MAG-19).

**Fix.** Replace `Event`'s capability and kind numbers with a UniFFI `enum` of typed desktop signals. Export `check_path`, backoff, discovery ranking and the bounds through UniFFI. Delete the Kotlin copies and their JVM tests in the same unit.

### AND-6: Desktop keys and global actions run with no mirror session (Important, Security)

**Evidence.** `LinkService.kt:114-115` forwards `MirrorGlobal` and `MirrorKey` straight to `MirrorInput.instance?.global(...)` and `key(...)`. `MirrorInput.key` (`mirror/MirrorInput.kt:194-212`) calls `ACTION_SET_TEXT` on whichever editable field has focus, and `global` performs Back, Home or Recents. Only touches are gated, by `geometry`, which is set only while streaming (`MirrorService.kt:166-170`).

**Why it matters.** Whenever the accessibility service is enabled, a paired desktop can drive the phone's UI and type into any focused field with no screen-capture consent and nothing visible to the person. Combined with AND-1, the attacker gets that desktop.

**Fix.** Ignore `MirrorKey` and `MirrorGlobal` unless `MirrorService.state` is `Streaming`, and clear the accessibility input when streaming stops.

### MAG-15: Forget blocks the D-Bus executor (Minor, Correctness)

**Evidence.** `devices.rs:516-518`: the sync `#[zbus::interface]` method `revoke` calls `self.revocations.wait_applied(device_id, generation, FORGET_ACK_TIMEOUT)` (2 s, a condition-variable wait). zbus runs sync interface methods on its internal executor thread.

**Why it matters.** During a Forget, calls to `Devices1` and `Mirror1` from the app, Siderita and the shell wait up to 2 s.

**Fix.** Make `forget` and `unpair` `async fn` and wait with an async notify, or reply before waiting for the acknowledgement.

### MAG-16: Share resources are not bounded or swept (Minor, Security)

**Evidence.**
- `ShareOffer.size` is an unchecked `u64` (`proto/daily/share.rs:66`). `magnetita_net::MAX_PAYLOAD_SIZE` (64 GiB) exists but no code on the own wire uses it (`rg MAX_PAYLOAD_SIZE`).
- An accepted offer takes a global `PayloadPermit` (`share.rs:195`) and holds it until a stream arrives; `incoming` entries are never expired. The same holds for `outgoing` until the phone answers.
- `.magnetita-receive-<pid>-<n>.part` files persist across restarts, and there is no sweep (`incoming_file.rs:47-66`).

**Why it matters.** A paired phone can fill `~/Downloads`, or starve all transfers, including desktop to phone, for the whole session. Crashes leave hidden partial files behind.

**Fix.** Enforce `MAX_PAYLOAD_SIZE` at offer time. Expire unanswered offers after a few minutes. Sweep stale `.magnetita-receive-*.part` files at startup.

### MAG-17: FUSE client bounds and create semantics (Minor, Correctness)

**Evidence.**
- `StorageClient::list` loops while `listing.more` (`link_wire/storage.rs:118-139`) with no cap on the total.
- `Inodes` and `attrs` grow for the life of the mount, with no `forget` implemented.
- `create` sends `write(path, 0, &[], truncate=true)` (`:739`). Its `ENOENT` may come from a directory cache up to 15 s old (`lookup`, `:461-478`), so a file created on the phone in that window is truncated.

**Fix.** Cap listings, for example at 64k entries. Implement `forget`, or put a limit on the caches. Make `create` ask the phone to create exclusively; a `create_new` flag on `Write` would be an additive key.

### MAG-18: `commands.json` is written non-atomically (Minor, Correctness)

**Evidence.** `link_wire/commands.rs:118`: `std::fs::write(path, bytes)`. Memory is changed before `persist` runs (`:88-101`). `load()` treats a corrupt file as an empty registry (`:44-49`). Settings, trust and identity already use `celestina_core::atomic_file::replace`.

**Why it matters.** A crash during a save, or a full disk, empties the author's registered commands, and the next save persists the empty list.

**Fix.** Use `atomic_file::replace`, and apply the change to memory only after it succeeds, as `Settings::update` does.

### MAG-19: The clipboard bound differs between owners (Minor, Architecture)

**Evidence.**
- `magnetita-core/src/clipboard.rs:7`: `MAX_CLIPBOARD_BYTES = 64 * 1024`, documented as "the one rule both directions share".
- `magnetita-proto/src/bound.rs`: `MAX_CLIPBOARD = 256 * 1024`.
- Kotlin: `ClipboardPolicy.MAX_BYTES = 256 * 1024`.

For text from the phone, the daemon applies `is_syncable` and only logs "not syncable" (`mod.rs:1186-1195`), while the phone shows "clipboard sent".

**Fix.** Keep one bound, in proto, and have `magnetita-core` reuse it. Return a refusal the phone can show to the person.

### MAG-22: Models are replaced wholesale on every change (Minor, Performance)

**Evidence.** `controller.rs:421-...` sets about 15 parallel `QStringList` properties on every device snapshot, and `messages.rs` does the same for conversations. Every `Changed` signal triggers a re-read; the controller's own comment mentions a "1 Hz media position update". `handle_media` calls `notify_change()` for every phone media state (`mod.rs:975`).

**Why it matters.** Bindings are re-evaluated and delegates rebuilt every second during playback, scroll and focus are lost every 3 s on the Messages page, and every other `Devices1` consumer re-reads.

**Fix.** Use `QAbstractListModel` with row updates. Throttle or separate position-only media updates from the `Changed` signal.

### MAG-23: A second niri IPC client inside Magnetita (Minor, Architecture)

**Evidence.** `mirror_view.rs:108-205` shells out to `niri msg --json windows|workspaces|outputs` with `Command::output()` and no timeout. It searches the runtime directory for `niri.<display>.*.sock` and hard-codes `const NIRI_GAP: i32 = 12` ("as the author's configuration sets it"). The shell already owns a typed `niri_ipc` adapter (`celestina/src/niri_adapter.rs`).

**Why it matters.** Compositor policy lives in the phone app. It breaks when the author's gap changes, and a hung `niri msg` blocks an owned thread that the model joins on drop.

**Fix.** Record where window sizing belongs (shell, or a `niri` seam crate), bound the subprocess with `subprocess::wait_bounded`, and read the gap from niri's configuration.

### MAG-24: Documentation contradicts the checkout (Minor, Documentation)

**Evidence.**
- `magnetita/STATUS.md:66-68` says the link mirror "plays in an `mpv` window the daemon owns … falls back to `adb`". The same file (`:18`) says "the daemon spawns no `mpv`", and ADR/STATUS record the adb picture as retired.
- `STATUS.md:158` says the own-protocol program "is planned and not started".
- `STATUS.md:153` lists `MAG-M1` as planned debt, while the same file lists it as done and archived.
- `STATUS.md:169`: "installed daemon carries everything committed through `3419cff`".
- `magnetita/AGENTS.md:29-31` still requires "measured KDE Connect invariants … payloads use 1739–1764". `:20` names only core, net and daemon as owners and omits proto and link.
- `magnetitad.service:1-15` describes "CP0 daemon", "binds UDP+TCP 1716", Valent, and a `cargo build` install.
- `protocol.md:16`: "Refused before any other field is read". In fact `Envelope::decode` checks the version when key 0 is reached, in any position. `protocol.md:57`: negotiation (see MAG-8).
- ADR 0001:65-66: discovery "over the `zbus`", "either side may dial" (see MAG-11).
- `magnetita-android/STATUS.md:73` says "versioned at `0.1.0`"; the Gradle file and version history say `1.0.1`.
- `magnetita-android/README.md:32-33` builds `assembleDebug` and then installs the release APK.
- `MessagesPage.qml:11` says "daemon signals no SMS change" (see MAG-12).

**Fix.** Rewrite STATUS "Current checkout truth" from scratch. Update the local AGENTS boundary to proto, link, mobile and the own-wire invariants. Update the service header, the protocol wording, the ADR discovery paragraph and the Android STATUS and README.

### MAG-25: Product fixes landed as `maintenance` (Minor, Documentation)

**Evidence.** `git log --since=2026-09-01` shows 37 `magnetita(-android)-maintenance: Fix …` subjects, for example `43f6f19` ("Fix the mirror window unsized…") and `c56605b` ("Fix the mirror window aborting in libmpv…"), against 4 bug or milestone subjects. `docs/contracts/versioning.md:55-58`: "`maintenance` … is not a way to avoid a required bump. A bug fix uses `bug`."

**Why it matters.** The deployed daemon and app changed behaviour with no PATCH bump or history row, so the version no longer identifies what is installed.

**Fix.** Use `-bug` for user-visible corrections from now on. If the author waived the bumps, record that waiver in the plan.

### MAG-26: KDE Connect leftovers in `magnetita-net` (Minor, Architecture)

**Evidence.**
- `Cargo.toml:14-17` justifies rustls with "The KDE Connect link is TLS … No async runtime: CP0" and enables the `tls12` and `logging` features; the only QUIC user restricts to TLS 1.3.
- `cert.rs:65-67` still writes `O=KDE`, `OU=Kde connect` into new certificates.
- `TrustCheck` and `TrustStore::check` have no caller except the re-export (`magnetita-link/src/lib.rs:39`).
- `MAX_PAYLOAD_SIZE` is unused.
- Module docs describe TOFU and the "short code" of KDE pairing.

**Fix.** Drop `tls12` and `logging`. Remove `TrustCheck`/`check` or use them for MAG-1. Change the DN for new certificates only; pins are by fingerprint, so existing ones are unaffected. Rewrite the justifications.

### MAG-27: No SIGTERM path in the daemon (Minor)

**Evidence.** `magnetitad/src/main.rs:169-171`: `loop { thread::park(); }`. There is no signal handling, so `systemctl stop` (used by every deploy) kills the process. `LinkWire::drop`, the FUSE `BackgroundSession` unmount and the `Advertisement` teardown never run. `mount::clear_stale` only repairs this at the next start, and the unit has no `ExecStopPost`.

**Why it matters.** After a stop, Siderita sees a dead mountpoint ("Transport endpoint is not connected") until the daemon runs again. The "deterministic shutdown" rule is not met.

**Fix.** Handle SIGTERM with a small self-pipe or `signal-hook`, and drop the wire and the mounts. Add `ExecStopPost=-/usr/bin/fusermount3 -uz %t/magnetita/%i` or equivalent.

### MAG-28: The mobile FFI layer is untested (Minor, Tests)

**Evidence.** `magnetita-mobile` has two tests, both in `storage.rs`. `mobile.rs` (901 lines) and `phone.rs` (1,039 lines) have none. The mappings there are lenient:
- `send_call_event` maps any unknown state to `CallState::Ended` (`mobile.rs:~700`);
- `pointer_button` maps any unknown button to `Middle`;
- `send_mirror_started` maps any codec other than 1 to HEVC.

Proto refuses unknown values, while the FFI layer silently changes them.

**Fix.** Add unit tests for the `Event` mapping and the FFI coercions, and refuse unknown codes with `MobileError`. Add the slow-sender test from MAG-4.

### MAG-29: Notification bodies rely on the server not supporting markup (Minor, Security)

**Evidence.** `notify.rs:46-60` sends phone-provided `summary` and `body` as-is to `org.freedesktop.Notifications.Notify`. ADR 0001 §11 promises "rendered as plain text". This holds only because the suite's own server omits `body-markup` (`celestina-shell-core/src/notifications.rs:206`).

**Why it matters.** Under any server that does parse markup, text chosen by third parties (SMS, chat) is interpreted: links and images, or a garbled body.

**Fix.** Call `GetCapabilities` once and escape `&<>` when `body-markup` is present.

### MAG-30: Small cleanups (Minor, Quick win)

- `link_wire/commands.rs:154`: `let _ = GroupPolicy::Terminate;` is a dead statement that hides an unused-item warning. Remove it, or use the policy.
- `magnetita-proto/src/codec.rs`, `hello.rs`, `pair.rs`: 37 `unwrap()` calls on `Encoder<Vec<u8>>`. They cannot fail (`Infallible`), but only `envelope.rs` states that. Centralise them in one helper that documents the invariant.
- `magnetita/src/controller.rs:974`: the `xdg-open` child is spawned and dropped without `wait`, which leaves a zombie per click until the app exits.
- `magnetita-peer/src/lib.rs:23`: a third `avahi-browse` invocation, alongside `link_wire/discovery.rs` and `mirror_discovery.rs`.

### AND-7: Polling loops and an unbounded input queue on the phone (Minor, Performance)

**Evidence.** `LinkController.hold`'s reporter wakes every `pollMs` (1 s) to drain `outbound` with `tryReceive` (`LinkController.kt:184-190`), instead of suspending on `outbound.receive()`. `LinkService.inputExecutor` is a single-thread executor with an unbounded queue (`LinkService.kt:~300`); pointer moves are never merged, and `pointerButton` and `scroll` are blocking sends.

**Fix.** Suspend on the channel with `select` against the drop request. Merge moves in a conflated channel.

### AND-8: Backup rules are the scaffold template (Minor, Security)

**Evidence.** `res/xml/data_extraction_rules.xml` and `backup_rules.xml` are the Android Studio samples, including "TODO: Use <include> and <exclude>". For `targetSdk 36`, `allowBackup="false"` does not disable device-to-device transfer. The identity (`privateKey.pem`) and `trust.json` live under `filesDir/magnetita` (`core/Core.kt:18`).

**Fix.** Exclude `file/magnetita/` from `cloud-backup` and `device-transfer` explicitly, and remove the template comments.

### AND-9: Received files on the phone (Minor, Correctness)

**Evidence.**
- `magnetita-mobile/src/phone.rs:446-464` accepts any `offer.size` into `cacheDir/received`.
- The name sanitising (`:453-458`, default `"file"`) duplicates the daemon's `incoming_file::safe_filename` (default `"archivo"`).
- `share/Downloads.kt:20-26` returns `null` without deleting the inserted `IS_PENDING=1` row when `openOutputStream` fails or the copy throws.

**Fix.** Enforce a size cap. Move `safe_filename` into `magnetita-proto` or `magnetita-core`, used by both sides. Delete the pending row on failure.

## Limits

- No Android toolchain was used: Gradle, lint and the JVM tests did
  not run, and no SDK or NDK was available.
- No real device, Wayland or FUSE behaviour beyond the existing
  loopback tests.
- `mirror_stream.rs`, `media.rs` (MPRIS polling) and
  `magnetita-core/src/mirror.rs` were only skimmed.
- MAG-20's battery cost is unmeasured.

## Follow-up

Each finding closes in the program unit that carries it; the program
ids, the rulings and the dependency order are in the
[monorepo audit](2026-09-26-monorepo-audit.md) record, and the units are ledger rows of the plans named
below. The suite rows are in the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md).

| Program | Ledger unit | Plan | Findings of this area it closes |
|---|---|---|---|
| P-2 | `AUD-1-D` | `docs/plans/active/2026-09-26-monorepo-hardening.md` | MAG-9 |
| P-3 | `AND-6-D` | `magnetita-android/docs/plans/active/2026-09-13-app-design.md` | AND-1, AND-4, AND-6, AND-8 |
| P-11 | `MAG-D1-D` | `magnetita/docs/plans/active/2026-09-13-app-design.md` | AND-4, MAG-1, MAG-2, MAG-3, MAG-10, MAG-16, MAG-18, MAG-29 |
| P-12 | `MAG-D1-E` | `magnetita/docs/plans/active/2026-09-13-app-design.md` | MAG-4, MAG-5, MAG-6, MAG-7, MAG-11, MAG-13, MAG-14, MAG-15, MAG-17, MAG-20, MAG-21, MAG-22, MAG-24, MAG-25, MAG-26, MAG-27, MAG-30 |
| P-13 | `AUD-1-E` | `docs/plans/active/2026-09-26-monorepo-hardening.md` | AND-2, AND-3, AND-5, AND-7, AND-9, MAG-8, MAG-12, MAG-19, MAG-28 |

Unscheduled backlog (Minor; taken when the file is next touched): MAG-23.

MAG-25 cannot be repaired: history is immutable. Ruling R-A5 records a
waiver in `magnetita/docs/plans/active/2026-09-13-app-design.md`, and
Magnetita product fixes use `magnetita-bug` from now on.

### Proposed units, as the auditor wrote them

The program replaces the component and `suite:` prefixes proposed here
with the owning product's primary prefix, as section 6 of the
[monorepo audit](2026-09-26-monorepo-audit.md) record explains; the grouping below is kept as the
auditor's reasoning.

In order of value for effort:

1. **`magnetita-android:` Make pairing and remote actions consent-bound.** Covers AND-1 and AND-6, plus the `PairLink` use from AND-5. Adds a confirmation screen for any pairing intent, refuses non-LAN addresses, and gates key and global actions on an active mirror. S to M.
2. **`magnetita:` Harden own-wire admission and identity.** Covers MAG-1, MAG-2 and MAG-3. Binds the session id to the pinned fingerprint, moves handshakes off the accept loop, and keeps the QR window open until a verified proof. M.
3. **`magnetita:` Make session I/O cancel-safe and bounded on both ends.** Covers MAG-4, MAG-5, MAG-7 and MAG-14, and AND-2 (paired `magnetita-android:` change for `pump`). Adds a phone reader task, a daemon writer task with deadlines, a bounded mirror FIFO, and adapters on blocking threads. M.
4. **`suite:` Register Magnetita's real production and verification inputs.** Covers MAG-9, with MAG-10 as its `magnetita:` counterpart. Adds proto, link and the fluorita crates to the fingerprint, declares Android inputs, and tests and lints proto, link, mobile and peer in `verify-production.sh`. S.
5. **`magnetita:` Release held input and stop discovery churn.** Covers MAG-6, MAG-11, MAG-20 and MAG-27. Releases pressed keys on session end, removes or fixes the dial loop, polls adb only on demand, uses a 10 s keep-alive, and handles SIGTERM. S to M.
6. **`magnetita:` Make capability negotiation and protocol ownership real.** Covers MAG-8, MAG-19, MAG-28 and AND-5, with a paired `magnetita-android:` change to delete the Kotlin copies. Adds real hello sets and negotiated gating, typed UniFFI signals, one clipboard bound, and FFI tests. M to L.
7. **`magnetita:` Cheap, accessible Messages and mirror paths.** Covers MAG-12, MAG-13, MAG-21 and MAG-22, plus AND-3 and AND-4 (paired `magnetita-android:`). Drives Messages from signals, caches the mirror connection and merges moves, makes the conversation row accessible, uses a thread query for SMS, and returns `ENOTEMPTY` instead of deleting recursively. M.
8. **`magnetita:` Truth and hygiene.** Covers MAG-15 through MAG-18, MAG-23 through MAG-26, MAG-29, MAG-30, and AND-7 through AND-9 (Android parts under `magnetita-android:`). Documentation rewrite, atomic `commands.json`, share caps and sweep, `magnetita-net` cleanup, and the versioning practice. M.
