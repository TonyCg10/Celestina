# Mirror without discovery — MAG-R2

- **Date:** 2026-08-19
- **Scope:** `MAG-R2-A`, the whole of
  [`../plans/archive/2026-08-19-mirror-without-discovery.md`](../plans/archive/2026-08-19-mirror-without-discovery.md)
- **Environment:** CachyOS; `adb` (`android-tools` 37.0.0, no mDNS support);
  `avahi-browse`; `scrcpy` 4.1; a real Samsung Galaxy S25 Ultra (SM-S938U,
  Android 16, unrooted) over Wi-Fi
- **Artifact:** magnetita 1.2.0

## Procedure

### What was in question

`MAG-R1` mirrors in one press, but only while Android advertises. The question
the author asked was whether that dependency can be removed — whether anything
survives Android turning wireless debugging off.

### The investigation, on the phone itself

`adb tcpip` was suspected to open a listener independent of wireless debugging.
Rather than assume, each step was measured:

```
$ adb shell settings get global adb_wifi_enabled     → 1
$ adb tcpip 5555                                     → restarting in TCP mode port: 5555
$ adb connect 10.0.0.190:5555                        → connected   (no mDNS, no pairing)
$ adb shell settings put global adb_wifi_enabled 0   → wireless debugging OFF
$ avahi-browse -rpt _adb-tls-connect._tcp            → 0 records
$ /dev/tcp/10.0.0.190/5555                           → still OPEN
$ adb devices                                        → 10.0.0.190:5555  device
```

So the two listeners are genuinely separable, and the fixed one survives. The
setting also proved writable from `adb shell`, so a connected phone can be told
to re-enable its own wireless debugging — which re-randomises the port, as the
replacement advertisements on `33721` and later `42293` showed.

### What cannot be done

`setprop persist.adb.tcp.port 5555` returned `Failed to set property`, and the
phone has no `su`. Surviving a reboot of the *phone* is root-only, and the
checkpoint excludes it rather than pretending otherwise.

### The implementation

`magnetita-core` gains a `FIXED_PORT`, a remembered endpoint that only ever
holds that port, and a target rule preferring a live advertisement over it.
`magnetitad` pins the device after a discovered endpoint comes up — `adb tcpip`
restarts `adbd`, so this is a disconnect and a fresh dial, not a live
reconfiguration — and persists the result.

## Result

Deployed, then driven from a cold start with the remembered file deleted:

```
[mirror] _adb-tls-connect._tcp at 10.0.0.190:33721
[mirror] pinned 10.0.0.190:5555 — discovery is no longer needed
[mirror] connected to 10.0.0.190:5555
[mirror] scrcpy 949404 mirroring 10.0.0.190:5555
```

`~/.config/magnetita/mirror-endpoint` then held `10.0.0.190:5555`. With wireless
debugging turned off and `magnetitad` restarted — losing everything held in
memory — the mirror reported `available` rather than `idle`, from the file
alone.

### The defect this exposed

Pressing Mirror in that state **failed**, with `connect-failed`, while port 5555
was answering. Avahi was still serving the advertisement from its **cache**
about a minute after Android had stopped publishing it, and the daemon dialled
that dead port and treated the failure as final.

Preferring a live advertisement is right; treating its failure as the end was
not. A failed connection now falls back to the remembered fixed port, once, so
a dead pair cannot loop. `a_stale_advertisement_falls_back_to_the_remembered_port`
fails against the old rule and passes against the new one.

Automated: 132 core tests, 84 daemon tests, `clippy -D warnings`,
`cargo fmt --all --check`.

## Limits

The corrected build is **not deployed**: `magnetita/scripts/verify-production.sh`
runs a workspace-wide `cargo fmt --all --check`, and unrelated in-flight work in
`grafita-core` was failing it at the time. The deployed daemon therefore carries
the pin and the memory but not the stale-advertisement fallback.

`VAL-MAG-09` — the author's own pass across a real phone reboot, which is the
one case no test reproduces — is not claimed.
