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
active-window snapshots. `ProtocolDecoder` owns bounded line framing and
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
- [x] Reproducible Qt/LayerShellQt dev environment with recorded tool versions
      (README's toolchain table, 2026-07-30: declared minimums beside the
      versions actually built on; the KF6 floor now states the 6.19 the blur
      path needs instead of accepting any KF6)
- [x] Verify one panel per output with correct geometry, exclusive zone, and **no keyboard focus** on real Niri, beside Noctalia
      (author-observed 2026-07-31 — see the live acceptance below)
- [x] Verify invalid imports / root / layer setup fail visibly and non-zero
      (2026-07-30, all three exercised — see the failure-path evidence below)
- [x] Consume the canonical CelestinaStyle source for both panel and chooser;
      CMake and runtime provision the same `CelestinaStyle` URI alias. No inline
      palette or local style copy remains; an installed module belongs to STYLE-D
- [x] Rust helper + bounded QProcess/decoder boundary: deterministic shutdown,
      capped framing, invalid-state clearing and restart backoff, with Rust
      reduction tests and a focused QtTest decoder target
- [ ] Build, startup and resource baselines (artifact size, start time, PSS/RSS, wakeups, GPU cost)

**Failure-path evidence (2026-07-30).** Each mode was provoked and each exited
non-zero with a message naming what was wrong; every temporary edit was reverted
and the panel maps again afterwards. *Layer setup:* on a platform without layer
shell the shell now refuses before mapping anything — "needs a Wayland session
with layer-shell support; this one reports the platform plugin xcb", exit 1.
This was found doing the opposite: LayerShellQt only logs when it declines, Qt
maps an ordinary window, and the host went on to report "panel mapped on output
HDMI-A-1" — a claim it could not support. `layerShellSupport()` now decides,
with Qt's offscreen platform named explicitly as headless so nothing observed
under it is mistaken for compositor evidence, and unit-covered for wayland,
wayland-egl, offscreen, xcb, minimal and an empty name. *Invalid import:* an
unresolvable module in `Panel.qml` gave "could not load the panel component:
module ... is not installed", exit 1. *Invalid root:* a root that fails to load
gave the same load error, and a root that loads but is not a window reached the
host's own guard — "Celestina's panel component is not a window" — exit 1.
**Not evidence of** anything a compositor decides; these are failure paths, and
the successful-mapping checks remain the live acceptance.

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
- [x] Focus requests show pending / failed / confirmed (a click is a request, not proof of success).
      Pending → confirmed observed on the real session 2026-07-31; the failing
      path has automated and adapter-level evidence, not a live one
- [ ] Accept on real Niri that the panel survives IPC/helper loss and restart
      without stale state. **Half accepted 2026-07-31:** killing the helper
      cleared the workspaces and the panel recovered on its own. Restarting the
      compositor itself is the untested half and keeps this box open

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
      real hotplug acceptance remains separate. *A tiling compositor decides the
      size* was true of the card but not of its contents: the card clamps to the
      window, yet the row of screens kept its own fixed height while the buttons
      stay anchored to the foot, so a window shorter than the card asked for had
      the row sitting on top of Cancelar/Compartir (measured: 81 px of overlap in
      a 220 px window). The row now yields — it takes the height it wants or
      whatever the buttons leave, whichever is smaller, keeping the same
      breathing space the card reserves when it asks for its height. QuickTest
      covers both directions (squeezed and roomy)
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

- [x] Extract `PanelManager` from `main.cpp` into `src/panelmanager.*` without
      introducing the shared abstraction yet
- [x] Command channel and transient `celestina msg <verb>` CLI delivered under
      the stable-owner/versioned-interface contract in the work order; every
      later keybind routes here
- [x] Popup path selected by the work order is proven with one real
      `GlassContextMenu` anchored from a panel region on Niri
- [x] After the popup proves a second real consumer, extract only the shared
      surface intersection into `src/surfacemanager.*` so OSD/launcher/popup
      surfaces do not copy `ensurePanel`
- [ ] Integrated R0 exit accepted on real Niri per the work order; record dated
      evidence here before closing the phase. **Nearly complete (2026-07-31):**
      six of the eight named checks passed on the real session; a visibly
      failing or timing-out request and the resource baseline are the remainder

**R0-A evidence (2026-07-30).** The panel lifecycle moved verbatim to
`src/panelmanager.{h,cpp}` (registered in `CMakeLists.txt`); `main.cpp` keeps
bootstrap, the style import alias and `--pick-output`, and now passes the
`CELESTINA_REDUCED_MOTION` reading it already performed for the chooser into the
manager instead of the manager reading the environment itself. No layer-shell,
blur or provider behaviour changed. Passed: the suite architecture guard, Rust
fmt/Clippy `-D warnings`/tests, CMake configure and build, `all_qmllint`, and
both CTest targets. **Not evidence of** live mapping, geometry, exclusive zone,
focus or hotplug — those stay pending for R0-E/F on the real session.

**R0-B evidence (2026-07-30).** The focus-request path landed end to end: the
adapter grew a bounded stdin reader, a 32-slot command queue, one action worker
on its own short-lived socket and a shared writer that serializes every frame;
snapshots now carry Niri's workspace id as a decimal string (a `u64` would lose
precision as a JSON number). `WorkspaceFocusRequests` owns the policy — an ack
is acceptance, not arrival, and only a later snapshot reporting the requested
workspace active on the requested output confirms it — and `NiriClient` drives
it with a generation per helper process. The strip's pills are click targets
that publish `requestState`. Passed: 9 Rust tests (framing recovery, command
parsing, id serialization), 8 new QtTest cases for the request policy, the
existing decoder and chooser tests, fmt, Clippy `-D warnings`, build and
`all_qmllint`. A live adapter run against the running compositor discarded an
oversized command, answered an unknown kind and an invalid workspace with
`failed` frames carrying their own request ids, emitted a real snapshot with
string ids, and exited on stdin EOF. **Not evidence of** the pending →
confirmed path on the real session: no valid focus action was ever sent to the
author's compositor, so the S1 checkbox above stays open until R0-E/F accepts
it live, together with failure and timeout.

**R0-C evidence (2026-07-30).** Panel mode owns `org.celestina.Shell` and
exports `org.celestina.Shell1` at `/org/celestina/Shell1` with `GetState`,
`Command`, `Changed` and `CommandResult`; every payload carries `version`. The
name is claimed before a single surface is mapped, so a second panel-mode
process defers instead of flashing a duplicate panel, and a session without a
bus keeps its panels and loses only the channel. `celestina msg` is a transient
client with no window and no Wayland connection; `focus-workspace` is the first
verb and routes to the R0-B request path. Passed: two new QtTest targets — verb
and option parsing (typing, duplicates, bounds) and the exported interface over
a real session bus (introspected interface and member names, the state version,
unknown-verb, invalid-argument and never-sent-request refusals) — plus the
existing four targets, fmt, Clippy, build, `all_qmllint` and the suite guard.
Live, against a real host started offscreen so nothing was mapped on the
session: `msg get-state` returned the running compositor's real workspaces and
exited 0; an unknown verb, a workspace that does not exist and a second
panel-mode process each failed visibly and exited non-zero. Against a stand-in
owner of the name, the client reported pending then `confirmed` (exit 0),
`failed` (exit 1), an unresolved request after its timeout (exit 1) and the
owner leaving the bus mid-request (exit 1). **Not evidence of** the channel
under real panels, of any command that changes compositor state — no valid
focus action was sent — or of `Changed` under a live session; those belong to
the R0-F acceptance.

**Real-session sighting (2026-07-30).** The author reports that everything
renders correctly on the live session, menu included, after the sizing fix.
That covers appearance only: geometry measurements, exclusive zone, focus
behaviour, keyboard, dismissal, IPC restart and the resource baseline remain
the integrated R0 exit, unobserved.

**R0-D evidence (2026-07-30).** Both popup candidates exist, side by side and
unfactored: an `xdg_popup` of the panel and its own anchored layer surface
(overlay layer, on-demand keyboard, exclusive zones ignored so its margins are
plain screen coordinates). Each owns surface mechanics only and adopts a
content window it never inspects — the content is `qml/PanelMenu.qml`, a real
shared `GlassContextMenu` with one item per workspace, and every item is the
same focus request a click on the strip makes. The menu is opt-in through
`CELESTINA_POPUP_CANDIDATE` (`xdg` / `layer`): with nothing selected the panel
has no context menu at all, because an unproven surface must not join the daily
panel and a control that does nothing would be a fake control. Passed: a new
offscreen QtTest target — parenting, flags, screen and position per candidate,
refusal to open twice, reopening, external dismissal reported and cleaned up,
no window left behind, the environment selection, and the real menu file loaded
from source and adopted by a candidate — plus the other five targets, fmt,
Clippy, build, `all_qmllint` and the suite guard. **Not evidence of** anything a
compositor decides: placement, keyboard, dismissal and focus return are exactly
what R0-E must observe on real Niri. One fact the probe already has to account
for, observed while testing under Wayland: Qt refuses to create a grabbing
`xdg_popup` whose parent has not received input, and the panel takes no
keyboard input by design.

**R0-E evidence (2026-07-30).** The author ran both candidates on the real
session. Both map, both render the workspace menu and both answer the pointer
identically — no observable difference on those axes, which also resolves the
earlier offscreen warning: an `xdg_popup` of the panel *does* map, because a
right-click means the panel has received pointer input. Keyboard operation was
not separately exercised, and that is where the tie is broken on protocol rather
than taste: a popup of a `KeyboardInteractivityNone` surface cannot inherit a
keyboard focus its parent refuses, while a layer surface asks for its own. The
choice is therefore the anchored layer surface, recorded with its falsifier in
the work order's settled table; the `xdg_popup` candidate is retired. **Not
evidence of** keyboard operation, dismissal-on-outside-click or focus return
being verified — the integrated R0 exit still owes those.

**R0-F extraction (2026-07-30).** With a second surface proven, the
create/configure/map intersection is now `src/surfacemanager.{h,cpp}`: a
`LayerSurfaceSpec` plus `mapLayerSurface`, carrying only fields the panel and
the menu both set with *different* values (scope, screen, anchors, margins,
desired size, exclusive zone, layer, keyboard interactivity, activate-on-show,
close-on-dismissed, focus). The panel and the menu now describe themselves
through it instead of configuring LayerShellQt by hand, and the OSD and launcher
surfaces of R2/R3 add a description rather than a copy of `ensurePanel`. On the
author's decision the same day, the menu is part of the panel rather than an
opt-in probe: right-clicking a workspace pill opens it, and
`CELESTINA_PANEL_MENU=0` is the recorded way back. It adds no action that is
only reachable through it — every workspace it offers is the press action of its
own pill — because Qt Quick exposes no assistive show-menu action; what the
panel's mouse gestures open beyond this stays R1/R5 work.

**The menu's future (2026-07-30).** The author keeps the gesture and wants its
content to become a view of the workspace's *windows* rather than a list of
workspaces. That is W1 in the W-list below, deliberately scheduled after the
replacement: the menu stays as it is until then, and W1 does not enter any
R-phase's gate.

**Menu sizing defect and fix (2026-07-30).** On the real session the menu mapped
as a sliver and every click in it landed on the first workspace. Measured, not
guessed: the window sized itself to the laid-out menu while `Popup` fits itself
to its window minus its margins, so the pair shrank one margin per pass —
232×116 → 232×68 → 232×20 — and a 20 px surface has room for one row. The window
now takes its height from the menu's content-derived `implicitHeight` and its
width from the width the shared component fixes, with the popup's
window-relative clamp disabled because the surface exists only to carry it, and
the surface is inset by the room `GlassSurface` draws its shadow in. Two latent
faults were closed with it: the surface never stated a size, and a layer surface
anchored to two adjacent edges may not leave its size to the compositor —
observed directly as `zwlr_layer_surface_v1: width 0 requested without setting
left and right anchors` when a test ran against the live compositor; and a menu
near an output edge is now placed where it fits whole. A regression test pins
the floor at one usable row per workspace and was confirmed to fail on the old
sizing. **Not evidence of** the corrected menu on the real session: the geometry
is measured offscreen, and how it looks and dismisses under Niri is the author's
next look.
Passed after the extraction: the six CTest targets (the surface recipe and menu
lifetime among them), the Rust tests, fmt, Clippy, build, `all_qmllint` and the
suite guard; the panel still maps offscreen through the extracted path. The rest
of R0-F — geometry, exclusive zone and focus on real Niri, CLI → host under real
panels, live pending → confirmed, adapter and compositor restart, invalid setup
failing visibly, and the resource baseline — remains the open integrated exit.

**Live acceptance (2026-07-31, author-observed).** Run from `scripts/run.sh` on
the real session with Noctalia's bar hidden, against this checklist: one panel
per output with correct geometry and exclusive zone and no focus theft while
typing; the context menu driven by arrows and Enter, dismissed by Escape and by
a click outside, with focus returning where it was; the second-aligned Spanish
clock centred and the wheel stepping through workspaces on the pills; a click
showing pending before the compositor confirmed it; `celestina msg get-state`
answering with real panels mapped; and killing the Niri helper clearing the
workspaces and recovering unattended. The author reports all of it working.

This is an author's observation against an enumerated list, not a measurement:
nothing here was captured, timed or logged by the shell itself. **Still owed by
the integrated exit:** a request that visibly fails or times out on the real
session, and the CP0 resource baseline (start time, PSS/RSS, wakeups, GPU cost),
which needs the running process. Compositor restart — as opposed to helper
restart — also remains untested.

**Gate:** nothing retired yet; Noctalia untouched.

### R1 — The bar

**Goal:** the panel reaches parity with the bar configuration actually lived
in — the widget list above, not upstream's 33 widget types — so Noctalia's bar
hides permanently. The work order owns the Rust/Qt runtime contract and each
slice's implementation details; this file owns their status.

- [x] R1-A — provider runtime boundary defined in the work order landed and its
      deterministic lifecycle tests pass before any production provider
- [x] R1-B — composable flanks, workspace gestures and lived clock format;
      future caffeine/notification/weather extension points exist structurally
      but paint no visible placeholders
- [x] R1-C — bounded CPU/RAM provider plus the existing external screenshot flow
- [x] R1-D — desktop MPRIS mini (artwork/title/progress) using the settled
      execution default in the work order; phone media remains on `Devices1`
- [x] R1-E — truthful volume/mic state using the settled execution default;
      middle-click still opens the external mixer
- [x] R1-F — read-only NetworkManager/BlueZ indicators plus confirmed
      power-profile indicator/cycle
- [ ] R1-G — per-output DDC brightness, with coalesced scroll steps and unknown
      state instead of GUI blocking or stale values
- [ ] R1-H — StatusNotifierItem host + DBusMenu drawer, including passive items,
      landed as the phase's separate final provider slice
- [ ] R1-I — integrated bar exit accepted on the real session and exact
      persistent hide/rollback evidence recorded here

**R1-B evidence (2026-07-31).** The panel's sides are ordered rows now, not
anchors: `PanelFlank` grows from its own edge, clips against the space the
centred clock leaves, and a later widget joins by being added in the order it
should appear. That is the whole extension point for caffeine (R3), the unread
badge (R4) and weather (R5) — structural, painting nothing until something real
is there. The clock carries the lived format `HH:mm:ss - MMMM - dddd dd`,
realigned to each second boundary rather than to the minute, and asks for its
month and weekday names in Spanish instead of inheriting whatever locale the
process started with: rendered offscreen it read `13:38:01 - julio - viernes 31`,
where before the same code printed `July - Friday`. The strip gained
scroll-to-switch with wrap over both mouse and touchpad, and it steps from the
newest request still in flight rather than from stale state, so a burst of
notches advances by that many workspaces instead of asking twice for the same
one. Assistive technology reaches the same step through `ScrollUp`/`ScrollDown`
actions; the panel surface still takes no keyboard, so the compositor's binds
remain the keyboard route. Passed: five new Qt Quick Test cases for the step
arithmetic (origin, both wrap ends, burst, foreign output, empty output), 7/7
CTest, `all_qmllint` clean and the suite guard. **Not evidence of** how any of
it feels: the offscreen render proves layout, format and language, not pointer
behaviour, elision at real widths or the panel on three real outputs.

**R1-C progress — CPU and memory (2026-07-31).** The first real provider joins
the R1-A runtime, and with it the helper is wired into the panel for the first
time. Reading `/proc` is IO; deciding what it means is `celestina-shell-core`'s
`sysmon`, all of it functions over text: the aggregate `cpu` line counts idle
and iowait as idle and tolerates a kernel adding a column; used memory is total
minus *available*, not minus free, so reclaimable cache is not reported as
used; a percentage is whole-number integer arithmetic end to end. CPU is a rate,
so the first sample publishes nothing rather than inventing a number from one
reading, and a `/proc` that becomes unreadable withdraws the provider instead of
freezing its last value. The thresholds are the ones already lived with —
`elevated` at 80, `critical` at 90, matching `cpuWarningThreshold` and
`cpuCriticalThreshold` in the Noctalia settings — and the panel maps those
states to colour rather than deciding them. A click opens the system monitor
through a typed helper command, launching `missioncenter`: the one entry of the
author's configured `externalMonitor` chain that is actually installed. The
panel does not become a system monitor.

Evidence: nine new crate tests (unreadable `/proc` refused rather than guessed,
the first-sample rule, a reset sampler, cache not counted as used, a prefix
field not answering for the real one, percentages never leaving 0–100), 35 crate
tests in total, the helper run against real `/proc` publishing `cpu`/`ram` with
their load states every two seconds, an unknown verb answered
`'sysmon' does not serve the verb 'nope'`, the host spawning the helper and it
exiting with the host, and an offscreen render showing `CPU 84 %` in the
elevated colour beside a muted `RAM 24 %`. 7/7 CTest, `all_qmllint`, Clippy
`-D warnings` and the suite guard. **Not evidence of** the widget on the real
panel, of the click actually opening `missioncenter`, or of what the poll costs
in wakeups.

**R1-C — the capture button (2026-07-31).** The button asks Niri to open its own
screenshot UI, which saves where `screenshot-path` already points. The shell
captures nothing: a second, worse screenshot tool is exactly what the slice was
told not to build. It travels the R0-B command path — the helper that owns the
compositor socket gained a `screenshot` command, and every compositor action now
goes through one `perform` so "accepted" means the same thing for all of them.
Nothing confirms it afterwards, because the compositor takes over the screen, so
the button reports only a request it could not make: a refusal, or a helper that
went away, paints it destructive for a moment and then it goes quiet again.

**A correction to the session facts:** the plan recorded screenshots among the
"keybinds that already bypass Noctalia". There is no screenshot keybind in
`config.kdl`, nor in its include — only `screenshot-path`, which says where a
capture would be saved. The lived screenshot flow was Noctalia's bar button,
which is why this button replaces it rather than duplicating a bind.

Evidence: the command parses to its own request id (new adapter test, 10 in
total), the panel builds and lints clean, and an offscreen render shows the
button in the right flank. **Not exercised:** pressing it. Doing so opens Niri's
screenshot UI over whatever is on screen, which is not something to trigger on
someone's session unasked — it is the first thing to try on the next live run.

**R1-D progress — what the desktop is playing (2026-07-31).** The suite already
had an MPRIS vocabulary and a parser for playerctl's metadata format; the parser
was private to `magnetitad`, so a second reader would have been a second dialect
of the same thing. It moved to `magnetita-core::mpris` beside the `PlayerState`
it produces, with the format string it matches, and the daemon now calls it
there: one recipe, two consumers — the phone bridge and the panel — and 86 core
plus 59 daemon tests still pass over it.

The provider polls rather than following: one `playerctl` spawn every two
seconds while a player exists, backing off to five when none does, each bounded
by a deadline that kills a player which will not answer. Position rides in the
same call as microseconds appended to the shared format, so a poll is one
subprocess and the arithmetic stays integer. `playback_progress()` decides what
a position means, so a live stream is not drawn as a track 40 hours long.

Two things the first real coming-and-going provider exposed. The runtime could
only *retire* a provider, so a media provider with no player unregistered
itself — and a transport command then came back "no provider named 'media' is
running in this helper", which was true for a second and misleading as an
answer. `ProviderRuntime::withdraw` now drops a value while the provider keeps
carrying itself, and the refusals read `no player is running` and `'media' does
not serve the verb 'Rewind'`. And the real host caught a QML binding that read
the position while nothing was playing: `visible: false` does not stop a binding
from evaluating, so the guard moved into the expression.

Evidence: 36 crate tests, the helper answering both refusals correctly with no
player present, the widget rendered offscreen with an elided title over a
proportional progress line, and the real host starting clean. **Not exercised:**
anything with music actually playing — this machine has no MPRIS player running,
so title, position and the play/pause round trip are the first things to watch
on the next live run.

**R1-D — cover art (2026-07-31).** The suite already decided what counts as an
image worth opening: a bounded size and a known signature, written for covers
arriving from a phone. Those two answers are not KDE Connect's, they are the
suite's, so they moved to `celestina-core::image` and the daemon now asks them
there — one rule, two sources of untrusted bytes.

The panel accepts only a cover it can actually check. A `file://` path is
stat-ed, bounded and read for its signature before Qt is ever pointed at it, and
the decode is capped to twice the drawn size, because a checked signature says a
file *starts* like an image, not that it is a sane thing to expand. Anything
else shows no cover: an `https://` cover — Spotify's, typically — would have to
be downloaded, and a shell that fetches what a media player tells it to is a
shell with a fetcher in it. That is a named loss, not an oversight.

The cache `magnetitad` keeps is deliberately *not* copied here. It exists
because bytes arrive over a socket with no home on disk; a local cover already
has one, and copying it into `$XDG_RUNTIME_DIR` would duplicate a file to gain
nothing.

Evidence: five new adapter tests — a real PNG accepted, a renamed archive
refused, `https://` and `data:` refused, a relative and a missing path refused,
and a file-sized cover refused before it is read — plus 12 core tests, the
daemon's 59 still passing over the shared checks, and an offscreen render with
the cover beside the title. **Not exercised:** a real player's cover, which
needs music playing.

**R1-E evidence (2026-07-31).** Volume and microphone come from `wpctl`, the
tool the session's own keybinds already use, and the panel uses the session's
own numbers: `0.05` steps with the `-l 1.0` ceiling the keys pass, because
overdrive is off in the lived configuration. The keys keep their larger step;
what the panel matches is the bar's. Middle-click opens `pavucontrol` — the one
mixer installed, since `qpwgraph` is a patchbay. Reading one `wpctl` line is
pure and lives in `celestina-shell-core::audio`, parsed as text with no float
anywhere: `wpctl` prints hundredths, so a level is already whole percent once
the point is read, and rounding a float would only be a way to lose it.

Two decisions worth naming. A muted device is not a device at zero, so the
number stays and says it is not being heard rather than pretending the level
moved; and an unreadable default device withdraws the widget instead of
claiming silence. The microphone is shown only when it is muted — the rest of
the time the panel says nothing about it rather than carrying a permanent
indicator for a state that is almost always the same.

It reads as text, not a glyph: the suite's icon catalogue is closed and
vendored from Lucide, and with no network to fetch the canonical speaker, a
hand-drawn one would have put non-canonical artwork into a set that is
canonical everywhere else. A number beside CPU and RAM is also the language
this panel already speaks.

Evidence: nine crate tests over the reading (hundredths, overdrive, muted, and
five things that are not a reading at all), 40 crate tests in total, and the
provider run against the real session — it published `volume 60`, `micVolume 70`
from this machine's actual devices, `toggle-mute` muted and restored the sink,
and an unknown verb came back `'audio' does not serve the verb 'explode'`. A
command re-reads the device immediately rather than leaving the panel a poll
behind its own click. Offscreen render shows the level and a struck-through
microphone. **Not exercised:** the wheel and the middle click on a real panel,
and what the two-second poll costs in wakeups — `pactl subscribe` is the
documented optimisation if that budget ever fails.

**R1-F evidence (2026-07-31).** Three readings, four installed tools, one slow
poll — network, Bluetooth and the power profile change rarely enough that five
seconds is generous, and each is its own provider so an unreadable tool takes
only its own widget away.

The network reading is the one that earned its design. This machine has both
cable and wifi connected, so "prefer ethernet" — the obvious guess — would have
been **wrong**: the default route goes through wifi. The panel therefore reads
`ip route show default` and reports the link actually carrying the session,
matched against NetworkManager's device list. With no default route there is no
link to report, because a connected device that carries nothing is not how a
session is online.

Bluetooth is read-only until R5's control centre, and speaks only when
something is connected — a powered adapter with nothing on it is not news.
Worth recording: on this session the one connected device is the phone
Magnetita already shows on the right of the same panel, so the indicator will
usually restate it. R5 decides whether it earns its place.

The power profile is a confirmed request, like every other action here: the
click asks the daemon for the next profile it offers and the panel paints what
the daemon reports next, never the profile it asked for. Exercised on the real
session — `power-saver` cycled to `performance` and was restored — and an
unknown verb came back `'power' does not serve the verb 'boost'`.

**The guard caught the cost of all this.** `provider_adapter.rs` reached 818
lines against an 800-line ceiling that admits no new exceptions, which is
exactly the alarm the contract describes: the file had grown a second, third
and fourth reason to change. It is now a directory — plumbing in `main.rs`,
one module per provider, and the tool-running they share in `tools.rs`, none
over 270 lines. Adding a provider is adding a module and a line, not editing a
loop that knows about all of them. The split was verified by behaviour, not by
compiling: the same five providers publish, and the same two refusals come back
word for word.

Evidence: 21 new crate tests over the three readings (route parsing with two
default routes, connection names containing spaces and colons, loopback refused
as a link, nameless Bluetooth devices, an indented `CpuDriver:` line refused as
a profile, cycling that wraps and refuses what it cannot place), 52 crate tests
in total, the helper publishing all five providers from this machine, 7/7 CTest
and the guard green. **Not exercised:** the click and the wheel on a real panel,
and how any of it looks while the network changes.

**Gate:** Noctalia's bar hidden permanently on all three outputs; Noctalia
keeps running headless for everything else. Named at the gate: the cat is an
optional cosmetic epilogue; music-search is not replaced 1:1 (see losses);
clipper's button moves to R2; screen-toolkit stays external tools.
**Fallback:** restore the recorded persistent config inverse, then run
`noctalia msg bar-show` for the current session.

**R1-A evidence (2026-07-31).** The runtime is complete and proven before any
provider joins it. `celestina-shell-core` owns the whole aggregate as pure
policy — bounded framing, one serialized writer, the provider envelope, command
parsing with refusals, coalescing, and `ProviderRuntime`, which decides which
providers a helper carries, that an identical value is not news, that a
withdrawn provider takes its value with it, that a new generation publishes
nothing of the previous one and that a burst becomes one frame per window. It
holds no thread, clock or pipe: 26 crate tests cover every rule without a
process. `celestina-provider-adapter` is the one binary that carries every
provider needing long-lived non-Qt IO — never one per widget — and is now only
threads and IO around that runtime. `ShellProvidersClient` is the single Qt
bridge, revalidating every bound on its side because a helper is a separate
binary that may be older or broken; seven refusal cases plus set-replacement,
generation and clearing are covered by a new QtTest target.

Run end to end: the helper answers an unknown provider by name, refuses a
malformed one with its request id, logs unreadable input it cannot answer, and
exits 0 on stdin EOF. Driven from the real Qt client over a real pipe: the first
frame arrived (`available=true`, empty set), a command came back
`failed — no provider named 'sysmon' is running in this helper`, and destroying
the client left no helper process behind. Also in this change: the line framer
was renamed `ProtocolDecoder`, since two clients now share it and its name said
"Niri". 7/7 CTest, 9 adapter tests, 26 crate tests, fmt, Clippy `-D warnings`,
build, `all_qmllint` and the suite guard. **Not wired into the panel by design:**
no production provider or widget exists yet, so nothing spawns the helper in a
daily session — R1-C is its first consumer.

**R1-A groundwork (2026-07-30).** `celestina-shell-core` exists and is registered
in the workspace, holding what a helper and its Qt host share: bounded line
framing, one serialized writer, the provider envelope (bounded identity and
payloads, generations, and "the same value is not news" so an idle panel stays
idle) and the command vocabulary whose refusals carry the request id whenever
one can be recovered. It was *extracted, not invented*: the Niri adapter's
private framing and writer moved into it and the adapter now consumes it, so the
crate shipped with a real consumer and one copy fewer. 20 crate tests plus the
adapter's 9 pass, with workspace fmt, Clippy `-D warnings`, the shell's CMake
build, CTest and the suite guard. **Still owed by R1-A:** the aggregate
`celestina-provider-adapter` binary and the bounded `ShellProvidersClient`
QObject, with the fixture-snapshot, command-rejection, provider-disappearance,
generation-reset and deterministic-shutdown tests the work order names. No
production provider or widget has landed, by design.

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

## Beyond the replacement — wanted, not owed (W-list)

Ideas the author asked for that replace nothing Noctalia does. They are kept
here, apart from R0–R9, for one reason: **nothing on this list may gate,
reorder or expand a replacement phase.** A W entry is scheduled after R8, may
reuse contracts an earlier phase happens to land, and never widens that phase's
exit test. Each entry names what already exists, what does not, and the question
that decides its shape — so a future session can size it without rediscovering
the constraints.

### W1 — Workspace overview (author idea, 2026-07-30)

Opening a workspace's *windows* from its panel pill, laid out as tiles rather
than a text list — the arrangement macOS calls Mission Control — where picking a
tile focuses that window. The pill menu shipped in R0 is a placeholder for this
gesture; its content is superseded when W1 lands, and the gesture question
(secondary click vs. hover vs. long press, and what a primary click keeps doing)
is decided here rather than assumed.

**Already exists.** The layer-surface recipe (`LayerSurfaceSpec`, R0-F) maps the
surface; the request contract (R0-B) generalizes unchanged — a window focus is
`Action::FocusWindow { id }`, pending until a snapshot reports that window
focused, failed on timeout or refusal; the menu surface already owns a
transient surface's lifetime and dismissal.

**Does not exist.** The adapter reduces windows only as far as the active
title. The snapshot must carry per-workspace window identity — Niri's `Window`
offers `id`, `title`, `app_id`, `is_focused`, `is_floating`, `is_urgent` and a
`layout` with real tile and window sizes — as a backward-compatible extension of
the `snapshot` frame, and `celestina msg` gains a focus-window verb under the
existing rules.

**The question that decides its look:** a Wayland client cannot draw another
window's pixels. `wlr-screencopy` captures whole outputs and Niri's window
screenshot writes a file; neither is a live thumbnail source. So the honest
first version is icon-and-title tiles proportioned to each window's real
`layout` size — a truthful map of the workspace, not a preview wall. Whether
live previews are reachable at acceptable cost is a separate investigation, and
no design here may assume them.

## Non-goals

Do not chase upstream Noctalia's full surface — the parity target is the
author's lived subset, inventoried above. Do not support another compositor,
build a plugin framework/runtime, plan other Celestina apps here, fork Niri,
overwrite unrelated dotfiles, or add a shell surface the inventory does not
justify — the W-list above is the only channel for an author-requested addition,
and it stays behind the replacement rather than inside it. Security-sensitive lock and Polkit flows stay composed from external
tools until their phases (R6, R8) are explicitly opened by the author — never
implemented as a side effect of another phase.
