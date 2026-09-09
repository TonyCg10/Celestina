# Magnetita

Celestina's first-party KDE Connect-compatible phone link: a headless Rust
service plus a thin native device/settings application.

## User contract

- Pair with the stock KDE Connect Android application over the local network,
  keep trusted devices available and explain connection failures in the app.
  Under [ADR 0001](docs/decisions/0001-own-protocol-and-android-app.md) this
  wire is being replaced by Magnetita's own QUIC protocol and its own
  Android application (`MAG-P0` through `MAG-P7`); the KDE Connect wire is
  removed in `MAG-P7`, and until then both are served.
- Mount phone storage under the owned runtime path so Siderita browses it as an
  ordinary filesystem; expose identity, connection, battery, media and actions
  through the versioned `org.celestina.Devices1` contract.
- Provide the daily plugin set already implemented: battery, notifications,
  file sharing both ways, find-my-phone, clipboard and MPRIS media integration,
  with persisted per-plugin settings.
- The phone→desktop clipboard path remains manual because Android prevents the
  stock background client from reading ordinary clipboard changes reliably.
- The own protocol carries battery, clipboard, notifications, find, share,
  media, commands, trackpad and keyboard, the screen mirror, SMS, contacts,
  telephony and, last, storage. Magnetita is not a cloud service, a drawing
  tablet, a presenter, or a feature-parity clone of every KDE Connect plugin.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/magnetita-core` | Pure packets, pairing, plugin and MPRIS domain |
| `../celestina-rs/crates/magnetita-net` | UDP discovery, identity-bound TCP/TLS, trust and payload transport |
| `../celestina-rs/crates/magnetitad` | Connections, admission/revocation, mounts, plugins, settings and D-Bus service |
| `src/controller.rs` | Off-GUI D-Bus coordination and confirmed snapshot application |
| `src/devices.rs`, `src/projection.rs` | D-Bus decoding and pure UI projection |
| `qml/Main.qml`, `qml/pages/`, `qml/components/` | Device/settings composition only |
| `magnetitad.service`, desktop entry | User service and application integration |
| `../celestina-style` | Canonical visual tokens, controls and assets |

## Build and use

The service requires the KDE Connect LAN ports and `sshfs`/FUSE for mounted
storage. The phone remains the stock KDE Connect Android app.

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
- [Roadmap history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
