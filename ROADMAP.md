# Celestina — suite roadmap

> The umbrella roadmap for the whole monorepo. Each project keeps its own
> checkpoint list and release timing; this file owns the shared vision, the
> suite-level checkpoints, and the contracts between projects.
>
> Per-project roadmaps: [celestina-rs](celestina-rs/ROADMAP.md) ·
> [celestina-style](celestina-style/ROADMAP.md) ·
> [celestina-desktop](celestina-desktop/ROADMAP.md) ·
> [siderita-adma](siderita-adma/ROADMAP.md)
>
> Checklist legend: `[x]` done · `[ ]` planned. Source presence is not runtime
> evidence — a goal stays unchecked until it is verified, not merely written.

## What Celestina is

Celestina is a personal computing suite for a Niri/Wayland session: a small,
truthful **shell** plus a growing set of **first-party apps**, all sharing one
core stack, one visual language and one set of conventions.

The thesis is simple. General-purpose desktop apps (a GNOME/KDE file manager, a
generic panel, a stock viewer) drag in large dependency closures and impose a
look and behavior that were never designed for this session. Because the session
owns its own shell, it can instead own lean apps that:

- share a single **Rust core** for domain, IO, state and coordination;
- share a single **QML visual language** so every surface feels like one system;
- talk to the rest of the desktop through **standards**, not private glue; and
- earn every dependency and every feature by a demonstrated daily need.

The goal is a session that is dependable, coherent and light — not feature
parity with any existing desktop environment, and not a product for anyone but
its author.

**Current focus:** a minimal Niri shell (`celestina-desktop`) and the file
manager (`siderita-adma`). Everything else waits behind a proven daily gap.

## The pieces

| Project | Role | Stack | Consumes | Consumed by |
|---|---|---|---|---|
| [celestina-rs](celestina-rs/) | Shared domain cores | Rust | — | siderita-adma, future apps |
| [celestina-style](celestina-style/) | Shared visual language | QML | — | celestina-desktop, siderita-adma, future apps |
| [celestina-desktop](celestina-desktop/) | Niri shell / session | C++ · QML (+ Rust bridge) | celestina-style | the session |
| [siderita-adma](siderita-adma/) | File manager (first app) | Rust · QML (CXX-Qt) | celestina-rs | the user |

Dependencies flow one way — cores and style never depend on apps or the shell:

```
celestina-rs ─────┐                 celestina-style ──┐
  (domain cores)  ├──► siderita-adma        (tokens +  ├──► celestina-desktop
                  │      (file mgr)          components)│      (Niri shell)
                  └──► future apps ◄────────────────────┘──► future apps
```

> `siderita-adma` now renders from the shared `celestina-style` module — its
> private theme and glass were removed and the canonical copies live in
> `celestina-style`. `celestina-desktop` still uses a small inline palette;
> finishing that half of the convergence is a Checkpoint 1 goal.

## Shared foundations (the stack contract)

These hold across every project and are the reason the suite is worth building
as a suite rather than four unrelated apps:

- **Rust core, QML frontend, thin bridge.** Domain, IO, state and coordination
  in Rust; presentation in QML; the C++/CXX-Qt layer kept to the generated
  bridge. Qt models mutate only on the GUI thread; background work is bounded
  and joins on shutdown.
- **One visual language.** `celestina-style` owns semantic tokens and generic
  controls. Apps art-direct within the tokens; they do not fork the look.
- **Standards over glue.** Interop via XDG/freedesktop (URIs, MIME, Trash, icon
  themes, portals, notifications, `.desktop` entries), never private APIs.
- **Measured lightweight.** "Light" is a number: installed closure, start time,
  PSS/RSS, wakeups and GPU cost are tracked per app; the shared Qt runtime is
  amortized across the suite, not used to excuse any single app's waste.
- **Truthful state.** A click is a request, never proof of success. The UI never
  presents a location or result it has not verified.
- **Versioned contracts, independent releases.** Crates and the style module are
  consumed by pinned version for any release; path deps are a development
  convenience only. The monorepo owns shared history and the contracts.

## Status snapshot (2026-07-20)

- ✅ Monorepo git baseline established (this repository).
- `celestina-rs` — four cores compile; fmt, Clippy, 30 tests pass; read-only.
- `celestina-style` — now the canonical shared module (semantic tokens +
  working glass + fallback icons), builds with CMake and is consumed by
  siderita-adma; a clean-prefix installable release is still open.
- `celestina-desktop` — host builds and QML-lints; geometry/zone/focus not yet
  verified on real Niri; no Rust yet.
- `siderita-adma` — read-only slice runs; measured offscreen only; install
  staging, watcher, native model and real-Wayland numbers still open.

---

## Checkpoint 0 — Foundations
**Goal:** every project has a recoverable baseline, a declared toolchain, and a
truthful first slice; the shared contracts exist in a form apps can consume.

- [x] Monorepo git baseline
- [ ] **celestina-rs CP0** — freeze & version the read-only core API
- [ ] **celestina-style CP0** — module installable/importable from a clean prefix, glass APIs made truthful
- [ ] **celestina-desktop CP0** — panel geometry, exclusive zone and no-focus verified on real Niri
- [ ] **siderita-adma CP0** — ship the read-only slice from a staged install with real-Wayland resource/frame numbers; ratify or reopen Qt/QML

**Done when:** no project needs a sibling source checkout to build; the shell
maps correctly on every output without stealing focus; the file manager runs
from an install and its budget is met or the frontend is explicitly reopened.

## Checkpoint 1 — Daily driver
**Goal:** the shell and file manager are usable as the primary session and
visibly share one design language.

- [ ] **celestina-desktop CP1** — real Niri workspaces + focused window via a Rust adapter, with pending/failed/confirmed focus requests
- [ ] **celestina-desktop CP2** — opt-in Niri startup contract composing external session tools with verified fallbacks, before Noctalia leaves autostart
- [ ] **siderita-adma CP1** — loss-free file operations (create/rename/copy/move/trash) on disposable fixtures, source never removed before destination is verified
- [ ] **celestina-rs CP1** — the write-side domain those operations stand on
- [ ] **celestina-style CP1** — stable, accessible design contract (compat/deprecation, truthful glass, font/icon fallbacks, a11y)
- [x] **Convergence (Siderita)** — `siderita-adma` renders from the shared CelestinaStyle module (semantic tokens + working glass + icons); its private theme/glass were removed
- [ ] **Convergence (desktop)** — migrate `celestina-desktop` off its inline palette onto CelestinaStyle

**Done when:** the author can run a Niri session on Celestina's shell with
Siderita as the file manager for daily use; both consume the same installed
style release; no data-loss path exists in file operations.

## Checkpoint 2 — One suite
**Goal:** the apps behave as one suite, not a folder of separate programs.

- [ ] Suite conventions: single-instance behavior, a small IPC/activation convention, `open-with`/handler wiring, drag-and-drop between first-party apps — all over freedesktop standards
- [ ] One settings + theming source shared by the shell and every app
- [ ] Additional first-party apps (editor, viewer, media player) added **one at a time**, each only after recurring friction with the tool it replaces proves the need; each reuses `celestina-rs` + `celestina-style` and adds its own domain crate

## Later / someday
- [ ] Packaging and distribution beyond the author's machine (reproducible install, dependency diagnostics), once the suite is worth shipping
- [ ] Toolkit-neutral shared assets or more extracted cores, only after ≥2 apps demonstrate reusable demand

## Cross-cutting principles

| Principle | Reason |
|---|---|
| Monorepo holds shared history; each app keeps its own roadmap and release | Shared contracts and one source of truth, without coupling release timing |
| New domain/IO/state in Rust; presentation in QML; bridge stays thin | Testable logic, mature UI, minimal hand-written C++ |
| Consume cores and style by pinned version for releases | A path dependency is not a public interface |
| One visual language, art-directed per app | The suite must feel like one system; widget count is not progress |
| Interop via XDG/freedesktop, not private APIs | Every app stays a good desktop citizen and avoids lock-in |
| Every dependency and feature earns its place by a daily need, and is measured | A personal suite should stay lean and honest, not chase parity |
| Truthful state everywhere | Trust in the session depends on never showing an unverified result |

## Non-goals (suite level)

Celestina is not a general desktop environment for other users, does not target a
second compositor, does not chase GNOME/KDE feature parity, does not add heavy
frameworks (Qt Concurrent, WebEngine, Multimedia, KDE/GNOME libraries) without a
measured need, does not build apps speculatively before a daily gap is proven,
and does not overwrite the user's Niri configuration or unrelated dotfiles.
