# QUIC round-trip under a continuous bulk stream — MAG-P0-A, desktop half

- **Date:** 2026-09-07
- **Scope:** `MAG-P0-A` of
  [`../plans/archive/2026-09-07-own-protocol-spikes.md`](../plans/archive/2026-09-07-own-protocol-spikes.md);
  verifies the [transport discussion](../discussions/2026-09-04-transport-quic-or-tcp.md)
- **Environment:** CachyOS, Linux 7.2.3, distro `rust 1.98.1`; scratch crate
  in the session scratch directory (deleted with the session): `quinn 0.11.11`
  with `rustls-ring` and `runtime-tokio`, `rustls 0.23` (`ring`), `rcgen 0.13`,
  `tokio 1.53`; loopback only, one connection, one process
- **Artifact:** not applicable

## Procedure

One `quinn` server and one client in the same process, self-signed
certificate pinned through a root store. Three rounds of 300 requests, 10 ms
apart, each request a fresh bidirectional stream carrying 32 bytes echoed
back; the second round runs while one unidirectional stream on the same
connection writes 64 KiB chunks without pause and is finished only after the
round ends.

```sh
cargo build --release && ./target/release/quic-latency
```

## Result

- **Exit:** 0
- **Observed:**

```
quinn loopback, continuous bulk on one uni stream, 300 pings per round, 10 ms apart
idle                         n=300 p50=  0.071 ms p90=  0.089 ms p99=  0.451 ms max=  2.540 ms
under continuous bulk        n=300 p50=  0.220 ms p90=  0.626 ms p99=  1.829 ms max=  2.609 ms
bulk: 4590 MiB in 3.38 s = 1359 MiB/s, one uni stream
after bulk                   n=300 p50=  0.066 ms p90=  0.088 ms p99=  0.515 ms max=  1.598 ms
```

  The discussion's threshold was a round-trip above 20 ms under load. Under
  a stream saturating loopback at 1.36 GiB/s the p99 is 1.8 ms and the worst
  of 300 is 2.6 ms — the same worst case as the idle round. A small message
  on its own stream is not queued behind the bulk stream; this is the
  head-of-line property the KDE Connect link lacks.

## Limits

- Loopback, not the LAN and not the phone: this proves stream independence
  inside `quinn`, not Wi-Fi behaviour, Doze survival or connection
  migration. The phone half of `MAG-P0-A` is blocked on the host toolchain
  the plan lists (`rustup` with the Android target, NDK, SDK, `cargo-ndk`,
  `uniffi-bindgen`).
- One connection, one process, no packet loss; the datagram path was not
  exercised.
- The scratch program is not kept; the numbers are.

## Follow-up

The phone half of `MAG-P0-A` once the toolchain exists; the transport
discussion cites this record for the desktop half.
