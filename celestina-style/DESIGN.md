# CelestinaStyle — design contract

- **Status:** accepted current visual contract
- **Language:** Samsung One UI 8.5 adapted to a pointer-driven Niri desktop
- **Runtime floor:** Qt 6.9; newer APIs mentioned below are observations, not a
  silent increase of the declared minimum
- **History:** the S1-S6 migration, audits, measurements and dated evidence are
  preserved in the
  [roadmap archive](docs/history/roadmap-through-2026-08-03.md)

This document defines visual semantics and component behaviour. Current
implementation state belongs to [STATUS.md](STATUS.md), implementation work to
[ROADMAP.md](ROADMAP.md), and compositor/perceptual/assistive-technology checks
to [VALIDATION.md](VALIDATION.md).

QML reserves `on<Capital>` names for signal handlers. Surface/foreground pairs
therefore use `<surface>Ink` in the public API (`accentInk` is the accepted
spelling of `onAccent`); the semantic pairing remains mandatory.

## 1. Direction

Celestina uses calm near-black neutrals, one restrained interactive accent,
generously rounded grouped cards, comfortable type under large page headers and
frosted glass reserved for floating layers. Samsung's desktop posture in DeX —
transparent floating taskbar, popup surfaces and flat borderless windows — is
the reference for translating the language to a PC.

The adaptation follows five rules:

1. Reachability patterns do not transfer blindly. Dialogs are centred; bottom
   sheets, bottom search and floating back buttons are not desktop defaults.
2. Pointer interaction is first-class. Every interactive surface has a defined
   hover and pressed state.
3. Keyboard interaction is first-class. Focus visibility, order, containment
   and restoration are part of the component contract.
4. A 34→21 collapsing header is suitable only for a scrolling page with enough
   height; compact windows start collapsed.
5. Samsung dp values map 1:1 to logical px as a baseline. Responsive screen
   geometry remains local to each product.

## 2. Reference values

The reference values come from Samsung's shipped SESL/One UI support libraries
and official design guidance. Celestina adapts them through semantic tokens; an
application never consumes this table as raw literals.

### Shape and structure

- Master radius: 26 for dialogs, menus and grouped list cards.
- Buttons: 18; search/input: 22; medium surfaces: 20; small items: 12.
- Pills are reserved for switches, chips and floating tab/navigation strips.
- In-app corners are circular. Squircle/superellipse belongs only to app icons.
- Grouped rounded-card lists are the primary settings/list pattern. Hierarchy
  comes from grouping and whitespace, not a forest of separators.
- Side margins start at 24 logical px and adapt to the surface.

### Reference palette

| Role | Dark reference | Light reference |
|---|---|---|
| Window | `#010102` | `#f1f1f3` |
| Card/content | `#17171a` | `#fcfcff` |
| Elevated | `#2d2d30` | `#f6f6f8` |
| Primary text | `#fafaff` | `#010102` |
| Secondary text | `#99999e` | `#848487` |
| Divider | `#3a3a3d` | `#e4e4e7` |
| Accent | `#387aff` | `#387aff` |
| Danger/success/warning | `#fc6c65` / `#58db9c` / `#fc864c` | `#d93e36` / `#11a85f` / `#e65b17` |

Celestina currently ships one dark scheme with its own tuned values. The light
column is reference material only; it is not a claim that a light scheme is
implemented.

### Typography, glass, depth and motion

- Reference type: variable grotesque, weights 400 for body and 600 for titles;
  expanded header 34→21, row title 17, secondary 13, dialog title 17, body 14,
  buttons 15 and toast 14.
- Glass: backdrop blur, slight desaturation and dim, tint, restrained noise, a
  thin dark outline and a top-edge inner glow. It is frosted/matte, not an
  Apple-style refractive lens.
- Samsung's current visual-depth guidance treats blur and dim as restrained
  hierarchy tools rather than decoration to apply everywhere. Celestina follows
  that boundary with two opt-in shell roles: dense matte material only for
  information-bearing content cards and panel capsules, and a nearly
  transparent veil for the contextual carrier. The numeric strengths are a
  measured adaptation of the author's reference crop, not published Samsung
  constants. See [Samsung visual depth](https://developer.samsung.com/one-ui/structure/visual-depth.html)
  and [Samsung basic layout](https://developer.samsung.com/one-ui/layout/basic.html).
- Depth uses blur paired with dim, dim alone or a soft low-opacity shadow.
  Never combine modal dim and a depth shadow on the same surface.
- Default motion curve: cubic-bezier `(0.22, 0.25, 0, 1)`; expressive variants
  use SineInOut80 `(0.33, 0, 0.2, 1)` or SineInOut90
  `(0.33, 0, 0.1, 1)`. Durations remain between 100 and 500 ms.
- Press recoil is approximately 0.96 scale in 100 ms with a 350 ms release.
  Linear timing is reserved for opacity-only changes.

## 3. Platform contract

The compiled module requires Qt 6.9. The author's Qt 6.11/Niri 26.04
environment established an upper capability observation, not the portable
floor.

- A layer-shell host may request real compositor glass through
  `ext-background-effect-v1` and render `GlassSurface.ExternalBackdrop` above
  that result. `GlassSurface` owns the shared tint, noise, outline, lit edge and
  fallback; the host remains the sole owner of the compositor effect and its
  region.
- Application windows cannot sample other clients. They use bounded in-scene
  capture instead; an external-backdrop surface must never invent a
  `ShaderEffectSource` for another Wayland client.
- In-scene glass uses `ShaderEffectSource`/`MultiEffect` with a bounded
  `sourceRect`, downsampled pyramid blur and explicit update scheduling. Keep
  `blurMax` at or below 32 and use noise to dither banding.
- `liveCapture` is an explicit cost decision. The accepted default is
  event-driven recapture on show, move or resize; per-frame GUI-thread
  resampling is prohibited for an idle always-on surface.
- `RectangularShadow` is the Qt 6.9 floor for soft elevation. Shape paths and
  gradient strokes use GPU-backed `Shape` where available; CPU `Canvas` is not
  the shared lit-edge implementation.
- Inter Variable and `font.features` provide typography and tabular numerics.
  On Wayland, use Qt/curve text rendering where fractional scaling would make
  native hinting unstable.
- Animation performs no JavaScript work per frame. Always-on surfaces prefer
  render-thread animators; an interruptible spring needs a bounded integrator.
- An authored composite shader may replace multiple glass passes only when it
  preserves the same degradation and minimum-Qt contract.
- Offscreen QPA does not visually prove `ShaderEffectSource`, `MultiEffect` or
  compositor blur. Headless output may prove construction/layout only.

## 4. Token architecture and colour

`CelestinaTheme` is a typed singleton with three conceptual tiers:

- `ref.*`: primitive ramps and seeds, never consumed directly by applications.
- `sys.*`: semantic roles such as canvas, card, elevated, text, textMuted,
  accent, danger, divider, focusRing, scrim, glass, elevation and motion.
- `comp.*`: stable component anatomy such as button/field padding, switch,
  checkbox, slider and indicator metrics. Screen geometry is not a token.

Every semantic surface carries a matching ink role. Contrast is checked on the
actual painted pair rather than inferred from names.

The accepted dark mapping starts from `#050608` canvas, `#14171c` grouped card,
`#1a1e25` strong tonal, `#222831` elevated, `#f7f8fc` primary text and
`#9ba3af` secondary text. The sealed interactive accent is `#3e91ff`, with
dark `accentInk = #050608`. `accentLift = #fcfcff` is derivation input and is
never painted as body-size ink on the bright accent.

Accent is the only interactive hue seed. Link, hover, pressed, focus and accent
washes derive inside `CelestinaTheme`, never in a consumer. `favorite` is the
one warm product exception. A closed semantic ink palette may distinguish
informational content glyphs, but it never colours surfaces, labels, thumbnails
or selection state.

Schemes are data, not comment-swapped blocks. Only the dark scheme currently
ships. A future light scheme requires a complete token set, consumer evidence
and its own accepted checkpoint.

## 5. Visual system

### 5.1 Shape

`radiusLg 26` · `radiusMd 20` · `radiusButton 18` · `radiusInput 22` ·
`radiusSm 12` · `radiusPill 9999`. Radius scales down with element size.

### 5.2 Elevation and surfaces

| Level | Surface | Treatment |
|---|---|---|
| L0 | Window canvas | Opaque `canvas` and the canonical subtle `CelestinaBackdrop` gradient |
| L1 | Grouped/content card | Opaque semantic surface, whitespace and one quiet outline; no shadow |
| L2 | Menu, tooltip, tab pills, toast | Regular glass plus soft shadow |
| L3 | Dialog/modal | Strong glass plus scrim; no simultaneous depth shadow |
| Shell content card / panel capsule | Layer-shell surface | One host-owned compositor blur region, or one region shared by the complete menu, with dense shadowless `ContentSurface` material |
| Contextual menu carrier | Layer-shell surface | The same single host-owned compositor blur region with a nearly transparent shadowless `ContextualVeil` and a readable fallback |

`CelestinaSurface` owns L0/L1 fill, ink, radius and quiet outline. Consumers
choose a semantic role (`Canvas`, `Panel`, `Grouped`, `Content`, `Tonal`,
`Elevated`, `Selected`) and own only layout, size and content. Raw `Rectangle`
remains valid for masks, thumbnails, progress and tiny indicators, not as a
parallel public styling API.

Compositor glass has separate tint and fallback roles because the scene below
it is hostile input. They are checked after compositing over black and white.
One host region may support several `GlassSurface.ContentSurface` sections;
those sections and panel capsules use the same dense matte material without
multiplying compositor regions or capturing their own window. The menu's
`ContextualVeil` attenuates tint, noise, outline and lit edge together so the
outer field remains only an organizing trace. Both shell roles have zero
elevation; the general-purpose default material remains compatible for every
other suite consumer. A real session is still required to prove that blur
itself is active.

### 5.3 Glass

For `InSceneCapture`, the accepted order is bounded capture (approximately
0.5× texture), pyramid blur, slight desaturation and scheme-tuned dim,
Regular/Strong tint, ±1–2/255 noise, 1 px exterior outline and restrained
top-edge glow. `ExternalBackdrop` omits only the capture and blur passes because
the compositor supplies them; it retains the same material ordering. Failure
to capture or supply an external backdrop degrades to a readable translucent
tint.

`StandardMaterial` preserves that existing full-strength recipe.
`ContentSurface` applies the reference-derived `0.64` strength to the complete
decorative stack and pairs its neutral material polarity with the host's
foreground polarity. `ContextualVeil` applies `0.12` to the same stack; because
its normal highlight tint is itself translucent, the usual visible tint is
approximately two percent. These values describe Celestina's adaptation of the
supplied One UI 8.5 image, not a claim about Samsung's private implementation.

The surface recaptures on its own size change. A movable host explicitly rearms
on show or position change. Wheel/pointer ownership and lifecycle remain the
host's responsibility unless a component below says otherwise.

### 5.4 Typography

Inter Variable (OFL) ships in the module; no public component depends on an
accidental fontconfig choice. Roles are:

`display 34` · `headerExpanded 30` · `headerCollapsed 20` · `title 17` ·
`rowTitle 15` · `body 13–14` · `rowSecondary 12–13` · `caption 11` ·
`mini 10`.

Body uses weight 400 and titles 600. Panel numerics enable `tnum`. A mono face
or fallback must be declared by the public font contract before a component
depends on it.

### 5.5 Motion

`easeOneUi` is the default curve. The duration ladder is `motionFast 100`,
`motionNormal 200`, `motionSlow 350`, with a hard 500 ms ceiling. Recoil applies
only when it does not compromise pointer precision or content stability.

Hosts inject `CelestinaTheme.reducedMotion` from
`CELESTINA_REDUCED_MOTION`. Spatial and scale motion becomes instant or is
disabled. An opacity fade may remain only when the component deliberately
specifies it. Every new or modified `Behavior`/`Transition` must expose this
route.

### 5.6 Iconography

UI glyphs use Lucide through the `CelestinaIcons` semantic-name mapping and one
flat stroke colour. A consumer resolves semantic names; it does not address an
asset path or draw a competing glyph.

Content icons are filled objects. Folder anatomy is owned in-tree; file-type
shapes derive from Phosphor's filled set through `CelestinaIconShapes`. Both
catalogues answer the same semantic names and fall back to the stroke glyph
rather than disappearing.

Content washes are derived in OKLCH and mapped into gamut by reducing chroma;
never lighten/darken in HSL or clip sRGB channels independently. Sheet, pocket
and emblem ink pair with the surface beneath them, and the guard enforces a 3:1
non-text floor against canvas/card/elevated. Never mask a gradient into a
stroked glyph: resampling makes the stroke fat and jagged.

## 6. Component contract

`qmldir`, CMake/QRC and every in-tree linked consumer must expose the same
public type inventory. A public type is version 1.0 until an accepted
compatibility policy changes that contract.

### 6.1 Exported components

| Component | Contract |
|---|---|
| `CelestinaTheme` | Semantic colour, metric, type, motion and reduced-motion source of truth |
| `CelestinaIcons` | Stable semantic UI-icon lookup with a visible fallback |
| `CelestinaIconShapes` | Stable filled content-shape catalogue; consumers do not call its path data directly |
| `CelestinaSurface` | L0/L1 semantic container; role owns fill/ink/radius/outline |
| `CelestinaBackdrop` | Canonical quiet window background; no product state |
| `CelestinaButton` | Tonal, filled-accent, destructive, selected and ghost emphasis; compact/regular/prominent density; keyboard focus ring and recoil |
| `CelestinaIconButton` | Icon-only action with the same emphasis, tooltip/name and focus requirements as a button |
| `CelestinaIcon` | One name/fallback/tone API for Lucide-style UI glyphs |
| `CelestinaSectionLabel` | Semantic section heading with shared type/spacing, not product navigation state |
| `CelestinaFocusRing` | Reusable 2 px exterior ring shown for `visualFocus`, never merely for pointer focus |
| `CelestinaTextField` | Radius-22 search/input anatomy, clear focus/error/disabled states and accessible naming |
| `CelestinaSlider` | Shared track/fill/focus/keyboard anatomy plus a separate requested-but-unconfirmed mark |
| `CelestinaSwitch` | Desktop-tuned 44×26 pill, shared inset/thumb/track tokens, white thumb and accent track when on |
| `ListSection` | Grouped-card list anatomy; row data/actions remain with the host |
| `CelestinaInputShield` | Floating surface owns pointer hover/buttons/drag over its own box; wheel deliberately remains available to content unless the host overrides it |
| `CelestinaModalLayer` | Scrim, input shielding, focus containment/restoration and modal accessibility floor |
| `CelestinaFolderIcon` | Filled in-tree folder shape with semantic tone and contrast-safe internal ink |
| `CelestinaFileIcon` | Filled semantic file-type shape with stroke fallback |
| `GlassSurface` | Regular/Strong material over bounded in-scene capture or an explicit compositor-supplied backdrop, with a readable fallback |
| `GlassCard` | Glass surface with shared card anatomy/elevation, no application state |
| `GlassContextMenu` | Floating menu container with focus/input ownership and event-driven recapture |
| `GlassMenuItem` | Keyboard/pointer-operable menu row with role/name/state and semantic ink |

One screen uses a coherent button emphasis hierarchy rather than several equal
primaries. `CelestinaSlider` owns the pending mark because media requests are
not confirmed positions; the consumer owns labels, units and playback wording.

### 6.2 Specified, not exported

These are accepted shapes, not an implementation backlog. Add one only with a
real consumer and update `qmldir`, CMake/QRC, gallery, status and affected
consumer evidence in the same checkpoint.

| Component | Accepted specification |
|---|---|
| `CollapsingHeader` | Page-owned 34→21 hierarchy; compact windows start collapsed and scroll owns the transition |
| `CelestinaDialog` | Centred, approximately 360 px wide, radius 26, modal scrim, contained/restored focus and explicit primary/cancel semantics |
| `TabPills` | Floating pill strip for peer destinations; not a substitute for document-tab lifecycle |
| `Toast` | Brief non-modal status, readable without focus theft and announced when semantically important |
| `Tooltip` | Delayed pointer/keyboard label for an existing control; never the only source of required instructions |

## 7. States and accessibility

Every interactive component defines `hover`, `pressed`, `selected`, `disabled`
and `focusVisible` when those states apply. Disabled uses a dedicated semantic
token, not arbitrary opacity. Selection uses an accent-derived treatment and
keyboard focus uses the exterior focus ring.

Normal text meets at least 4.5:1 and large text 3:1 in every state. Meaningful
non-text shapes meet 3:1 against their actual surface. Static contrast checks
cover deterministic pairs; hostile artwork/wallpaper and real assistive
technology remain author validation.

Actions expose an accessible role, name, state and action. Lists, tabs,
selection, progress, errors, switches and sliders expose semantic state rather
than appearance alone. Modal components contain focus, disable the lower
surface and restore focus to the exact invoker.

## 8. Engineering and verification contract

- `celestina-style/gallery/` is the living review surface for every exported
  component and applicable state. A style change updates it with the component.
- `qmldir`, `CMakeLists.txt`, resources and consumer registrations stay in
  parity. Shared source consumption is by canonical relative link, never copy.
- `qmlformat --check`, `all_qmllint`, the style contract guard, contrast checks,
  production build and affected consumers form automated evidence according to
  [the verification standard](../docs/standards/verification.md).
- The exact canonical artifact is produced by `scripts/build-production.sh`
  and checked by `scripts/verify-production.sh`. CelestinaStyle is not
  deployable; `scripts/status-production.sh` reports artifact provenance.
- Appearance, compositor glass, motion perception, physical keyboard focus and
  AT-SPI are recorded separately in [VALIDATION.md](VALIDATION.md).
- Components never reach into `ref.*`. Until the compatibility checkpoint is
  closed, renaming a public semantic token or component is a breaking change
  requiring explicit author approval and all-consumer migration.

## 9. Sealed decisions

Sealed by the author on 2026-07-27; accent tuned on 2026-07-28:

1. **Accent:** One UI-adapted blue `#3e91ff`, interactive/active only, with
   `accentInk #050608`; `#fcfcff` is derivation input, not foreground ink.
2. **Typeface:** Inter Variable (OFL), shipped inside the module.
3. **UI icon set:** Lucide (ISC), behind the semantic-name mapping.
4. **Schemes:** dark only for now. Light reference values remain documented,
   but a light implementation requires a future accepted checkpoint.

Changing a sealed decision requires an explicit new decision record; a local
component or app may not reinterpret it.

## 10. Sources

Samsung: One UI Design Guidelines and developer guide; SESL mirrors
`tribalfs/sesl-androidx`, `tribalfs/oneui-design` and
`OneUIProject/oneui-core`; Samsung Newsroom and contemporary One UI 8.5/DeX
coverage for the glass/floating-layer direction.

Qt/Wayland: Qt documentation for MultiEffect, RectangularShadow,
Shape/PathRectangle, ShaderEffect, text variable axes/features, easing,
FrameAnimation, QML modules and qmllint; Qt effect/blur guidance; the
`ext-background-effect-v1` protocol; Niri 26.04 window-effects documentation;
KWindowSystem's compositor-effect client support.
