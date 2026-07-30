# Celestina Desktop roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the Niri
> shell only. Checklist legend: `[x]` done · `[ ]` planned.

## Overview

**Purpose.** The independent shell/session for a personal Niri environment. The
first product is a small, truthful top panel with real Niri workspace control.
Since 2026-07-29 the destination is explicit (author decision): **replace
Noctalia entirely**, in staged phases, each phase retiring one reversible
responsibility bundle only after its replacement is verified on the real session.
External tools stay part of the answer where they are the mature choice
(grim/slurp, an external locker until R6, the greeter; the clipboard-history
backend is deliberately decided in R2) — the goal is that nothing in the
session depends on Noctalia, not that every feature is reimplemented in-tree.
The plan lives in "Replacing Noctalia" below.

**Current state.** A C++20/Qt Quick host maps one 40 px top layer-shell surface
per output (scope `celestina-panel`), reserves each edge and rejects keyboard
focus. The first read-only S1 slice is live: a separate Rust helper pinned to
`niri-ipc 26.4.0` reduces Niri's event stream into output-local workspace and
active-window snapshots. `NiriProtocolDecoder` owns bounded line framing and
recovers after discarding an oversized frame; `NiriClient` owns the helper
process, JSON validation, confirmed Qt snapshots and bounded restart backoff.
`PanelBlurController` separately owns KWindowEffects capability/retry/geometry
and publishes whether QML must use its denser fallback. The panel composes the
real workspaces and active window on the left, the minute-aligned clock in the
geometric centre and real phone state on the right, with no simulated tray.
A previous real-session capture accepted visible compositor blur only after a
finite surface-local region was committed; protocol logs alone were explicitly
rejected as proof. Geometry, exclusive zone and focus still need their complete
direct acceptance checks, and IPC-loss/restart is implemented but not yet
accepted by restarting the real compositor.

**Session dependency.** The current Niri config starts Noctalia, which still owns
the launcher (`Mod+Space`), lock/idle/DPMS, its Polkit agent, greeter sync and
night light. Each path is preserved until the phase that replaces it (R1–R8;
R0 supplies the shared foundations) verifies its replacement on the real
session; Noctalia leaves autostart only at the R8 gate, last.

**Key decisions.** Desktop owns shell/session work only; Niri-only until a real
second need exists; new domain/IO in Rust, presentation in QML, a thin C++ host;
external state is provider-confirmed (a click is a request, never proof); the
in-tree shell imports the canonical CelestinaStyle source through an explicit
URI alias, while an installable out-of-tree module remains STYLE-D work;
Noctalia stays the dev fallback through Checkpoint 1; never silently overwrite
the user's Niri config or dotfiles. Since 2026-07-29, "no Noctalia feature
parity" is retired as a non-goal: the parity target is the author's lived
configuration (inventoried in the replacement plan below), never upstream
Noctalia's full surface — no plugin runtime, no community palettes, no desktop
widgets, no app-theming templates.

## Checkpoint 0 — Visible, truthful panel (S0)
**Goal:** a clean monorepo build launches one correct Niri top panel on every
connected output, showing real local time with no fake controls, while Noctalia
stays available as the fallback. Packaging without the source tree is deferred
with CelestinaStyle STYLE-D rather than hidden inside this checkpoint.

- [x] Monorepo git baseline (previously blocker #1)
- [x] Per-output panel create / hotplug / teardown implemented (QPointer-safe, uniquely namespaced, fail-fast)
- [x] Minute-aligned real local clock; no simulated workspaces or tray placeholders
- [ ] Reproducible Qt/LayerShellQt dev environment with recorded tool versions
- [ ] Verify one panel per output with correct geometry, exclusive zone, and **no keyboard focus** on real Niri, beside Noctalia
- [ ] Verify invalid imports / root / layer setup fail visibly and non-zero
- [x] Consume the canonical CelestinaStyle source for both panel and chooser;
      CMake and runtime provision the same `CelestinaStyle` URI alias. No inline
      palette or local style copy remains; an installed module belongs to STYLE-D
- [x] Rust helper + bounded QProcess/decoder boundary: deterministic shutdown,
      capped framing, invalid-state clearing and restart backoff, with Rust
      reduction tests and a focused QtTest decoder target
- [ ] Build, startup and resource baselines (artifact size, start time, PSS/RSS, wakeups, GPU cost)

**Done when:** the monorepo build consumes only the canonical sibling style tree;
exactly one panel maps on every connected output with correct height, exclusive
zone and namespace and no focus theft while Noctalia runs; the clock crosses a
minute boundary and every visible action is truthful; invalid setup fails
visibly; the helper boundary survives updates, overload and shutdown without
stale UI, leaks or cross-thread model mutation. A source-free packaged build is
separate STYLE-D evidence.

## Checkpoint 1 — Real Niri workspace panel (S1)
**Goal:** keep the per-output lifecycle while showing real Niri state from a Rust
adapter, surviving IPC loss and restart without stale state.

- [x] Rust adapter observing Niri's event stream (separate-process pipe provides
      backpressure; bounded validation and model mutation stay on the GUI thread)
- [x] Show real output-local workspaces and the active window title
- [ ] Focus requests show pending / failed / confirmed (a click is a request, not proof of success)
- [ ] Accept on real Niri that the panel survives IPC/helper loss and restart
      without stale state. The decoder, clearing and bounded relaunch paths are
      implemented; framing recovery has automated coverage, not compositor proof

**First-slice evidence (2026-07-29).** Rust formatting, adapter tests and
Clippy `-D warnings` passed; the exact 26.04 adapter produced a live snapshot
from the current Niri socket; CMake, QML compilation and `all_qmllint` completed;
the suite architecture guard passed. A real session inspection showed
`celestina-panel` on the top layer with `keyboard_interactivity: None` and the
three-region panel rendered against compositor blur. The host waits for the
asynchronous blur capability, submits a finite surface-local region and requests
the frame that commits it. This was accepted only after an untraced normal
`./scripts/run.sh` capture visibly removed wallpaper detail from the 40 px strip;
the earlier protocol-only result was explicitly rejected. CMake now provisions
the same source-tree URI alias for `qmllint` that runtime uses. That build-time
correction and the decoder QtTest do not add interaction, IPC-restart, AT-SPI or
exclusive-zone acceptance, and the earlier blur capture does not validate the
new controller/fallback tint by itself.

## Checkpoint 2 — Dependable personal session (S2)
**Goal:** an opt-in Niri startup contract that composes external session tools
with verified fallbacks before Noctalia leaves autostart.

> Absorbed by the replacement plan below: composing verified external tools is
> now the interim stage inside each R-phase rather than the end state, and
> "remove Noctalia from autostart" is the R8 gate. The portals table stays
> canonical here.

The former S2 requirements no longer own parallel status. Their canonical
checkboxes are the R0–R8 phase items and final integrated gates below:

| Former S2 requirement | Canonical owner |
|---|---|
| Opt-in startup without overwriting dotfiles | Per-phase config items and the R8 integrated gate |
| External fallbacks and dependency diagnostics | The phase that adopts each dependency, especially R2, R3 and R8 |
| Verified fallback keybindings | R2/R3 integrated gates |
| Remove Noctalia from autostart | R8 only |

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
      size, not the dialog. Live screen snapshots preserve selection by output
      name rather than list index; QuickTest covers reorder and removal, while
      real hotplug acceptance remains separate
**Later portal work, without a second status checklist.** Removing the GNOME
backend remains an author-operated cleanup after enough real calls have been
lived with. R7 owns `Settings` from Celestina: colour scheme and accent are the
smallest portal worth owning because the shell decides how the session looks.
That step makes the GTK backend optional rather than assumed. Serving
`ScreenCast` / `Screenshot` from Celestina remains later work, only after the
shell has a real Niri adapter (CP1): capture belongs to whoever knows outputs
and windows. Until then wlr holds the interface. R7 and the future portal phase
own the corresponding status; this table only owns routing.

**Not a goal:** removing GTK. Zen is Firefox and Slack is Electron; both link the
toolkit directly, so it stays on disk whatever the portals say. What is
achievable — and what this section is about — is that GTK and GNOME stop
*deciding* how this session's dialogs look and behave.

## Replacing Noctalia — the phase plan (R0–R9)

> Author decision 2026-07-29. Noctalia v5 (`noctalia-git` 5.0.0.r4301 — since
> v5 a native Wayland/GLES binary, no longer Quickshell) currently provides
> most of the session. This plan retires it one reversible responsibility bundle
> at a time.
> **Per-phase work orders — files, contracts, evidence, gates, rollbacks and
> the open-decisions log — live in
> [NOCTALIA-REPLACEMENT.md](NOCTALIA-REPLACEMENT.md); status stays here.**
> Rules of the road: every phase has a verifiable exit gate on the real
> session and a named fallback until that gate; a Noctalia service is switched
> off only in its own phase, never as a side effect; domain/IO lands in
> `celestina-rs` crates, thin adapters in `src/`, presentation in `qml/`,
> tokens in `celestina-style` — the suite contract unchanged; evidence follows
> the root matrix (build + qmllint + tests always; real-session capture for
> visuals; keyboard + AT-SPI before an interactive surface is called done).

### What Noctalia serves this session today (inventory, 2026-07-29)

Measured from `~/.config/noctalia/settings.json` (v59), `plugins.json`, the
Niri config and the installed packages — not from upstream's feature list.

| Piece | As actually lived |
|---|---|
| Bar (top, framed, transparent, all 3 outputs) | left: workspace pills · CPU/RAM text · catwalk cat · media mini — center: music-search · clock with seconds · weather — right: tray drawer · notification badge · network · bluetooth · volume · brightness (DDC) · screenshot · screen-toolkit · clipper · caffeine · performance toggle · power profile |
| Launcher (`Mod+Space`) | app grid, 19 pins, categories, `kitty -e` for terminal apps; Noctalia v5's built-in clipboard history + clipper panel; web/gif/kaomoji/unicode/music providers |
| Notifications | freedesktop daemon; bottom compact toasts; history + unread badge; DND |
| OSD | top-right, ~2 s, all kinds. Volume/brightness/media keybinds go **straight to `wpctl`/`brightnessctl`/`playerctl`** — Noctalia's OSD only observes |
| Session / lock | `Mod+Shift+Escape` = lock-and-suspend; lock-on-suspend; session menu (numbered shortcuts, 10 s countdown); `Mod+Shift+A` = dpms-off |
| Idle | 300 s lock chain configured but **neutralized by autostart `caffeine-enable`** — the lived flow is manual lock |
| Night light | forced **constant 2700 K**, day and night (wlr-gamma-control) |
| Wallpaper | manual random from `~/Imágenes/Fondos`, crop, disc transition; a Niri layer-rule places it in the backdrop |
| Color / theming | monochrome-from-wallpaper dark palette; `syncGsettings` dark sync; app templates **off**; `noctalia.kdl` a frozen include since 2026-06-11 |
| Control center | right-click on bar; quick toggles + audio/brightness/weather/media-sysmon cards |
| Weather / calendar | Open-Meteo, auto-located; calendar month + events |
| Dock | bottom, auto-hide, running apps only |
| Polkit agent | the `polkit-agent` plugin — the invisible hard dependency |
| Screen extras | screenshot / screen-toolkit / screen-recorder plugins; keyboard capture + OCR flows already bypass them (niri native, grim/slurp/tesseract) |
| Greeter | `noctalia-greeter` on greetd — a separate package that runs without the shell |

Explicitly unused today — **not replacement targets**: app-theming templates
(`enableUserTheming: false`), hooks, desktop widgets (feature on, zero
placed), Wallhaven, wallpaper automation/rotation, launcher session/window
search, notification sounds, fprintd, dark/light scheduling, the
wallpaperengine plugin. Stale config worth knowing: `monitorForColors` names a
nonexistent `DP-3`; the frozen `noctalia.kdl`'s blue accent no longer matches
the monochrome palette.

### The order and why

R0 foundations → R1 bar → R2 launcher → R3 session verbs (OSD, night light,
caffeine/DPMS, composed lock) → R4 notifications → R5 control center + session
menu + weather/calendar → R6 first-party lock & idle → R7 wallpaper & look →
R8 Polkit + dock + Noctalia leaves → R9 greeter (someday).

Visible daily value first: the bar is looked at all day and begins with bounded,
mostly read-only providers; its few write actions remain request/confirmation
flows rather than a broad settings surface. The launcher second, because it forces the two
contracts everything later reuses (on-demand keyboard surfaces and the shell
command channel). Bus-ownership handovers (notifications) and write-side
multi-provider controls (control center) come after those narrower slices prove
the adapters. Security-sensitive surfaces (lock, Polkit) sit behind their own
author gates, composed from external tools until then. The greeter is a
separate package that outlives the shell swap.

### R0 — Foundations every later phase stands on

**Goal:** close the panel's pending acceptance and land the three contracts
the replacement reuses everywhere: a second-surface recipe, a popup path and a
command channel. The geometry/focus and IPC-restart checkboxes remain canonical
in S0/S1 above; R0's final slice closes those prerequisites without duplicating
their status here.

- [ ] Extract `PanelManager` from `main.cpp` into `src/panelmanager.*` without
      introducing the shared abstraction yet
- [ ] Command channel and transient `celestina msg <verb>` CLI delivered under
      the stable-owner/versioned-interface contract in the work order; every
      later keybind routes here
- [ ] Popup path selected by the work order is proven with one real
      `GlassContextMenu` anchored from a panel region on Niri
- [ ] After the popup proves a second real consumer, extract only the shared
      surface intersection into `src/surfacemanager.*` so OSD/launcher/popup
      surfaces do not copy `ensurePanel`
- [ ] Integrated R0 exit accepted on real Niri per the work order; record dated
      evidence here before closing the phase

**Gate:** nothing retired yet; Noctalia untouched.

### R1 — The bar

**Goal:** the panel reaches parity with the bar configuration actually lived
in — the widget list above, not upstream's 33 widget types — so Noctalia's bar
hides permanently. The work order owns the Rust/Qt runtime contract and each
slice's implementation details; this file owns their status.

- [ ] R1-A — provider runtime boundary defined in the work order landed and its
      deterministic lifecycle tests pass before any production provider
- [ ] R1-B — composable flanks, workspace gestures and lived clock format;
      future caffeine/notification/weather extension points exist structurally
      but paint no visible placeholders
- [ ] R1-C — bounded CPU/RAM provider plus the existing external screenshot flow
- [ ] R1-D — desktop MPRIS mini (artwork/title/progress) using the settled
      execution default in the work order; phone media remains on `Devices1`
- [ ] R1-E — truthful volume/mic state using the settled execution default;
      middle-click still opens the external mixer
- [ ] R1-F — read-only NetworkManager/BlueZ indicators plus confirmed
      power-profile indicator/cycle
- [ ] R1-G — per-output DDC brightness, with coalesced scroll steps and unknown
      state instead of GUI blocking or stale values
- [ ] R1-H — StatusNotifierItem host + DBusMenu drawer, including passive items,
      landed as the phase's separate final provider slice
- [ ] R1-I — integrated bar exit accepted on the real session and exact
      persistent hide/rollback evidence recorded here

**Gate:** Noctalia's bar hidden permanently on all three outputs; Noctalia
keeps running headless for everything else. Named at the gate: the cat is an
optional cosmetic epilogue; music-search is not replaced 1:1 (see losses);
clipper's button moves to R2; screen-toolkit stays external tools.
**Fallback:** restore the recorded persistent config inverse, then run
`noctalia msg bar-show` for the current session.

### R2 — Launcher

**Goal:** `Mod+Space` opens a Celestina launcher: the app grid actually used —
pins, categories, fuzzy search, `kitty -e` for `Terminal=true` — plus a
separate clipboard-history panel using the backend selected in the work order.

- [ ] Pure desktop-entry index + fuzzy match/rank module in
      `celestina-shell-core` (usage boost optional), unit-tested; `.desktop`
      scanning generalizes the
      `siderita/src/apps.rs` precedent instead of duplicating it — compare
      before writing
- [ ] On-demand keyboard overlay surface (`KeyboardInteractivityOnDemand`,
      overlay layer) opened via `celestina msg launcher-toggle`; Escape and
      focus-loss dismissal; full keyboard operation + AT-SPI before it is
      called done (`PickerWindow`/`CelestinaModalLayer` patterns)
- [ ] Pinned apps, categories, launch (spawn detached); a launch is a request —
      failures surface, they don't vanish
- [ ] Clipboard-history backend decision closed in the work order and its
      selected implementation delivered; clipper's notecards/pins remain the
      named loss recorded below
- [ ] Web-search provider (open URL in the default browser); emoji/kaomoji/
      unicode providers are optional later, never gates
- [ ] Integrated R2 exit accepted on the real session per the work order;
      record dated launcher/clipboard/accessibility evidence here

**Gate:** `Mod+Space` rebinds to the shell; Noctalia's launcher and clipper go
unused. **Fallback:** the old bind is one line in `config.kdl`.

### R3 — Session verbs: OSD, night light, caffeine, DPMS, composed lock

**Goal:** the keyboard-driven session verbs stop passing through Noctalia; the
shell answers them and shows truthful OSD.

- [ ] Volume/brightness keybinds route through `celestina msg` verbs that
      apply the change and raise the OSD (today binds hit `wpctl`/
      `brightnessctl` directly and the OSD merely observes); the OSD is an
      overlay surface, top-right, ~2 s, honoring `reducedMotion`
- [ ] Night light holds the lived fixed 2700 K using the settled execution
      default in the work order and releases gamma cleanly on exit
- [ ] Caffeine: shell-owned idle-inhibit toggle; the idle chain itself
      (lock 300 s → screen off → suspend) lands disabled-by-default,
      mirroring the lived autostart `caffeine-enable`
- [ ] `celestina msg session lock-and-suspend` composes a chosen, verified
      external locker selected under the work order's author gate + logind
      suspend; `dpms-off` maps to `niri msg action power-off-monitors`
- [ ] Autostart drops `noctalia msg caffeine-enable` /
      `nightlight-force-toggle`; `Mod+Shift+Escape` / `Mod+Shift+A` rebind to
      the shell
- [ ] Integrated R3 exit accepted on the real session per the work order;
      record dated OSD/idle/DPMS/lock-and-suspend evidence here

**Gate:** Noctalia's OSD, night light, idle and lock paths off; it still
serves notifications, panels, wallpaper and Polkit. **Fallback:** each path is
a config flag plus a bind line, both in git-tracked dotfiles.

### R4 — Notifications

**Goal:** the shell is the session's freedesktop notification server.

- [ ] Pure notification state-machine module in `celestina-shell-core`: ids,
      `replaces_id`, expiry, urgencies, actions and capabilities, unit-tested
      against the spec
- [ ] Notification bus/state producer delivered through the R1-A runtime; icon
      bytes are treated as hostile (size caps, signature checks, disposable
      cache)
- [ ] Toasts bottom/compact (the lived config); history capped like
      `RecentLog`; DND verb; the unread badge fills its R1 bar slot
- [ ] Handover: Noctalia's daemon disabled first (`enable_daemon = false`),
      then the shell claims `org.freedesktop.Notifications`. `magnetitad`'s
      `notify.rs` (Notify / replaces_id / CloseNotification) is the in-house
      consumer that must keep working — its behavior is part of this phase's
      evidence
- [ ] Integrated R4 exit accepted on the real session per the work order;
      record dated handover and Magnetita compatibility evidence here

**Gate:** Noctalia notifications off. **Fallback:** flip `enable_daemon` back.

### R5 — Control center, session menu, weather & calendar

**Goal:** the panels behind the bar's clicks and the session menu are
Celestina's; this is the first multi-provider write surface.

- [ ] Control center panel: the quick toggles actually used (network,
      bluetooth, night light, caffeine, DND, power profile) + audio card
      (PipeWire outputs/inputs/streams — a write-capable surface), brightness
      sliders, media and sysmon cards; `ListSection`/`CelestinaSwitch` per
      DESIGN.md, which reserves them for exactly this
- [ ] Session menu: lock / suspend / reboot / logout / shutdown with the lived
      numbered shortcuts and countdown, over logind — actions are requests
      with visible outcomes
- [ ] Weather module in `celestina-shell-core` plus provider-helper IO
      (Open-Meteo, manual or IP location — a justified external dependency)
      feeding the R1 bar slot + a card; calendar month view. Account sync is
      excluded from R5 per the work order
- [ ] Shell settings persistence exactly per `magnetitad/src/settings.rs`
      (serde defaults, `atomic_file::replace`, persist-before-publish)
- [ ] Integrated R5 exit accepted on the real session per the work order;
      record dated control/session/persistence evidence here

**Gate:** bar mouse actions open Celestina panels; Noctalia's control center,
session and calendar panels go unused. **Fallback:** `noctalia msg panel-open`
still answers while it runs.

### R6 — First-party lock & idle (author gate: security)

Lock stays composed from an external locker (R3) until this phase is
explicitly opened by the author — owning lock/auth flows was a standing
non-goal and the caution stands.

- [ ] ext-session-lock + PAM locker: every output covered; no bypass on crash
      (the compositor keeps the session locked if the locker dies); hotplug
      under lock; lock-on-suspend via a logind `PrepareForSleep` inhibitor
- [ ] The lived look: blurred/tinted capture, visible password characters,
      session buttons; no media controls, countdown or fprintd (unused)
- [ ] Idle chain revisited with real timings once caffeine-off living exists
- [ ] Integrated R6 security exit accepted on real Niri per the work order;
      record dated credential/crash/hotplug/suspend evidence here

**Gate:** the external locker becomes the fallback, not the default.

### R7 — Wallpaper & look

- [ ] Per-output wallpaper layer surfaces (crop fill; manual random from
      `~/Imágenes/Fondos`); the Niri backdrop layer-rule moves from Noctalia's
      namespace to the shell's; the disc-style transition is optional and
      respects `reducedMotion`
- [ ] The `Settings` portal served by the shell (CP2's table) + gsettings dark
      sync, replacing `syncGsettings`; the session is permanently dark today,
      so this is one value served truthfully
- [ ] Replace the frozen `noctalia.kdl` include with colors stated from
      `CelestinaTheme` — a one-time generation, documented; no template
      runtime
- [ ] Wallpaper-derived palettes are **not** replaced: the lived scheme is
      monochrome-on-dark, visually adjacent to the sealed CelestinaTheme dark
      scheme; an accent-from-wallpaper pipeline is a someday decision
- [ ] Integrated R7 exit accepted on the real session per the work order;
      record dated wallpaper/portal/login/rollback evidence here

**Gate:** Noctalia wallpaper and theming off.

### R8 — Polkit, dock, and Noctalia leaves

- [ ] Polkit authentication agent selected under the work order's author gate,
      installed and accepted; any first-party agent remains a separate explicit
      authorization
- [ ] Dock decision closed in the work order: if retained, its per-output
      implementation and interaction evidence are accepted; if dropped, the
      decision closure is recorded there without leaving an impossible checkbox
      here
- [ ] Remove `noctalia` from autostart; archive `~/.config/noctalia`;
      uninstall `noctalia-git` only after at least seven consecutive daily
      sessions complete without a Noctalia process or an unrecorded fallback
- [ ] Integrated R8 exit accepted after the seven-session soak per the work
      order; record dated Polkit, conditional dock, reboot and rollback evidence
      here

**Gate:** the session runs no Noctalia process; every remaining external tool
is named in this roadmap. **Fallback:** reinstalling the package is one
command; the archived config restores the old session.

### R9 — Greeter (someday; default: keep it)

`noctalia-greeter` is a separate package (greetd + its own bundled wlroots
compositor) that works without the shell; only the polkit-gated appearance
sync goes unused after R8. Default decision: **keep it**. If it ever
regresses: regreet/tuigreet. A first-party greeter means owning a
display-manager-grade compositor and is not planned.

### Not replaced 1:1 — named losses and where they live instead

| Today (Noctalia plugin/feature) | After the plan |
|---|---|
| music-search (YouTube search + mpv library in the bar) | not replaced; external mpv/playerctl workflows — Fluorita-shaped territory if it ever comes in-tree |
| clipper notecards / pincards | the clipboard backend chosen in R2 keeps plain history; notes and pins are lost unless a small file-backed flow is requested |
| catwalk cat | optional cosmetic widget someday; never a gate |
| screen-toolkit (annotate, pin, QR, measure, webcam, palette) | one external tool per function (`satty` for annotate, `niri msg pick-color`, existing grim/tesseract OCR bind) — or dropped; record which after living without |
| screen-recorder plugin | `gpu-screen-recorder` CLI directly (the plugin is already unplaced) |
| giphy / kaomoji / unicode / emoji providers | optional launcher providers later |
| NoctaliaPerformance toggle | obsolete — nothing heavy left to disable |
| plugin runtime, plugin store, community palettes, app templates, desktop widgets | never — non-goals |

## Non-goals

Do not chase upstream Noctalia's full surface — the parity target is the
author's lived subset, inventoried above. Do not support another compositor,
build a plugin framework/runtime, plan other Celestina apps here, fork Niri,
overwrite unrelated dotfiles, or add a shell surface the inventory does not
justify. Security-sensitive lock and Polkit flows stay composed from external
tools until their phases (R6, R8) are explicitly opened by the author — never
implemented as a side effect of another phase.
