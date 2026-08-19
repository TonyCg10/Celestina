# MAG-R1 — One-button wireless screen mirror

- **Opened:** 2026-08-19
- **Closed:** 2026-08-19
- **Plan ID:** wireless-mirror
- **Status:** done
- **Authorization:** the author asked for `~/Scripts/cpy.sh` to become a Mirror
  button in Magnetita, requiring no terminal step and no manual port or code
  entry beyond enabling Wireless debugging on the phone, and chose option (a):
  scrcpy keeps its own window rather than being embedded in QML
- **Scope:** magnetita, magnetita-core, magnetita-net, magnetitad
- **Implementation checkpoint:** MAG-R1
- **Author-validation checkpoint:** `VAL-MAG-08` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)
- **Successor:** none

## Hypothesis

Everything `cpy.sh` does by hand exists because this host's `adb` was built
without mDNS. `adb mdns check` answers `mdns is not supported by this version
of adb` on `android-tools 37.0.0`, so the script substitutes a hardcoded IP, a
ping, a cached port, and a 20000-port TCP sweep for one service lookup, and
three `zenity` prompts for one pairing exchange.

The phone already publishes both facts on the LAN. With Wireless debugging on
it advertises `_adb-tls-connect._tcp` (the address and the random port the
sweep is hunting) and `_adb-tls-pairing._tcp` (the address and port the first
`zenity` box asks the author to read off the phone). `avahi-daemon` is active
on this host and speaks D-Bus, and `magnetitad` already depends on `zbus` 5.

So the whole manual surface collapses into: browse two mDNS service types,
pair once, and reconnect on the connect service reappearing. What remains
manual is only the Wireless debugging toggle, which Android does not permit a
host to set.

## Tangible outcome

The author enables Wireless debugging on the S25U and presses one Mirror
control in Magnetita. The first time, the app shows a pairing step; after that
there is no step at all — magnetitad notices the phone's advertisement,
connects, and opens scrcpy in its own window. Toggling Wireless debugging off
and on, suspending the desktop, or changing the phone's DHCP lease changes the
port and the address, and none of them require anything from the author. No
hardcoded IP, no port sweep, no cached port file, no terminal.

## Scope

- mDNS browse of `_adb-tls-connect._tcp` and `_adb-tls-pairing._tcp` through
  the running Avahi daemon, as validated endpoints with the host, port and
  service name checked. The probe below settled that this polls a terminating
  `avahi-browse` rather than parking on Avahi's D-Bus signals.
- A pure link state machine: `Unpaired → Pairing → Paired → Connected →
  Mirroring`, with a typed reason for every failure the UI must explain.
- Pairing without terminal input, and reconnection without any input.
- Owned `adb` and `scrcpy` subprocesses in magnetitad: bounded, reaped, and
  killed by the pid this daemon started.
- A new versioned `org.celestina.Mirror1` D-Bus interface.
- A Mirror control in the app, icon-first, reflecting only confirmed snapshots.

## Exclusions

- Embedding the scrcpy surface in QML. The author chose option (a). Embedding
  would mean speaking to `scrcpy-server` and decoding H.264 in-process, which
  is a project the size of Fluorita, not a button.
- Extending `org.celestina.Devices1`. Mirroring is not KDE Connect and does not
  belong in the phone-link contract that Siderita and the shell consume.
- Replacing this host's `adb` with the AUR `android-sdk-platform-tools` build.
  Avahi is already running, already D-Bus, and already how this suite discovers
  things; depending on Google's binary to get mDNS back is the worse trade.
- Audio forwarding, recording, OTG, and any scrcpy feature beyond the mirror
  the author's script already used.
- Any change to `~/Scripts/cpy.sh`. It stays as the author's fallback until
  `VAL-MAG-08` passes.

## Hostile input at the new boundary

An mDNS advertisement is peer-chosen data from an unauthenticated LAN source,
and the host, port, and service name it carries become `adb` and `scrcpy`
arguments. That is exactly the shape of MAG-C1, where an SFTP reply could
become an `sshfs` option. The same discipline applies here and is a
precondition of the first unit, not a later hardening pass:

- Validate host, port, and service name where they become typed, and refuse
  rather than salvage. A host must parse as an IP literal; a port must be a
  non-zero `u16`; a service name must match the exact name this host generated.
- Never interpolate any of them into a shell. Subprocesses take argument
  vectors.
- ADB's own TLS pairing is what authenticates the device. An advertisement is a
  hint about where to look, never proof of who answered.
- Kill scrcpy by the pid magnetitad started. `cpy.sh`'s `pkill -9 scrcpy` would
  kill an unrelated scrcpy the author is using, and this suite has already lost
  a live session to a kill-by-name.

## Build order

1. **MAG-R1-A — discovery and the pure state machine.** The Avahi browse
   watcher in `magnetita-net`, the validated typed endpoint, and the link state
   machine in `magnetita-core` with its transition tests. No subprocess yet, so
   this unit is testable without a phone.
2. **MAG-R1-B — pairing without terminal input.** Match an advertised pairing
   endpoint and run `adb pair` with a code the app supplies. Ships with the
   six-digit path first because it is free once (1) exists: the author reads six
   digits off the phone into the app, and never the port. The QR path is the
   goal and is decided separately (see the open decision below).
3. **MAG-R1-C — the resident keeper and owned processes.** Reconnect on the
   connect service reappearing, own the `adb` and `scrcpy` pids with the
   bounded-subprocess recipe MAG-S1-D already established, and serve
   `org.celestina.Mirror1`.
4. **MAG-R1-D — the Mirror control.** Delivered as two controls rather than
   one, per the author's follow-up request: an icon on `DeviceControls.qml`
   that only starts or stops the mirror, and a separate settings icon opening
   `MirrorSettingsSheet.qml` (resolution, frame rate, bitrate, which side plays
   audio, screen-off, stay-awake), backed by `MirrorChoiceRow.qml`. The same
   action was also added to the shell's own phone menu
   (`celestina/qml/PhoneMenu.qml`), beside the device it mirrors, so the
   author does not need Magnetita open to reach it. Icon-first, no tooltips
   (`QuietIconButton.qml`), reflecting confirmed state only.
5. **MAG-R1-E — production completion.** `scripts/complete-production.sh`, and
   `celestina/scripts/complete-production.sh` only if `magnetita-core` changed.

Order is forced: (2) cannot be automatic without (1)'s pairing endpoint, and
(3)'s reconnection is (1)'s watcher plus a process owner.

## Implementation exit

```sh
cd celestina-rs
cargo fmt --all --check
cargo clippy -p magnetita-core -p magnetita-net -p magnetitad --all-targets --locked -- -D warnings
cargo test -p magnetita-core -p magnetita-net -p magnetitad --locked
qmllint magnetita/qml/components/*.qml
```

Loopback and unit tests do not close this checkpoint. The mirror is a phone,
network, and Wayland feature, so `VAL-MAG-08` against the real S25U is the
exit.

## Open decision for the author

**QR pairing, and one new dependency.** Android's "Pair device with QR code"
lets the host generate the service name and password, render them as a QR, and
pair with no typing at all — the phone then advertises the pairing service
under the name the host chose. This is the only way to reach the author's
"activate Wireless debugging and nothing else" for the very first pairing;
after that, both paths are equally hands-free because the ADB key persists.

It costs one QR-encoder dependency, in a workspace that has admitted very few.
The alternative keeps zero new dependencies and asks the author to type six
digits exactly once per phone. MAG-R1-B ships the six-digit path either way, so
this decision does not block the plan.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-R1-A | `magnetita:` | done | [exact inventory](../../inventories/2026-08-19-wireless-mirror/MAG-R1-A.numstat.tsv) | 27 files, +3414/-53 | The whole one-button mirror in one commit: the pure `mirror` module (validated `MirrorEndpoint`, the `MirrorLink` state machine, typed failure reasons) and the ranking, de-duplicating Avahi discovery that feeds it; pairing on six digits alone; the resident worker that honours a standing intent against whatever endpoint is advertised now, waits for `adb` to call the device ready, owns the scrcpy child by pid, resolves the session's display at spawn time rather than daemon start, and serves `org.celestina.Mirror1`; the mirror's own persisted options (resolution, frame rate, bitrate, audio side, screen-off, stay-awake); and the Mirror control as two icons beside the device — one that starts or stops, one that opens the settings sheet — replacing the row this plan originally sketched under Media | `cargo fmt --all --check`, `cargo clippy -p magnetita-core -p magnetitad --all-targets --locked -- -D warnings`, `cargo test -p magnetita-core -p magnetitad` (123 + 81 pass), `cargo test` for the app (12 pass), `qmllint`; `scripts/complete-production.sh` passed and installed the bundle — recorded in [mirror discovery evidence](../../evidence/2026-08-19-mirror-discovery.md) | `VAL-MAG-08` |

The build order above stayed five conceptual steps because the dependency
between them is real (pairing needs discovery's endpoint; the worker needs
both), but they were found, fixed and verified together in the same files and
landed as one commit, so one ledger unit and one inventory is what actually
happened — separate per-step inventories would each claim a boundary no single
commit produced. The same start action was also added to the shell's own phone
menu (`celestina/qml/PhoneMenu.qml`, `SoftMenuRow.qml`); that is a `celestina:`
change and is not part of this inventory or this checkpoint.

`VAL-MAG-08` — the real phone, unattended, across a Wireless-debugging
toggle — is not yet claimed clean: it was run once against the pre-fix daemon
and surfaced the scrcpy-exit-order defect the hypothesis above describes; the
corrected daemon has mirrored successfully but the toggle has not been
repeated since.

## The probe, and what it changed

Run and recorded in
[mirror discovery evidence](../../evidence/2026-08-19-mirror-discovery.md).
Wireless debugging was off, so the phone could not answer; a stand-in service
published with `avahi-publish` settled the mechanism instead. Two corrections
came out of it and are already in the code:

- One advertisement resolves **once per interface and address family** — five
  times on this host, including a `127.0.0.1` on `lo` that would point `adb` at
  this desktop and an unusable `fe80::` link-local. Candidates are ranked and
  filtered, never taken first-seen.
- Discovery **polls `avahi-browse`** rather than parking on Avahi's D-Bus
  signals: `zbus`'s blocking `SignalIterator::next` has no timeout and so no
  deterministic shutdown, while `subprocess.rs` already owns bounded, reaped
  children. This moves the watcher from `magnetita-net` (which has no `zbus`
  and no subprocess owner) to `magnetitad`, a deliberate departure from the
  scope above.

What the phone itself does on this network is still unverified, and remains
`VAL-MAG-08`'s to settle.
