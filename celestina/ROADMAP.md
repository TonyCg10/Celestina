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
per output (scope `celestina-panel`), reserves each edge and rejects keyboard
focus. The first read-only S1 slice is live: a separate Rust helper pinned to
`niri-ipc 26.4.0` reduces Niri's event stream into output-local workspace and
active-window snapshots; a bounded Qt adapter clears them if the provider is
unavailable. The panel composes those real workspaces and the active window on
the left, the minute-aligned clock in the geometric centre and the real phone
state on the right, with no simulated tray. Configure, build and QML lint pass;
a real-Niri protocol trace showed the blur request reaching the panel's layer
surface, but visual comparison correctly rejected that as proof because the
wallpaper remained sharp. Matching Noctalia's finite surface-local region and
committing it after capability discovery produced visible blur in a subsequent
normal `./scripts/run.sh` capture. Noctalia stayed the reversible fallback.
Geometry,
exclusive zone and focus still need their complete direct acceptance checks;
IPC-loss/restart recovery is implemented but not yet accepted by restarting the
real compositor.

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

- [x] Rust adapter observing Niri's event stream (separate-process pipe provides
      backpressure; bounded validation and model mutation stay on the GUI thread)
- [x] Show real output-local workspaces and the active window title
- [ ] Focus requests show pending / failed / confirmed (a click is a request, not proof of success)
- [ ] Panel survives Niri IPC loss and restart without presenting stale state

**First-slice evidence (2026-07-29).** Rust formatting, adapter tests and
Clippy `-D warnings` passed; the exact 26.04 adapter produced a live snapshot
from the current Niri socket; CMake, QML compilation and `all_qmllint` completed;
the suite architecture guard passed. A real session inspection showed
`celestina-panel` on the top layer with `keyboard_interactivity: None` and the
three-region panel rendered against compositor blur. The host now waits for the
asynchronous blur capability, submits a finite surface-local region and requests
the frame that commits it. This was accepted only after an untraced normal
`./scripts/run.sh` capture visibly removed wallpaper detail from the 40 px strip;
the earlier protocol-only result was explicitly rejected. `qmllint` retains the
known source-import/context warnings; no interaction, IPC restart, AT-SPI or
exclusive-zone acceptance is claimed by this evidence.

## Checkpoint 2 — Dependable personal session (S2)
**Goal:** an opt-in Niri startup contract that composes external session tools
with verified fallbacks before Noctalia leaves autostart.

- [ ] Opt-in Niri startup contract that never silently overwrites the user's config or dotfiles
- [ ] Compose external launcher, notification daemon, lock/idle tools, portals, keyring, Polkit agent, wallpaper, and optional Xwayland
- [ ] Dependency diagnostics + verified fallback keybindings
- [ ] Remove Noctalia from autostart only after fallbacks are verified

### The portal backends, and how they leave

The session's desktop portals are the last place a foreign desktop still decides
how this one behaves. The migration is deliberate and staged — each interface
leaves only when something here can answer it truthfully.

| Interface | Serves | Today | Next | Eventually |
|---|---|---|---|---|
| `FileChooser` | "open a file" / "save as" in every application | **siderita** | siderita | siderita |
| `ScreenCast`, `Screenshot` | screen sharing and capture requests | **wlr** (+ our chooser) | wlr | celestina |
| `Settings` | the light/dark preference applications read | gtk | gtk | celestina |
| `Print`, `Notification`, `AppChooser`, `Access`, `Account`, `Email`, `Inhibit`, `Lockdown`, `DynamicLauncher` | the rest | gtk | gtk | gtk, until one of them proves a daily need |

- [x] `FileChooser` — served by Siderita (its CP5), routed in `niri-portals.conf`
- [x] **`ScreenCast` / `Screenshot` moved to `xdg-desktop-portal-wlr`.** Niri
      speaks both the Mutter screencast API *and* `zwlr_screencopy_manager_v1`,
      so the wlroots backend can serve them — and it carries no Nautilus
      dependency, which was the only reason a GNOME file manager was still
      installed. Verified end to end: a real ScreenCast request reached the wlr
      backend and opened this project's own chooser. **The cost stands and is
      not hidden:** `wlr-screencopy` captures whole outputs, so sharing a
      *single window* in a call is lost, and there are no restore tokens. Undo is
      two lines in `niri-portals.conf`
- [x] **The output chooser is ours** — `celestina --pick-output`
      (`qml/OutputChooser.qml`, invoked directly; the binary self-provisions the
      CelestinaStyle import path, so no wrapper script is needed).
      xdpw brings no dialog of its own: it runs a command and keeps whatever
      output name that command prints, which makes the chooser a replaceable
      part — so the session wears its own dialog long before the shell serves
      the portal itself. It lays the outputs out **in a row**, each tile keeping
      its screen's real proportions, because a desktop is arranged left to right
      and picking a monitor is a spatial gesture, not a menu choice.
      *Hosted in the shell's own binary rather than a loose `qml` runtime*, which
      buys the two things that matter: a real stdout to answer on, and a stable
      Wayland `app_id` (`celestina`) — without it a window rule has nothing to
      match, and niri tiles the dialog into a column. With the rule it opens
      floating and centred (verified: `is_floating: true`). It is a centred card
      inside its window regardless, since a tiling compositor decides window
      size, not the dialog
- [ ] Remove the GNOME backend for good — `pacman -Rs xdg-desktop-portal-gnome
      nautilus nautilus-python`, once a few real calls have been lived with
- [ ] `Settings` from celestina — the smallest portal worth owning (it answers
      with a colour scheme and an accent) and the one the session should own
      anyway, since the shell is what decides how the session looks. This is the
      first portal this project serves, and the step that makes the GTK backend
      optional rather than assumed
- [ ] `ScreenCast` / `Screenshot` from celestina — only after the shell has a
      real Niri adapter (CP1): capture belongs to whoever knows the outputs and
      windows, and that is the shell. Until then wlr holds the interface

**Not a goal:** removing GTK. Zen is Firefox and Slack is Electron; both link the
toolkit directly, so it stays on disk whatever the portals say. What is
achievable — and what this section is about — is that GTK and GNOME stop
*deciding* how this session's dialogs look and behave.

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
