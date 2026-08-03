# Celestina Rust workspace

The shared Rust foundation for Celestina: interface-neutral domain crates plus
the deliberately contained Magnetita transport/daemon and Fluorita render seam.

## User contract

- Applications consume tested Rust contracts instead of duplicating document,
  media, file-operation, shell or phone-link rules in Qt/QML adapters.
- Pure cores do not depend on Qt, QML, Niri or application modules.
- Network transport, multimedia decode and hand-written Qt render integration
  remain isolated behind narrow contracts.
- This workspace is not a user interface and does not own application layout or
  interaction wording.

## Architecture

| Area | Responsibility |
|---|---|
| `crates/celestina-core` | Generations, cancellation, durable state replacement, percent encoding and XDG helpers |
| `crates/siderita-*` | Read models, loss-free file operations and the opaque Qt-facing view contract |
| `crates/grafita-core` | Text classification, document/edit history, safe save, workers and host-neutral session state |
| `crates/fluorita-core` | Media identity, catalogue projections, artwork requests and confirmed playback state |
| `crates/fluorita-engine` | Bounded scanning, metadata, derived artwork, trailers and libmpv sessions |
| `crates/fluorita-qt` | Shared C++ `QQuickFramebufferObject` render seam; no domain behaviour |
| `crates/magnetita-core` | KDE Connect wire domain and typed plugin contracts |
| `crates/magnetita-net` | Discovery, TCP/TLS, trust and bounded payload transport |
| `crates/magnetitad` | Headless phone-link service and `org.celestina.Devices1` producer |
| `crates/celestina-shell-core` | Bounded helper framing, provider envelope and shell command vocabulary |
| `crates/dotfiles-core` | Conflict-aware planning only; it does not apply system changes |

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
