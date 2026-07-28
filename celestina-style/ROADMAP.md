# CelestinaStyle roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the shared
> style library only. Checklist legend: `[x]` done · `[ ]` planned.

## Overview

**Purpose.** The independent Qt Quick/QML visual library for the suite. It owns
reusable semantic tokens and generic controls — not app state, Niri integration,
dotfiles or workflows. Consumers keep independent roadmaps and release timing.

**Current state.** CelestinaStyle is the canonical shared source: a semantic
token singleton (promoted from Siderita), working backdrop-blur glass
(`GlassSurface`/`GlassCard`/`GlassContextMenu`/`GlassMenuItem`, replacing the
earlier `CelestinaGlassPanel` that blurred its own fill), semantic backdrop,
surface, button, icon, section-label, text-field, switch, modal-layer and grouped
list components, plus bundled fallback icons. Both apps consume it live by
symlinking the sources into their own CXX-Qt modules — the official mechanism,
guarded in CI so a copy can never silently reappear. A compatibility/deprecation
policy and verified accessibility/motion behavior are still open; the installable
module is deferred (see below).

**Key decisions.** Style is an independent shared-library project. **In-tree
delivery is source compilation via symlinks**: each app links the canonical
`.qml` files into its own CXX-Qt QML module, so the canonical file *is* the
interface, the dev loop has no install step, and app binaries stay
self-contained — the property the suite's staged installs are built on. An
installed/versioned QML module is the right tool only when a consumer lives
*outside* this tree, and is deferred until one exists. It owns reusable
presentation only; backdrop blur is achieved in-scene by sampling injected
content (`ShaderEffectSource`) — the module offers tint/glow/tokens, while
compositor effects belong to the surface owner; accessibility is part of a
public control's contract; the component set grows only from proven reuse
(widget count is not progress).

**Design contract.** The visual direction — One UI 8.5 adapted to desktop:
reference values, the Qt/niri platform ceiling, the audit of the current
system, the target system and a phased build plan (S1–S5) — lives in
[DESIGN.md](DESIGN.md) (v1.0, decisions sealed). This roadmap tracks
execution; the design contract says what "done" looks like.

**Phase status.** **S1 (tokens v2 + typography) — done** (2026-07-27): the
singleton was rebuilt into `ref`→`scheme`→`sys` tiers with the dark `ColorScheme`
as data (Rosé Pine and the comment-toggled palette retired); every opaque surface
carries its foreground pair (`<surface>Ink`, the contract's `on*` — QML reserves
`on<Capital>`); the accent moved from white to One UI blue `#387aff` with
`accentInk` (`CelestinaButton` primary adopts it, killing `accent → canvas`);
Inter Variable (OFL) ships embedded in each app's qrc; radius/type/motion scales
match DESIGN §6.3/§6.6/§6.7 (the One UI bezier tokens are defined, ready for S2's
motion retune); all consumers (siderita, magnetita, the shell chooser, the six
shared components) were migrated. Verified by app builds + offscreen smokes +
grabbed swatch/button/window captures, and a clean module `all_qmllint`.

**S2 (glass v2 + elevation) — done** (2026-07-27): `GlassSurface` was rebuilt to
the 8.5 recipe (DESIGN §6.5) — bounded capture → pyramid blur → *slight
desaturation* (the earlier saturation boost is gone) → tint/dim → tiled noise
dither → a thin dark outline → the lit top-edge glow, now a GPU
`Shape`/CurveRenderer gradient ring instead of the CPU `Canvas`. `elevation`
adds the L2 `RectangularShadow` under floating layers (menus float instead of
pasting); modals (L3) dim with the `scrim` token, never a shadow. The surface
self-tracks its scene position (a `FrameAnimation` while live), so menus/cards
dropped their hand-wired `refreshBackdrop()` choreography. Verified on the real
session (blur/shadow do not render under the offscreen QPA): grabbed glass /
elevated-surface / real Siderita-window captures, plus app builds + offscreen
smokes + clean `all_qmllint`. Deferred within the recipe: the composite
QQEM/qsb shader (blur+desaturate+tint+noise+stroke in one pass) stays layered
for now.

**S3 (iconography) — done** (2026-07-27): the ad-hoc
hand-drawn fallbacks were replaced by the **Lucide** set (ISC, license shipped
in `icons/`) behind the existing freedesktop-name mapping, so consumers keep
resolving by name; a `phone` icon was added (Siderita's sidebar referenced a
`phone` fallback that did not exist). The shell panel's emoji glyphs (`📱`,`⚡`)
became real icons, compiled into the shell from the canonical files. `eject`
has no Lucide equivalent, so the existing (Lucide-consistent) mark is kept.
The app-icon tiles became true **squircles** (a superellipse n=5 shared by both
launcher marks, replacing the rounded-rect tile — One UI reserves the
superellipse for app icons, §6.3); the amber-rhombohedron / steel-octahedron
marks are unchanged. Verified by offscreen icon + app-icon specimens + app/shell
builds + smokes. **Still open (out of S3):** the panel's broader migration off
its hardcoded Rosé Pine palette (shell buildout, gated by CLAUDE.md).

**S4 (gallery + first components) — done** (2026-07-27): the two One UI
signature controls landed — **`CelestinaSwitch`** (the pill toggle, white thumb
on an accent track) and **`ListSection`** (the grouped-card "focus block"), with
**Magnetita's Settings surface** as their first real consumer (the
first-consumer gate DESIGN §6.8 sets for pre-specced components; CP2's own
≥2-consumer bar below stays open):
its plugin toggles are now switches and its device/plugin lists are grouped
cards, dropping the 🟢/⚪/🔑 emoji for a status dot and tabular fingerprints, and
the toggle keeps truthful state (the switch re-binds to the daemon's answer, not
the optimistic click). The **gallery** (`gallery/Gallery.qml`, run via
`gallery/run.sh` — dev-only, no build step) puts every token, control, icon and
glass surface on one scrollable screen: the review surface DESIGN §7 asked for.
Verified by a full real-session gallery grab + a standalone component grab +
Magnetita build/smoke + module `all_qmllint`. Still on demand (CP2): further
components (`CelestinaDialog`, `TabPills`, `Toast`…) wait for their first
consumers.

**S5 (panel compositor glass) — done** (2026-07-27): the shell panel was migrated
off its hardcoded Rosé Pine palette onto `CelestinaTheme` (Panel + Clock), with a
translucent `glassTint` background, and the shell now asks the compositor to blur
the wallpaper behind it — `KWindowEffects::enableBlurBehind` on the layer-shell
surface, driving niri's `ext-background-effect` (best-effort: a compositor
without it just leaves the panel a translucent tint). `main.cpp` self-provisions
the `CelestinaStyle` import path (a runtime symlink under the URI name), so the
panel and the output chooser resolve the style from source without a wrapper
pre-setting `QML_IMPORT_PATH`. Verified by the shell build (KWindowSystem
linked), a live run confirming both outputs map and the theme resolves with no
token errors, and the author confirming the panel renders on the session (a clean
screenshot is obstructed by the session's own bars). The suite's phased restyle
(S1–S5, DESIGN §8) is complete.

**Semantic-surface follow-up — done** (2026-07-28): `CelestinaSurface` became
the closed L0/L1 container contract after the same need was proven by Siderita's
sidebar/content panels and Magnetita's cards/activity. Apps choose only a role
and geometry; fill, ink, radius and outline stay in the style module. Magnetita's
monolithic QML window was split into `pages/` and `components/`; Siderita's QML
was grouped into `views/`, `components/`, `dialogs/` and `menus/`, with
`SidebarInfo` extracted before the sidebar migration. `GlassSurface` gained
regular/strong semantic densities and was retuned against the supplied dark
capsule reference: slightly denser tint, restrained lit edge, softer shadow and
more backdrop colour retained.

**Semantic-control follow-up — done** (2026-07-28): the remaining repeated
presentation contracts became finite shared types: `CelestinaBackdrop`,
`CelestinaIcon` / `CelestinaIconButton`, `CelestinaSectionLabel` and
`CelestinaModalLayer`. `CelestinaButton` now exposes one closed role instead of
contradictory booleans; Siderita's floating button remains deliberately local
because no second app shares its backdrop-aware behavior. Siderita grouped its
components into `chrome/`, `sidebar/` and `entry/` before the separately scoped
Sidebar/FolderView decomposition. The public API inventory now lives in README.

**Siderita composition follow-up — done** (2026-07-28): the three remaining
large QML hosts were reduced without moving domain behavior. `Sidebar` delegates
saved rows and context menus; `FolderView` delegates list/grid presentation,
shortcuts, actions/dialogs, operation status and floating content chrome; the
portal picker delegates its top/bottom controls to `PickerChrome`. All three
coordinators are now below the approximate 800-line ceiling, and every extracted
QML type is registered explicitly in `build.rs`.

**Token-hardening follow-up — done** (2026-07-28): `ref.accent` is now the one
suite-wide hue seed; link/hover/pressed/focus plus every accent wash derive from
it. Shared control anatomy (borders, state opacity, button/text-field padding,
switch, checkbox, linear track, slider handle and status indicator) moved into
theme tokens, and Siderita/Magnetita no longer carry visual literals for those
roles. `scripts/check-style-contract.sh`, wired into CI, rejects colours, local
colour transforms and raw state/anatomy values outside the canonical theme.
Responsive page geometry deliberately remains consumer-owned.

## Checkpoint 0 — The canonical source, enforced (STYLE-0)
**Goal:** one canonical source tree that every consumer compiles from, with
drift made impossible, and glass APIs that mean what they say.

- [x] Canonical module builds with CMake: semantic token singleton (`CelestinaTheme`, promoted from Siderita) + working glass (`GlassSurface`, `GlassCard`, `GlassContextMenu`, `GlassMenuItem`) + `CelestinaButton` + `CelestinaTextField` + bundled fallback icons
- [x] Working backdrop-blur glass — the broken `CelestinaGlassPanel` (blurred its own fill, not the backdrop) and `CelestinaContextMenu` were removed and replaced by Siderita's proven `ShaderEffectSource`-capture `GlassSurface`
- [x] First real consumer proven: `siderita` renders entirely from this module (theme, glass, icons), verified by build + offscreen run
- [x] Single source made real and enforced: both apps consume by symlink into their own CXX-Qt modules (Siderita's six committed copies were replaced by links, 2026-07-26), and CI refuses any style file in an app's `qml/` that is not a symlink
- [x] Token boundary enforced: CI rejects visual literals and app-local colour/state recipes; `ref.accent` is the single seed for all accent roles
- [x] Inventory the public QML types, semantic properties, assets and generated metadata contract in README; keep it aligned with `qmldir` / CMake
- [x] Resolve the qmllint `OUTPUT_DIRECTORY` module-path warning — the module now sets `OUTPUT_DIRECTORY .../CelestinaStyle` to match its URI; verified by a clean `cmake` configure (no `Qt6QmlMacros` warning) and a clean `all_qmllint` (S1, 2026-07-27)

**Done when:** every consumer builds from the one canonical tree with no copy
anywhere, the guard proves it on every push, and the public surface is written
down.

## Deferred — the installable module (STYLE-D)
**Gate:** a consumer *outside* this tree (a third-party app, a packaged
release beyond the author's machine). Until one exists, an installed module
would only add an install step and a runtime dependency that the staged
self-contained apps deliberately avoid.

- [ ] Choose the smallest complete install topology (module + plugin + type metadata) and make it importable from a clean prefix without the source checkout
- [ ] Clean-prefix fixtures: one theme fixture + one interactive-control fixture that import only the staged prefix
- [ ] Prove relocation (move the prefix, re-import) and missing/corrupt-module failures
- [ ] Record artifact/load baselines and consumer instructions with no sibling paths

## Checkpoint 1 — Stable, accessible design contract (STYLE-1)
**Goal:** a versioned contract that Desktop and Siderita can both adopt
independently.

- [ ] Compatibility + deprecation policy for the 1.0 surface
- [ ] Truthful glass APIs — tint/glow/tokens vs. real in-scene blur vs. compositor blur kept clearly separate
- [~] Font + icon fallback contracts — Inter Variable (OFL) ships embedded in each app's qrc, with an honest fallback where it is not compiled in (the shell → application default), and the Lucide set landed in S3; the mono face and the written fallback policy remain
- [ ] Keyboard, focus, AT-SPI, reduced-motion and high-contrast behavior for the finite component set
- [x] Both apps consume the same canonical source (symlink-compiled; an installed release belongs to STYLE-D)

**Done when:** every consumer renders from the same canonical source, and
accessibility/motion behavior is verified, not assumed.

## Checkpoint 2 — Grow only by proven reuse
**Goal:** the component set grows from demonstrated demand, not from widget
count.

- [x] Add `CelestinaSurface` after ≥2 real consumers demonstrated the shared container need; Siderita and Magnetita now consume the same role-based contract
- [x] Add backdrop, icon/button and section-label contracts after both apps demonstrated the same repeated presentation; add the pre-specified modal layer with Siderita's seven dialog consumers
- [ ] Add any further component or toolkit-neutral asset only after ≥2 real consumers demonstrate reusable demand

## Non-goals

Do not become an application framework, a global configuration daemon, a complete
Qt Controls replacement, a compositor integration layer, or the owner of consumer
layouts and domain state.
