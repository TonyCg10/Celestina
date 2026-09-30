# Suite — Haze glass, desktop scale and one top bar

- **Date:** 2026-09-30
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Scope:** `celestina-style` and its five desktop consumers: Grafita,
  Hematita, Fluorita, Magnetita and Siderita
- **Out of scope:** the halted `celestina` shell (its `ContentSurface` and
  `ContextualVeil` roles keep compiling and lose only the edge layers every
  role loses); `magnetita-android`, which is the reference and does not change
- **Proposed units:** `STYLE-G7-O` and `STYLE-G7-P` in the active style plan
  and `AUD-1-I` in the active suite plan (see §6 for why the style prefix
  cannot own the guard), then one unit per application in its own roadmap,
  in the order of §6

## 1. The problem, as the author demonstrated it

The author compared the five desktop applications against the phone side of
Magnetita, whose floating tab pill uses Haze 0.7 with its defaults, and named
three defects.

**The glass is not the same glass.** Haze does three things: a 20 dp blur, the
background colour laid over it at 70 %, and grain at 0.15. `GlassSurface`
does eight: a much wider blur (`blurMax 32` × multiplier 3 over a 55 %
capture), a `#1a1e25` tint at 60 % or 74 %, slight desaturation, 2.5 % grain,
a 1 px dark outline and a lit top edge. The desktop menus and dialogs read as
outlined cards; the phone pill reads as matte glass. The author wants the
phone's glass, exactly, and no glass in more places than today.

**The scale is a phone's.** Grouped cards at radius 26 with 54 px rows, a
centred uppercase page title (Siderita), a floating navigation pill above the
content (Hematita), a floating view-mode pill below it (Siderita), a large
eyebrow-plus-title header and loose circular action buttons (Magnetita). On a
1440 px monitor these read as a tablet layout inside a window.

**Radii were never checked against their insets.** DESIGN.md §5.1 states the
concentric rule (outer radius = inner radius + inset) and only
`CelestinaTreemap` and `CelestinaUsageList` apply it. Measured in the
screenshots: Fluorita's grid tiles (radius 20) draw the file name edge to
edge along the bottom, so the corner arc clips the first and last glyphs;
Fluorita's now-playing row starts its icon 10 px inside a radius-26 surface;
Siderita's sidebar rows (radius 12) sit 8 px inside a radius-26 card;
Magnetita's activity card cuts its last row with the card's corner. Siderita
carries 143 numeric margin literals and Magnetita 89, so none of this is
governed by a token.

**Layouts follow implementation order.** Magnetita's `DevicesPage` stacks, top
to bottom: a "no phone" line, the pairing sheet, a "service unavailable" line,
the call banner, the device cards, a row of six circular buttons, the pairing
code, the mirror settings sheet, the media card and the activity log. Messages
and Settings are two header buttons that replace the whole page. Fluorita
stacks an image grid and a music list in one page depending on the folder.

## 2. Decisions the author sealed in brainstorming

1. **Pure Haze.** The glass recipe becomes blur, tint, grain and nothing else.
   The dark outline and the lit edge are removed from every glass surface.
   Glass stays where it is today: floating layers only (menus, dialogs, pills,
   docks). No application window becomes translucent and no card becomes
   glass.
2. **Icon-only actions, labelled navigation.** Toolbars, top bars and action
   rows use `CelestinaIconButton` with `helpText` only. Destinations (sidebar
   rows, page tabs, library folders) keep icon plus label. `CelestinaButton`
   with text survives only inside dialogs.
3. **One fixed top bar per application**, spanning the sidebar and the
   content, with navigation at the left, a left-aligned title, and icon-only
   actions at the right. Floating pills, centred headers and loose circular
   buttons are removed.
4. **Desktop density**, option 1 of the token table in §3.2: cards 20, rows
   40, top bar 48, window margin 16.
5. **Reorganisation per application** is part of each application's unit,
   not a later pass.
6. **Verification** is an automatic guard for radius insets and concentric
   nesting, plus gallery screenshots, plus a validation row per application
   for what only a real session shows (blur and grain).

## 3. Style module: `STYLE-G7-O`, `AUD-1-I`, `STYLE-G7-P`

### 3.1 The Haze recipe in `GlassSurface`

| Layer | Today | New | Verified by |
| --- | --- | --- | --- |
| Capture | `sampleScale 0.55` | `sampleScale 1.0` (the downsample was part of the blur width) | token |
| Blur | `blur 1.0`, `blurMax 32`, `blurMultiplier 3.0` | `blur 1.0`, `blurMax 24`, `blurMultiplier 1.0` | gallery reference, see below |
| Desaturation | `glassSaturation -0.03` | `0` | token |
| Tint | `glassTint #991a1e25`, `glassTintStrong #bd1a1e25` | one value: `canvas` at 0.70 (`#b3050608`); `glassTintStrong` becomes an alias of it | `check-contrast-contract.py`, composited over black and white as today |
| Grain | `icons/glass-noise.png` at `glassNoiseOpacity 0.025`, over the tint | Haze's own noise texture (`haze_noise.webp`, 1.6.10, converted to PNG), vendored under `icons/` with its Apache-2.0 notice, at `0.15`, **under** the tint: Haze composes blur, then noise, then tints | same file, same alpha, same order as the Android side; a test asserts the order |
| Dark outline | 1 px `glassOutline` | removed; `celestina-glass-outline` and `celestina-glass-silhouette-outline` no longer exist | `tst_glasssurface.qml` asserts the object names are absent |
| Lit top edge | `Shape` gradient ring | removed; `celestina-glass-lit-edge` and the silhouette lit edge no longer exist | same test |
| Shadow | `CelestinaShadow` at `elevation 2` | unchanged; the Android pill carries `shadowElevation 8.dp` | unchanged test |
| Fallback | `surfaceStrong` translucent | `canvas` opaque, which is Haze's `backgroundColor` behaviour when it cannot blur | `tst_glasssurface.qml` |

The blur target is numeric: Android turns a 20 dp blur radius into a Gaussian
of σ ≈ 12 px (`sigma = 0.57735 × radius + 0.5`). The gallery's glass page
gains a reference panel: the same backdrop image passed through a σ = 12
Gaussian by `scripts/glass-reference.py` (Pillow, offline, run by hand),
tinted and grained with the same numbers, shown beside the live
`GlassSurface`. Calibrating `blurMax` is done against that panel and the
chosen value is recorded in the unit's evidence with both crops. The runtime
never reads the reference image.

API stays compatible: `Density.Strong`, `MaterialRole`, `BackdropMode`,
`silhouettePath`, `elevation`, `materialTint`, `materialOpacity` and every
signal keep their names and types. `Density.Strong` paints identically to
`Regular`. `ContentSurface` keeps its `0.64` strength and `ContextualVeil`
its `0.12`; both simply have no edge layers left to suppress, so
`materialEdgesVisible` is removed as a public property. The tokens
`glassOutline`, `glassBorder` and `glassEdge*` are deleted from the theme;
the style guard already refuses a consumer that names a token the theme
lacks. `glassHighlight` stays because it is the veil role's tint and the
halted shell reads it. `glassSaturation` and `glassBlurMultiplier` stay as
names with the Haze values (`0` and `1.0`) because Magnetita's media card and
Fluorita's ambient light read them for blurs of their own. A new
`glassFallback` names the opaque canvas the surface paints when it cannot
blur.

### 3.2 Token scale

Existing names keep their meaning so no consumer renames anything; the values
change.

| Token | Today | New | Used for |
| --- | --- | --- | --- |
| `radiusLg` | 26 | 20 | grouped cards, sidebars, dialogs, menus |
| `radiusMd` | 20 | 12 | rows, tiles, content surfaces |
| `radiusSm` | 12 | 8 | chips, thumbnails, small indicators |
| `radiusXs` | 3 | 3 | marquee, tiny indicators |
| `radiusButton` | 18 | 10 | text buttons (dialogs only) |
| `radiusInput` | 22 | `radiusPill` | search and text fields |
| `radiusWindow` | 14 | 14 | unchanged, compositor-owned |
| `rowHeight` | 54 | 40 | one-line list rows |
| `rowHeightLg` | 66 | 52 | two-line rows |
| `windowMargin` | 14 | 16 | window edge to first surface |
| `iconMd` | 19 | 20 | action glyphs, top-bar buttons |
| `iconLg` | new | 24 | navigation glyphs: sidebar rows, segments |

New tokens:

| Token | Value | Meaning |
| --- | --- | --- |
| `spaceCardGap` | 12 | between sibling cards |
| `spaceCardInset` | 8 | from a card's edge to its rows or tiles |
| `topBarHeight` | 48 | the fixed top bar |
| `compSegmentHeight` | 32 | segmented control, 8 px of air inside the bar |
| `glassFallback` | `canvas` | what glass paints when it cannot blur |
| `cornerInset(radius)` | `Math.ceil(radius * 0.3)` | minimum inset for text or a glyph that touches a corner |

The bar's icon buttons are ordinary `CelestinaIconButton`s at their
`controlHeightXs` circle with `iconSize: iconMd`; no separate size token.

`cornerInset` is the horizontal reach of a circular corner at the height
where a cap-height glyph starts (`r(1 − 1/√2) ≈ 0.29r`), rounded up. With this
ladder the concentric rule holds without arithmetic in consumers: a card
(20) inset `spaceCardInset` (8) holds rows (12); a row or tile (12) inset
`spaceXs` (4) holds thumbnails (8).

`CelestinaSurface` roles map to the new ladder unchanged: `Grouped`, `Panel`,
`Elevated` at `radiusLg`; `Content`, `Tonal`, `Selected` at `radiusMd`.
`CelestinaTextField` moves to `radiusPill` and keeps its height. `ListSection`
takes `spaceCardInset` between its card and its rows.

### 3.3 The radius guard

`scripts/check-architecture-contract.sh` gains a rule, `scripts/radius_contract.py`,
a Python module beside `architecture_scanners.py` with its own hermetic
tests, that walks every QML root the registry declares (applications and the
style module) and, for each object whose `radius`/`cornerRadius` is a theme
token (`CelestinaTheme.radius*`), including a rounded `background:` object of
a control:

- **Inset rule.** A direct child whose `padding`, `anchors.margins`,
  `leftPadding`/`rightPadding`, `anchors.leftMargin`/`rightMargin` or `x`
  resolves to a token smaller than `cornerInset(radius)` fails, unless the
  child is `CelestinaFocusRing` (DESIGN §5.1 lets only a focus ring enter the
  inset). A child anchored to `verticalCenter` with a numeric `y` of zero is
  not a corner contact and passes.
- **Concentric rule.** A direct child with its own token radius fails unless
  `parentRadius == childRadius + inset`, where inset is the child's token
  margin, or unless the child's radius is `radiusPill` (a pill inside any
  larger radius is always concentric enough).
- **Literal rule.** Inside such an object, a numeric literal for any of the
  properties above is refused, the way a numeric alpha already is; the
  message names the token ladder.

Failures print `path:line: rule: detail`. Findings are counted per project
against a new `scripts/radius-baseline.tsv` (`findings<TAB>project`), the
shape `qmllint-baseline.tsv` has, registered in `docs/projects.toml` as a
shared ratchet file so every application prefix may lower its own row in the
commit that pays the debt. `STYLE-G7-O` therefore lands with the five
applications' current debt recorded, and each application's unit takes its
row to zero. The baseline shrinks only; the guard refuses a baseline row that
exceeds what it finds.

### 3.4 New exported components

Both are accepted shapes in DESIGN.md §6.2 (`TabPills`, and the header slot
`CollapsingHeader` leaves empty for compact windows). Five consumers satisfy
the "one real consumer" rule.

**`CelestinaTopBar`.** A `Pane` of `topBarHeight`, fill `canvas`, no card, no
glass; it is window chrome, not a floating layer. It spans the full window
width above sidebar and content. Three slots:

- `leading` (default alias `leadingData`): navigation, left-aligned. A
  `CelestinaSegmentedControl`, a back button, or empty.
- `title` and `subtitle` (strings): `fontTitle` 17 weight 600 and
  `fontRowSecondary` 12 in `textMuted`, left-aligned after `leading`, eliding
  right. Never centred, never uppercased.
- `trailing` (alias `trailingData`): icon-only actions, right-aligned,
  `spaceXs` apart, each a `CelestinaIconButton` with `iconSize: iconMd`. A
  `CelestinaButton` inside `trailingData` fails the style guard.

Side margins `windowMargin`. The bar exposes `Accessible.role: ToolBar` and
takes no keyboard focus of its own; each button remains its own Tab stop in
reading order.

**`CelestinaSegmentedControl`.** A row of segments (icon plus label, or icon
only when `iconOnly: true`) inside a `radiusButton` surface filled `card`;
the active segment paints `surfaceSelected` at `radiusButton −
spaceXs` (concentric). Height `compSegmentHeight`. One Tab stop; Left/Right
and Home/End move, activation is immediate (a segment is a page, not a
choice awaiting confirmation). `Accessible.role: PageTabList` on the control
and `PageTab` with `selected` on each segment. Model: a list of
`{ key, icon, label }`, the shape Hematita's `NavStrip` already uses.

Nothing else is added: no `CelestinaDialog`, `Toast`, `Tooltip` or
`CelestinaSidebar`. Icon names still reach people only through `helpText`, as
sealed.

### 3.5 Gallery, tests, documentation

- `gallery/Gallery.qml`: the glass page gains the σ = 12 reference panel; a
  new page shows the top bar in its three configurations and the segmented
  control in both modes; every existing card and row shows the new ladder.
- `tests/`: `tst_topbar.qml` and `tst_segmented.qml` (geometry, focus,
  accessible name and role, reduced motion); `tst_glasssurface.qml` updated
  for the absent edge objects and the opaque fallback.
- `scripts/test-radius-guard.sh` with fixtures for each rule and for the
  baseline behaviour.
- DESIGN.md: §2 reference values, §5.1 shape, §5.2 elevation table, §5.3
  glass and §6.1 components rewritten to state this contract; §6.2 loses
  `TabPills` and gains nothing. STATUS, ROADMAP and VALIDATION rows as the
  governance standard requires. Version: MINOR bump (two new public types,
  deleted tokens are a break the author approves in this spec).

## 4. Shared rules for every application unit

- The top bar replaces every floating pill, centred header and loose action
  button. Sidebar and content sit below it, `windowMargin` from the window
  edge and `spaceCardGap` from each other.
- Actions become `CelestinaIconButton` with `helpText`; only dialogs keep
  text buttons.
- Margin and padding literals inside rounded surfaces become tokens; the
  application's rows in `radius-baseline.tsv` are removed in the same unit.
- Rows are `rowHeight` 40 (`rowHeightLg` 52 for two lines); sidebars are one
  `CelestinaSurface.Grouped` at `radiusLg` with rows at `radiusMd` inset
  `spaceCardInset`.
- Evidence: a before/after screenshot pair per window in `docs/evidence/`,
  taken with the window focused (the author's Niri rule dims unfocused
  windows to 0.75), and the guard output. `VALIDATION.md` gains one row for
  the glass in a real session.
- Exit: `complete-production.sh`, a PATCH or MINOR bump per the project's
  policy, and the inventory.

## 5. Per-application map

### 5.1 Grafita

- `TabStrip` becomes the bar's `leading`: document tabs as labelled segments
  with a close glyph, the new-tab button last.
- `DocumentFooter` is removed. Undo, redo, find, close and save move to
  `trailing`; save keeps `Primary` emphasis.
- `FindBar` stays as its own strip under the bar; it is already icon-only.
- `UnsavedDialog` and `EncodingDialog` keep their text buttons and take the
  Haze `GlassCard`.

### 5.2 Hematita

- `NavStrip` becomes a `CelestinaSegmentedControl` in `leading`, icon plus
  label, same model.
- The bar title is the active page; the subtitle is the datum each page
  currently paints in its own header (the processor name, the mount point,
  the sensor source).
- Page actions scattered in internal headers (services filters and
  start/stop/restart, storage duplicates/empties/scan) move to `trailing`
  and change with the page.
- The resource column and detail cards keep their structure at the new
  ladder; device names no longer truncate at the row width.

### 5.3 Fluorita

- The search field (pill, centred) and the thumbnail action move into the
  bar; the item count becomes the subtitle.
- `LibraryView` stops stacking two collections. `leading` holds a segmented
  control with two views, gallery and music, each with its own list. The
  synthetic "all" folder is removed because it mixed two anatomies.
- Grid tiles go to `radiusMd` 12 with the name inset `spaceXs` 4; the
  now-playing row goes to `radiusMd` with `cornerInset` before its icon.
- `ContentDock`, `EditToolbar` and `PlayerTransport` stay as floating Haze
  glass; they are already icon-only.

### 5.4 Magnetita

From one column in implementation order to sidebar plus content.

- **Sidebar**, `CelestinaSurface.Grouped`: a `PHONES` section with one row per
  paired device (name, state, battery) and a fixed "pair another" row; below
  it `MESSAGES` and `SETTINGS` as labelled destinations. `AppHeader` (eyebrow,
  30 px title, two page-swapping buttons) is deleted.
- **Bar**: title is the selected device, subtitle its state and mount path.
  `trailing` carries the six `DeviceControls` actions in frequency order:
  files, mirror, mirror settings, ring, pair, unpair. The loose circular
  buttons are deleted.
- **Device content**, fixed order, each block absent rather than empty when
  it does not apply: call banner only during a call; now-playing card only
  while something plays; pairing code only while pairing; activity always,
  filling the remaining height with its own scroll. "No phone" and "service
  unavailable" become the content's centred empty state.
- **Messages**: conversation list in the content with the thread beside it
  in two columns when the width allows, instead of a page swap with a back
  button.
- **Settings**: the three existing `ListSection`s, content unchanged.

### 5.5 Siderita

- `TopBar` is split: the centred uppercase title becomes the left-aligned
  bar title; the crumb path becomes a pill field in the centre; search moves
  to `trailing`.
- `BottomControls` (hidden files, view mode, sizes) moves to `trailing`
  beside search; the floating pill is deleted.
- `InfoPill` (folder count and size) becomes the sidebar's footer inside the
  same card instead of a separate card below it.
- Sidebar sections keep their order (`LUGARES`, `DISPOSITIVOS`,
  `FAVORITOS`, `MARCADORES`) at 40 px rows, `radiusMd`, inset
  `spaceCardInset` in a `radiusLg` card.
- `FolderView` keeps its grid; the 25 menu and dialog files change only
  recipe and radius. `PickerWindow` follows the same bar contract.

## 6. Order and dependencies

The brainstorming label `STYLE-G8` became three ledger units, because the
`celestina-style:` commit prefix may touch only `celestina-style/` and the
guard's ratchet baseline must live under root `scripts/` and be registered in
`docs/projects.toml`, which only `suite:` may edit. The active style
checkpoint `STYLE-G7` explicitly extends with demonstrated consumers, so no
new checkpoint is opened.

1. `STYLE-G7-O` (style, milestone, §3.1, §3.2, the glass part of §3.5).
   Lands alone; every application changes glass and scale at once.
2. `AUD-1-I` (suite, maintenance, §3.3), stacked on it: the radius guard and
   the baseline that records the applications' debt.
3. `STYLE-G7-P` (style, milestone, §3.4 and the rest of §3.5), stacked on
   the guard: the top bar and the segmented control.
4. Grafita, then Hematita, then Fluorita, then Magnetita, then Siderita:
   smallest surface first, the largest QML tree last. Each depends on the
   three units above and can be prepared in parallel worktrees; they land in
   this order so the segmented control and the bar are proven on the simplest
   host before Magnetita's reorganisation and Siderita's 127 files.

The implementation plan for the three style-side units is
[2026-09-30-style-haze-desktop-scale.md](../plans/2026-09-30-style-haze-desktop-scale.md).

## 7. Risks the author accepted

- Between step 1 and each application's unit, that application runs the new
  tokens under its old layout; the guard baseline records the debt rather
  than hiding it.
- Removing `glassOutline` and the lit edge is a visual break for the halted
  shell's layer surfaces; the shell compiles unchanged and its look is
  reviewed only when it is resumed.
- `blurMax 24` at full-resolution capture costs more per frame than the
  55 % capture did for live-capture surfaces (menus, dialogs). If a real
  session shows it, the fallback is `sampleScale 0.5` with `blurMax 12`,
  which is the same σ; the decision is recorded in the unit's evidence.
- Fluorita's "all" folder disappears; a person who used it now switches
  views with the segmented control.
