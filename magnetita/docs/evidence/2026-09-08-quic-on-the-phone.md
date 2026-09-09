# QUIC and single-stream TCP from the S25U over the author's Wi-Fi — MAG-P0-A, phone half

- **Date:** 2026-09-08
- **Scope:** `MAG-P0-A` of
  [`../plans/active/2026-09-07-own-protocol-spikes.md`](../plans/active/2026-09-07-own-protocol-spikes.md);
  verifies the [transport discussion](../discussions/2026-09-04-transport-quic-or-tcp.md)
  and the desktop half in
  [the loopback record](2026-09-07-quic-loopback-latency.md)
- **Environment:** Samsung Galaxy S25 Ultra (Android 16) on the author's
  5 GHz 11ax Wi-Fi at -49 dBm, 1080 Mbps link; desktop on the same access
  point over `wlan0` at -41 dBm; the phone was also streaming a YouTube
  video for part of the session. Toolchain installed by the author on
  2026-09-08: `rustup 1.29.1` with `aarch64-linux-android`, `cargo-ndk
  4.1.2`, Android Studio 2026.1.3 (Flatpak) with NDK 30.0.16248370, AGP
  9.3.2, Kotlin 2.2.10, Gradle 9.5, Compose BOM 2026.02. Scratch crate
  `magspike` (`quinn 0.11.11` + `rustls`/`ring`, `uniffi 0.29`) as a
  `cdylib` inside a throwaway Compose activity; scratch desktop server
  `quic-latency --serve` on UDP 1760 and TCP 1761, ports the author's
  `ufw` already admits inside the KDE Connect range. No firewall, phone
  setting or repository crate was changed
- **Artifact:** not applicable (`app-debug.apk` of the author's Android
  Studio project, 19 MB, `libmagspike.so` 3.9 MB)

## Procedure

The phone opens one connection to the desktop and runs 100 requests of 32
bytes, 10 ms apart, each on its own QUIC bidirectional stream; then the same
100 while a bulk writer on the same connection pushes 64 KiB chunks —
either unpaced or paced to 1 MiB/s, the regime of a 1080p HEVC mirror. The
TCP twin does the same on **one** TCP connection with `TCP_NODELAY`, frames
`[kind][len][payload]`, so a request waits behind whatever chunk is already
in the socket — the KDE Connect shape. Each run is launched by intent and
read back from `logcat`; the phone is kept awake by `KEYCODE_WAKEUP`.

```sh
cargo ndk -t arm64-v8a -o app/src/main/jniLibs build --release -p magspike
./gradlew assembleDebug && adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n com.example.magnetita/.MainActivity --es addr 10.0.0.134:1760 --ei rounds 100 --ei pace 0
adb shell am start -n com.example.magnetita/.MainActivity --es addr 10.0.0.134:1761 --ei rounds 100 --ei pace 0 --ez tcp true
adb logcat -d -s magspike
```

## Result

- **Exit:** 0 for every run below; the build for `aarch64-linux-android`
  succeeded on the first attempt with no C dependency beyond the NDK
- **Observed** (phone awake, runs alternated back to back):

| Run | Idle p50 / p99 | Under load p50 / p99 / max | Bulk |
|---|---|---|---|
| QUIC, unpaced | 6.8 / 12.3 ms | 27.3 / 82.6 / 91.9 ms | 13.5 MiB/s |
| One TCP, unpaced | 6.4 / 10.0 ms | **512.5 / 3458.9 / 4225.6 ms** | 16.7 MiB/s |
| QUIC, 1 MiB/s | 6.5 / 26.2 ms | 6.8 / 163.7 / 168.3 ms | 1.0 MiB/s |
| One TCP, 1 MiB/s | 6.7 / 12.4 ms | 7.0 / 67.4 / 89.7 ms | 1.0 MiB/s |

  - Idle, both transports sit at the Wi-Fi's own round-trip (6–7 ms; ICMP
    from the desktop to the phone averaged 55 ms with the phone's radio in
    power save, 6 ms awake).
  - At the mirror's rate (1 MiB/s) the two are equal at the median; the
    p99 tails of 45–165 ms appeared in both transports across six paced
    runs and are Wi-Fi jitter, not transport.
  - Saturating the link is where they part: on one TCP connection a small
    message waited half a second at the median and up to 4.2 s, because
    the kernel's send buffer holds seconds of chunks ahead of it; on QUIC
    the same message stayed under 100 ms with its own stream. That is the
    head-of-line property the discussion named, measured.
  - The discussion's 20 ms threshold under load is met at 1 MiB/s (7 ms)
    and exceeded at full saturation (27 ms), where the alternative is 500
    ms; the threshold was written assuming an unsaturated link.
  - With the phone's screen off (`mWakefulness=Dozing`) longer runs ended
    in `connection lost`, `Broken pipe` or a reset from the phone for both
    transports: Android cuts a background app's sockets, which is why
    `MAG-P3` puts the session in a foreground service. Two early QUIC runs
    in that state also showed 199 lost packets, 21 black holes and an MTU
    pinned at 1200; the awake runs lost 34–99 packets of 2 700–47 000 and
    reached a 1.2 MB congestion window. `MAG-P2` should set the initial
    MTU to 1200 and treat path-MTU probing as optional.

## Limits

- One phone, one access point, one evening, with a video stream on the
  phone for part of it. The numbers rank the transports; they do not
  characterise the author's Wi-Fi.
- Certificate verification was disabled on the phone side (`TrustAnything`);
  pairing is `MAG-P1`'s job.
- The Wi-Fi toggle returned the same address, so address migration itself
  was not observed, only recovery through a ten-second outage.
- The scratch code is not kept; the throwaway activity and native library
  in the author's Android Studio project are deleted with the spike.

### Migration across a Wi-Fi toggle

One QUIC connection held for 240 s with one request per second (30 s
timeout each) while the author switched the phone's Wi-Fi off, waited about
ten seconds and switched it back on, the phone staying reachable over USB.

```
hold   ok=240 failed=0 rtt: 35.4ms, cwnd: 2904, congestion_events: 1, lost_packets: 15, sent_packets: 31907
```

Every request was answered; none timed out and the connection was never
re-established. The 15 lost packets are the requests and acknowledgements
sent into the dead radio, retransmitted by QUIC when it came back. The
phone returned with the same address (`10.0.0.16`), so this exercised loss
recovery on one path rather than a path change; a DHCP lease change would
add the address migration that `quinn` also supports, untested here.

## Follow-up

None for `MAG-P0-A`: the transport discussion cites this record and the
desktop half together. `MAG-P2` inherits two notes — initial MTU 1200 with
optional probing, and a foreground service before any long session
(`MAG-P3`).
