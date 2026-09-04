# CelestinaStyle

The canonical Qt Quick/QML visual language shared by Celestina applications and
the shell.

## User contract

- The module supplies semantic tokens, embedded fonts and icons, opaque and
  glass surfaces, focus/modal primitives and a finite set of reusable controls.
- One UI 8.5 adapted to a pointer-driven desktop is the accepted visual
  direction; [DESIGN.md](DESIGN.md) owns that durable contract.
- Applications own responsive layout and product workflows. The style module
  never owns application state, Niri integration, D-Bus or filesystem policy.
- In-tree consumers use this source tree directly. A separately installed QML
  module remains conditional on a real external consumer.

## Architecture

| Area | Responsibility |
|---|---|
| `CelestinaTheme.qml` | Semantic colour pairs, typography, radii, anatomy, motion and `reducedMotion` |
| `CelestinaIcons.qml`, `CelestinaIconShapes.qml` | Closed Lucide UI names and generated Phosphor content shapes |
| `CelestinaSurface.qml`, `CelestinaBackdrop.qml` | Canonical opaque L0/L1 composition |
| `Glass*.qml` | Canonical floating/modal material over an explicit in-scene capture or compositor backdrop, with opt-in dense-content and contextual-veil roles |
| `CelestinaButton.qml`, `CelestinaIconButton.qml`, `CelestinaCapsule.qml`, `CelestinaRowHighlight.qml`, `CelestinaTextField.qml`, `CelestinaSlider.qml`, `CelestinaSwitch.qml` | Shared interactive controls, the grouped-glyph capsule, the shared row fill and their focus/accessibility contracts |
| `CelestinaModalLayer.qml`, `CelestinaInputShield.qml`, `CelestinaFocusRing.qml` | Modal lifecycle, pointer floor and keyboard-focus presentation |
| `CelestinaFolderIcon.qml`, `CelestinaFileIcon.qml` | Token-derived filled content icons |
| `qmldir`, `CMakeLists.txt` | Matching public source and compiled-module inventories |
| `gallery/`, `tests/` | Development review surface and Qt Quick interaction tests |

Siderita, Magnetita, Grafita and Fluorita compile canonical files through
relative links. The Celestina shell imports the same tree through its supported
URI alias. Consumers do not copy the module or maintain a private palette.

## Build and use

The declared module floor is Qt 6.9 with CMake 3.20 or newer.

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
```

The first command builds the registered production module; the second verifies
that exact artifact and its local visual contracts without installing it;
status reports whether the verification seal still matches the current inputs.
CelestinaStyle is not deployable. When a shared change affects an application,
run every affected consumer's registered `complete-production.sh`; verification
of the module alone does not place the changed style in the author's installed
applications. For a human visual review,
`gallery/run.sh` opens the source gallery; its offscreen mode does not prove
blur or compositor effects.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
- [Visual design contract](DESIGN.md)
- [Roadmap history through 2026-08-03](docs/history/roadmap-through-2026-08-03.md)
