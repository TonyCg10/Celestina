# CelestinaStyle

The suite's Qt Quick/QML visual language: a semantic token singleton and a small
set of generic glass surfaces and controls. It owns reusable presentation only —
not app state, Niri integration, dotfiles or workflows — and is the single source
of truth for how the suite looks.

- **Role:** shared visual language (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** pure QML · Qt 6 Quick + `QtQuick.Effects` · CMake
- **Consumed by:** [siderita](../siderita/) (live) · [magnetita](../magnetita/) (live) · [celestina](../celestina/) (output chooser live; panel still on an inline palette)

## Build

```sh
cmake -S . -B build -G Ninja
cmake --build build
```

Within the monorepo, Siderita and Magnetita symlink these files into their own
CXX-Qt QML modules, and the `celestina` output chooser imports the module
directly through a runtime import path. A relocatable install to a clean prefix
(module + plugin + type metadata) is Checkpoint 0.

## Layout

| Path | Responsibility |
|---|---|
| `CelestinaTheme.qml` | singleton design tokens: color, type, spacing, radius, motion, glass |
| `GlassSurface.qml` | frosted surface that blurs injected backdrop content (bounded capture, one-shot or live) |
| `GlassCard.qml` | `GlassSurface` specialization for modal dialog cards |
| `GlassContextMenu.qml`, `GlassMenuItem.qml` | glass `Menu` + styled item |
| `CelestinaButton.qml` | the suite button, in its three roles: normal / primary / destructive |
| `CelestinaTextField.qml` | the suite text field, themed fill and focus border |
| `icons/`, `icons.qrc` | minimal freedesktop-name SVG fallbacks + the app launcher icons |

See [ROADMAP.md](ROADMAP.md) for status, checkpoints and the design decisions.
