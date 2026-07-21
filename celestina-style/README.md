# CelestinaStyle

The suite's Qt Quick/QML visual language: a semantic token singleton and a small
set of generic glass surfaces and controls. It owns reusable presentation only —
not app state, Niri integration, dotfiles or workflows — and is the single source
of truth for how the suite looks.

- **Role:** shared visual language (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** pure QML · Qt 6 Quick + `QtQuick.Effects` · CMake
- **Consumed by:** [siderita-adma](../siderita-adma/) (live) · [celestina-desktop](../celestina-desktop/) (planned)

## Build

```sh
cmake -S . -B build -G Ninja
cmake --build build
```

Within the monorepo, Siderita compiles these files into its own binary via
CXX-Qt. A relocatable install to a clean prefix (module + plugin + type metadata)
is Checkpoint 0.

## Layout

| Path | Responsibility |
|---|---|
| `CelestinaTheme.qml` | singleton design tokens: color, type, spacing, radius, motion, glass |
| `GlassSurface.qml` | frosted surface that blurs injected backdrop content (bounded, one-shot) |
| `GlassContextMenu.qml`, `GlassMenuItem.qml` | glass `Menu` + styled item |
| `CelestinaButton.qml` | ghost-style button |
| `icons/`, `icons.qrc` | minimal freedesktop-name SVG fallbacks |

See [ROADMAP.md](ROADMAP.md) for status, checkpoints and the design decisions.
