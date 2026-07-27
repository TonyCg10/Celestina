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

**Consumption contract:** apps symlink these sources into their own CXX-Qt QML
modules and compile them in — the canonical file is the interface, the binaries
stay self-contained, and CI refuses any copy that would drift. The `celestina`
output chooser still imports the built module through a runtime import path (its
own outlier, gone once the shell compiles the style in like the apps do). A
relocatable installed module is deferred until a consumer exists outside this
tree (see [ROADMAP.md](ROADMAP.md), STYLE-D).

## Layout

| Path | Responsibility |
|---|---|
| `DESIGN.md` | the design contract: One UI 8.5 desktop-adapted — reference values, platform ceiling, target system, build phases |
| `CelestinaTheme.qml` | singleton design tokens, tiered `ref`→`scheme`→`sys`: the dark `ColorScheme` as data (surface→ink pairs), Inter Variable, type/radius/motion/glass scales |
| `GlassSurface.qml` | frosted surface that blurs injected backdrop content (bounded capture, one-shot or live) |
| `GlassCard.qml` | `GlassSurface` specialization for modal dialog cards |
| `GlassContextMenu.qml`, `GlassMenuItem.qml` | glass `Menu` + styled item |
| `CelestinaButton.qml` | the suite button, in its three roles: normal / primary / destructive |
| `CelestinaTextField.qml` | the suite text field, themed fill and focus border |
| `icons/`, `icons.qrc` | the Lucide (ISC) icon set behind freedesktop names, the app launcher icons + the glass noise-dither texture |
| `fonts/`, `fonts.qrc` | Inter Variable (OFL) — the suite typeface, compiled into each app's binary |

See [DESIGN.md](DESIGN.md) for the design contract (One UI 8.5,
desktop-adapted) and [ROADMAP.md](ROADMAP.md) for status and checkpoints.
