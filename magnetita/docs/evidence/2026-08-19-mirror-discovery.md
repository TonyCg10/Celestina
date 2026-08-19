# Wireless mirror discovery — MAG-R1-A

- **Date:** 2026-08-19
- **Scope:** `MAG-R1`, the whole one-button wireless mirror, of
  [`../plans/archive/2026-08-19-wireless-mirror.md`](../plans/archive/2026-08-19-wireless-mirror.md)
- **Environment:** CachyOS; `adb` (`android-tools` 37.0.0, no mDNS support);
  `avahi-browse`; `scrcpy` 4.1; a real Samsung Galaxy S25 Ultra (SM-S938U,
  Android 16) over Wi-Fi
- **Artifact:** magnetita 1.1.0

## Procedure

### What was in question

The plan rested on one unverified claim: that this host can see the ADB
wireless-debugging advertisements at all. `adb` here is
`android-tools 37.0.0`, built without mDNS —

```
$ adb mdns check
adb: mdns is not supported by this version of adb
```

— which is the entire reason the author's `~/Scripts/cpy.sh` hardcodes an IP,
pings it, caches a port and sweeps ports 30000–50000. If Avahi could not
substitute for that, the plan was wrong and not merely unfinished.

### Probe

With Wireless debugging off, both browses returned nothing, so the phone could
not answer the question. A stand-in service was published locally instead:

```sh
avahi-publish -s "adb-FAKE123-test" _adb-tls-connect._tcp 37059 &
avahi-browse -rpt _adb-tls-connect._tcp
```

Avahi resolved it, which settles the mechanism: `avahi-daemon` is active, it
serves this service type, and `avahi-browse -rpt` terminates on its own with
parsable output. The publisher was stopped by the pid this shell started.

### What the probe changed

The output was not the single line the plan assumed. One advertisement resolved
**five times** — once per interface and address family:

```
=;wlan0;IPv6;…;2601:403:c487:dad0::2fc1;37059;
=;wlan0;IPv4;…;10.0.0.134;37059;
=;enp9s0;IPv6;…;fe80::12ff:e0ff:feb7:9294;37059;
=;enp9s0;IPv4;…;10.50.0.1;37059;
=;lo;IPv4;…;127.0.0.1;37059;
```

Two of those are traps. The `lo` resolution would point `adb connect` at this
desktop, and the `fe80::` one carries no scope and is unusable as written.
Taking the first line, or any single line, was not safe. So `parse_browse`
ranks and de-duplicates candidates and refuses loopback, link-local and
unspecified addresses outright, preferring IPv4. The captured output is the
fixture the parser is tested against, so this finding cannot regress silently.

### Second decision the probe forced

Avahi's D-Bus browse API is signal-driven, and `zbus`'s blocking
`SignalIterator::next` has no timeout, so a watcher thread parked on it has no
deterministic shutdown — the defect `MAG-M1` exists to remove elsewhere in this
daemon. `avahi-browse -rpt` terminates by itself and goes through
`subprocess.rs`, which already owns the deadline, the cancellation flag and the
process-group reaping established by `MAG-S1-D`. Discovery therefore polls a
terminating command rather than parking on a signal.

### What the rest of the checkpoint added

`MAG-R1-B` and `MAG-R1-C` are one resident worker in `magnetitad/src/mirror.rs`.
Two of its decisions came from reading the author's script rather than from the
plan:

- `adb connect` answering `connected` only means a socket opened. The script
  needed six one-second retries before the device left `offline`, so the worker
  does not start scrcpy until `adb devices` calls it a `device`.
- The script's `pkill -9 scrcpy` would kill an unrelated scrcpy the author had
  open. The worker keeps the `Child` it spawned and terminates that process
  group only. It is also the one long-lived child here, so it is polled with
  `try_wait` rather than waited on, which is what lets closing the window be
  noticed without blocking the loop.

`MAG-R1-D` is the app side. The mirror's state moves on the phone's schedule
rather than on a bus event, so the daemon publishes no change signal for it and
the card polls `ReloadMirror` at 2 s while it is on screen — noted here because
it is a departure from how every other surface in this app updates.

### What the production contracts caught

The card was written with a plain Qt `TextField` and a `CelestinaButton.Normal`
role. Neither survived `verify-production.sh`, and both were real:

- The architecture contract refused the raw `TextField` — a Qt control rebuilt
  outside the baseline. `celestina-style` already owns `CelestinaTextField`,
  which is now symlinked into `qml/` and registered like the other shared
  controls, so the code field carries the suite's focus ring and fill rather
  than a private imitation.
- `CelestinaButton` has no `Normal` role; the roles are `Tonal`, `Primary`,
  `Destructive`, `Selected` and `Ghost`. `qmllint` caught it as a missing
  property, and the qmllint ratchet refused the build for growing Magnetita's
  warning count from 27 to 28. The running state is now `Tonal`, which is also
  the right reading: prominent while it is the thing to press, quiet once the
  mirror is up and the button only offers to stop.

`verify-production.sh` then passed whole, including its headless smoke: the
client constructed and lived 8 s under an isolated XDG/D-Bus with no QML errors,
so the Mirror card genuinely instantiates rather than merely compiling.

### Automated evidence

```
cargo fmt --all --check
cargo clippy -p magnetita-core -p magnetita-net -p magnetitad --all-targets --locked -- -D warnings
cargo test -p magnetita-core -p magnetita-net -p magnetitad --locked
cd magnetita && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked
qmllint qml/components/*.qml qml/pages/*.qml qml/Main.qml
```

All clean. 112 tests in `magnetita-core` (14 new, in `mirror`), 79 in
`magnetitad` (12 new, across `mirror_discovery` and `mirror`), 45 unchanged in
`magnetita-net`, and 12 in the app (4 new, in `projection`).

The state-machine tests cover what the author asked the button to do: one press
reaches a mirror; toggling Wireless debugging off and on reconnects on the new
random port with no input; closing the scrcpy window is a decision that is not
undone by a re-advertisement; a stop never targets a scrcpy this daemon did not
start; and a pairing code is refused unless a pairing screen is actually open.

## Result

### The real phone, end to end

Wireless debugging was enabled and the whole sequence the daemon performs was
run by hand against the S25U, in the same order and with the same arguments.

The phone advertised `adb-RFCY60WBFAH-Cjvgoe` at `10.0.0.190:39799`
(`name=SM-S938U`, `api=36.1`), which settles the last open question: the
advertisement does cross this network, and Avahi sees it. Two things about that
address are worth recording, because both are exactly what the author's script
could not survive:

- The IP is **not** the `10.0.0.85` the script hardcodes. The phone's lease had
  moved, so the script's very first `ping` would have failed and it would have
  reported the phone unreachable while it was sitting on the LAN advertising.
- The port was **39799**, chosen at random by Android. A sweep of 30000–50000
  would eventually have reached it, one TCP connection at a time.

`adb connect` then failed while the port accepted TCP and the phone answered
`ping` — the network was fine and the trust was not. That is the case the
pairing unit exists for, so it was exercised for real:

```
$ avahi-browse -rpt _adb-tls-pairing._tcp     # → 10.0.0.190:41059
$ adb pair 10.0.0.190:41059 637847
Successfully paired to 10.0.0.190:41059 [guid=adb-RFCY60WBFAH-Cjvgoe]
$ adb connect 10.0.0.190:39799
connected to 10.0.0.190:39799
$ adb devices -l
10.0.0.190:39799  device  model:SM_S938U  transport_id:2
```

The author read six digits off the phone and nothing else. The pairing port —
`41059`, different again from the connect port — was discovered, never typed.

scrcpy was then started with the daemon's exact argument vector and came up:

```
[server] INFO: Device: [samsung] samsung SM-S938U (Android 16)
```

So every step `MirrorLink` sequences — discover, pair, connect, wait for
`device`, start the mirror — is confirmed against the real phone.

### The deployed daemon, and the defect the toggle found

`magnetita/scripts/complete-production.sh` ran and `org.celestina.Mirror1`
appeared on the session bus reporting `State = "available"` unprompted — the
resident worker had already discovered the phone with no one asking. Calling
`Start` over the bus, which is exactly what the control does, took it to
`mirroring` in about nine seconds with a scrcpy owned by `magnetitad`.

Then Wireless debugging was toggled, and the behaviour the whole feature exists
for did **not** happen. The port moved from `39799` to `45461`, `adb` followed
it and reported `device` — and the mirror stayed dark, with `State` back at
`available`.

The cause is an ordering the unit tests had assumed the other way round.
Toggling the switch kills the adb link immediately, so **scrcpy exits several
seconds before the mDNS record lapses**. The link therefore saw
`MirrorExited` before `ServiceLost`, and read that first exit as the author
closing the window — which deliberately clears the standing intent so an
automatic reconnection cannot reopen a window someone just closed. The intent
was gone by the time the new advertisement arrived.

The discriminator is the exit status, not the ordering. A window the author
closes exits cleanly; a scrcpy whose device vanished does not. Only a clean exit
now clears the intent. `scrcpy_dying_before_the_advertisement_lapses_still_reconnects`
fails against the old rule and passes against the new one.

The same session exposed a second gap: the mirror worker logged nothing at all,
so the daemon's journal said nothing about a feature that had just misbehaved.
It now records discovery appearing and lapsing, pairing, connecting, and how
scrcpy ended.

## Limits

The reconnection fix is built and verified but **not deployed**: the installed
daemon still carries the defective rule, and re-running
`complete-production.sh` needs the author. Until it does, the toggle test has
not been re-run and the corrected behaviour has been observed only in tests.

The Mirror control itself has still never been pressed in the app — `Start` was
called over the bus, which is the same path, but not through the UI.

`celestina/scripts/complete-production.sh` was refused because the author's
shell is live, and rewriting those binaries restarts the provider adapter, whose
DDC probe has twice ended in `amdgpu: device lost from bus!`. It is not
load-bearing here: the `magnetita-core` change is a new module the shell never
calls, and the phone projection it does consume is untouched.

`MAG-R1-E` has not run. `build-production.sh` and `verify-production.sh` both
pass and leave the service untouched, but `complete-production.sh` — the one
step that stops, replaces and restarts the live `magnetitad` — was refused by
the session's guard on actions against a running service. The installed daemon
therefore still has no mirror, and the author has to authorise that step.
