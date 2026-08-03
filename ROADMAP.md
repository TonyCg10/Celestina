# Celestina — suite roadmap

> The umbrella roadmap for the whole monorepo. Each project keeps its own
> checkpoint list and release timing; this file owns the shared vision, the
> suite-level checkpoints, and the contracts between projects.
>
> Per-project roadmaps: [celestina-rs](celestina-rs/ROADMAP.md) ·
> [celestina-style](celestina-style/ROADMAP.md) ·
> [celestina](celestina/ROADMAP.md) ·
> [siderita](siderita/ROADMAP.md) ·
> [magnetita](magnetita/ROADMAP.md) ·
> [fluorita](fluorita/ROADMAP.md) ·
> [grafita](grafita/ROADMAP.md)
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

**Current focus:** the shell's first real daily panel slice is now active work.
`celestina` reads Niri's event stream through a pinned Rust adapter and renders
real output-local workspaces plus the active window beside its clock and phone
state. It remains read-only until focus requests can expose pending, failed and
provider-confirmed outcomes; Noctalia stays the reversible fallback. Since
2026-07-29 the shell's destination is explicit: replace Noctalia entirely in
staged phases (R0–R9 in the [shell roadmap](celestina/ROADMAP.md)), each phase
retiring one responsibility behind a real-session gate.

## The pieces

| Project | Role | Stack | Consumes | Consumed by |
|---|---|---|---|---|
| [celestina-rs](celestina-rs/) | Shared domain cores | Rust | — | siderita, magnetita, future apps |
| [celestina-style](celestina-style/) | Shared visual language | QML | — | celestina, siderita, magnetita, future apps |
| [celestina](celestina/) | Niri shell / session | C++ · QML (+ Rust bridge) | celestina-style | the session |
| [siderita](siderita/) | File manager (first app) | Rust · QML (CXX-Qt) | celestina-rs, celestina-style | the user |
| [magnetita](magnetita/) | Phone link (KDE Connect) | Rust · QML (CXX-Qt) | celestina-rs, celestina-style | the user, siderita (via `org.celestina.Devices1`) |
| [grafita](grafita/) | Text editor | Rust · QML (CXX-Qt) | celestina-rs, celestina-style | the user, siderita (embedded editing surface) |

Dependencies flow one way — cores and style never depend on apps or the shell:

```
celestina-rs ───────► siderita, magnetita, future apps
  (domain/IO)

celestina-style ────► celestina, siderita, magnetita, future apps
  (tokens/components)
```

> `siderita` now renders from the shared `celestina-style` module — its private
> theme and glass were removed and the canonical copies live in
> `celestina-style`. `celestina` renders from it too since the restyle's S5
> (panel + chooser, imported from source); the half that remains is consuming
> an *installed* release, deferred with STYLE-D until an out-of-tree consumer
> exists.

**Planned apps** — not built, listed in the README's
[Authorized / ready to implement](README.md#authorized--ready-to-implement) section and each
reusing `celestina-rs` + `celestina-style`:

- **[Fluorita](fluorita/)** — the local media library/player: Gallery for
  images/video, Music for albums/artists/tracks, plus shared static artwork and
  bounded live-preview contracts. `Space` in Siderita opens its minimal player;
  double-click/Enter opens the full app. F1 builds the shared media/library core.
- **[Grafita](grafita/)** — the general text editor, no longer only planned:
  G1 delivered the content-based shared document core, G2 the embedded Siderita
  modal that `Space` opens (now driven by content on double-click/Enter too),
  G3 the standalone application with its own window, desktop entry and
  installer, G4 find/replace/go-to-line/highlight, and G6 tabs — one running
  instance, one session per document, save-as for untitled tabs and a
  recent-documents list.

Fluorita has a tested core crate and no UI surface yet; Grafita has both
surfaces built with G0–G4 and G6 done, and both driven by the author with a
real keyboard and mouse (typing, shortcuts, find bar, tabs, drag-to-reorder),
with a few bugs found and fixed along the way.
**[Magnetita](magnetita/) has left the planning stage** (shipped 1.0.0; see the
status snapshot above). That is
deliberate: Siderita already consumes the freedesktop artwork cache and has
separate Quick Look branches ready to be replaced by bounded consumers. Grafita
owns text document truth. Fluorita owns local catalogue, playback and derived
media truth, including static image/video/audio artwork and on-demand trailers.
Each standalone app and Siderita keep distinct UI surfaces over those shared
contracts. Both build gates are open at their first core milestone.

## Shared foundations (the stack contract)

These hold across every project and are the reason the suite is worth building
as a suite rather than four unrelated apps:

- **Rust core, QML frontend, thin bridge.** Domain, IO, state and coordination
  in Rust; presentation in QML; the C++/CXX-Qt layer kept to the generated
  bridge. Qt models mutate only on the GUI thread. Owned workers are bounded and
  join on shutdown; any best-effort watcher without a deterministic lifecycle is
  named as debt rather than treated as the pattern.
- **One visual language.** `celestina-style` owns semantic tokens and generic
  controls. Apps art-direct within the tokens; they do not fork the look.
- **Standards over glue.** Inter-process integration uses XDG/freedesktop (URIs,
  MIME, Trash, icon themes, portals, notifications, `.desktop` entries).
  In-process consumers reuse narrow Rust core APIs rather than copying domain
  rules or reaching into another app's UI.
- **Measured lightweight.** "Light" is a number: installed closure, start time,
  PSS/RSS, wakeups and GPU cost are tracked per app; the shared Qt runtime is
  amortized across the suite, not used to excuse any single app's waste.
- **Truthful state.** A click is a request, never proof of success. The UI never
  presents a location or result it has not verified.
- **Versioned contracts, independent releases.** Rust crates are pinned for a
  release. In-tree visual consumers build from or import the canonical style
  source directly; a relocatable installed style module is deferred to STYLE-D
  until an out-of-tree consumer exists. The monorepo owns shared history and the
  contracts.

## Status snapshot (2026-07-29)

- ✅ Monorepo git baseline established (this repository).
- `celestina-rs` — eight crates compile: the five pure cores plus Magnetita's
  protocol core, TLS transport and headless daemon; fmt, Clippy and the
  workspace tests pass.
- `celestina-style` — the canonical shared source (semantic tokens, working
  glass, Lucide controls and a host-controlled reduced-motion input). Official
  consumption contract: apps symlink the
  sources into their own CXX-Qt modules and compile them in, CI-guarded
  against copies; the shell (panel and chooser) imports the source tree via a
  self-provisioned import path, with the same URI alias supplied to `qmllint`.
  The visual/contrast guard covers all three consumers, and an offscreen Qt
  Quick Test proves modal focus containment/restoration, Escape and pointer
  blocking. Full legacy-motion, real-session focus rendering and AT acceptance
  remain STYLE-1 work; an installable clean-prefix release is deferred until an
  out-of-tree consumer exists.
- `celestina` — a pinned Rust Niri event adapter feeds a bounded C++/Qt decoder
  and host; framing recovery has a focused QtTest target. The panel renders from
  `CelestinaTheme` (restyle S5) and asks the compositor for wallpaper blur via
  a per-surface KWindowEffects controller (niri `ext-background-effect`,
  best-effort) with an explicit readable fallback. Geometry/zone/focus and real
  IPC-restart acceptance remain open; the earlier blur capture does not validate
  the current controller/tint by itself.
- `siderita` — **v1.0 (now 1.0.1): Iteration 1 concluded (2026-07-25)**, the full CP0 → CP5
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
- `magnetita` — **1.0.0: CP0–CP4 complete (2026-07-26)** was verified live against
  the real phone. UDP discovery, TLS with TOFU pinning, pairing with a temporary
  comparison code, reconnect-as-trusted, a systemd user service; the phone
  mounted over sshfs and served on `org.celestina.Devices1` — the suite's
  **first internal contract** — consumed by Siderita's sidebar, by the
  standalone app (pair/unpair, a connection log that says *why*, Settings with
  per-plugin toggles) and by the `celestina` panel. Daily plugins live:
  battery, notifications, file share both ways, find-my-phone, clipboard,
  MPRIS media both ways. The UI now keeps blocking zbus work off the Qt thread
  and coalesces refreshes. Pairing v8/identity/admission, durable revocation,
  post-transfer publication barriers and typed MPRIS/progress/artwork corrections
  have unit or loopback coverage but await a fresh phone/Wayland acceptance pass.
  The UI action worker and daemon MPRIS worker
  are bounded and joined; the best-effort D-Bus read/signal watchers still lack
  deterministic shutdown. Known phone-side limit: clipboard phone → desktop is manual
  (Android forbids background clipboard reads).

---

## Checkpoint 0 — Foundations
**Goal:** every project has a recoverable baseline, a declared toolchain, and a
truthful first slice; the shared contracts exist in a form apps can consume.

- [x] Monorepo git baseline
- [ ] **celestina-rs CP0** — freeze & version the read-only core API
- [x] **celestina-style CP0** — one canonical source consumed directly by every in-tree surface: symlink-compiled by both apps and source-imported by the shell, CI-guarded; the public API/qmldir/CMake inventory and qmllint path are aligned. The installable clean-prefix module is deferred until an out-of-tree consumer exists
- [ ] **celestina CP0** — panel geometry, exclusive zone and no-focus verified on real Niri
- [x] **siderita CP0** — ship the read-only slice from a staged install with real-Wayland resource/frame numbers; ratify or reopen Qt/QML

**Done when:** every in-tree project consumes the canonical sibling sources with
no copies or forks; the shell maps correctly on every output without stealing
focus; the file manager runs from an install and its budget is met or the
frontend is explicitly reopened. A source-free style package is STYLE-D work.

## Checkpoint 1 — Daily driver
**Goal:** the shell and file manager are usable as the primary session and
visibly share one design language.

- [ ] **celestina CP1** — real Niri workspaces + focused window via a Rust adapter, with pending/failed/confirmed focus requests
- [ ] **celestina CP2** — opt-in Niri startup contract composing external session tools with verified fallbacks, before Noctalia leaves autostart
- [x] **siderita CP1** — loss-free file operations (create/rename/copy/move/trash) on disposable fixtures, source never removed before destination is verified
- [x] **celestina-rs CP1** — the write-side domain those operations stand on (`siderita-ops`: loss-free verbs, consumed live by Siderita; the dotfiles apply API remains open in its own roadmap)
- [ ] **celestina-style CP1** — stable, accessible design contract (compat/deprecation, truthful glass, font/icon fallbacks, a11y)
- [x] **Convergence (Siderita)** — `siderita` renders from the shared CelestinaStyle module (semantic tokens + working glass + icons); its private theme/glass were removed
- [x] **Convergence (desktop)** — the panel and chooser render from CelestinaStyle (restyle S5, confirmed on the live session); the installed-release half of convergence stays deferred with STYLE-D

**Done when:** the author can run a Niri session on Celestina's shell with
Siderita as the file manager for daily use; both consume the same canonical
style source without drift; no data-loss path exists in file operations. An
installed style release is not a prerequisite before STYLE-D's external gate.

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
- [ ] **Retire Noctalia (celestina R0–R9)** — the shell replaces Noctalia one
      responsibility at a time (bar → launcher → session verbs/OSD →
      notifications → control center → lock → wallpaper/Settings portal →
      Polkit/dock), each phase gated on real-session evidence with a named
      fallback; the phases live in the [shell's roadmap](celestina/ROADMAP.md)
      and the per-phase work orders in
      [NOCTALIA-REPLACEMENT.md](celestina/NOCTALIA-REPLACEMENT.md)
- [ ] Suite conventions: single-instance behavior, a small IPC/activation convention, `open-with`/handler wiring, drag-and-drop between first-party apps — all over freedesktop standards
- [ ] One settings + theming source shared by the shell and every app
- [ ] Additional first-party apps — **Grafita** has landed G0–G4 and G6 (both
      surfaces, find/replace, tabs) and **Fluorita** its F1 media core. Each
      reuses `celestina-rs` + `celestina-style`, adds its own bounded
      domain/engine crates and advances
      through reviewable milestones rather than a feature batch

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
