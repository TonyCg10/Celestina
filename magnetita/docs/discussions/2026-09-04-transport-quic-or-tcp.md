# Transport: QUIC, or TCP + TLS with a multiplexer

- **Opened:** 2026-09-04
- **Status:** applied
- **Question:** does the own protocol ride on QUIC (`quinn`), or on TCP + TLS
  with a stream multiplexer written in `magnetita-link`?

## Context

Everything above the transport — messages, pairing, capabilities — is the same
either way, but the transport decides three things the author has felt: a
large payload stalling small messages behind it (one TLS stream, head-of-line
blocking), the session dying whenever the phone changes address (the
reconnect churn recorded against the KDE Connect link), and how cheaply the
mirror's video can share the link with everything else. The choice blocks
`MAG-P2` and the shape of `magnetita-mobile`.

## Strongest case

QUIC gives independent streams, so a file transfer or a video stream cannot
delay a clipboard update; connection migration, so a phone moving between
Wi-Fi and the same LAN's other access point keeps its session; unreliable
datagrams for pointer motion; and 0-RTT resumption for the phone's constant
reconnects. `quinn` is pure Rust on the `rustls`/`ring` stack `magnetita-net`
already pins, so the phone side inherits it through UniFFI with no C
dependency and no second TLS implementation.

## Counter-case

QUIC is UDP: some networks throttle or drop it, `NsdManager` discovery and a
UDP listener need a multicast lock and the Android foreground service to hold
the socket alive, and Doze can still kill it — the phone will reconnect
often, which is exactly what 0-RTT is for but must be measured. `quinn` also
requires an async runtime (`tokio`), which `magnetita-net` deliberately never
adopted; the daemon would host one runtime thread for the link. And QUIC's
congestion control is tuned for the Internet; on a LAN the mirror wants low
latency more than fairness, which may need pacing tuned by hand.

## Alternatives

- TCP + TLS 1.3 with mutual certificates and a small in-house multiplexer
  (length-prefixed frames tagged by stream id). Keeps `magnetita-net`'s
  blocking, runtime-free style; costs writing and testing flow control per
  stream, and gives no migration and no datagrams.
- TCP + TLS with one control connection and one extra connection per bulk
  transfer, KDE Connect's own shape. Simplest; keeps every limit above.
- WebRTC data channels. Solves NAT and migration but drags a large native
  stack into both ends for a LAN-only product.

## Falsifiers and evidence needed

`MAG-P0-A`: `quinn` + `rustls`/`ring` built for `aarch64-linux-android` with
`cargo-ndk`, wrapped by UniFFI, exchanging a hello with `magnetitad` over the
author's LAN; measured round-trip for a small message while a 200 MB stream
is in flight, and a session surviving the phone toggling Wi-Fi off and on.
Round-trip above 20 ms under load, or a session that cannot migrate, overturns
the leading case.

## Conclusion

**QUIC**, concluded by the author on 2026-09-04 on the strength of the
leading case: per-capability streams, migration and datagrams are the three
properties the KDE Connect link measurably lacks, and `quinn` inherits the
`rustls`/`ring` stack both ends already pin. The counter-case stays as the
first spike: `MAG-P0-A` measures round-trip under load and migration across a
Wi-Fi toggle, and the ADR's *Revisit when* names the TCP + multiplexer
fallback if the numbers fail. Applied in
[ADR 0001](../decisions/0001-own-protocol-and-android-app.md) §2 and
`MAG-P2` in the [roadmap](../../ROADMAP.md). Desktop-half verification on
2026-09-07: under a continuous 1.36 GiB/s stream on the same connection the
32-byte round-trip p99 was 1.8 ms, worst 2.6 ms, equal to idle — see
[the loopback record](../evidence/2026-09-07-quic-loopback-latency.md).
Phone-half verification on 2026-09-08, from the S25U over the author's
Wi-Fi with the same Rust core through UniFFI — see
[the phone record](../evidence/2026-09-08-quic-on-the-phone.md): idle, QUIC
and one TCP connection both sit at the link's 6–7 ms; at 1 MiB/s they are
equal at the median; with the link saturated a small message on QUIC stayed
at p50 27 ms / p99 83 ms while on one TCP connection it waited p50 513 ms
and up to 4.2 s behind the bulk — the head-of-line case measured. A
240-second session survived the author switching the phone's Wi-Fi off and
on with 240 of 240 requests answered and no reconnection. The falsifier is
not met: the 20 ms line is held at the mirror's rate and crossed only at
full saturation, where the alternative is half a second. The verdict
stands; `MAG-P2` inherits an initial MTU of 1200 and a foreground service
before long sessions.
