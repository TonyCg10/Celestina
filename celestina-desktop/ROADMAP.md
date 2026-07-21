# Celestina Desktop roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the Niri
> shell only. Checklist legend: `[x]` done · `[ ]` planned.

## Overview

**Purpose.** The independent shell/session for a personal Niri environment. The
first product is a small, truthful top panel with real Niri workspace control. A
usable daily session composes mature external tools instead of reimplementing a
launcher, notification daemon, lock screen, auth agent or wallpaper manager — a
dependable personal shell that grows from demonstrated daily needs.

**Current state.** A C++20/Qt Quick host maps one 40 px top layer-shell surface
per output (scope `celestina-desktop-panel`), reserves each edge and rejects
keyboard focus; the only visible information is a real minute-aligned clock, with
no simulated workspaces or tray. Configure, build and QML lint pass; a live Niri
check mapped exactly one namespaced, non-focus surface per output while Noctalia
kept running. Geometry, exclusive zone and focus still need direct acceptance
checks. There is no Rust yet.

**Session dependency.** The current Niri config starts Noctalia, which still owns
the launcher (`Mod+Space`), lock/idle/DPMS, its Polkit agent, greeter sync and
night light. Those paths must be preserved until Checkpoint 2 installs and
verifies explicit replacements.

**Key decisions.** Desktop owns shell/session work only; Niri-only until a real
second need exists; new domain/IO in Rust, presentation in QML, a thin C++ host;
external state is provider-confirmed (a click is a request, never proof); style
comes from an installed CelestinaStyle contract; Noctalia stays the dev fallback
through Checkpoint 1; never silently overwrite the user's Niri config or dotfiles.

## Checkpoint 0 — Visible, truthful panel (S0)
**Goal:** a clean build launches one correct Niri top panel on every connected
output, showing real local time, with no fake controls and no checkout-relative
dependencies, while Noctalia stays available as the fallback.

- [x] Monorepo git baseline (previously blocker #1)
- [x] Per-output panel create / hotplug / teardown implemented (QPointer-safe, uniquely namespaced, fail-fast)
- [x] Minute-aligned real local clock; no simulated workspaces or tray placeholders
- [ ] Reproducible Qt/LayerShellQt dev environment with recorded tool versions
- [ ] Verify one panel per output with correct geometry, exclusive zone, and **no keyboard focus** on real Niri, beside Noctalia
- [ ] Verify invalid imports / root / layer setup fail visibly and non-zero
- [ ] Consume CelestinaStyle from an installed module instead of the inline palette
- [ ] Rust/QML bridge + bounded executor + deterministic-shutdown spike, kept out of the S0 user surface
- [ ] Build, startup and resource baselines (artifact size, start time, PSS/RSS, wakeups, GPU cost)

**Done when:** a clean build needs no sibling checkout; exactly one panel maps on
every connected output with correct height, exclusive zone and namespace and no
focus theft while Noctalia runs; the clock crosses a minute boundary and every
visible action is truthful; invalid setup fails visibly; the bridge probe
survives updates, overload and shutdown without stale UI, leaks or cross-thread
model mutation.

## Checkpoint 1 — Real Niri workspace panel (S1)
**Goal:** keep the per-output lifecycle while showing real Niri state from a Rust
adapter, surviving IPC loss and restart without stale state.

- [ ] Rust adapter observing Niri's event stream (bounded queue, GUI-thread-only model mutation)
- [ ] Show real workspaces and the focused window
- [ ] Focus requests show pending / failed / confirmed (a click is a request, not proof of success)
- [ ] Panel survives Niri IPC loss and restart without presenting stale state

## Checkpoint 2 — Dependable personal session (S2)
**Goal:** an opt-in Niri startup contract that composes external session tools
with verified fallbacks before Noctalia leaves autostart.

- [ ] Opt-in Niri startup contract that never silently overwrites the user's config or dotfiles
- [ ] Compose external launcher, notification daemon, lock/idle tools, portals, keyring, Polkit agent, wallpaper, and optional Xwayland
- [ ] Dependency diagnostics + verified fallback keybindings
- [ ] Remove Noctalia from autostart only after fallbacks are verified

## Checkpoint 3 — Later
**Goal:** add surfaces only when a real daily workflow proves the need.

- [ ] Audio / battery / brightness status, only when the hardware workflow proves the panel needs it
- [ ] Tray, launcher or notifications considered one at a time, on recurring friction with the external component
- [ ] Settings UI or richer theming only after file configuration and the daily session are stable

## Non-goals

Do not pursue Noctalia feature parity, support another compositor, build a plugin
framework, own security-sensitive lock/auth flows, plan other Celestina apps
here, fork Niri, overwrite unrelated dotfiles, or add a shell surface without a
demonstrated daily workflow gap.
