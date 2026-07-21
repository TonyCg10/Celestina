# CelestinaStyle roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the shared
> style library only. Checklist legend: `[x]` done · `[ ]` planned.

## Overview

**Purpose.** The independent Qt Quick/QML visual library for the suite. It owns
reusable semantic tokens and generic controls — not app state, Niri integration,
dotfiles or workflows. Consumers keep independent roadmaps and release timing.

**Current state.** CelestinaStyle is now the canonical shared module: a semantic
token singleton (promoted from Siderita), working backdrop-blur glass
(`GlassSurface`/`GlassContextMenu`/`GlassMenuItem`, replacing the earlier
`CelestinaGlassPanel` that blurred its own fill), `CelestinaButton` and bundled
fallback icons. It builds with CMake and is consumed live by `siderita-adma`. A
clean-prefix installable release, a compatibility/deprecation policy and verified
accessibility/motion behavior are still open.

**Key decisions.** Style is an independent shared-library project; delivery is an
installed/versioned QML module (source siblings and build symlinks are not a
public interface); it owns reusable presentation only; backdrop blur is achieved
in-scene by sampling injected content (`ShaderEffectSource`) — the module offers
tint/glow/tokens, while compositor effects belong to the surface owner;
accessibility is part of a public control's contract; the component set grows only
from proven reuse (widget count is not progress).

## Checkpoint 0 — Relocatable, honest module (STYLE-0)
**Goal:** a staged prefix contains a complete module that clean fixtures can
import and operate without the source checkout, and the glass APIs mean what
they say.

- [x] Canonical module builds with CMake: semantic token singleton (`CelestinaTheme`, promoted from Siderita) + working glass (`GlassSurface`, `GlassContextMenu`, `GlassMenuItem`) + `CelestinaButton` + bundled fallback icons
- [x] Working backdrop-blur glass — the broken `CelestinaGlassPanel` (blurred its own fill, not the backdrop) and `CelestinaContextMenu` were removed and replaced by Siderita's proven `ShaderEffectSource`-capture `GlassSurface`
- [x] First real consumer proven: `siderita-adma` renders entirely from this module (theme, glass, icons), verified by build + offscreen run
- [ ] Inventory the public QML types, imports, properties, assets and generated files
- [ ] Choose the smallest complete install topology (module + plugin + type metadata) and make it importable from a clean prefix without the source checkout
- [ ] Clean-prefix fixtures: one theme fixture + one interactive-control fixture that import only the staged prefix
- [ ] Prove relocation (move the prefix, re-import) and missing/corrupt-module failures
- [ ] Resolve the qmllint `OUTPUT_DIRECTORY` module-path warning
- [ ] Record artifact/load baselines and consumer instructions with no sibling paths

**Done when:** clean configure/build/install produce an accounted module tree;
fixtures resolve only the staged module with the source hidden; moving the prefix
works after declaring its new import root; missing artifacts fail visibly instead
of falling back to a sibling/global copy.

## Checkpoint 1 — Stable, accessible design contract (STYLE-1)
**Goal:** a versioned contract that Desktop and Siderita can both adopt
independently.

- [ ] Compatibility + deprecation policy for the 1.0 surface
- [ ] Truthful glass APIs — tint/glow/tokens vs. real in-scene blur vs. compositor blur kept clearly separate
- [ ] Font + icon fallback contracts (Inter / JetBrains Mono / icon set)
- [ ] Keyboard, focus, AT-SPI, reduced-motion and high-contrast behavior for the finite component set
- [ ] Both apps consume the same installed release (see the suite convergence goal)

**Done when:** Desktop and Siderita accept the same installed style release
independently, and accessibility/motion behavior is verified, not assumed.

## Checkpoint 2 — Grow only by proven reuse
**Goal:** the component set grows from demonstrated demand, not from widget
count.

- [ ] Add a component or toolkit-neutral asset only after ≥2 real consumers demonstrate reusable demand

## Non-goals

Do not become an application framework, a global configuration daemon, a complete
Qt Controls replacement, a compositor integration layer, or the owner of consumer
layouts and domain state.
