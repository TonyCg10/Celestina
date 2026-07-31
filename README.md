# Celestina

A personal computing suite for a Niri/Wayland session: a small, truthful shell
plus first-party apps that share one Rust core, one QML visual language and one
set of conventions — lean alternatives to heavyweight external apps, made
possible because the session owns its own shell.

**Current focus:** the Niri shell continues its first real daily panel slice.
Grafita is now a working app on both its surfaces, and Fluorita has its core, its
libmpv engine and a guarded scaffold. Siderita and Magnetita already proved the
suite contracts both new apps reuse.

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

### Authorized / ready to implement

Both were ratified on 2026-07-30 and both build gates are open. Grafita is no
longer only planned: its shared document core, its embedded Siderita modal and
its standalone application all exist, verified headlessly. Fluorita's F1 media
and library contract is done and tested, its decode-backend spike measured on
this machine, its engine built over the chosen backend, and its application is a
guarded scaffold that does not play yet.

| Project | Role | Stack |
|---|---|---|
| [fluorita](fluorita/) | local media library + player — Gallery · Music | Rust · QML |
| [grafita](grafita/) | general text editor | Rust · QML |

Fluorita has finished F1: `fluorita-core` classifies media, projects the
Gallery/Music library and freezes the thumbnail key Siderita already reads. Its F2 spike measured closure, decode cost, derived
resources and real-session presentation for every installed candidate, and the
author chose libmpv on that evidence. `fluorita-engine` now probes metadata,
publishes video posters and embedded covers into the shared thumbnail cache and
runs truthful playback sessions over that backend, verified against real libmpv.
Its application plays: a Qt Quick surface libmpv renders into, a session owned
off the GUI thread and a transport that only moves when the engine confirms,
verified with real video and audio in the author's Wayland session.

Grafita has finished G1, G2 and G3: `grafita-core` opens, edits and safely saves
real files; `Space` in Siderita opens its editing modal; and the standalone
application opens a document named on the command line, guards its own quit, and
installs with a desktop entry and icon. Both surfaces have been driven
headlessly — including proof that editing a CRLF file through Qt's own text
widget leaves its line endings alone — but neither has been seen in a real
session. Siderita's `Space` activation has explicit shared contracts for both
apps: Grafita edits text and Fluorita views/plays local media.

**Fluorita** is the suite's local media library and player. Its full app has a
**Gallery** for images/video and **Music** for albums, artists and tracks. Its
shared core/engine produces image thumbnails, video posters, audio covers and
bounded on-demand video trailers. `Space` on media opens a minimal Fluorita
player inside Siderita; double-click or Enter starts that item in the complete
app. Static artwork remains freedesktop-compatible, while the decode engine is
loaded lazily only for explicit playback or preview.

**Grafita** is the suite's general text editor — graphite is what a pencil
writes with. A light editor, not an IDE, it accepts textual content by bytes
rather than by extension or a closed MIME list. `Space` on text in Siderita
opens a simple, nearly full-window Grafita editing modal; double-click or Enter
opens the complete standalone app. Both surfaces consume the same pure
`grafita-core` but keep their own thin adapter and QML composition.

## Principles

- Rust core, QML frontend, thin bridge.
- One visual language (`celestina-style`); apps art-direct within its tokens.
- Interop between processes via XDG/freedesktop; in-process suite reuse through
  narrow Rust core APIs, not copied domain logic.
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
