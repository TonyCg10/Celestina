# Magnetita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** MAG-P7
- **Related author validation:** `VAL-MAG-01` through `VAL-MAG-08` in
  [VALIDATION.md](VALIDATION.md); they do not block implementation

`MAG-P7` is executing under
[its plan](docs/plans/active/2026-09-10-storage-and-retirement.md), paired
with the `magnetita-android` project's own `AND-5`. `MAG-P6` is closed under
[its archived plan](docs/plans/archive/2026-09-09-link-mirror.md), paired
with `AND-4`, its last observation the author's `VAL-MAG-14`. `MAG-P5` is
closed under
[its archived plan](docs/plans/archive/2026-09-09-remote-control.md), paired
with `AND-3`. `MAG-P4` is closed under
[its archived plan](docs/plans/archive/2026-09-09-daily-set.md), paired with
`AND-2`. `MAG-P3` is closed under
[its archived plan](docs/plans/archive/2026-09-09-android-foundation.md),
paired with `AND-1`. `MAG-P2` is closed under
[its archived plan](docs/plans/archive/2026-09-09-link.md), `MAG-P1` under
[its archived plan](docs/plans/archive/2026-09-09-protocol-core.md), `MAG-P0`
under [its own](docs/plans/archive/2026-09-07-own-protocol-spikes.md).
`MAG-S1`'s
units are all done and committed under
[its archived plan](docs/plans/archive/2026-08-05-network-input-hardening.md);
its canonical production exit is still a pending deployment action recorded in
[STATUS.md](STATUS.md). `MAG-M1` remains settled and has no execution plan.

## MAG-S1 — Hostile network input at the daemon's boundaries

## Hypothesis and tangible outcome

Every defect the 2026-08-05 static audit raised against Magnetita is one
boundary trusting a value the peer chose, so validating each where the value
becomes typed removes the class rather than the instances. The tangible outcome
is a daemon a paired phone cannot turn into command execution, a mount cannot
be redirected away from the authenticated link, a handshake that ends on an
absolute deadline, a protocol floor the peer cannot argue its way below, and a
private key no other local user can read.

## Scope

- Validate the `kdeconnect.sftp` user, path and password at the decode boundary
  and mount only against the TLS-authenticated address (`MAG-C1`, `MAG-M6`).
- Bound the whole handshake with one absolute deadline and log admission
  exhaustion (`MAG-A1`).
- Floor the protocol at 8, take the peer identity only from the encrypted
  channel, restrict dialling to the standard port on local addresses, and
  decide trust before publishing a device (`MAG-A2`, `MAG-M2`, `MAG-M5`).
- Apply the existing bounded-subprocess discipline to the clipboard and the
  mount (`MAG-A3`).
- Create the private key owner-only and atomically; bound peer-chosen text and
  the notification map; render remote strings as plain text (`MAG-M7`).

## Exclusions

- Commit, version transition, production build, deployment, and any restart or
  inspection of the live `magnetitad`.
- `MAG-M1`'s app-side read/watch lifecycle.
- The `Revocations`/registry locks held across payload file I/O, which cannot
  be shortened without reordering the revocation barrier's locking policy.
- Any project other than Magnetita and its three registered crates.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-S1-A | done | none | The SFTP reply cannot become an sshfs option or redirect the mount | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |
| MAG-S1-B | done | none | One absolute handshake deadline from the crate's single owner of that recipe | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |
| MAG-S1-C | done | MAG-S1-B | Protocol floor, encrypted-only identity, bounded dial target, trust before publication | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |
| MAG-S1-D | done | none | Clipboard and mount subprocesses bounded and reaped like media's | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |
| MAG-S1-E | done | none | Owner-only atomic key, bounded peer text, plain-text rendering | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |

## Implementation exit

Close `MAG-S1` when every unit's tests pass alongside format, Clippy, the
workspace check, QML lint and the architecture contract, **and** the author has
requested the canonical `scripts/complete-production.sh` exit. The corrections
were authorized without a production build or deployment, so the units stay
`active` and the installed daemon still carries the uncorrected bytes until
that request arrives. `magnetita-core` changed, so closing will also require
`celestina/scripts/complete-production.sh`.

## MAG-M1 — Deterministic app read/watch lifecycle

## Hypothesis and tangible outcome

Owning the app's best-effort D-Bus reads and watcher lifecycle will let Magnetita
close deterministically without losing coalescing or blocking Qt. The tangible
outcome is a client that can be created, flooded with refreshes and destroyed
repeatedly with every owned worker joined and no stale snapshot applied.

## Scope

- Inventory every read/watch thread and callback currently detached from the
  app QObject lifecycle.
- Replace detached ownership with a bounded cancellation/shutdown contract.
- Preserve one ordered action worker and at most one coalesced follow-up read.
- Reject callbacks after the owning QObject begins shutdown.
- Add repeated create/burst/close tests and update lifecycle documentation.

## Exclusions

- Changing the KDE Connect wire protocol or `org.celestina.Devices1` semantics.
- Re-pairing, deleting trust state or restarting the live daemon.
- New plugins or Android-side work.
- Real phone, mount, artwork and Wayland acceptance.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-M1-A | planned | none | Complete ownership map and failing close/burst regression | Focused app lifecycle test |
| MAG-M1-B | planned | MAG-M1-A | Cancelable, joined read/watch lifecycle | Repeated create/burst/close test |
| MAG-M1-C | planned | MAG-M1-B | App, daemon and D-Bus consumers remain compatible; installed bytes are current | `scripts/complete-production.sh` |

## Implementation exit

Close `MAG-M1` when every app-owned thread has a deterministic termination path,
refresh coalescing still delivers the newest confirmed snapshot, post-shutdown
callbacks are rejected and the exact production artifacts pass
`scripts/complete-production.sh`, including deployment to the author's normal
test destination without a second build. If an implementation unit changes
`magnetita-core`, it also runs `celestina/scripts/complete-production.sh` so the
installed shell bundle carries the same shared contract without activating the
live session. Do not wait for a real phone session to close the checkpoint.

## MAG-R1 — One-button wireless screen mirror

## Hypothesis and tangible outcome

This host's `adb` is built without mDNS, so every manual step in the author's
`~/Scripts/cpy.sh` — a hardcoded IP, a ping, a cached port, a 20000-port TCP
sweep and three prompts — substitutes for one service lookup the phone already
publishes. Browsing `_adb-tls-connect._tcp` and `_adb-tls-pairing._tcp` through
the running Avahi daemon, which `magnetitad` can reach over the `zbus` it
already depends on, collapses all of it. The tangible outcome is a Mirror
control that opens scrcpy on the phone with no terminal, no address, no port
and no code after the first pairing, and that survives the phone's port
changing every time Wireless debugging is toggled.

## Scope

- Avahi mDNS watchers for the two ADB service types, as typed appear/disappear
  events with validated host, port and service name.
- A pure link state machine with a typed reason for every failure the UI shows.
- Pairing and reconnection with no port or address entered by hand.
- Owned, bounded `adb` and `scrcpy` subprocesses killed by the pid the daemon
  started, never by process name.
- A new versioned `org.celestina.Mirror1` interface and a Mirror control.

## Exclusions

- Embedding the scrcpy surface in QML. The author chose scrcpy's own window;
  embedding means decoding `scrcpy-server`'s stream in process.
- Extending `org.celestina.Devices1`. Mirroring is not KDE Connect.
- Replacing this host's `adb` with the AUR platform-tools build.
- Audio, recording, OTG and any scrcpy feature beyond the author's script.
- Enabling Wireless debugging on the phone, which Android reserves to the user.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-R1-A | done | none | Validated endpoints and the pure link state machine | `cargo test -p magnetita-core -p magnetita-net` |
| MAG-R1-B | done | MAG-R1-A | Pairing against a discovered endpoint, no port typed by hand | Pairing unit tests |
| MAG-R1-C | done | MAG-R1-B | Resident reconnection, owned processes and `org.celestina.Mirror1` | Producer/consumer tests |
| MAG-R1-D | done | MAG-R1-C | The Mirror control over confirmed snapshots | `qmllint` and app lifecycle test |
| MAG-R1-E | done | MAG-R1-D | Installed bytes carry the mirror | `scripts/complete-production.sh` |

## Implementation exit

Every unit is implemented, tested and deployed: `scripts/complete-production.sh`
passed and `celestina/scripts/complete-production.sh` carried the
`magnetita-core` addition to the installed shell bundle. Against the real S25U,
discovery, pairing with only the six digits read off the phone, connecting and
mirroring all happened exactly as designed, and a defect the first live attempt
found — an exited scrcpy read as the author closing the window when the phone
had merely gone away — is corrected and deployed. What has not yet been
observed live is the reconnection itself completing unattended end to end: the
toggle test was run once, against the pre-fix daemon, and has not been repeated
since. Unlike `MAG-M1`, this checkpoint does not close on tests alone: `VAL-MAG-08`
is the exit, because a loopback test cannot observe an mDNS advertisement
crossing the LAN, and it is not yet claimed.

Plan: [wireless mirror](docs/plans/archive/2026-08-19-wireless-mirror.md).

## MAG-R2 — The mirror without discovery

## Hypothesis and tangible outcome

`MAG-R1` made the mirror one press, but only while Android was advertising.
Android turns wireless debugging off constantly — on every reboot and on its
own besides — and the advertisement goes with it, so the author was back to
enabling it by hand each time.

Measured on the author's S25U: `adb tcpip` and wireless debugging are two
different listeners. Turning wireless debugging off stopped the mDNS
advertisement dead while the fixed port stayed open and the device stayed
`device`. Pinning the phone to that port after the first connection, and
remembering it, therefore removes the discovery dependency for every mirror
after the first. The tangible outcome is a Mirror control that works with
nothing advertised at all.

## Scope

- Pin the device to `adb tcpip` port 5555 once a discovered endpoint is up, and
  reconnect there.
- Remember that endpoint across daemon restarts, validated on load like any
  other value that becomes a subprocess argument.
- Prefer a live advertisement, fall back to the remembered port when the
  connection fails.

## Exclusions

- Surviving a reboot of the *phone*. `persist.adb.tcp.port` is the only thing
  that would, and setting it was attempted and refused: it needs root the
  author's phone does not have. One manual enable per phone reboot remains.
- Any change to how pairing works. The six-digit path `MAG-R1` delivered is
  untouched.
- The QR pairing decision, still open and still independent.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-R2-A | done | MAG-R1 | The mirror reaches the phone with nothing advertised | `cargo test -p magnetita-core -p magnetitad` |

## Implementation exit

Close `MAG-R2` when the mirror connects with wireless debugging off and the
daemon restarted, which was observed on the real phone. `VAL-MAG-09` carries
the author's own acceptance across a phone reboot, which no test can stand in
for.

Plan: [mirror without discovery](docs/plans/archive/2026-08-19-mirror-without-discovery.md).

## The own-protocol program — MAG-P0 through MAG-P7

[ADR 0001](docs/decisions/0001-own-protocol-and-android-app.md) (accepted
2026-09-04) turns Magnetita from a KDE Connect client into its own protocol
with its own Android application. The program is eight checkpoints in
dependency order. `MAG-P0` ran from 2026-09-07 to 2026-09-09 under
[its archived plan](docs/plans/archive/2026-09-07-own-protocol-spikes.md) and
verified every choice the [discussions](docs/discussions/README.md) had
concluded; no fallback was triggered. `MAG-P1` opened on 2026-09-09.

## MAG-P0 — Spikes that verify the accepted choices

## Hypothesis and tangible outcome

Every material choice in ADR 0001 has one measurement that verifies it, and
each can be taken in isolation before any product code exists. The tangible
outcome is five dated evidence records with numbers that either confirm the
accepted choices or trigger the ADR's named *Revisit when* fallbacks —
cheaply, before a crate exists.

## Scope

- Build `quinn` + `rustls`/`ring` for `aarch64-linux-android` with
  `cargo-ndk`, wrap a hello in UniFFI, exchange it with a throwaway daemon
  peer over the author's LAN; measure small-message latency under a bulk
  stream and survival of a Wi-Fi toggle.
- A throwaway Kotlin activity: `MediaProjection` → `MediaCodec` HEVC → QUIC →
  a throwaway desktop decoder; glass-to-glass latency at 1080p60.
- A throwaway accessibility service: tap and drag latency from a desktop
  event to the gesture landing.
- `/dev/uinput` under a udev rule on the author's host, injecting into the
  development nest only; presence of a RemoteDesktop portal on the session.
- QR pairing on the S25U and typed pairing against a headless peer.

## Exclusions

- Any code under `celestina-rs/crates/magnetita-*` or `magnetita/`; spikes
  live in a scratch directory and are deleted, their numbers kept.
- Touching the installed daemon, the live session or the paired trust.
- Deciding anything the numbers do not decide.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-P0-A | done | none | Rust QUIC core runs on the phone through UniFFI; latency and migration measured | [loopback](docs/evidence/2026-09-07-quic-loopback-latency.md), [phone](docs/evidence/2026-09-08-quic-on-the-phone.md) |
| MAG-P0-B | done | MAG-P0-A | Own-app mirror latency measured on the S25U | [record](docs/evidence/2026-09-09-own-mirror-latency.md) |
| MAG-P0-C | done | none | Accessibility-service input latency measured | [record](docs/evidence/2026-09-09-accessibility-input-latency.md) |
| MAG-P0-D | done | none | `uinput` and portal availability on the host; nest injection deferred to `MAG-P5-B` | [record](docs/evidence/2026-09-07-uinput-and-portal.md) |
| MAG-P0-E | done | MAG-P0-A | QR and typed pairing each pair once with no other input | [record](docs/evidence/2026-09-08-qr-and-code-pairing.md) |
| MAG-P0-F | done | A–E | Each evidence record cited from its discussion; no fallback triggered | [record](docs/evidence/2026-09-09-spike-program-bookkeeping.md) |

## Implementation exit

Closed on 2026-09-09: every record carries its measurement, every conclusion
cites it, no fallback was triggered, and the author judged the mirror latency
usable. The nest half of `MAG-P0-D` is delivered by `MAG-P5-B`, which needs
exactly that proof.

## MAG-P1 — The protocol core, `magnetita-proto`

## Hypothesis and tangible outcome

One pure crate can own the envelope, the message catalog, capability
negotiation, pairing state machines and every hostile-input bound, and be the
single implementation both ends link. The tangible outcome is a crate with no
I/O whose tests encode the wire: golden CBOR vectors for every message, a
pairing state machine driven from both roles, and a decoder that refuses
every oversized, malformed or out-of-capability input with a typed reason.

## Scope

- Envelope `{version, capability, kind, id, body}` in CBOR with integer keys;
  unknown keys ignored, unknown capabilities declined in hello.
- Hello and capability negotiation with per-capability versions.
- Pairing: QR payload, one-time secret, possession proof over both
  fingerprints; the typed-code path per the concluded discussion.
- Message catalog for `battery`, `clipboard`, `notifications` (post, dismiss,
  reply, actions), `find`, `share` (offer, accept, stream id, resume offset),
  `media` (both directions), `commands` (desktop-registered ids only),
  `input` (typed pointer, scroll, key and text events), `mirror` (session
  offer, codec, resolution, stream ids, consent state), `sms` (conversation
  list, thread page, send request, received message, MMS attachment as a
  `share` stream), `contacts` (vCard 4.0 sync with a per-contact version so
  only changes travel), `telephony` (ringing, answered, missed, ended;
  mute, answer, hang up).
- Bounds on every string, list, size and count, tested at the boundary.
- A wire document in `docs/` of this project describing the catalog, the
  vectors and the versioning rule.

## Exclusions

- Sockets, TLS, discovery and time: `magnetita-link`.
- `storage`: `MAG-P7`.
- Removing anything from `magnetita-core` or `magnetita-net`.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-P1-A | done | MAG-P0 | Envelope, hello, negotiation and the bound rule with golden vectors | [record](docs/evidence/2026-09-09-protocol-envelope-and-hello.md) |
| MAG-P1-B | done | MAG-P1-A | Pairing state machines for both roles, both paths | [record](docs/evidence/2026-09-09-protocol-pairing.md) |
| MAG-P1-C | done | MAG-P1-A | The daily-set catalog and the wire document | [record](docs/evidence/2026-09-09-protocol-daily-catalog.md) |
| MAG-P1-D | done | MAG-P1-C | `commands`, `input` and `mirror` messages | [record](docs/evidence/2026-09-09-protocol-control-and-mirror.md) |
| MAG-P1-E | done | MAG-P1-C | `sms`, `contacts` and `telephony` messages with bounded bodies, names and numbers | [record](docs/evidence/2026-09-09-protocol-phone-surface.md) |

## Implementation exit

Met on 2026-09-09: 63 tests with every message pinned by a committed vector,
Clippy, format and the architecture contract all pass, and
[the wire document](docs/protocol.md) maps the catalog. `MAG-P2` took the
checkpoint the same day.

## MAG-P2 — The link, the daemon's second wire and a headless peer

## Hypothesis and tangible outcome

`magnetita-link` can carry the protocol over QUIC with pinned mutual
certificates on one runtime thread inside the thread-based daemon, and a
headless peer built from the same crates can stand in for the phone in every
daemon test. The tangible outcome is `magnetitad` accepting and dialling own
protocol sessions next to KDE Connect ones, publishing them on
`org.celestina.Devices1` unchanged, and a `magnetita-peer` binary that pairs,
connects and exercises every capability from a shell — the same headless
observation the author prefers for Siderita.

## Scope

- `magnetita-link`: certificate generation (reusing `magnetita-net::cert`
  where the recipe is the same), trust store, QUIC endpoint, stream
  allocation per capability, reconnection with backoff, connection migration,
  the absolute handshake deadline from `MAG-S1`.
- Avahi advertise and browse of `_magnetita._udp` through the existing `zbus`
  mirror-discovery module, generalised only if the semantics are the same.
- The daemon: one owned runtime thread for the link; typed events into the
  existing device registry; pairing acceptance through the app; the existing
  `Forget` barrier covering the new trust store.
- `magnetita-peer`: a headless peer for loopback and LAN tests.

## Exclusions

- Any Kotlin.
- Changing `org.celestina.Devices1` beyond additive `a{sv}` keys.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-P2-A | done | MAG-P1 | Endpoint, trust, handshake deadline, streams, loopback tests | [record](docs/evidence/2026-09-09-link-endpoint.md) |
| MAG-P2-B | done | MAG-P2-A | Discovery both ways through Avahi | [record](docs/evidence/2026-09-09-link-discovery.md) |
| MAG-P2-C | done | MAG-P2-B | The daemon hosts both wires; devices publish unchanged | [record](docs/evidence/2026-09-09-daemon-own-wire.md) |
| MAG-P2-D | done | MAG-P2-C | `magnetita-peer` pairs, reconnects, migrates | [record](docs/evidence/2026-09-09-headless-peer.md) |

## Implementation exit

Close `MAG-P2` when the peer pairs with the daemon on loopback and over the
LAN from another host, a Forget revokes it durably, a migration keeps the
session, and the KDE Connect phone still pairs and mounts as before under
`scripts/complete-production.sh`. On 2026-09-09 every unit is implemented
(`95a4cc8`), the daemon was deployed by `scripts/complete-production.sh` and
the headless peer paired, reported, was rung and was forgotten against it
on the real interface — see
[the deployed record](docs/evidence/2026-09-09-own-wire-deployed.md). The
author ruled the same day that there is no other host — Magnetita links one
desktop and one phone — so the "another host" clause is met by the phone
itself in `MAG-P3`, and the migration stays proven by the loopback test until
the phone toggles its Wi-Fi in `MAG-P3`. `MAG-P2` is closed; `MAG-P3` took the
checkpoint the same day.

## MAG-P3 — The Android application foundation

## Hypothesis and tangible outcome

A Kotlin application that links `magnetita-mobile` through UniFFI can hold a
paired session in a foreground service across Doze, app switches and Wi-Fi
changes, with all protocol truth in Rust and only platform adapters in
Kotlin. The tangible outcome is the registered `magnetita-android/` project:
discovery, QR pairing, a device screen with battery and ping, reconnection
without touching the phone, and a signed release APK produced by the
project's own build script.

## Scope

- Project scaffold: Gradle KTS, version catalog, Kotlin 2.x, Compose Material
  3, coroutines/`Flow`, `DataStore`, `minSdk 31`; `cargo-ndk` and UniFFI
  bindings generated in the build; Spanish product copy per ADR 0007.
- Foreground service with `connectedDevice` type owning the link; multicast
  lock for discovery; persistent trust in app storage.
- QR scan (CameraX + ML Kit barcode) and the typed path.
- Device screen: name, connection state, battery both ways, ping, find.
- Registry entry, README/STATUS/ROADMAP/VALIDATION, build/verify/deploy
  scripts (deploy installs by `adb` to the paired phone only when the author
  asks), signing key outside the repository. The application is the
  registered project `magnetita-android` with its own checkpoint `AND-1`;
  this checkpoint owns the Rust side and the pairing of the two.

## Exclusions

- The daily set, commands, input and mirror.
- Google Play, any cloud, any analytics, any third-party SDK beyond
  AndroidX, Material and the barcode scanner.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-P3-A | done | MAG-P2 | `magnetita-mobile` UniFFI crate and the Gradle scaffold building it | `cargo test -p magnetita-mobile`, `./gradlew assembleDebug` |
| MAG-P3-B | done | MAG-P3-A | Foreground service holding a session; discovery; trust | JVM unit tests, Rust tests |
| MAG-P3-C | done | MAG-P3-B | Pairing screens, device screen, battery, ping, find | JVM tests, `qmllint`-equivalent Android lint |
| MAG-P3-D | done | MAG-P3-C | Registered project with scripts, signed artifact, docs set | documentation contract, `verify-production.sh` |

## Implementation exit

Close `MAG-P3` when the release build passes lint and tests, the app pairs
with the daemon on the LAN, and a session survives screen off, app switch and
Wi-Fi toggle. `VAL-MAG-11` carries the author's first pairing on the S25U.

Met on 2026-09-09: the release APK passes lint and its unit tests through
the suite's runner; the S25U paired by scanning the QR the daemon armed, and
the same session held through screen off, an app switch and a Wi-Fi toggle
in a manual run (no instrumented test); the desktop app shows that QR
(`MAG-P3-B`). The author's scan of the desktop's screen is `VAL-MAG-11`.

## MAG-P4 — The daily set on the own wire

## Hypothesis and tangible outcome

The daemon's existing plugin modules keep their domain and gain the second
wire, and the phone side delivers what the stock client could not: automatic
phone-to-desktop clipboard while the app is in the foreground or via its
quick-settings tile and share target, notification actions and replies, and
resumable file transfers on their own streams. The tangible outcome is the
author's daily set working end to end without the stock KDE Connect client.

## Scope

- Clipboard both ways, with the Android background limit stated in the UI
  rather than hidden.
- Notifications through `NotificationListenerService`: post, update,
  dismiss, actions, inline reply, app icon; desktop rendering as plain text.
- File share both ways on dedicated streams with resume and the existing
  revocation barrier.
- Media control both ways: desktop MPRIS to the phone, phone
  `MediaSession` to the desktop's existing `org.mpris` projection.
- Battery and find-my-phone already from `MAG-P3`.
- Contacts: one-way vCard 4.0 sync from the phone, kept in the daemon's
  memory for the session; names resolve numbers for the two capabilities
  below.
- SMS: conversation list and thread pages on the desktop app, send by
  conversation id, received messages as notifications with inline reply,
  MMS attachments as `share` streams; nothing at rest on the desktop unless
  the author enables it per capability.
- Telephony: ringing and missed-call notifications with the resolved name,
  mute the ringer, answer and hang up through `TelecomManager`; call state
  published additively on `org.celestina.Devices1` so the shell's phone menu
  can show it.

## Exclusions

- RCS, call audio on the desktop, presenter, drawing tablet, storage.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-P4-A | done | MAG-P3 | Clipboard both ways with the tile and share target | peer tests, JVM tests |
| MAG-P4-B | done | MAG-P3 | Notifications with actions and replies | peer tests, JVM tests |
| MAG-P4-C | done | MAG-P3 | Resumable file share both ways under revocation | peer tests |
| MAG-P4-D | done | MAG-P3 | Media control both ways | peer tests, MPRIS consumer tests |
| MAG-P4-E | done | MAG-P1-E | Contacts sync and name resolution | peer tests, JVM tests |
| MAG-P4-F | done | MAG-P4-E | SMS conversations, send, receive, MMS attachments | peer tests, JVM tests |
| MAG-P4-G | done | MAG-P4-E | Call state, mute, answer, hang up; shell phone menu shows the call | peer tests, shell consumer tests |

## Implementation exit

Close `MAG-P4` when `magnetita-peer` exercises every capability against the
daemon in tests, the app's JVM tests cover each adapter, and
`scripts/complete-production.sh` passes with both wires. `VAL-MAG-12` carries
the author's daily use.

Met on 2026-09-09: every capability of the daily set has its loopback test
against the daemon and its adapter's JVM tests; `complete-production.sh`
passed and deployed with both wires. Clipboard, notifications, file share
and media were also seen on the S25U; contacts and conversations reached
the desktop on the session's request; calls and real SMS traffic are the
author's `VAL-MAG-12`.

## MAG-P5 — Commands, trackpad and keyboard

## Hypothesis and tangible outcome

Remote control can be added without a single peer-chosen string reaching a
shell or the compositor: the phone triggers desktop-registered command ids,
and typed input events become virtual-device input through the path
`MAG-P0-D` settled. The tangible outcome is a trackpad and keyboard screen on
the phone that moves the desktop pointer and types into the nest, and a
commands screen that runs the author's registered scripts.

## Scope

- Commands: registered in the desktop app's settings (name, program,
  arguments as a typed vector), published to the phone as ids, executed by
  the daemon's bounded subprocess discipline.
- Input: relative motion, scroll, buttons, tap gestures, key codes and text
  through `uinput` (or the portal), owned and destroyed by the daemon.
- Rate and size bounds on input; input rejected from an untrusted or
  revoked source at the decode boundary.

## Exclusions

- Presenter mode, absolute-position tablet, gamepad.
- Any test against the author's live session.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-P5-A | done | MAG-P4 | Registered commands, published and executed by id | peer tests, subprocess tests |
| MAG-P5-B | done | MAG-P0-D | Virtual pointer and keyboard in the daemon, nest-only tests | daemon tests, nest evidence |
| MAG-P5-C | done | MAG-P5-B | Trackpad and keyboard screens on the phone (`AND-3-A`) | JVM tests |

## Implementation exit

Close `MAG-P5` when the peer's synthetic input moves the nest's pointer and
types a sentence observed through the nest's own IPC, commands run only by
registered id, and the host change (`uinput` udev rule) is recorded in
`HOST-HYGIENE.md`. `VAL-MAG-13` carries the author's hand use.

Met on 2026-09-09 as far as the records allow: the loopback test finds the
peer's key, text and motion in the recorder, the device opens on this host,
commands run by id only, the rule is recorded, and the phone's screen
shipped as `AND-3-A`. The nest's IPC was not consulted: even the nest
cannot be targeted exclusively by a `uinput` device, so no event was
emitted from here; the pointer's motion is the author's `VAL-MAG-13`.

## MAG-P6 — The mirror as a capability of the link

## Hypothesis and tangible outcome

If `MAG-P0-B` and `MAG-P0-C` satisfied the author, the mirror moves onto the
paired link: capture and encode on the phone, decode and present in a
daemon-owned window on the desktop, input back through the accessibility
service, consent on the phone. The tangible outcome is a Mirror control that
works after a phone reboot with nothing enabled by hand and no `adb`.

## Scope

- Phone: `MediaProjection` foreground service, `MediaCodec` HEVC/H.264,
  `AudioPlaybackCapture`, an accessibility service for gestures, global
  actions and text.
- Desktop: decoder and window (the seam chosen in the concluded discussion),
  `org.celestina.Mirror1` extended additively, the existing options
  (resolution, bit rate, audio) mapped onto the new session.
- The `adb`/`scrcpy` path untouched during this checkpoint.

## Exclusions

- Recording, OTG, secure-surface capture, screen-off mirroring, which an
  unprivileged app cannot do.
- Retiring the `adb` path: `MAG-P7`, after `VAL-MAG-14`.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-P6-A | done | MAG-P0-B | Capture, encode and stream on the phone (`AND-4-A`); the core's stream and the peer's file | JVM tests, peer decode test |
| MAG-P6-B | done | MAG-P6-A | Desktop decoder window and `Mirror1` extension | daemon tests, consumer tests |
| MAG-P6-C | done | MAG-P0-C | Input back through the accessibility service (`AND-4-B`) | peer tests |
| MAG-P6-D | done | MAG-P6-B | The Mirror control chooses the link mirror | `qmllint`, app lifecycle test |

## Implementation exit

Close `MAG-P6` when the peer decodes a synthetic stream the phone build
produced, `scripts/complete-production.sh` passes, and the mirror is
observed after a phone reboot on the S25U as `VAL-MAG-14` — this checkpoint,
like `MAG-R1`, closes on the author's observation because no test can see
the phone reboot.

Implemented on 2026-09-09: the loopback test streams into the recording
window and carries a touch back, a synthetic HEVC stream survives the
daemon's remux, and `complete-production.sh` passed. The checkpoint stays
open on `VAL-MAG-14`.

## MAG-P7 — Storage over the own wire and retiring the second path

## Hypothesis and tangible outcome

With the daily set, control and mirror on the own link, the last two KDE
Connect dependencies — the `sshfs` mount Siderita browses and the wire
itself — can be replaced by a `storage` capability and removed, leaving one
trust store, one discovery and one wire. The tangible outcome is Siderita
browsing the phone through the daemon without `sshfs`, and a daemon that
opens no KDE Connect port, as the concluded discussion directs.

## Scope

- `storage`: list, stat, read ranges, write, rename, delete over dedicated
  streams, with the Android `MediaStore`/SAF limits stated.
- The daemon's mount replaced by a FUSE file system at the same path, as
  [the discussion](docs/discussions/2026-09-10-storage-mount-or-dbus.md)
  opened at `MAG-P7`'s start proposes; the author's word closes it.
- Removal of `magnetita-net`'s KDE Connect wire, the pairing v8 path and the
  mount subprocess, as the concluded discussion directs.
- Removal of the `adb`/`scrcpy` mirror path (`MAG-R1`/`MAG-R2`) once
  `VAL-MAG-14` has observed the own mirror after a phone reboot.

## Exclusions

- Keeping any second path: the concluded discussions rule out a frozen KDE
  Connect wire and a precision-mode `scrcpy`.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-P7-A | done | MAG-P6 | `storage` capability both ends (`AND-5-A` on the phone) | peer tests, JVM tests |
| MAG-P7-B | done | MAG-P7-A | Siderita browses the phone without `sshfs`: a FUSE mount at the same path | daemon mount test, Siderita unchanged |
| MAG-P7-C | done | MAG-P7-B | The KDE Connect wire, pairing v8 and the mount removed | workspace tests, `scripts/complete-production.sh` |
| MAG-P7-E | done | MAG-P7-B | The mount's listing, attribute and read-window caches | daemon mount test |
| MAG-P7-F | done | MAG-P7-E | The range bound under the envelope's body limit | protocol tests |
| MAG-P7-D | planned | VAL-MAG-14 | The `adb`/`scrcpy` mirror path removed; `Mirror1` keeps its methods | daemon tests, `scripts/complete-production.sh` |

## Implementation exit

Close `MAG-P7` when Siderita's phone browsing tests pass against the peer,
the daemon binds only the own protocol's port, spawns neither `sshfs` nor
`adb` nor `scrcpy`, and the architecture contract records the removed crates.

`MAG-P7-A` through `-C` implemented on 2026-09-10: the mount test browses
the peer's tree through `std::fs`, the daemon binds only the own wire's
port and spawns no `sshfs`, and the baselines record the shrunk daemon.
`MAG-P7-D` waits on `VAL-MAG-14`.

## Closed evidence

The released CP0-CP4 implementation and 2026-07-29 hardening record are
preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
