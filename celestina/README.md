# Celestina Desktop

The shell/session for a personal Niri (Wayland) environment. Its first product
is a small, truthful top panel with real Niri control. The destination (author
decision, 2026-07-29) is to replace Noctalia entirely in staged phases — bar,
launcher, session verbs/OSD, notifications, control center, lock, wallpaper,
Polkit — composing mature external tools as interim fallbacks and as the
permanent answer where noted; the plan lives in [ROADMAP.md](ROADMAP.md).

- **Role:** Niri shell / session (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust 2021 (`niri-ipc`) · C++20 · Qt 6.9+ Quick · LayerShellQt ·
  KWindowSystem (KF6, compositor blur) · Cargo + CMake
- **Consumes:** [magnetita-core](../celestina-rs/crates/magnetita-core/) — the
  suite's one MPRIS vocabulary and playerctl parser, shared with the phone
  bridge · [celestina-shell-core](../celestina-rs/crates/celestina-shell-core/) —
  what its Rust helpers share with the Qt host: bounded framing, one serialized
  writer, the provider envelope and the command vocabulary ·
  [celestina-style](../celestina-style/) — panel and chooser import
  the canonical source tree through URI-shaped aliases provisioned by CMake for
  lint and by the host at runtime. The style's font qrc is not bundled yet, so
  shell text falls back to the system face.

## Build / run

```sh
scripts/run.sh                             # build (Release) + activate the panel
cmake --build build --target all_qmllint   # QML lint (after a first build)
ctest --test-dir build --output-on-failure # decoder, focus policy, command line, D-Bus, chooser
```

The running shell answers on the session bus, and `celestina msg` is a
transient client of it — it never starts a shell and never claims the name:

```sh
celestina msg get-state
celestina msg launcher-toggle    # opens/closes the app launcher overlay
celestina msg clipboard-toggle   # opens/closes the clipboard-history overlay
```

Panel mode is layer-shell or nothing: on a platform that cannot carry a layer
surface the shell refuses to start rather than mapping ordinary windows and
calling them panels. Qt's `offscreen` platform is allowed and says so — it is
useful for headless checks, and nothing seen under it is evidence about a
compositor.

`scripts/run.sh` is the one script the shell needs: it builds and *activates*
the shell — maps the panel on every output — in the foreground (Ctrl-C to stop).
Unlike the apps it is not a launcher entry, so running it is activating it.
The CMake build compiles the pinned Rust Niri adapter automatically; no second
build command is required.

During development, keep Noctalia running and hide only its bar so launcher,
notification, idle, lock, Polkit, theme and greeter services stay available:

```sh
noctalia msg bar-hide
scripts/run.sh
noctalia msg bar-show
```

Celestina uses its own layer-shell namespace (`celestina-panel`) so Niri
rules and diagnostics can tell both shells apart.

Right-clicking a workspace pill opens the panel's context menu — jump to any
workspace on that output. It is on by default; `CELESTINA_PANEL_MENU=0` turns
it off if it ever misbehaves on a session. The gesture is settled; what it shows
is not — [ROADMAP.md](ROADMAP.md)'s W1 replaces the workspace list with a view of
that workspace's open windows, after the replacement phases.

## Toolchain

The declared minimums are the contract; the versions beside them are the
environment this shell was last built and run on (2026-07-30). A minimum only
moves when a newer API is actually used — the KF6 floor is 6.19 because that is
where `KWindowEffects` speaks the protocol niri answers, and an older one would
build while silently never reporting the blur effect as available.

| Tool | Declared minimum | Verified on |
|---|---|---|
| Qt (Quick, QuickControls2, DBus, Test, QuickTest) | 6.9 | 6.11.1 |
| LayerShellQt | 6.6 | 6.7.3 |
| KF6WindowSystem | 6.19 | 6.28.0 |
| CMake | 3.20 | 4.4.0 |
| Rust | 1.85 (`rust-version`) | 1.97.1 |
| `niri-ipc` | `=26.4.0` (pinned) | `=26.4.0` |
| Niri | — | 26.04 (8ed0da4) |
| C++ | C++20 | GCC 16.1.1 |

## Panel composition

The daily panel has three stable regions. The left side shows Niri's real
workspaces for that output plus the active window title; the clock remains
geometrically centred; phone state from `org.celestina.Devices1` stays anchored
to the right. A missing provider removes its data instead of leaving a stale or
simulated value. Clicking a workspace pill is a *request*: it shows pending
until Niri reports that workspace active on that output, and shows a failure
when the compositor refuses or never answers. Live acceptance of that loop on a
real session is still owned by the R0 exit test.

## Layout

| Path | Responsibility |
|---|---|
| `CMakeLists.txt` | Qt executable/module + LayerShellQt + KWindowSystem |
| `src/main.cpp` | process bootstrap, style import self-provisioning, `--pick-output` mode |
| `src/panelmanager.cpp`, `.h` | per-output layer-shell panel lifecycle: creation, hotplug and teardown, one surface per `QScreen` |
| `src/provider_adapter/` | the aggregate provider helper: one process for every bar provider needing long-lived, non-Qt IO. `main.rs` is the plumbing, `tools.rs` the bounded way it runs the session's own tools, and one module per provider (`sysmon`, `media`, `audio`, `session`, `launcher`, `clipboard`) |
| `src/providerstates.cpp`, `.h` | host-side validation of what that helper publishes, and the provider state QML reads |
| `src/shellprovidersclient.cpp`, `.h` | the single Qt bridge to the provider helper: process lifecycle, framing, confirmed marshaling and bounded commands |
| `src/niri_adapter.rs` | pinned Niri event stream, state reduction and reconnect loop; emits narrow workspace snapshots and performs the host's bounded focus requests on a separate short-lived socket |
| `src/protocoldecoder.cpp`, `.h` | bounded line framing for the Rust helper; discards an oversized frame through its newline and recovers for the next one |
| `src/niriclient.cpp`, `.h` | `QProcess` lifecycle, bounded JSON validation and Qt marshaling of confirmed snapshots on the GUI thread |
| `src/shellservice.cpp`, `.h` | the session's only command channel: owns `org.celestina.Shell`, exports `org.celestina.Shell1` (`GetState`, `Command`, `Changed`, `CommandResult`) and makes panel mode single-instance |
| `src/shellcommandline.cpp`, `.h` | pure, bounded parsing of `msg <verb> [key=value ...]` into the `a{sv}` the channel takes |
| `src/shellclient.cpp`, `.h` | the transient `celestina msg` client: answers on stdout, exits non-zero on rejection, bus loss or an unresolved request |
| `src/workspacefocusrequests.cpp`, `.h` | pure policy for focus requests: pending until a snapshot reports the requested workspace active, failed on rejection or timeout |
| `src/surfacemanager.cpp`, `.h` | the shared layer-surface recipe: a `LayerSurfaceSpec` plus `mapLayerSurface`, holding only what the panel and the menu both set differently, and the platform check that refuses a session without layer shell |
| `src/panelmenusurface.cpp`, `.h` | the menu's surface and lifetime: adopts a content window, maps it through the shared recipe, cleans up a compositor dismissal |
| `src/panelmenucontroller.cpp`, `.h` | builds the menu window and routes an item back to the same focus request a click makes |
| `src/overlaysurface.cpp`, `.h` | the third surface kind: a centered, on-demand-keyboard overlay opened from a keybind rather than a click — empty `LayerSurfaceSpec::anchors` is what centers it; mechanics only, shared by the launcher and the clipboard history |
| `src/overlaycontroller.cpp`, `.h` | loads one QML component, toggles its `OverlaySurface`, and nothing else — the launcher and the clipboard history are two instances of this one class; each component talks to `providerSource` directly like any bar widget |
| `src/panelblurcontroller.cpp`, `.h` | per-surface KWindowEffects capability/retry/geometry lifecycle and the explicit readable fallback state |
| `src/trayitems.cpp`, `.h` | what a StatusNotifierItem is once distrusted: registrations, absent properties, title fallback, bounded lists |
| `src/traywatcher.cpp`, `.h` | the panel's SNI host: registers with the session's watcher, reads items asynchronously, resolves their icons and publishes them |
| `src/trayicons.cpp`, `.h` | foreign icons: where the session's themes are, and how another application's raw pixels become an image |
| `src/trayiconprovider.cpp`, `.h` | serves `image://tray/…` from what the host already decoded |
| `src/traymenu.cpp`, `.h` | what another application's DBusMenu means: mnemonics, separators, headings, and the bounds on a tree from another process |
| `src/devicesclient.cpp`, `.h` | asynchronous, burst-coalesced QtDBus client of `org.celestina.Devices1` (the phone in the panel) |
| `tests/protocoldecoder_test.cpp` | QtTest coverage for fragmented/multiple frames and recovery after the 1 MiB limit |
| `tests/trayitems_test.cpp` | QtTest coverage for the tray rules, written against the items this session actually publishes |
| `tests/traymenu_test.cpp` | QtTest coverage for reading another application's menu, written against blueman's and nm-applet's real ones |
| `tests/trayicons_test.cpp` | QtTest coverage for reading the session's icon theme and converting another application's pixels |
| `tests/providerstates_test.cpp` | QtTest coverage for every provider frame the host refuses, set replacement, generations and clearing |
| `tests/shellcommandline_test.cpp` | QtTest coverage for verb/option parsing, typing and every refusal |
| `tests/shellservice_test.cpp` | QtTest coverage over a real session bus (skipped without one): the exported interface, the state version and each rejected command, including the two overlay toggles refused without a controller wired |
| `tests/workspacefocusrequests_test.cpp` | QtTest coverage for the request policy: acceptance is not arrival, matched confirmation, rejection, timeout, adapter loss |
| `tests/surfacemanager_test.cpp` | QtTest coverage offscreen for the shared recipe, the menu surface and the overlay surface: panel and menu specs, reopening, dismissal, cleanup, and the real menu/launcher/clipboard QML files loading |
| `tests/tst_workspacestrip.qml` | Qt Quick Test coverage for the scroll step: origin, wrap at both ends, bursts, foreign outputs |
| `tests/tst_outputchooser.qml` | Qt Quick Test coverage for preserving the selected output across reorder and removal snapshots |
| `qml/Panel.qml` | hidden-until-configured three-region root window: two ordered flanks around a geometrically centred clock |
| `qml/SessionStatus.qml` | how the session is online, what is connected over Bluetooth and which power profile it runs; the profile cycles on click |
| `qml/AudioLevel.qml` | the session's volume and, when it is muted, its microphone; scroll steps, click mutes, middle-click opens the mixer |
| `qml/BrightnessLevel.qml` | this output's DDC brightness as a gauge, with the value on hover and three distinct states: absent, unknown, read back |
| `qml/TrayMenu.qml` | a tray item's own menu, drawn in the panel's surface |
| `qml/TrayDrawer.qml` | the system tray, collapsed by default; an item asking for attention is always visible, and one with no resolvable icon shows its name |
| `qml/CaptureButton.qml` | asks Niri to open its own screenshot UI; reports only a request it could not make |
| `qml/MediaMini.qml` | what the desktop is playing: checked cover art, title, play state and a progress line for media that has a real length; a click asks the player to toggle |
| `qml/SysMon.qml` | CPU and memory from the `sysmon` provider, coloured by load state; a click asks the host to open the system monitor |
| `qml/PanelFlank.qml` | one side of the panel as an ordered, clipped row — where a later widget is added |
| `qml/WorkspaceStrip.qml` | per-output workspace indicators + active window title; a click or a scroll step asks for focus and the pill shows pending, confirmed or failed; a right-click asks the host for the panel menu |
| `qml/Clock.qml` | the lived clock format, realigned every second |
| `qml/PhoneStatus.qml` | phone identity, charge state and battery |
| `qml/PanelMenu.qml` | the panel's context menu content: a shared `GlassContextMenu` whose items are real focus requests |
| `qml/LauncherOverlay.qml` | `Mod+Space`'s content: a search field and a keyboard-driven results list, talking to the `launcher` provider directly; no per-entry icon (named loss, R2) |
| `qml/ClipboardOverlay.qml` | the clipboard-history content: a keyboard-driven list of previews, talking to the `clipboard` provider directly; select, remove and clear all address a row by index, never by its full text |
| `qml/OutputChooser.qml` | the screen-share chooser dialog (consumes CelestinaStyle) |
| `scripts/run.sh` | build (Release) + activate the panel — the one script the shell needs |
| `ROADMAP.md` | status, checkpoints, design decisions and the R0–R9 phase list |
| `NOCTALIA-REPLACEMENT.md` | per-phase work orders for the replacement: session facts, files and contracts each phase touches, evidence, gates, rollbacks, open decisions |

See [ROADMAP.md](ROADMAP.md) for status, checkpoints and the design decisions,
and [NOCTALIA-REPLACEMENT.md](NOCTALIA-REPLACEMENT.md) to pick the work up in a
new session.
