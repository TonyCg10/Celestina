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

### Planned

Design-stage — not built; each is started only when a recurring daily gap proves
the need, and reuses the shared core and style.

| Project | Role | Stack |
|---|---|---|
| Fluorita *(working name)* | media player — audio · video · image; later a shell widget | Rust · QML |
| Grafita *(working name)* | text / code editor | Rust · QML |
| [magnetita](magnetita/) | phone link | Rust · QML |

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

See the [suite roadmap](ROADMAP.md) for the vision, checkpoints and contracts.
