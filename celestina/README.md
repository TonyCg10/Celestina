# Celestina Desktop

The shell/session for a personal Niri (Wayland) environment. Its first product is
a small, truthful top panel with real Niri control; a usable daily session first
composes mature external tools rather than reimplementing a launcher,
notification daemon, lock screen, auth agent or wallpaper manager.

- **Role:** Niri shell / session (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** C++20 · Qt 6 Quick · LayerShellQt · KWindowSystem (KF6, compositor blur) · CMake + Ninja
- **Consumes:** [celestina-style](../celestina-style/) — live for the panel *and* the chooser, imported from source via a self-provisioned import path (the style's font qrc is not bundled yet, so shell text falls back to the system face)

## Build / run

```sh
scripts/run.sh                             # build (Release) + activate the panel
cmake --build build --target all_qmllint   # QML lint (after a first build)
```

`scripts/run.sh` is the one script the shell needs: it builds and *activates*
the shell — maps the panel on every output — in the foreground (Ctrl-C to stop).
Unlike the apps it is not a launcher entry, so running it is activating it.

During development, keep Noctalia running and hide only its bar so launcher,
notification, idle, lock, Polkit, theme and greeter services stay available:

```sh
noctalia msg bar-hide
scripts/run.sh
noctalia msg bar-show
```

Celestina uses its own layer-shell namespace (`celestina-panel`) so Niri
rules and diagnostics can tell both shells apart.

## Layout

| Path | Responsibility |
|---|---|
| `CMakeLists.txt` | Qt executable/module + LayerShellQt + KWindowSystem |
| `src/main.cpp` | process bootstrap, per-output layer-shell lifecycle (+ compositor blur request), style import self-provisioning, `--pick-output` mode |
| `src/devicesclient.cpp`, `.h` | QtDBus client of `org.celestina.Devices1` (the phone in the panel) |
| `qml/Panel.qml` | hidden-until-configured root window, clock + phone indicator |
| `qml/Clock.qml` | minute-aligned local time |
| `qml/OutputChooser.qml` | the screen-share chooser dialog (consumes CelestinaStyle) |
| `scripts/run.sh` | build (Release) + activate the panel — the one script the shell needs |

See [ROADMAP.md](ROADMAP.md) for status, checkpoints and the design decisions.
