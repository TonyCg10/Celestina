# Magnetita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** MAG-S1
- **Related author validation:** `VAL-MAG-01` through `VAL-MAG-08` in
  [VALIDATION.md](VALIDATION.md); they do not block implementation

`MAG-S1` is executing under
[its plan](docs/plans/active/2026-08-05-network-input-hardening.md). `MAG-M1`
remains the next settled reliability checkpoint and has no execution plan.

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

## Closed evidence

The released CP0-CP4 implementation and 2026-07-29 hardening record are
preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
