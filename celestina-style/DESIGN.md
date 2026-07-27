# CelestinaStyle — Design Contract (One UI 8.5, desktop-adapted)

> **Status: v1.0 (2026-07-27) — the four author decisions are sealed (§9).
> S1 (tokens v2 + typography) and S2 (glass v2 + elevation, §8) shipped
> 2026-07-27; S3–S5 gated on the author.** Grounded in Samsung's real shipped
> values (the SESL/One UI support library and the official One UI design
> guide), not in screenshots-by-eye, and in verified Qt 6.11 / niri 26.04
> capabilities.
>
> One implementation note where the platform overrode the contract: the `on*`
> foreground pairs (§6.9/§9.1) ship as `<surface>Ink` (`accentInk` = `onAccent`)
> because QML reserves the `on<Capital>` identifier namespace for signal
> handlers — same pairs, a spelling the engine accepts.

## 1. Direction

Celestina's visual language imitates **Samsung One UI 8.5** — the 2026
"glass" iteration — **adapted to a pointer-driven Niri desktop**. The
identity: calm near-black neutrals, one restrained accent, generously rounded
grouped cards, large comfortable type under a big collapsing header, and
frosted glass reserved for *floating* layers (panels, menus, pills) with a
lit top edge. Samsung's own desktop posture (DeX on One UI 8.5: transparent
floating taskbar, popup app drawer, flat borderless windows) is the precedent
for what this language looks like on a PC.

## 2. The reference, digested — One UI 8.5 real values

Values verified from Samsung's shipped SESL library (mirrors:
`tribalfs/sesl-androidx`, `tribalfs/oneui-design`, `OneUIProject/oneui-core`)
and the official design guide (`design.samsung.com` PDF +
`developer.samsung.com/one-ui`). dp values map 1:1 to logical px on desktop.

**Shape** — master radius **26** (dialogs, popup menus, grouped list cards);
buttons **18** (small 16); search field **22**; radius scales down with
element size (26/20/12 thumbnails). Pills for floating tab bars, toggles,
live chips (the Now Bar). In-app corners are **plain circular** — the
squircle/superellipse is reserved for **app icons** only.

**Color** — every neutral has a subtle cool cast, never pure gray:

| Role | Dark | Light |
|---|---|---|
| Window (behind cards) | `#010102` | `#f1f1f3` |
| Card / content bg | `#17171a` | `#fcfcff` |
| Elevated | `#2d2d30` | `#f6f6f8` |
| Primary text | `#fafaff` | `#010102` |
| Secondary text | `#99999e` | `#848487` |
| Divider (sparingly) | `#3a3a3d` | `#e4e4e7` |
| Accent (interactive only) | `#387aff` (links `#598fff`) | `#387aff` (pressed `#376fde`) |
| Danger / Success / Warning | `#fc6c65` / `#58db9c` / `#fc864c` | `#d93e36` / `#11a85f` / `#e65b17` |

Doctrine: overwhelmingly neutral surfaces; blue only on interactive/active
elements (switches, checked states, links); gradients as the sanctioned
decorative device (8.5 tints selected states and popups with soft gradients).
One UI 8.5's community-criticized "gray drift" in dark mode is treated as a
misstep — Celestina keeps the near-black intent.

**Typography** — One UI Sans (variable grotesque; weights used: 200/300/400/
600). Scale (sp): expanded header **34 → 21** on collapse; list row **17** +
secondary **13**; dialog title 17, body 14; buttons 15; search 21; toast 14.
Character: large sizes, generous line height, semibold (600) titles instead
of heavy bolds. Contrast floors: 4.5:1 body, 3:1 large text.

**Glass (the 8.5 recipe)** — strong frosted blur + **slight desaturation of
the backdrop** + dim, a thin dark outline, and an **inner glow on the top
edge** ("cards appear like physical pieces of glass"). Frosted/matte, not
Apple-style refractive lensing. Applied to floating layers (quick panel,
notification cards, floating pill tab bars, address bars) — never to content
itself. Scroll edges dissolve via progressive blur instead of a hairline.
Blur/transparency are user-tunable and degrade gracefully on weak hardware.

**Depth doctrine (official)** — three tools: *blur* (always paired with dim),
*dim*, *shadow*. Shadows soft and light, "must not suggest 3D depth". Never
dim + shadow on the same surface. One hierarchy level exposed per screen.

**Motion** — official curve **cubic-bezier(0.22, 0.25, 0, 1)** (fast start,
long settle); variants SineInOut80 `(0.33, 0, 0.2, 1)` / SineInOut90
`(0.33, 0, 0.1, 1)`. Durations **100–500 ms** (dialogs 100; press-recoil:
press 100, release **350**). Character: damped-fluid with selective spring —
overshoot on panel reveals, press-shrink "recoil" on buttons/cards, morph
continuity (app opens preserving the icon's corner radius). Linear only for
opacity. A "reduce animations" accessibility mode swaps transitions for fades.

**Structure** — the signature is the **grouped rounded card list** ("focus
blocks"): settings/list rows grouped into 26-radius cards floating on the
window background; separation by grouping, not hairlines. Row anatomy:
[icon] title 17 + subtitle 13 + trailing control; switch = 35dp pill track,
white thumb, accent track when on. Buttons: one style per screen (text /
tonal gray / filled accent). Dialogs: radius 26, width 320–360, dim behind.
Side margins ≥ 24. Bottom-anchoring (search bars, floating back button,
bottom sheets) is a *reachability* device — it does not transfer to desktop.

## 3. Desktop adaptation rules

1. **Reachability dies, the vocabulary stays.** No bottom sheets, no bottom
   search, no floating back button. Dialogs center. The pill/floating-layer
   vocabulary (tab pills, chips, floating panels) transfers intact.
2. **Pointer is first-class.** One UI has no hover; Celestina defines hover
   tokens for every interactive surface (already partially true). Press
   states adopt One UI's recoil (§6.7).
3. **Keyboard is first-class.** A visible focus-ring system is mandatory
   (phones don't need one; a desktop does).
4. **The big collapsing header** (34→21) applies to app pages with their own
   scroll (Settings-like surfaces, Magnetita's window); Samsung itself drops
   the expanded header below 580dp of height — small windows start collapsed.
5. **Sizes map 1:1** (dp→logical px) as the starting point, tuned per surface
   with screenshots during implementation — not invented in the doc.

## 4. Platform ceiling — Qt 6.11 + niri 26.04 (verified)

What the stack can actually do today:

- **Real compositor glass for the panel.** niri 26.04 implements
  `ext-background-effect-v1` (blur/xray/saturation/noise per window or layer
  surface). **X-ray mode blurs the wallpaper once and reuses it — near-zero
  steady cost.** Qt route: `KWindowEffects::enableBlurBehind` (KWindowSystem
  ≥ ~KF 6.19 speaks the new protocol) or a ~100-line
  `QWaylandClientExtensionTemplate` implementing it directly. The in-scene
  glass remains the fallback and the mechanism *inside* app windows (a
  Wayland client can never sample other windows — confirmed design).
- **In-scene glass ceiling** = our current architecture (capture with
  `sourceRect` + MultiEffect), which is the officially documented pattern.
  MultiEffect's blur is a downsample **pyramid** (not gaussian): keep
  `blurMax ≤ 32` (4 internal passes), prefer `blurMultiplier` for reach, and
  **dither its banding with noise**. Live capture only when content moves
  beneath; one-shot + `scheduleUpdate()` is near-free at steady state.
- **Shadows:** `RectangularShadow` (Qt 6.9+, per-corner radii in 6.11) —
  analytic SDF, far cheaper than MultiEffect shadow. All elevation shadows
  use it.
- **Shape:** `Rectangle` per-corner radii (6.7+); `Shape` +
  `preferredRendererType: CurveRenderer` for GPU gradient strokes (replaces
  the CPU `Canvas` lit edge); `PathRectangle` (6.8+). No built-in
  superellipse — irrelevant for surfaces (One UI in-app corners are
  circular); for app icons, an SDF or cubic approximation if ever needed.
- **Typography:** variable fonts via `font.variableAxes` (6.7+), OpenType
  `font.features` (`tnum` for panel numerics). On Wayland use
  `Text.QtRendering` (or `CurveRendering` for large display text); native
  hinting breaks under fractional scale.
- **Motion:** bezier easing tokens (`easing.bezierCurve`); Qt 6.11 has a
  named `easingCurve` value type. Real interruptible springs via
  `FrameAnimation` integrator when needed; render-thread Animators for the
  always-on panel. Zero JS per animation frame.
- **Authored effects:** Qt Quick Effect Maker composes multi-node effects
  into **one** baked shader (`.qsb`), exported self-contained (tool is
  GPL-3/commercial; exports are ours). The eventual "glass composite" pass
  (blur mix + desaturate + tint + noise + SDF stroke) is a QQEM or
  hand-written `qsb` candidate.
- **Tooling gates:** `qt_add_qml_module` generates `all_qmllint`;
  `qmlformat --check`; `.qmllint.ini` per dir. These become part of the
  style's quality gate.
- **Verification constraint:** offscreen QPA renders no
  `ShaderEffectSource`/`MultiEffect` — glass is invisible in headless grabs.
  Visual proof of glass needs the real session; token/layout changes remain
  verifiable offscreen.

## 5. Audit — what "made on the fly" concretely means today

1. **Palettes swapped by comment blocks** in `CelestinaTheme.qml` (A active,
   B commented) — not selectable, not testable, drift-prone. No light scheme.
2. **Accent is white** (`#FFFFFF`) — a placeholder, not a One UI accent; it
   caused the white-on-white button bug and makes "accent" semantically
   meaningless (it collides with `text`).
3. **No elevation system**: no shadows anywhere — menus/dialogs don't float,
   they paste. No scrim/dim doctrine.
4. **Glass is close in spirit** (bounded capture, lit top edge — the right
   instincts) **but half the 8.5 recipe**: saturation *boost* instead of
   slight desaturation + dim; no noise (pyramid banding shows); outline and
   glow tuned ad hoc; consumer-driven `refreshBackdrop()` choreography (the
   menu needs 4 signal hooks + 2 `callLater` to avoid stale blur) — fragile
   API.
5. **Lit edge drawn with CPU `Canvas`** — raster repaints on resize; Qt 6.11
   does GPU gradient strokes (`Shape`/CurveRenderer).
6. **Typography undefined**: `Qt.application.font.family` (whatever
   fontconfig says — Inter is not even installed), arbitrary px sizes, no
   shipped font, no numeric `tnum`, no collapsing-header pattern.
7. **Iconography**: 14 ad-hoc monochrome SVGs + **emoji as UI glyphs** in the
   panel (`📱`, `⚡` in `celestina/qml/Panel.qml`); no coherent set, no
   grid/stroke discipline.
8. **Motion underspecified**: 3 durations + `OutBack` overshoot; no official
   curve, no press-recoil, no reduced-motion story.
9. **States incomplete**: disabled = ad-hoc opacity; no focus-ring system;
   hover/pressed tokens exist only implicitly inside components.
10. **Inconsistent adoption**: Magnetita rebuilds surfaces inline instead of
    using Glass components; the shell panel uses a hardcoded Rosé Pine
    palette while the chooser uses the module; `OutputChooser.qml` is full of
    magic layout numbers.
11. **No living gallery** — components can only be seen inside the apps, so
    regressions are discovered in production surfaces.
12. **No QML gates**: the known qmllint warning is parked; qmlformat is not
    enforced.

Current usage at stake: **21 glass instances across 11 Siderita files**, plus
Magnetita's window and the shell chooser — the migration surface is real but
bounded.

## 6. Target system

### 6.1 Token architecture v2

Three tiers, all in the typed `CelestinaTheme` singleton (compiled module,
qmllint-checkable):

- **ref.*** — primitive ramps (the SESL grays L1–L10/D1–D10, accent ramp,
  radii, type sizes, durations). Never used directly by apps.
- **sys.*** — semantic roles (what apps consume): `canvas`, `card`,
  `elevated`, `text`, `textMuted`, `accent`, `onAccent`, `danger`,
  `onDanger`, `divider`, `focusRing`, `scrim`, glass tokens, elevation
  tokens, motion tokens. **Every surface token ships its `on*` pair** — the
  contrast contract becomes explicit instead of tribal knowledge.
- **comp.*** — per-component knobs only where a component genuinely needs
  art direction (`comp.switch.trackWidth`), kept minimal.

**Schemes as data, not comments**: the palette lives in a scheme object the
singleton exposes — never again in comment-toggled blocks. Sealed (§9):
**dark only for now** — one `dark` scheme ships; the light values in §2 stay
documented for the day a light scheme earns its build; Rosé Pine retires.
Because every color hides behind a `sys.*` token, adding a scheme later is
data work (one property flip at runtime, all bindings re-evaluate once — 
verified cheap), not a migration.

### 6.2 Color mapping

Adopt the SESL values from §2 as `ref.*`, with Celestina keeping its
near-black doctrine (`#010102` window / `#17171a` cards in dark). Accent
(sealed, §9): **One UI blue `#387aff`**, used exactly as Samsung uses it —
interactive/active only — with `onAccent = #fcfcff` and the pressed/link
variants from §2. `favorite` stays the one warm exception.

### 6.3 Shape

`radiusLg 26` (dialogs, menus, grouped cards) · `radiusMd 20` ·
`radiusButton 18` · `radiusInput 22` · `radiusSm 12` · `radiusPill 9999`.
Circular corners everywhere (One UI's own in-app practice); squircle only if
an app-icon pipeline ever needs it. Radius scales down with element size.

### 6.4 Elevation & depth

| Level | Surface | Treatment |
|---|---|---|
| L0 | window canvas | opaque `canvas`, optional subtle gradient |
| L1 | grouped card / content card | opaque `card`; separation by grouping, no shadow, no hairline |
| L2 | floating: menu, tooltip, tab pills, toasts | **glass** + `RectangularShadow` (soft, large blur, low opacity, no offset drama) |
| L3 | modal: dialogs, sheets | **glass strong** + `scrim` dim behind — *never* shadow + dim together |
| Panel | layer-shell bar | **compositor glass** (ext-background-effect, x-ray) over wallpaper; in-scene fallback |

### 6.5 Glass v2

One recipe, in order: bounded capture (`sourceRect`, ≤0.5× texture) →
pyramid blur (`blurMax ≤ 32`) → **slight desaturation + scheme-tuned dim** →
tint → **noise dither** (±1–2/255, kills banding) → 1px outline (dark
outside) → **top-edge inner glow** (the existing lit edge, kept — it *is*
the 8.5 signature). Long-term the color/noise/stroke steps collapse into one
composite shader (QQEM/qsb); short-term they layer on the current pipeline.

API v2 fixes the amateur part: the surface **tracks its own scene position**
(consumers stop wiring `refreshBackdrop()` by hand); `liveCapture` stays an
explicit, documented decision; degradation (no capture → translucent tint)
stays. The GPU `Shape` stroke replaces the `Canvas`.

### 6.6 Typography

Ship **Inter Variable** (sealed, §9; OFL) **in the module** — grotesque,
huge axis/feature set, `tnum` for numerics; no dependence on what fontconfig
happens to find. Roles (starting px, tuned with screenshots):

`display 34` · `headerExpanded 30` · `headerCollapsed 20` · `title 17` ·
`rowTitle 15` · `body 13–14` · `rowSecondary 12–13` · `caption 11` ·
`mini 10`. Weights: 400 body, 600 titles (One UI uses semibold, not heavy
bold). Panel numerics get `font.features: {"tnum": 1}`. The collapsing
big-header becomes a shared component pattern (§6.8).

### 6.7 Motion

Tokens: `easeOneUi = [0.22, 0.25, 0, 1]` (the official curve — default for
everything), `easeSineInOut80/90` for expressive decelerations, linear for
opacity-only. Duration ladder: `motionFast 100` · `motionNormal 200` ·
`motionSlow 350` · ceiling 500. **Press recoil** becomes a shared behavior:
scale ≈0.96 in 100 ms, release 350 ms on `easeOneUi` — buttons, cards, rows.
Panel/popup reveals may overshoot slightly (existing `easeEmphasized` slot,
retuned). `reducedMotion` token collapses transitions to fades. Render-thread
animators for anything on the always-on panel.

### 6.8 Components v2 (specs now, built on demand — CP2 discipline holds)

Upgraded: `CelestinaButton` (three emphases — text / tonal / filled-accent,
one style per screen doctrine, recoil, focus ring), `CelestinaTextField`
(radius 22, One UI search anatomy), `GlassSurface/Card/ContextMenu/MenuItem`
(glass v2 + elevation). New specs, each waiting for its first real consumer:
**`ListSection`** (the grouped-card list — the signature; first consumer:
Magnetita Settings, later shell settings), **`CelestinaSwitch`** (35×~20 pill,
white thumb, accent track), `CollapsingHeader` (34→21 pattern),
`CelestinaDialog` (centered, 360×r26, scrim), `TabPills` (floating pill
strip), `Toast`, `CelestinaSlider`, `Tooltip`. Every component documents its
token dependencies and its states.

### 6.9 States & accessibility contract

Interactive states, uniformly: `hover` (surfaceHover), `pressed` (recoil +
surfaceStrong), `selected` (accent-tinted per 8.5's gradient language),
`disabled` (dedicated token, not ad-hoc opacity), `focusVisible` (2px
`focusRing` outside the shape — keyboard only). Contrast floors: 4.5:1
normal text, 3:1 large — checked in the gallery against every shipped scheme.
`reducedMotion` honored by every Transition.

## 7. Engineering practice upgrades

- **A living gallery** (`celestina-style/gallery/`, dev-only QML app): every
  component × every state × both schemes on one screen. The review surface
  for every style change, and the screenshot-reference source.
- **Gates**: `qmlformat --check` + `all_qmllint` join the local quality gate
  (CI once Qt enters CI); the parked qmllint warning gets fixed in S1.
- **Screenshot discipline**: reference PNGs per gallery section; glass
  verified on the real session (offscreen can't render it), tokens/layout
  offscreen.
- **Token stability**: renaming a `sys.*` token requires a deprecation alias
  for one cycle; components never reach into `ref.*`.

## 8. Phased build plan (proposal — each phase gated on approval)

- **S1 — Tokens v2 + typography.** New tiered singleton, scheme-as-data
  machinery (dark only, per §9), shipped Inter Variable, `on*` pairs, motion
  tokens, accent flipped to `#387aff`. Mechanical migration of all consumers
  (rename-only where possible). The visual delta is deliberately small; the
  structure changes completely.
- **S2 — Glass v2 + elevation.** New recipe + self-tracking API + GPU
  stroke + `RectangularShadow` elevation; migrate the 21 glass instances;
  retune menus/dialogs. This is the visible "it looks professional now" step.
- **S3 — Iconography.** Adopt Lucide (§9) behind the freedesktop-name
  mapping; kill the panel emoji; formalize the app-icon squircle template.
- **S4 — Gallery + components on demand.** Gallery app; `ListSection` +
  `CelestinaSwitch` land with Magnetita Settings as first consumer; button
  emphases + recoil.
- **S5 — Panel compositor glass.** `celestina` requests ext-background-effect
  blur (KWindowSystem or ~100-line extension), x-ray mode, in-scene fallback;
  the panel finally *is* glass over the desktop.

## 9. Decisions (sealed by the author, 2026-07-27)

1. **Accent — One UI blue `#387aff`.** Interactive/active elements only,
   `onAccent #fcfcff`. The white-accent era (and its contrast trap) ends.
2. **Typeface — Inter Variable** (OFL), shipped inside the module.
3. **Icon set — Lucide** (ISC), adopted behind the freedesktop-name mapping
   layer so consumers keep resolving by name.
4. **Schemes — dark only for now.** Scheme machinery is data-driven from S1;
   the light reference values stay recorded in §2 for whenever a light
   scheme earns its build. Rosé Pine retires.

## 10. Sources

Samsung: One UI Design Guidelines PDF (design.samsung.com) · One UI
developer guide (developer.samsung.com/one-ui: color/system, comp/list,
comp/button, comp/dialog, iconography, motion/basic, structure/visual-depth,
accessibility) · SESL mirrors: github.com/tribalfs/sesl-androidx,
github.com/tribalfs/oneui-design, github.com/OneUIProject/oneui-core ·
One UI 8.5 coverage 2025–2026: Samsung Newsroom, SamMobile, 9to5Google,
Android Authority, Sammy Fans, SammyGuru, Android Police (glass/QS redesign,
floating tab bar, 3D icons, DeX).

Qt/Wayland: doc.qt.io — MultiEffect, RectangularShadow, Shape/PathRectangle,
ShaderEffect, qsb/qt_add_shaders, Text (variable fonts, rendering), easing,
FrameAnimation, qt_add_qml_module/qmllint, whatsnew 6.8–6.11 · qt.io blogs:
"Qt Quick and blurred panels", "A short guide to Qt Quick effects", "QQEM
6.8", "RectangularShadow in 6.9", "Text improvements in 6.7" ·
wayland.app/protocols/ext-background-effect-v1 · niri 26.04 release notes +
Window-Effects docs · KDE/kwindowsystem (ext-background-effect client) ·
github.com/OliverZhaohaibin/Qt-liquid-glass-widgets (MIT liquid-glass
reference).
