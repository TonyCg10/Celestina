# Celestina — suite roadmap

> The umbrella roadmap for the whole monorepo. Each project keeps its own
> checkpoint list and release timing; this file owns the shared vision, the
> suite-level checkpoints, and the contracts between projects.
>
> Per-project roadmaps: [celestina-rs](celestina-rs/ROADMAP.md) ·
> [celestina-style](celestina-style/ROADMAP.md) ·
> [celestina](celestina/ROADMAP.md) ·
> [siderita](siderita/ROADMAP.md) ·
> [magnetita](magnetita/ROADMAP.md)
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

**Current focus:** the phone link (`magnetita`), just started — the suite's first
cross-app integration and first networked app, built first because it hands the
phone to Siderita, mounted and always there. `siderita` shipped **v1.0** and is in
daily use; the Niri shell (`celestina`) awaits its Rust adapter.

## The pieces

| Project | Role | Stack | Consumes | Consumed by |
|---|---|---|---|---|
| [celestina-rs](celestina-rs/) | Shared domain cores | Rust | — | siderita, future apps |
| [celestina-style](celestina-style/) | Shared visual language | QML | — | celestina, siderita, future apps |
| [celestina](celestina/) | Niri shell / session | C++ · QML (+ Rust bridge) | celestina-style | the session |
| [siderita](siderita/) | File manager (first app) | Rust · QML (CXX-Qt) | celestina-rs | the user |
| [magnetita](magnetita/) | Phone link (KDE Connect) | Rust · QML (CXX-Qt) | celestina-rs, celestina-style | the user, siderita (via `org.celestina.Devices1`) |

Dependencies flow one way — cores and style never depend on apps or the shell:

```
celestina-rs ─────┐                    celestina-style ─────┐
  (domain cores)  ├──► siderita            (tokens +        ├──► celestina
                  │      (file mgr)         components)     │      (Niri shell)
                  └──► future apps ◄──────────────────────── ┘──► future apps
```

> `siderita` now renders from the shared `celestina-style` module — its private
> theme and glass were removed and the canonical copies live in
> `celestina-style`. `celestina` still uses a small inline palette; finishing
> that half of the convergence is a Checkpoint 1 goal.

**Planned apps** — design-stage, listed in the README's
[Planned](README.md#planned) section, each started only when a recurring daily
gap proves the need and each reusing `celestina-rs` + `celestina-style`:

- **[Fluorita](fluorita/)** *(working name)* — the media player (audio · video ·
  image) that produces the video/audio thumbnails Siderita consumes, and later
  runs as a shell widget.
- **[Grafita](grafita/)** *(working name)* — a light text/code editor, the
  edit-side companion to Siderita's read-only quick-look ("Abrir con Grafita").

Fluorita and Grafita each have a directory holding a README and a roadmap and no
code — **[Magnetita](magnetita/) has left this stage** (its protocol core has
begun; see the status snapshot above). That is
deliberate: two of Siderita's shipped decisions (consuming video thumbnails it
will not generate; a quick-look that hands video, audio and PDF to an info card
naming Fluorita) are already promises to these projects, and a promise is worth
writing down as a contract. The build gate is unchanged — each starts only when a
recurring daily gap proves the need.

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

## Status snapshot (2026-07-25)

- ✅ Monorepo git baseline established (this repository).
- `celestina-rs` — five cores compile (Magnetita's protocol core is the newest);
  fmt, Clippy and the workspace tests pass.
- `celestina-style` — now the canonical shared module (semantic tokens +
  working glass + fallback icons), builds with CMake and is consumed by
  siderita; a clean-prefix installable release is still open.
- `celestina` — host builds and QML-lints; geometry/zone/focus not yet verified
  on real Niri; no Rust yet.
- `siderita` — **v1.0: Iteration 1 concluded (2026-07-25)**, the full CP0 → CP5
  arc. CP0–CP3 are complete and ratified on real Wayland (staged self-contained
  install, loss-free operations, freedesktop interop, a native role model with a
  live hotplug/FS watcher, list/grid/details views, thumbnails, spacebar
  quick-look). **CP4** — natural name order, favourites, an organizable sidebar,
  per-folder views, drag comforts, batch rename, Recientes, per-collision
  conflicts — is implemented and headless-verified. **CP5** makes Siderita the
  desktop's file chooser: it serves `org.freedesktop.impl.portal.FileChooser`
  (`OpenFile`/`SaveFile`/`SaveFiles`) with its own picker window, type filters
  and `--portal` activation, verified end-to-end on the real session. Three
  items are carried past 1.0, named not hidden: CP4's drag/menu-blur real-Wayland
  validation, CP5's `parent_window` (needs `xdg-foreign`), and CP5's opt-in
  portal routing until it has been lived with.
- `magnetita` — **started (2026-07-25)**: the suite's phone link over KDE Connect,
  its first cross-app integration and first networked app. `magnetita-core` has
  begun — the packet envelope and the identity packet, offline-tested (8 tests).
  Next is the trusted TLS/pairing channel, a **standalone app** (pair, a
  connection log that says *why* a phone will not connect, and options), and the
  phone mounted in Siderita via a real sshfs mount plus `org.celestina.Devices1`,
  the suite's **first internal contract**.

---

## Checkpoint 0 — Foundations
**Goal:** every project has a recoverable baseline, a declared toolchain, and a
truthful first slice; the shared contracts exist in a form apps can consume.

- [x] Monorepo git baseline
- [ ] **celestina-rs CP0** — freeze & version the read-only core API
- [ ] **celestina-style CP0** — module installable/importable from a clean prefix, glass APIs made truthful
- [ ] **celestina CP0** — panel geometry, exclusive zone and no-focus verified on real Niri
- [x] **siderita CP0** — ship the read-only slice from a staged install with real-Wayland resource/frame numbers; ratify or reopen Qt/QML

**Done when:** no project needs a sibling source checkout to build; the shell
maps correctly on every output without stealing focus; the file manager runs
from an install and its budget is met or the frontend is explicitly reopened.

## Checkpoint 1 — Daily driver
**Goal:** the shell and file manager are usable as the primary session and
visibly share one design language.

- [ ] **celestina CP1** — real Niri workspaces + focused window via a Rust adapter, with pending/failed/confirmed focus requests
- [ ] **celestina CP2** — opt-in Niri startup contract composing external session tools with verified fallbacks, before Noctalia leaves autostart
- [x] **siderita CP1** — loss-free file operations (create/rename/copy/move/trash) on disposable fixtures, source never removed before destination is verified
- [ ] **celestina-rs CP1** — the write-side domain those operations stand on
- [ ] **celestina-style CP1** — stable, accessible design contract (compat/deprecation, truthful glass, font/icon fallbacks, a11y)
- [x] **Convergence (Siderita)** — `siderita` renders from the shared CelestinaStyle module (semantic tokens + working glass + icons); its private theme/glass were removed
- [ ] **Convergence (desktop)** — migrate `celestina` off its inline palette onto CelestinaStyle

**Done when:** the author can run a Niri session on Celestina's shell with
Siderita as the file manager for daily use; both consume the same installed
style release; no data-loss path exists in file operations.

## Checkpoint 2 — One suite
**Goal:** the apps behave as one suite, not a folder of separate programs.

- [ ] **Own the session's desktop portals.** A portal is where a foreign desktop
      still decides how this one behaves: an application asks
      `xdg-desktop-portal` for a file, a screen or the colour scheme, and a
      *backend* answers. Siderita already serves `FileChooser`; the rest leaves
      in stages — `ScreenCast`/`Screenshot` move from **gnome to
      xdg-desktop-portal-wlr** (which drops the last reason Nautilus is
      installed, at the cost of whole-output-only capture), then to `celestina`
      once the shell has a real Niri adapter; `Settings` moves to `celestina` as
      the first portal the shell serves. Everything else stays with **gtk** until
      one of them proves a daily need. Detail and the interface table live in the
      [shell's roadmap](celestina/ROADMAP.md). *Removing the GTK library is not a
      goal — Firefox and Electron link it regardless; removing GTK's and GNOME's
      say over this session's dialogs is*
- [ ] Suite conventions: single-instance behavior, a small IPC/activation convention, `open-with`/handler wiring, drag-and-drop between first-party apps — all over freedesktop standards
- [ ] One settings + theming source shared by the shell and every app
- [ ] Additional first-party apps — **Grafita** (text/code editor), **Fluorita** (media player: audio · video · image) — added **one at a time**, each only after recurring friction with the tool it replaces proves the need; each reuses `celestina-rs` + `celestina-style` and adds its own domain crate

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
