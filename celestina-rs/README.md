# Celestina Rust workspace

The shared Rust foundation for Celestina: interface-neutral domain crates plus
the deliberately contained Magnetita transport/daemon and Fluorita render seam.

## User contract

- Applications consume tested Rust contracts instead of duplicating document,
  media, file-operation, shell or phone-link rules in Qt/QML adapters.
- Pure cores do not depend on Qt, QML, Niri or application modules.
- Network transport, the phone-side protocol stack, multimedia decode and
  hand-written Qt render integration remain isolated behind narrow contracts.
- This workspace is not a user interface and does not own application layout or
  interaction wording.

## Architecture

| Area | Responsibility |
|---|---|
| `crates/celestina-core` | Generations and cancellation; small-file IO (state replacement, private state, media landing, bounded reads); XDG base, runtime and private directories; percent encoding, path keys and `file://` URIs; `.desktop` parsing and scanning; cover-art plausibility |
| `crates/celestina-shell-core` | The shell helpers' domain: line framing, provider envelope, typed host commands and the provider rules (connectivity, audio, brightness, notifications, launcher, settings, workspaces and more); its `journal` module is the one part that writes files |
| `crates/siderita-*` | Read models, loss-free file operations, archives, the pictures files carry inside themselves, and the opaque Qt-facing view contract |
| `crates/grafita-core` | Text classification, document/edit history, safe save, workers and host-neutral session state |
| `crates/hematita-core` | `/proc` and `/sys` readings as typed values, and the storage analyzer's walk, check and removal |
| `crates/fluorita-core` | Media identity, catalogue projections, artwork requests and confirmed playback state |
| `crates/fluorita-engine` | Bounded scanning, metadata, derived artwork, trailers and libmpv sessions |
| `crates/fluorita-qt` | Shared C++ `QQuickFramebufferObject` render seam; no domain behaviour |
| `crates/magnetita-core` | KDE Connect wire domain and typed plugin contracts |
| `crates/magnetita-net` | Discovery, TCP/TLS, trust and bounded payload transport for the KDE Connect wire |
| `crates/magnetita-proto` | Magnetita's own protocol (ADR 0001): pure, offline, shared by the daemon and the phone |
| `crates/magnetita-link` | That protocol on the wire: QUIC with pinned mutual TLS on tokio |
| `crates/magnetita-mobile` | The phone side of the protocol, exported to Kotlin through UniFFI |
| `crates/magnetita-peer` | A phone with no screen: the protocol's headless peer, driven from a command line |
| `crates/magnetitad` | Headless phone-link service and `org.celestina.Devices1` producer |
| `crates/dotfiles-core` | Conflict-aware planning only; it does not apply system changes and has no consumer yet |

`celestina-core` also holds owners that no application calls yet: the strict
`file_uri` parser, `xdg::runtime_dir` and `xdg::ensure_private_dir`,
`atomic_file::{replace_private, land_media, stage_media, read_bounded}`,
`desktop_entry::{read, scan, find}` and
`CancellationToken::is_cancel_requested`. Each module's documentation names the
product unit that moves that product's own copy onto it.

## Build and use

The workspace pins Rust 1.97.1 and declares an MSRV floor of 1.85. Building the
complete production workspace also requires the system libmpv development
surface used by `fluorita-engine`.

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
```

These commands create and verify the canonical release artifacts without
installing or activating an application. Status reports whether the verification
seal still matches the current inputs. This workspace is not deployable;
applications deploy through their own registered workflow.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation routing](VALIDATION.md)
- [Local agent delta](AGENTS.md)
- [Roadmap history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
