# Celestina Desktop

The shell/session for a personal Niri (Wayland) environment. Its first product is
a small, truthful top panel with real Niri control; a usable daily session first
composes mature external tools rather than reimplementing a launcher,
notification daemon, lock screen, auth agent or wallpaper manager.

- **Role:** Niri shell / session (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** C++20 · Qt 6 Quick · LayerShellQt · CMake + Ninja
- **Consumes:** [celestina-style](../celestina-style/) (the output chooser consumes it live; the panel still uses a small inline palette — migrating it is a CP0 goal)

## Build / run

```sh
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Debug
cmake --build build --parallel
cmake --build build --target all_qmllint
QT_QPA_PLATFORM=wayland ./build/celestina
```

During development, keep Noctalia running and hide only its bar so launcher,
notification, idle, lock, Polkit, theme and greeter services stay available:

```sh
noctalia msg bar-hide
QT_QPA_PLATFORM=wayland ./build/celestina
noctalia msg bar-show
```

Celestina uses its own layer-shell namespace (`celestina-panel`) so Niri
rules and diagnostics can tell both shells apart.

## Layout

| Path | Responsibility |
|---|---|
| `CMakeLists.txt` | Qt executable/module + LayerShellQt |
| `src/main.cpp` | process bootstrap, per-output layer-shell lifecycle, `--pick-output` mode |
| `src/devicesclient.cpp`, `.h` | QtDBus client of `org.celestina.Devices1` (the phone in the panel) |
| `qml/Panel.qml` | hidden-until-configured root window, clock + phone indicator |
| `qml/Clock.qml` | minute-aligned local time |
| `qml/OutputChooser.qml` | the screen-share chooser dialog (consumes CelestinaStyle) |
| `scripts/output-chooser.sh` | wrapper `xdg-desktop-portal-wlr` runs to launch the chooser |

See [ROADMAP.md](ROADMAP.md) for status, checkpoints and the design decisions.
