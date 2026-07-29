# Celestina

A personal computing suite for a Niri/Wayland session: a small, truthful shell
plus first-party apps that share one Rust core, one QML visual language and one
set of conventions — lean alternatives to heavyweight external apps, made
possible because the session owns its own shell.

**Current focus:** the Niri shell (`celestina`) has started its first real daily
panel slice after Siderita and Magnetita proved the suite contracts. It now
renders output-local Niri workspaces and the active window from a read-only Rust
event-stream adapter, alongside the shared clock and phone state; focus actions
and session takeover remain deliberately later.

## Projects

| Project | Role | Stack |
|---|---|---|
| [celestina-rs](celestina-rs/) | shared Rust domain cores | Rust |
| [celestina-style](celestina-style/) | shared QML visual language | QML |
| [celestina](celestina/) | Niri shell / session | Rust · C++ · QML |
| [siderita](siderita/) | file manager (first app) | Rust · QML (CXX-Qt) |
| [magnetita](magnetita/) | phone link (KDE Connect) — 1.0.0 | Rust · QML (CXX-Qt) |

Cores and style never depend on apps or the shell. Each project keeps its own
README and ROADMAP; the monorepo holds shared history and the contracts between
projects.

### Planned

Design-stage — not built; each is started only when a recurring daily gap proves
the need, and reuses the shared core and style.

| Project | Role | Stack |
|---|---|---|
| [fluorita](fluorita/) *(working name)* | media player — audio · video · image; later a shell widget | Rust · QML |
| [grafita](grafita/) *(working name)* | text / code editor | Rust · QML |

Each planned directory holds a README and a roadmap and nothing else: the design
and the contracts are written down so the apps they point at can be *consumed* by
name before they exist — Siderita already defers video thumbnails to Fluorita and
its quick-look hand-off to Grafita. Writing the contract is not starting the
project.

**Fluorita** is the suite's media app. It opens and plays whatever media it is
handed — a song, a clip, an image — a *player/viewer*, not a library (Siderita
is the browser). It owns the media decode stack that Siderita deliberately does
not carry, and produces video first-frames and audio covers into the shared
freedesktop thumbnail cache, which Siderita simply consumes. Later it runs as an
embeddable **shell widget** — a playing movie or now-playing music, live in the
panel — and that same widget backs a live-preview quick-look in Siderita. So the
media weight lives in one place, behind a standards-based hand-off, and never
leaks into the file manager.

**Grafita** is the suite's text editor — graphite is what a pencil writes with.
A light editor for text and code, not an IDE; it is where Siderita's read-only
quick-look hands off when you want to *change* a file rather than just peek at it
("Abrir con Grafita"). It shares the core's file and MIME handling and the one
visual language, so editing a file feels like the same session as browsing it.

## Principles

- Rust core, QML frontend, thin bridge.
- One visual language (`celestina-style`); apps art-direct within its tokens.
- Interop via XDG/freedesktop, not private glue.
- Measured lightweight; truthful state (a click is a request, never proof).

## Development contract

[`AGENTS.md`](AGENTS.md) is the canonical repository contract for code placement,
component boundaries, reuse and verification; `CLAUDE.md` is a symlink to the
same source so agent-specific copies cannot drift. Project directories add only
their local deltas in their own `AGENTS.md`; a task started from the repository
root must open the affected project's file explicitly.

Run the same architecture/style gate used by CI before closing a change:

```sh
bash scripts/check-architecture-contract.sh
```

See the [suite roadmap](ROADMAP.md) for the vision, checkpoints and contracts.
