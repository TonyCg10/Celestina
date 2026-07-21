# Celestina

A personal computing suite for a Niri/Wayland session: a small, truthful shell
plus first-party apps that share one Rust core, one QML visual language and one
set of conventions — lean alternatives to heavyweight external apps, made
possible because the session owns its own shell.

**Current focus:** a minimal Niri shell (`celestina-desktop`) and the file
manager (`siderita-adma`).

## Projects

| Project | Role | Stack |
|---|---|---|
| [celestina-rs](celestina-rs/) | shared Rust domain cores | Rust |
| [celestina-style](celestina-style/) | shared QML visual language | QML |
| [celestina-desktop](celestina-desktop/) | Niri shell / session | C++ · QML |
| [siderita-adma](siderita-adma/) | file manager (first app) | Rust · QML (CXX-Qt) |

Cores and style never depend on apps or the shell. Each project keeps its own
README and ROADMAP; the monorepo holds shared history and the contracts between
projects.

## Principles

- Rust core, QML frontend, thin bridge.
- One visual language (`celestina-style`); apps art-direct within its tokens.
- Interop via XDG/freedesktop, not private glue.
- Measured lightweight; truthful state (a click is a request, never proof).

See the [suite roadmap](ROADMAP.md) for the vision, checkpoints and contracts.
