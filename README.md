# Celestina

Celestina is a personal native desktop suite for a Niri/Wayland session: a
small shell and focused first-party applications that share Rust domain
contracts, one Qt/QML visual language and explicit desktop integration.

The suite optimizes for truthful state, loss-free local work, bounded background
activity and coherent keyboard-accessible surfaces. It is not a general desktop
environment or a framework for unrelated applications. The durable product
direction is [docs/VISION.md](docs/VISION.md); current work is
[STATUS.md](STATUS.md).

## Projects

| Project | Current role | Primary stack |
|---|---|---|
| [celestina-rs](celestina-rs/) | Pure shared domain, protocol, IO and engine crates | Rust |
| [celestina-style](celestina-style/) | Shared semantic tokens, assets and QML controls | Qt Quick/QML |
| [celestina](celestina/) | Niri shell, panel, overlays and session command service | Rust · C++20 · Qt/QML |
| [siderita](siderita/) | File manager and desktop file chooser | Rust · CXX-Qt · QML |
| [magnetita](magnetita/) | KDE Connect phone link, daemon and client | Rust · CXX-Qt · QML |
| [grafita](grafita/) | Text editor, standalone and embedded in Siderita | Rust · CXX-Qt · QML |
| [fluorita](fluorita/) | Local media library/player, standalone and embedded in Siderita | Rust · C++ · CXX-Qt · QML |

Each project owns a concise README, current STATUS, implementation-only ROADMAP,
author VALIDATION queue and local AGENTS delta. The machine-readable inventory
of paths, commit scopes, product version sources and production artifact
commands is
[docs/projects.toml](docs/projects.toml).

## Architecture

Dependencies point toward pure contracts:

```text
QML presentation
      |
Qt/CXX-Qt application state and adapters
      |
celestina-rs domain, protocol and testable IO

celestina-style ---> every visual host
```

Applications reuse domain contracts and narrow native seams; they do not import
one another's UI. Cross-process integration is backward-compatible D-Bus or an
XDG/freedesktop contract. The binding rules are documented in
[the architecture standard](docs/standards/architecture.md) and the current
content gesture mapping in
[the activation contract](docs/contracts/content-activation.md).

## Build and verification

Do not use `run.sh` as an ambiguous proof step. Every registered project exposes
separate production entries:

```sh
PROJECT/scripts/build-production.sh
PROJECT/scripts/verify-production.sh
PROJECT/scripts/complete-production.sh  # deployable applications only
PROJECT/scripts/status-production.sh
```

The first command creates the canonical release artifact; the second verifies
that exact artifact without installing or activating it; the last reports
whether it remains current. For a bug fix or milestone in a deployable app,
`complete-production.sh` is the required exit: it builds once, verifies those
exact bytes, deploys them to the author's normal test destination and confirms
the installed copy, so the author never recompiles the change. Running
`deploy-production.sh` as a separate operation or choosing another prefix still
requires an explicit request. The shell additionally has
`activate-production.sh`, kept separate because starting it mutates the live
session; completion updates its on-disk bundle but never activates it.

Before that build, a product bug, completed milestone or major release advances
PATCH, MINOR or MAJOR exactly and appends the same delivery to the version
history. Maintenance changes do not bump. See
[the version contract](docs/contracts/versioning.md).

The full contract, including artifact fingerprints and stale-input refusal, is
[docs/contracts/production-artifacts.md](docs/contracts/production-artifacts.md).
Run the repository architecture gate for every change:

```sh
bash scripts/check-architecture-contract.sh
```

## Documentation and agent entry points

- [AGENTS.md](AGENTS.md) is the vendor-neutral mandatory agent contract.
- [CONTRIBUTING.md](CONTRIBUTING.md) defines the human and agent workflow.
- [docs/README.md](docs/README.md) maps each kind of truth to one canonical
  document.
- [ROADMAP.md](ROADMAP.md) contains suite-level implementation only.
- [VALIDATION.md](VALIDATION.md) contains author-only checks and never blocks an
  implementation milestone.

Historical roadmaps and evidence are preserved under `docs/history/` and
project-local `docs/history/`; they are context, not current instructions.
