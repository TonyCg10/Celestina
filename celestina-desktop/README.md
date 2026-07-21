# Celestina Desktop

The shell/session for a personal Niri (Wayland) environment. Its first product is
a small, truthful top panel with real Niri control; a usable daily session first
composes mature external tools rather than reimplementing a launcher,
notification daemon, lock screen, auth agent or wallpaper manager.

- **Role:** Niri shell / session (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** C++20 · Qt 6 Quick · LayerShellQt · CMake + Ninja
- **Consumes:** [celestina-style](../celestina-style/) (planned; currently a small inline palette)

## Build / run

```sh
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Debug
cmake --build build --parallel
cmake --build build --target all_qmllint
QT_QPA_PLATFORM=wayland ./build/celestina-desktop
```

During development, keep Noctalia running and hide only its bar so launcher,
notification, idle, lock, Polkit, theme and greeter services stay available:

```sh
noctalia msg bar-hide
QT_QPA_PLATFORM=wayland ./build/celestina-desktop
noctalia msg bar-show
```

Celestina uses its own layer-shell namespace (`celestina-desktop-panel`) so Niri
rules and diagnostics can tell both shells apart.

## Layout

| Path | Responsibility |
|---|---|
| `CMakeLists.txt` | Qt executable/module, LayerShellQt, Style path |
| `src/main.cpp` | process bootstrap and per-output layer-shell lifecycle |
| `qml/Panel.qml` | hidden-until-configured root window and panel layout |
| `qml/Clock.qml` | minute-aligned local time |

See [ROADMAP.md](ROADMAP.md) for status, checkpoints and the design decisions.
