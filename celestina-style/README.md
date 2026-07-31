# CelestinaStyle

The suite's Qt Quick/QML visual language: a semantic token singleton and a small
set of generic glass surfaces and controls. It owns reusable presentation only —
not app state, Niri integration, dotfiles or workflows — and is the single source
of truth for how the suite looks.

- **Role:** shared visual language (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** pure QML · Qt 6.9+ Quick + `QtQuick.Effects` · CMake
- **Consumed by:** [siderita](../siderita/) (live) · [magnetita](../magnetita/)
  (live) · [celestina](../celestina/) (panel and output chooser live from the
  canonical source tree)

## Build

```sh
cmake -S . -B build
cmake --build build
ctest --test-dir build --output-on-failure
```

**Consumption contract:** CXX-Qt apps symlink these sources into their own QML
modules and compile them in — the canonical file is the interface, the binaries
stay self-contained, and CI refuses any copy that would drift. The shell has a
different in-tree delivery shape: panel and chooser import this same plain-source
module directly, through a `CelestinaStyle` alias created by CMake for `qmllint`
and recreated by the host at runtime. It has no inline palette and does not use a
separately built or installed style module. A relocatable installed module is
deferred until a consumer exists outside this tree (see
[ROADMAP.md](ROADMAP.md), STYLE-D).

## Layout

| Path | Responsibility |
|---|---|
| `AGENTS.md` | local agent contract for the style boundary, public API, states and verification |
| `DESIGN.md` | the design contract: One UI 8.5 desktop-adapted — reference values, platform ceiling, target system, build phases |
| `CelestinaTheme.qml` | singleton design tokens, tiered `ref`→`scheme`→`sys`: one accent seed and derived states, the dark `ColorScheme` as data (surface→ink pairs), Inter Variable, distinct in-scene/compositor glass tints, type/radius/motion/component scales, plus the host-controlled `reducedMotion` input |
| `CelestinaSurface.qml` | semantic non-floating container (`Canvas`, `Panel`, `Grouped`, `Content`, `Tonal`, `Elevated`, `Selected`); consumers own geometry/content while the style owns fill, foreground and shape |
| `CelestinaBackdrop.qml` | canonical L0 window gradient; consumers may add decorative children without rebuilding the canvas |
| `GlassSurface.qml` | frosted surface that blurs injected backdrop content (bounded capture, one-shot or live; `Regular` floating and `Strong` modal densities) |
| `GlassCard.qml` | `GlassSurface` specialization for modal dialog cards |
| `GlassContextMenu.qml`, `GlassMenuItem.qml` | glass `Menu` + styled item |
| `CelestinaButton.qml`, `CelestinaIconButton.qml` | text and icon-only buttons sharing closed role/density, focus, disabled and tooltip contracts |
| `CelestinaFocusRing.qml` | canonical exterior focus-visible outline; keeps the focus indicator independent from control fill and anatomy |
| `CelestinaIcons.qml`, `CelestinaIcon.qml` | closed semantic-name resolver + vendored Lucide renderer and tone; no desktop icon theme participates |
| `CelestinaTextField.qml` | the suite text field, with closed standard/search shapes, themed fill and exterior focus ring |
| `CelestinaSectionLabel.qml` | uppercase section eyebrow with compact/regular size and consumer-provided scale |
| `CelestinaSwitch.qml` | the One UI pill toggle: white thumb, accent track when on |
| `ListSection.qml` | the grouped-card list (One UI's "focus block" signature) with an optional header |
| `CelestinaModalLayer.qml` | shared L3 scrim, fade, focus containment/restoration and outside/Escape dismissal; dialog content stays app-owned |
| `CelestinaInputShield.qml` | the input floor a surface floating over live content declares: blocking hover and a drag claim on the press, plus an optional three-button swallow; the wheel still passes through |
| `gallery/` | dev-only review surface — every token, control and glass surface on one screen (`gallery/run.sh`) |
| `tests/` | Qt Quick Test coverage for modal focus entry, Tab/Backtab containment, exact restoration, Escape, pointer blocking through the exit fade, and that a sweep over a dialog never reaches a drag handler underneath |
| `scripts/check-style-contract.sh` | CI/local guard across Siderita, Magnetita, the shell and the shared module; rejects visual literals/local derivations and invokes the contrast contract |
| `scripts/check-contrast-contract.py` | derives current theme values and verifies hostile black/white backdrop contrast floors for compositor glass, artwork, primary/destructive controls and metadata |
| `scripts/sync-lucide-icons.sh` | reproducible sync of all 76 glyphs from the pinned Lucide release while retaining compatibility filenames |
| `icons/`, `icons.qrc` | the vendored Lucide (ISC) catalogue, its licence and the glass noise-dither texture |
| `fonts/`, `fonts.qrc` | Inter Variable (OFL) — the suite typeface, compiled into each app's binary |

## Public QML API

| Type | Semantic input |
|---|---|
| `CelestinaTheme` | singleton semantic tokens plus host-controlled `reducedMotion`; each host maps the presence of `CELESTINA_REDUCED_MOTION` into this property |
| `CelestinaSurface` | `role`; geometry and children remain consumer-owned |
| `CelestinaButton` | closed `role` (`Tonal`, `Primary`, `Destructive`, `Selected`, `Ghost`), `density` (`Compact`, `Regular`, `Prominent`), `helpText` |
| `CelestinaFocusRing` | `target`, `cornerRadius`, `shown`; colour and thickness remain token-owned |
| `CelestinaIcon` | semantic `name`, `fallbackName`, `tone`, optional token-backed `tintOverride`; names resolve locally through `CelestinaIcons` and geometry may be scaled by the consumer |
| `CelestinaIconButton` | `iconName`, `fallbackIcon`, `iconSize`, `helpText` plus the button role/density; its glyph always renders through a square viewport |
| `CelestinaTextField` | `shape`; ordinary `TextField` value/validation properties remain available |
| `CelestinaSectionLabel` | `size`, `textScale` |
| `CelestinaModalLayer` | `shown`, dismissal switches and `dismissRequested` |
| `CelestinaInputShield` | `active`, `swallowClicks`; it anchors to its parent and sits at `z: -1`, so the surface's own controls keep being delivered first |
| `GlassSurface` / `GlassCard` | `backdropSource`, `density`, capture mode and elevation |
| `GlassContextMenu` / `GlassMenuItem` | injected backdrop plus ordinary menu action/current-state properties, nested-menu chevrons and optional token-backed colour swatches |
| `ListSection` / `CelestinaSwitch` | section `title` + rows; switch `checked` state |

`qmldir` is the plain-source contract. CMake generates the matching QML type
metadata for the built module; in-tree apps compile the same canonical files
through relative symlinks and register `icons.qrc` / `fonts.qrc` in `build.rs`.

## Token boundary

Changing `ref.accent` (currently the approved `#3e91ff`) is the single
suite-wide accent dial. The theme derives
link, hover, pressed, focus, selection, badge and disabled variants from that
seed; consumers use only the resulting semantic properties. Colours, type
roles, radii, borders, state opacities, motion and stable control anatomy belong
to the theme. Responsive placement and screen-specific geometry remain with the
consumer.

Content glyphs use a second, deliberately narrow semantic layer: derived accent
for folders, cool silver for plain files, violet-blue for links, slate for
navigation and cyan for devices. Per-entry customization is limited to the six
named `iconAccentKeys`; consumers persist those stable keys, never hexadecimal
values. This palette changes icon ink only—thumbnails and interaction states
remain governed by their own roles.

Run the static contract locally with:

```sh
bash scripts/check-style-contract.sh
```

It audits Siderita, Magnetita, the shell and the shared components, then checks
the contrast floors derived from the current theme values. It is also part of CI.
CTest separately proves the modal's keyboard-focus and pointer-blocking contract,
including the animated exit interval,
offscreen. These checks remain static/headless evidence: they do not validate
blur, real-session focus rendering, assistive technology or reduced motion on a
compositor session.

See [DESIGN.md](DESIGN.md) for the design contract (One UI 8.5,
desktop-adapted) and [ROADMAP.md](ROADMAP.md) for status and checkpoints.
