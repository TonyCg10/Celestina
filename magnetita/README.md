# Magnetita

Celestina's phone link: a headless Rust service speaking the suite's own
QUIC protocol to its own Android application, plus a thin native
device/settings application.

## User contract

- Pair the own Android application by QR over the local network, keep
  trusted phones available and explain connection failures in the app.
  [ADR 0001](docs/decisions/0001-own-protocol-and-android-app.md) chose
  this wire over the KDE Connect one, which `MAG-P7` removed.
- Present the phone's shared folder under the owned runtime path so Siderita
  browses it as an ordinary filesystem; expose identity, connection, battery,
  media, the call and actions through the versioned `org.celestina.Devices1`
  contract.
- Carry battery, clipboard, notifications, find, share, media, commands,
  trackpad and keyboard, the screen mirror, SMS, contacts, telephony and
  storage, with persisted per-plugin settings. Magnetita is not a cloud
  service, a drawing tablet, a presenter, or a client of any other protocol.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/magnetita-proto` | The wire: envelope, hello, pairing and the capability messages |
| `../celestina-rs/crates/magnetita-link` | The QUIC link with pinned certificates and mDNS discovery |
| `../celestina-rs/crates/magnetita-mobile`, `magnetita-peer` | The phone's side of the link (UniFFI for Android) and its shell stand-in |
| `../celestina-rs/crates/magnetita-core`, `magnetita-net` | What both ends share: clipboard rule, notification and media shapes, the mirror state machine, the certificate, the trust store, the transfer limiter |
| `../celestina-rs/crates/magnetitad` | Sessions, revocation, the phone's FUSE mount, capabilities, settings and the D-Bus service |
| `src/controller.rs` | Off-GUI D-Bus coordination and confirmed snapshot application |
| `src/devices.rs`, `src/projection.rs` | D-Bus decoding and pure UI projection |
| `qml/Main.qml`, `qml/pages/`, `qml/components/` | Device/settings composition only |
| `magnetitad.service`, desktop entry | User service and application integration |
| `../celestina-style` | Canonical visual tokens, controls and assets |

## Build and use

The service binds the own wire's UDP port (1760) and mounts the phone's
shared folder through FUSE (`fusermount3`). The phone runs the suite's own
application, `../magnetita-android`.

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

Build produces both the application and `magnetitad` once. Verify checks those
exact artifacts without replacing the installed binary, touching trust state or
restarting the service. Status reports whether the seal still matches the
current inputs. Deploy consumes the verified manifest without recompiling and
owns the single stop→copy→start sequence. `scripts/run.sh` remains an
application-only human convenience, not the canonical workflow. A change to
`magnetita-core` also completes the Celestina shell because that bundle consumes
the shared phone projection; shell completion updates disk but does not replace
the live session.

After completion, open `magnetita` for pairing, diagnostics and
settings; the daemon continues to provide devices while the window is closed.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
- [The Magnetita wire](docs/protocol.md), the own protocol as `magnetita-proto` implements it
- [Roadmap history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
