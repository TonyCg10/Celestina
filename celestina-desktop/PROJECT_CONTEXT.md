# Celestina Desktop

> Current truth verified 2026-07-20. Near-term work lives in
> [ROADMAP.md](ROADMAP.md).

## Purpose

Celestina Desktop is the independent shell/session project for a personal Niri
environment. Its first product is a small, truthful top panel with real Niri
workspace control. A usable daily session initially composes mature external
tools instead of reimplementing a launcher, notification daemon, lock screen,
authentication agent or wallpaper manager.

The goal is a dependable personal shell that can grow from demonstrated daily
needs, not feature parity with Noctalia or a generic desktop environment.

Siderita, CelestinaStyle, the editor, viewer and player remain independent
projects. Desktop consumes only their installed packages and public contracts.

## Current state

- Git has no commits; all current source is untracked user work.
- The implementation is a C++20/Qt Quick host with Panel.qml and Clock.qml.
- The host creates one visible 40 px top layer-shell surface per connected
  output with the `celestina-desktop-panel` scope, reserves each edge and
  rejects keyboard focus. Screen add/remove signals create and destroy panels
  without terminating the shell.
- The only visible information is a real local clock aligned to minute changes;
  there are no simulated workspaces or tray placeholders.
- The first slice uses a small self-contained palette. CelestinaStyle remains
  deferred until it has a proven installed package contract.
- There is no Rust code, test suite, install contract, CI or resource baseline.
- Qt 6, LayerShellQt 6.7, CMake, Ninja and GCC are present on the host. Configure,
  build and QML lint pass. A live Niri check mapped exactly one namespaced,
  non-keyboard-interactive surface on DP-1, DP-2 and HDMI-A-1 while Noctalia
  remained running. Geometry, exclusive zones and focus preservation still need
  direct acceptance checks.

Source presence is not runtime evidence. Do not report the panel, blur or Niri
controls as working until a live test proves them.

## Current session dependency

The current Niri configuration starts Noctalia directly. Noctalia currently
provides more than the visible bar:

- `Mod+Space` opens its launcher;
- lock-and-suspend and DPMS keybindings call its IPC;
- its idle service owns screen-off and lock-and-suspend timeouts;
- its Polkit agent is enabled;
- greeter appearance auto-sync is enabled;
- caffeine and forced night light are enabled at session startup.

Development must preserve these paths until S2 installs and verifies explicit
replacements. The greeter itself is a pre-session service and is not owned by
the Celestina or Noctalia user-session process.

## Intended development commands

~~~sh
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Debug
cmake --build build --parallel
cmake --build build --target all_qmllint
QT_QPA_PLATFORM=wayland ./build/celestina-desktop
~~~

These are intended commands, not a green recipe on the current host.

During development, keep Noctalia running and hide only its bar:

~~~sh
noctalia msg bar-hide
QT_QPA_PLATFORM=wayland ./build/celestina-desktop
noctalia msg bar-show
~~~

This preserves the current launcher, notification, idle, lock, Polkit, theme
and greeter-sync services. Celestina must use its own layer-shell namespace so
Niri rules and diagnostics can distinguish both shells.

## Code map

| Path | Responsibility |
|---|---|
| CMakeLists.txt | Qt executable/module, LayerShellQt and current Style path |
| src/main.cpp | process bootstrap and per-output layer-shell lifecycle |
| qml/Panel.qml | hidden-until-configured root window and panel layout |
| qml/Clock.qml | minute-aligned local time |

## Decisions and constraints

| Decision | Reason |
|---|---|
| Desktop owns shell/session work only | Every Celestina app keeps its own roadmap and release |
| Niri only until a real second need exists | Avoid compositor abstractions and compatibility matrices in a personal shell |
| Top-edge layer-shell panel first | It is the smallest visible shell proof |
| The minimum owned UI is panel, clock and Niri state/control | Everything else must earn ownership through a daily workflow gap |
| Compose external session tools first | Mature launch, notification, lock, idle, portal and auth tools make the session usable sooner and reduce security risk |
| New domain/I/O logic in Rust; QML presentation; thin C++ host | Keep UI mature while isolating testable state and adapters |
| External state is provider-confirmed | A click is a request, never proof of success |
| Style comes from an installed/versioned contract | Sibling source paths are not a release interface |
| File configuration with good defaults precedes Settings UI | A personal shell does not need a configuration product before daily use |
| Noctalia remains the development fallback through S1 | Hiding its bar permits safe visual testing without removing session services |
| Backdrop blur belongs to Niri/surface integration | Local QML effects do not prove compositor blur |
| Lightweight is measured | Track installed closure, start time, PSS/RSS, wakeups and GPU cost |

If Rust is introduced, Qt models mutate only on the GUI thread, pure Rust does
not expose Qt/Niri wire types, queues are bounded, and shutdown joins all work.

## External daily-session contracts

S2 may pin exact choices after testing. The initial ownership boundary is:

| Need | Initial owner |
|---|---|
| Application launch | External launcher and Niri keybinding |
| Notifications | External Freedesktop notification daemon |
| Lock and idle | External session-lock and idle tools |
| Portals and secrets | XDG portals and keyring services |
| Privileged prompts | External Polkit agent |
| Wallpaper | External layer-shell wallpaper tool |
| Audio and brightness keys/OSD | PipeWire/system tools and external OSD |
| X11 compatibility | xwayland-satellite when required |

Celestina owns startup checks and documented integration, but must not silently
overwrite the user's Niri configuration or unrelated dotfiles.

## Current blockers

1. Establish a recoverable Git baseline before broad implementation changes.
2. Record the complete host tool versions and create a reproducible provisioning
   contract; CMake currently requires Qt 6.5 and LayerShellQt 6.6 or newer.
3. Consume CelestinaStyle only after its staged installed contract is proven.
4. Prove visibility, geometry, exclusive zone and no keyboard focus in a real
   Niri session while Noctalia remains available as the fallback.
5. Select a small Rust/QML bridge and bounded executor before live workspace
   state enters the product.
