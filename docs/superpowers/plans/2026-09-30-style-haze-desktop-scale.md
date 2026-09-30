<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Style — Haze glass, desktop token scale, radius guard and top bar

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `GlassSurface` paint exactly what Haze paints on the phone (blur, tint, grain, nothing else), move `CelestinaTheme` to the desktop token ladder, add a guard that refuses text in a corner's curve and non-concentric nesting, and publish the two shared components every application's fixed top bar needs.

**Architecture:** Three landed units. `STYLE-G7-O` (style, milestone) rewrites the glass recipe and the token values behind existing names, so every application changes at once through its symlinks. `AUD-1-I` (suite, maintenance) adds `scripts/radius_contract.py`, a bounded QML scanner wired into the common architecture guard with a per-project ratchet baseline registered as a shared ratchet file. `STYLE-G7-P` (style, milestone) publishes `CelestinaTopBar` and `CelestinaSegmentedControl`, the bar contract each application unit then consumes. Each unit is one session worktree branch stacked on the previous one; the landing bumps versions and writes inventories, never the session.

**Tech Stack:** Qt 6.11 / Qt Quick, QML (`MultiEffect`, `RectangularShadow`), Qt Quick Test, CMake ≥ 3.20, Bash guards, Python 3.14 (stdlib `unittest`, Pillow 12 for the reference image), Niri session tools (`niri msg`, `grim`) for real-session evidence.

**Spec:** [docs/superpowers/specs/2026-09-30-desktop-glass-design.md](../specs/2026-09-30-desktop-glass-design.md) §3 (the style module) and §4 (rules the application units inherit). The spec's `STYLE-G8` label is realised as the three ledger units named above; the amendment is recorded in the spec's §6.

## Global Constraints

- **Language contract:** English in code, comments, documents, tests and diagnostics; Spanish only inside `qsTr()` literals.
- **Commit scope:** `celestina-style:` may touch only `celestina-style/`; `suite:` may touch anything. A style unit that needs a root `scripts/` file is mis-scoped; move that work to `AUD-1-I`.
- **A session never** builds production, deploys, bumps a version, writes an inventory or sets a ledger row to `done`. The landing (`scripts/land-unit.py`) does that. Session commits on the unit branch are temporary; the landing publishes one squashed commit per unit.
- **Session worktrees:** open with `scripts/worktree.sh open <project> <unit> [--from unit/<project>/<unit>]` from the canonical checkout `/home/toni/CODIGO/CELESTINA`; the worktree lands under `/home/toni/CODIGO/CELESTINA.worktrees/<project>-<unit>/`. Every command below runs inside the unit's worktree unless it says otherwise.
- **Halted shell:** `celestina/` is halted; its QML is not edited. It keeps compiling because every public name it uses survives (`glassHighlight`, `glassTintStrong`, `ContentSurface`, `ContextualVeil`, `silhouettePath`, `materialTint`, `materialOpacity`).
- **Style guard:** no hex literal, `Qt.rgba`, numeric radius, padding, duration, opacity or numeric alpha outside `CelestinaTheme.qml`. Tokens only.
- **Sealed decisions** (DESIGN.md §9) stay: accent `#3e91ff`, Inter Variable, Lucide, dark only.
- **Fast checks a session runs:** `bash scripts/check-architecture-contract.sh` (root; includes the style and contrast guards), `bash scripts/test-architecture-scanners.sh`, `python3 scripts/test-language-contract.py`, the style module's own `cmake -S celestina-style -B celestina-style/build -DBUILD_TESTING=ON && cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure`, and `QT_QPA_PLATFORM=offscreen timeout 8 celestina-style/gallery/run.sh --offscreen` for construction. The real-session gallery screenshot is evidence, not a gate.
- **Haze reference (the numbers):** blur radius 20 dp → Gaussian σ = 0.57735 × 20 + 0.5 = 12.05 px at 1× density; tint = `background` at alpha 0.70; grain = `haze_noise.webp` (64×64 RGBA, random alpha) drawn with its alpha multiplied by 0.15, **under** the tint (Haze composes blur → noise → tints); fallback = opaque `backgroundColor`. Source: `chrisbanes/haze` tag `1.6.10`, `haze/src/androidMain/kotlin/dev/chrisbanes/haze/RenderEffect.android.kt` (`withNoise` then `withTints`) and the phone's `BottomTabs.kt`.

---

## File structure

| Path | Unit | Responsibility |
|---|---|---|
| `celestina-style/icons/haze-noise.png` | O | Haze's grain texture, converted losslessly from `haze_noise.webp` (1.6.10) |
| `celestina-style/icons/LICENSE-haze.txt` | O | Apache-2.0 notice for the texture |
| `celestina-style/icons/glass-noise.png` | O | deleted |
| `celestina-style/CelestinaTheme.qml` | O | glass scalars, scheme tint, token ladder, `cornerInset`, new spacing and bar tokens |
| `celestina-style/GlassSurface.qml` | O | the Haze recipe: capture, blur, grain, tint, shadow, fallback; no edge layers |
| `celestina-style/GlassCard.qml`, `GlassContextMenu.qml`, `CelestinaTextField.qml`, `ListSection.qml` | O | radii and insets on the new ladder |
| `celestina-style/scripts/check-contrast-contract.py` | O | `glassTint` composited over black and white |
| `celestina-style/scripts/glass-reference.py` | O | renders `gallery/reference/backdrop.png` and `gallery/reference/haze.png` with Pillow |
| `celestina-style/gallery/reference/*.png` | O | committed reference images the gallery shows beside live glass |
| `celestina-style/gallery/Gallery.qml` | O, P | glass page with reference panel; top bar and segmented control page |
| `celestina-style/tests/tst_glasssurface.qml` | O | recipe assertions |
| `celestina-style/DESIGN.md`, `STATUS.md`, `ROADMAP.md`, `VALIDATION.md` | O, P | contract text, checkout truth, checkpoint text, validation rows |
| `celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md` | O, P | ledger rows `STYLE-G7-O`, `STYLE-G7-P` |
| `celestina-style/docs/evidence/2026-09-30-haze-recipe-and-desktop-scale.md` | O | evidence record |
| `scripts/radius_contract.py` | I | the radius scanner (inset, concentric and literal rules) with a ratchet baseline |
| `scripts/test-radius-contract.py` | I | hermetic fixtures for every rule and the ratchet |
| `scripts/radius-baseline.tsv` | I | `findings<TAB>project` ratchet, shrink-only |
| `scripts/check-architecture-contract.sh` | I | runs the radius scanner after the style guard |
| `docs/projects.toml` | I | registers the baseline as a shared ratchet file |
| `docs/plans/active/2026-09-26-monorepo-hardening.md` | I | ledger row `AUD-1-I` |
| `docs/evidence/2026-09-30-radius-guard.md` | I | evidence record |
| `celestina-style/CelestinaSegmentedControl.qml` | P | peer destinations in one `radiusButton` plate |
| `celestina-style/CelestinaTopBar.qml` | P | fixed bar: leading, titles, trailing |
| `celestina-style/tests/tst_segmented.qml`, `tests/tst_topbar.qml` | P | geometry, focus, accessibility, reduced motion |
| `celestina-style/qmldir`, `CMakeLists.txt` | O, P | resource and type registration |
| `celestina-style/scripts/check-style-contract.sh` | P | refuses a text button inside a top bar's trailing slot |
| `celestina-style/docs/evidence/2026-09-30-top-bar-and-segments.md` | P | evidence record |

---

## Unit STYLE-G7-O — the Haze recipe and the desktop ladder

### Task 1: Open the unit and declare its ledger row

**Files:**
- Modify: `celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md` (ledger table, after the `STYLE-G7-N` row)
- Create: `celestina-style/docs/evidence/2026-09-30-haze-recipe-and-desktop-scale.md`

- [ ] **Step 1: Open the worktree from the canonical checkout**

```bash
cd /home/toni/CODIGO/CELESTINA && scripts/worktree.sh open celestina-style STYLE-G7-O && cd /home/toni/CODIGO/CELESTINA.worktrees/celestina-style-STYLE-G7-O
```

Expected: the branch `unit/celestina-style/STYLE-G7-O` exists from `origin/main`, the directory contains `.celestina-worktree`.

- [ ] **Step 2: Add the ledger row**

Insert directly under the `STYLE-G7-N` row (the first data row of the ledger table):

```markdown
| STYLE-G7-O | `celestina-style:` | active | `GlassSurface.qml`; `CelestinaTheme.qml`; `GlassCard.qml`; `GlassContextMenu.qml`; `CelestinaTextField.qml`; `ListSection.qml`; `icons/haze-noise.png`; `icons/LICENSE-haze.txt`; `scripts/check-contrast-contract.py`; `scripts/glass-reference.py`; `gallery/`; `tests/tst_glasssurface.qml`; `qmldir`; `CMakeLists.txt`; `DESIGN.md`; `STATUS.md`; `ROADMAP.md`; `VALIDATION.md` | — | Paint the phone's Haze recipe in `GlassSurface` (σ ≈ 12 px blur, canvas tint at 0.70, Haze's grain at 0.15 under the tint, opaque canvas fallback, no outline and no lit edge), move the token ladder to desktop values behind the existing names (`radiusLg` 20, `radiusMd` 12, `radiusSm` 8, `radiusButton` 10, `radiusInput` = pill, `rowHeight` 40, `windowMargin` 16) with `spaceCardGap`, `spaceCardInset`, `topBarHeight`, `compSegmentHeight`, `iconLg` and `cornerInset()`, and show a Pillow-rendered σ = 12 reference beside the live glass in the gallery | [evidence](../../evidence/2026-09-30-haze-recipe-and-desktop-scale.md) | `VAL-STYLE-07` |
```

- [ ] **Step 3: Create the evidence record from the template**

```markdown
# Evidence: 2026-09-30 the Haze recipe and the desktop scale

- **Date:** 2026-09-30
- **Scope:** `STYLE-G7-O` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2, Python 3.14 with Pillow 12.3, Niri session on a 2560×1440 output at scale 1 for the real-session crops, offscreen QPA for tests
- **Artifact:** not applicable in the session; the landing builds and verifies the registered module

## Procedure

(filled in by Task 8)

## Result

(filled in by Task 8)

## Limits

(filled in by Task 8)

## Follow-up

`VAL-STYLE-07` in [VALIDATION.md](../../VALIDATION.md).
```

- [ ] **Step 4: Commit**

```bash
git add celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md celestina-style/docs/evidence/2026-09-30-haze-recipe-and-desktop-scale.md
git commit -m "celestina-style-milestone: Open STYLE-G7-O for the Haze recipe and the desktop ladder"
```

### Task 2: Vendor Haze's grain texture

**Files:**
- Create: `celestina-style/icons/haze-noise.png`, `celestina-style/icons/LICENSE-haze.txt`
- Delete: `celestina-style/icons/glass-noise.png`
- Modify: `celestina-style/CMakeLists.txt:178` (RESOURCES), `celestina-style/icons.qrc:116`
- Test: `celestina-style/tests/tst_glasssurface.qml`

**Interfaces:**
- Produces: the resource alias `haze-noise.png` at `qrc:/qt/qml/CelestinaStyle/icons/haze-noise.png` and the source-tree path `icons/haze-noise.png`, both read by `GlassSurface` in Task 5.

- [ ] **Step 1: Write the failing test**

Append to the `TestCase` in `tests/tst_glasssurface.qml`:

```qml
    function test_grain_is_the_haze_texture() {
        const noise = findByObjectName(capturedGlass, "celestina-glass-noise")
        verify(noise)
        verify(noise.source.toString().endsWith("/haze-noise.png"),
               "grain must be Haze's texture: " + noise.source)
    }
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cmake -S celestina-style -B celestina-style/build -DBUILD_TESTING=ON && cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure -R celestina-style-modal-focus
```

Expected: FAIL in `GlassSurface::test_grain_is_the_haze_texture` because the source ends with `/glass-noise.png`.

- [ ] **Step 3: Fetch, convert and licence the texture**

```bash
scratch=$(mktemp -d) && curl -sSL --fail -m 60 -o "$scratch/haze_noise.webp" "https://raw.githubusercontent.com/chrisbanes/haze/1.6.10/haze/src/androidMain/res/drawable-nodpi/haze_noise.webp" && curl -sSL --fail -m 60 -o "$scratch/LICENSE" "https://raw.githubusercontent.com/chrisbanes/haze/1.6.10/LICENSE" && python3 - "$scratch" <<'PY'
import sys, pathlib
from PIL import Image
scratch = pathlib.Path(sys.argv[1])
image = Image.open(scratch / "haze_noise.webp").convert("RGBA")
assert image.size == (64, 64), image.size
image.save("celestina-style/icons/haze-noise.png", optimize=True)
notice = (
    "haze-noise.png is haze_noise.webp from chrisbanes/haze, tag 1.6.10,\n"
    "path haze/src/androidMain/res/drawable-nodpi/haze_noise.webp, converted\n"
    "losslessly to PNG (64x64 RGBA). Copyright 2023 Chris Banes. Licensed under\n"
    "the Apache License, Version 2.0; the full text follows.\n\n"
)
pathlib.Path("celestina-style/icons/LICENSE-haze.txt").write_text(
    notice + (scratch / "LICENSE").read_text(encoding="utf-8"), encoding="utf-8")
PY
git rm -q celestina-style/icons/glass-noise.png
```

If the network is unavailable, stop the task and record "not fetched" in the evidence: the texture must be Haze's, not a substitute.

- [ ] **Step 4: Register the resource**

In `CMakeLists.txt`, replace the line `        icons/glass-noise.png` with:

```cmake
        icons/haze-noise.png
        icons/LICENSE-haze.txt
```

In `icons.qrc`, replace `    <file alias="glass-noise.png">icons/glass-noise.png</file>` with:

```xml
    <file alias="haze-noise.png">icons/haze-noise.png</file>
```

In `GlassSurface.qml`, change both `glass-noise.png` occurrences in the `Image` source binding to `haze-noise.png` (the full recipe change is Task 5; this keeps the module building).

- [ ] **Step 5: Run the test to verify it passes**

Same command as Step 2. Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A celestina-style/icons celestina-style/CMakeLists.txt celestina-style/icons.qrc celestina-style/GlassSurface.qml celestina-style/tests/tst_glasssurface.qml
git commit -m "celestina-style-milestone: Vendor Haze's grain texture with its notice"
```

### Task 3: The token ladder and the glass scalars in the theme

**Files:**
- Modify: `celestina-style/CelestinaTheme.qml` (scheme block near line 514, radius block near line 707, spacing block near line 745, control metrics near line 759, glass scalars near line 834, comp knobs near line 898)
- Modify: `celestina-style/scripts/check-contrast-contract.py` (after the `compositorGlassTint` loop, line ~215)

**Interfaces:**
- Produces, read by every later task: `CelestinaTheme.radiusLg = 20`, `radiusMd = 12`, `radiusSm = 8`, `radiusButton = 10`, `radiusInput = radiusPill`, `rowHeight = 40`, `rowHeightLg = 52`, `windowMargin = 16`, `iconMd = 20`, `iconLg = 24`, `spaceCardGap = 12`, `spaceCardInset = 8`, `topBarHeight = 48`, `compSegmentHeight = 32`, `function cornerInset(radius) → int`, `glassTint = "#b3050608"`, `glassTintStrong = glassTint`, `glassBlurMax = 24`, `glassBlurMultiplier = 1.0`, `glassSaturation = 0`, `glassSampleScale = 1.0`, `glassNoiseOpacity = 0.15`, `glassFallback = canvas`. Deleted: `glassBorder`, `glassOutline`, `glassEdgeWidth`, `glassEdgeMidPosition`, `glassEdgeLowPosition`, `glassEdgeMidOpacity`, `glassEdgeLowOpacity`. Kept: `glassHighlight` (the veil role's tint, used by the shell), `glassContentSurfaceStrength`, `glassContextualVeilStrength`.

- [ ] **Step 1: Write the failing contrast check**

In `scripts/check-contrast-contract.py`, after the `for tint_name in ("compositorGlassTint", "compositorGlassFallback"):` loop, add:

```python
    # In-scene glass (the Haze recipe: canvas at 0.70 over the blur) sits over
    # the suite's own near-black canvas in every application, and over a
    # photograph only in Fluorita's floating dock. Primary text must therefore
    # read over both extremes; secondary text and the focus ring are required
    # only over black, because no in-scene glass carries them over a light image.
    glass_tint = literal("glassTint")
    for backdrop_name, backdrop in extremes():
        surface = composite(glass_tint, backdrop)
        require(f"glassTint/{backdrop_name}/text", text_hi, surface, 4.5)
    glass_on_black = composite(glass_tint, (0, 0, 0, 1))
    require("glassTint/black/secondary text", text_lo, glass_on_black, 4.5)
    require("glassTint/black/outer focus", focus_ring, glass_on_black, 3.0)
```

- [ ] **Step 2: Run it to verify it fails**

```bash
python3 celestina-style/scripts/check-contrast-contract.py
```

Expected: exit 1 naming `glassTint/white/text` (today's `#991a1e25` at 60 % over white leaves `#f7f8fc` under 4.5:1).

- [ ] **Step 3: Change the scheme tint**

In the `schemeDark` block, replace:

```qml
        glassTint: "#991a1e25"
        glassTintStrong: "#bd1a1e25"
```

with:

```qml
        // Haze's tint: the background colour at 0.70 over the blur. One value;
        // `glassTintStrong` survives as a name for GlassCard's Strong density
        // and paints the same material.
        glassTint: "#b3050608"
        glassTintStrong: "#b3050608"
```

Delete these two lines from the same block:

```qml
        glassBorder: "#24ffffff"
        glassOutline: "#4d000000"
```

and delete their `required property color glassBorder` / `glassOutline` declarations from `ColorScheme` (lines ~392 and ~394) and their `readonly property color glassBorder: scheme.glassBorder` / `glassOutline` reads (lines ~618 and ~620). Keep `glassHighlight`.

- [ ] **Step 4: Change the scalars**

Replace the radius block (from `readonly property int radiusNone: 0` through `readonly property int radiusLg: 26`) with:

```qml
    // ── Radius scale ───────────────────────────────────────────────────────────
    // The desktop ladder (spec 2026-09-30 §3.2). Nested corners are concentric
    // by construction: a radiusLg card inset spaceCardInset holds radiusMd rows
    // (12 + 8 = 20); a radiusMd row or tile inset spaceXs holds radiusSm
    // thumbnails (8 + 4 = 12). `cornerInset` is the least a glyph or text may
    // sit from a corner without the arc clipping it.
    readonly property int radiusNone: 0      // edge-to-edge surfaces with no exposed corner
    readonly property int radiusXs: 3        // selection marquee / tiny indicators
    readonly property int radiusSm: 8        // chips, thumbnails, small indicators
    readonly property int radiusMd: 12       // rows, tiles, content surfaces
    readonly property int radiusButton: 10   // text buttons (dialogs only)
    readonly property int radiusLg: 20       // grouped cards, sidebars, dialogs, menus
    readonly property int radiusInput: radiusPill // search and text fields
```

Move `readonly property int radiusPill: 9999   // full capsule` above that block (it is referenced by `radiusInput`), and add under `nestedRadius`:

```qml
    // The horizontal reach of a circular corner at the height where a
    // cap-height glyph starts: r(1 - 1/sqrt 2) ≈ 0.29 r, rounded up. Text or
    // a glyph closer than this to a corner is clipped by the arc. The radius
    // guard (scripts/radius_contract.py) computes the same value.
    function cornerInset(radius) {
        return Math.ceil(radius * 0.3)
    }
```

Change `readonly property int windowMargin: 14` to `16`. In the spacing block, after `readonly property int space3xl: 32`, add:

```qml
    // Between sibling cards, and from a card's edge to its rows or tiles.
    readonly property int spaceCardGap: 12
    readonly property int spaceCardInset: 8
```

In the control metrics, change `rowHeight: 54` to `40`, `rowHeightLg: 66` to `52`, `iconMd: 19` to `20`, and after `iconMd` add:

```qml
    readonly property int iconLg: 24         // navigation glyphs: sidebar rows, segments
    // The fixed top bar every application wears, and the segmented control
    // that sits in its leading slot with spaceSm of air above and below.
    readonly property int topBarHeight: 48
    readonly property int compSegmentHeight: 32
```

Replace the glass scalars block (from the `// ── Glass parameters (scalars)` comment through `readonly property real glassEdgeLowOpacity: 0.15`, keeping `ambientBrightness`/`ambientSaturation` and `glassSampleMargin`) with:

```qml
    // ── Glass parameters (scalars) ─────────────────────────────────────────────
    // The Haze recipe (spec 2026-09-30 §3.1): a 20 dp blur is a Gaussian of
    // σ ≈ 12 px, the capture is sampled at full resolution so the downsample no
    // longer widens it, and there is no desaturation. `glassBlurMax` is
    // calibrated against the σ = 12 reference the gallery renders with
    // scripts/glass-reference.py; the value below is the one recorded in the
    // 2026-09-30 evidence.
    readonly property real glassBlur: 1.0
    readonly property int glassBlurMax: 24
    readonly property real glassBlurMultiplier: 1.0
    readonly property real glassSaturation: 0
    readonly property real glassSampleScale: 1.0

    // Ambient light: a blurred copy of the content lighting the space around
    // it. Both values pull it back from the picture it sits behind — at full
    // strength the light competes with the thing it is meant to frame, and the
    // desaturation keeps a vivid photograph from washing the room in one hue.
    readonly property real ambientBrightness: -0.30
    readonly property real ambientSaturation: -0.20
    readonly property int glassSampleMargin:
            Math.ceil(glassBlurMax * (1 + glassBlurMultiplier))
    // Haze's grain: its texture drawn under the tint with alpha 0.15.
    readonly property real glassNoiseOpacity: 0.15
    // What Haze paints when it cannot blur: the opaque background colour.
    readonly property color glassFallback: canvas
```

- [ ] **Step 5: Run the contrast check and the style guard**

```bash
python3 celestina-style/scripts/check-contrast-contract.py && bash celestina-style/scripts/check-style-contract.sh
```

Expected: the contrast check passes. The style guard fails on `GlassSurface.qml` naming the deleted `glassBorder`/`glassOutline`/`glassEdge*` (the structural scanner refuses a token the theme lacks). That is Task 5's work; continue.

- [ ] **Step 6: Commit**

```bash
git add celestina-style/CelestinaTheme.qml celestina-style/scripts/check-contrast-contract.py
git commit -m "celestina-style-milestone: Move the theme to the desktop ladder and the Haze glass scalars"
```

### Task 4: The recipe's tests

**Files:**
- Modify: `celestina-style/tests/tst_glasssurface.qml`

- [ ] **Step 1: Rewrite the role test and the silhouette test for the edge-free recipe**

Replace `test_semantic_roles_preserve_the_default_and_separate_material_jobs` with:

```qml
    function test_semantic_roles_keep_their_strength_without_edge_layers() {
        compare(externalGlass.materialRole, GlassSurface.StandardMaterial)
        compare(externalGlass.materialStrength, 1)

        compare(contentGlass.materialRole, GlassSurface.ContentSurface)
        compare(contentGlass.materialTint, CelestinaTheme.canvas)
        compare(contentGlass.materialStrength,
                CelestinaTheme.glassContentSurfaceStrength)

        compare(contextualGlass.materialRole, GlassSurface.ContextualVeil)
        compare(contextualGlass.materialTint, CelestinaTheme.glassHighlight)
        compare(contextualGlass.materialStrength,
                CelestinaTheme.glassContextualVeilStrength)
        verify(contextualGlass.materialStrength < contentGlass.materialStrength)

        compare(contentGlass.captureActive, false)
        compare(contextualGlass.captureActive, false)
        compare(contentGlass.elevation, 0)
        compare(contextualGlass.elevation, 0)
        verify(!findByObjectName(contentGlass, "celestina-glass-shadow").visible)
        verify(!findByObjectName(contextualGlass, "celestina-glass-shadow").visible)

        const contentTint = findByObjectName(
            contentGlass, "celestina-glass-material-tint")
        const contextualTint = findByObjectName(
            contextualGlass, "celestina-glass-material-tint")
        const contextualNoise = findByObjectName(
            contextualGlass, "celestina-glass-noise")
        verify(contentTint)
        verify(contextualTint)
        verify(contextualNoise)
        verify(contextualNoise.visible)
        compare(contentTint.opacity,
                CelestinaTheme.glassContentSurfaceStrength)
        compare(contextualTint.opacity,
                CelestinaTheme.glassContextualVeilStrength)
        compare(contextualNoise.opacity,
                CelestinaTheme.glassNoiseOpacity
                * CelestinaTheme.glassContextualVeilStrength)

        // Haze has no outline and no lit edge; neither does any role now.
        for (const surface of [externalGlass, contentGlass, contextualGlass]) {
            compare(findByObjectName(surface, "celestina-glass-outline"), null)
            compare(findByObjectName(surface, "celestina-glass-lit-edge"), null)
        }
    }

    function test_grain_lies_under_the_tint() {
        const noise = findByObjectName(capturedGlass, "celestina-glass-noise")
        const tint = findByObjectName(capturedGlass, "celestina-glass-material-tint")
        verify(noise && tint)
        compare(noise.parent, tint.parent)
        const siblings = noise.parent.children
        let noiseIndex = -1
        let tintIndex = -1
        for (let index = 0; index < siblings.length; ++index) {
            if (siblings[index] === noise) noiseIndex = index
            if (siblings[index] === tint) tintIndex = index
        }
        verify(noiseIndex >= 0 && tintIndex >= 0)
        verify(noiseIndex < tintIndex, "Haze composes blur, then grain, then tint")
        compare(noise.opacity, CelestinaTheme.glassNoiseOpacity)
    }

    function test_fallback_is_the_opaque_canvas() {
        capturedGlass.captureEnabled = false
        verify(!capturedGlass.active)
        const tint = findByObjectName(capturedGlass, "celestina-glass-material-tint")
        compare(tint.color, CelestinaTheme.glassFallback)
        compare(tint.opacity, 1)
        compare(CelestinaTheme.glassFallback, CelestinaTheme.canvas)
    }

    function test_strong_density_paints_the_same_material() {
        compare(CelestinaTheme.glassTintStrong, CelestinaTheme.glassTint)
        capturedGlass.density = GlassSurface.Strong
        compare(capturedGlass.materialTint, CelestinaTheme.glassTint)
        capturedGlass.density = GlassSurface.Regular
    }
```

Replace `test_silhouette_is_opt_in_and_keeps_the_semantic_material` with:

```qml
    function test_silhouette_is_opt_in_and_keeps_the_semantic_material() {
        compare(capturedGlass.silhouettePath, "")
        verify(!capturedGlass.usesSilhouette)
        verify(silhouetteGlass.usesSilhouette)
        compare(silhouetteGlass.effectiveSilhouetteEdgePath,
                silhouetteGlass.silhouetteEdgePath)
        verify(findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-silhouette-base").visible)
        const silhouetteTint = findByObjectName(
            silhouetteGlass, "celestina-glass-silhouette-material-tint")
        verify(silhouetteTint.visible)
        compare(silhouetteTint.opacity, silhouetteGlass.materialStrength)
        compare(findByObjectName(
                    silhouetteGlass, "celestina-glass-silhouette-outline"), null)
        compare(findByObjectName(
                    silhouetteGlass, "celestina-glass-silhouette-lit-edge"), null)
        verify(!findByObjectName(
                   silhouetteGlass,
                   "celestina-glass-shadow").visible)

        silhouetteGlass.materialRole = GlassSurface.ContextualVeil
        verify(silhouetteTint.visible)
        compare(silhouetteTint.opacity,
                CelestinaTheme.glassContextualVeilStrength)
    }
```

Also remove the two `materialEdgesVisible` lines that remain anywhere in the file (`verify(!silhouetteGlass.materialEdgesVisible)` at the old line 217 is gone with the rewrite above; grep to be sure).

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```

Expected: the module still fails to load `GlassSurface.qml` (deleted tokens) or the new cases fail on the outline/lit-edge objects still existing. Either way, red.

- [ ] **Step 3: Commit the tests**

```bash
git add celestina-style/tests/tst_glasssurface.qml
git commit -m "celestina-style-milestone: Assert the Haze recipe in the glass tests"
```

### Task 5: `GlassSurface` paints the Haze recipe

**Files:**
- Modify: `celestina-style/GlassSurface.qml`

**Interfaces:**
- Keeps: every public property and enum (`Density`, `BackdropMode`, `MaterialRole`, `backdropSource`, `backdropMode`, `externalBackdropReady`, `captureEnabled`, `liveCapture`, `cornerRadius`, `silhouettePath`, `silhouetteEdgePath`, `usesSilhouette`, `effectiveSilhouetteEdgePath`, `sampleMargin`, `sampleScale`, `elevation`, `density`, `materialRole`, `contentData`, `materialTint`, `materialOpacity`, `materialStrength`, `captureActive`, `active`, `refreshBackdrop()`).
- Removes: `materialEdgesVisible` (public, read only by the tests and by nothing in the shell: `grep -rn materialEdgesVisible celestina/qml` is empty).

- [ ] **Step 1: Rewrite the header comment**

Replace the `// Recipe (One UI 8.5, DESIGN §6.5): ...` paragraph of the header with:

```qml
// Recipe (Haze 1.6, the phone's glass; DESIGN §5.3): bounded capture at full
// resolution → blur of σ ≈ 12 px → Haze's grain at 0.15 → the canvas tint at
// 0.70. Nothing else: no desaturation, no outline, no lit edge. A role only
// scales the grain and the tint (`materialStrength`). `elevation > 0` adds the
// L2 drop shadow, which is the phone pill's `shadowElevation`. When the surface
// cannot blur it paints the opaque canvas, which is what Haze's
// `backgroundColor` does. The shadow lives outside the clipped body, so the
// root itself does not clip.
```

- [ ] **Step 2: Remove the edge policy and the edge layers**

Delete the `materialEdgesVisible` property and its comment. Delete the four `Shape`/`Rectangle` items named `celestina-glass-outline`, `celestina-glass-lit-edge`, `celestina-glass-silhouette-outline` and `celestina-glass-silhouette-lit-edge`, together with the comment paragraphs that introduce them ("A thin dark outline…", "The lit glass edge…", "No seam treatment rides the silhouette…", "The opt-in silhouette keeps the same dark definition…"). Remove `import QtQuick.Shapes` only if no `Shape` remains; the two silhouette `Shape`s (`-base` and `-material-tint`) remain, so keep the import.

- [ ] **Step 3: Move the grain under the tint and stop desaturating**

In the `MultiEffect`, delete the `saturation: CelestinaTheme.glassSaturation` line and its two comment lines. Then cut the `Image { objectName: "celestina-glass-noise" ... }` block and paste it immediately **before** the `Rectangle { objectName: "celestina-glass-material-tint" ... }` block, with this comment above it:

```qml
        // Haze's grain, drawn over the blur and under the tint: its texture
        // tiled with alpha 0.15 (times the role's strength). The order is
        // Haze's — blur, noise, then tints — and it is what the phone shows.
```

The noise `source` reads `icons/haze-noise.png` (Task 2). Keep `smooth: false` and `fillMode: Image.Tile`.

- [ ] **Step 4: Fallback and silhouette fills**

In the rounded fallback `Rectangle` (the first child of `body`), and in the tint `Rectangle`, replace every `CelestinaTheme.surfaceStrong` with `CelestinaTheme.glassFallback`. Do the same in the two silhouette `Shape`s' `fillColor` bindings. The tint rectangle therefore reads:

```qml
        Rectangle {
            objectName: "celestina-glass-material-tint"
            anchors.fill: parent
            visible: !root.usesSilhouette
            radius: root.cornerRadius
            color: root.active ? root.materialTint : CelestinaTheme.glassFallback
            opacity: root.active
                     ? root.materialOpacity * root.materialStrength
                     : 1
        }
```

- [ ] **Step 5: Run the module tests, the style guard and the gallery construction smoke**

```bash
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure && bash celestina-style/scripts/check-style-contract.sh && QT_QPA_PLATFORM=offscreen timeout 8 celestina-style/gallery/run.sh --offscreen; echo "gallery exit $?"
```

Expected: ctest 1/1 passed with every `GlassSurface` case green; `QML visual contract: OK`; gallery exit 124 (killed by the timeout after constructing) with empty stderr.

- [ ] **Step 6: Commit**

```bash
git add celestina-style/GlassSurface.qml
git commit -m "celestina-style-milestone: Paint the Haze recipe in GlassSurface"
```

### Task 6: Consumers inside the module follow the ladder

**Files:**
- Modify: `celestina-style/GlassCard.qml:15`, `celestina-style/GlassContextMenu.qml:40`, `celestina-style/CelestinaTextField.qml:22-24`, `celestina-style/ListSection.qml`

- [ ] **Step 1: Write the failing assertions**

Append to `tests/tst_textfield.qml`'s `TestCase` (it already instantiates a search field; if its id differs, use that id):

```qml
    function test_search_field_is_a_pill() {
        compare(CelestinaTheme.radiusInput, CelestinaTheme.radiusPill)
    }
```

Add a new file `tests/tst_listsection.qml`:

```qml
import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

TestCase {
    id: testCase

    name: "ListSection"
    when: testWindow.visible

    Window {
        id: testWindow
        width: 320
        height: 240
        visible: true

        ListSection {
            id: section
            width: 280
            title: "Sección"
            Rectangle { objectName: "row"; width: parent.width; height: CelestinaTheme.rowHeight; radius: CelestinaTheme.radiusMd }
        }
    }

    function test_rows_sit_one_card_inset_inside_the_card() {
        const row = findChild(section, "row")
        verify(row)
        const holder = row.parent
        compare(holder.y, CelestinaTheme.spaceCardInset)
        compare(holder.x, CelestinaTheme.spaceCardInset)
        compare(holder.width, holder.parent.width - CelestinaTheme.spaceCardInset * 2)
        // Concentric: card radius = row radius + inset.
        compare(CelestinaTheme.radiusLg, CelestinaTheme.radiusMd + CelestinaTheme.spaceCardInset)
    }
}
```

- [ ] **Step 2: Run to verify they fail**

```bash
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```

Expected: `ListSection::test_rows_sit_one_card_inset_inside_the_card` fails on `holder.x` (today 0) and `holder.y` (today `spaceXs`).

- [ ] **Step 3: Apply the ladder**

`GlassCard.qml`: change `cornerRadius: CelestinaTheme.radiusMd` to `cornerRadius: CelestinaTheme.radiusLg` (a dialog is a floating card at the card radius, 20).

`GlassContextMenu.qml`: `cornerRadius: CelestinaTheme.radiusLg` stays as written; it now paints 20.

`CelestinaTextField.qml`: the `fieldRadius` binding stays (`radiusInput` is now the pill); no edit unless `radiusInput` is referenced elsewhere with arithmetic — grep confirms it is not.

`ListSection.qml`: replace the `CelestinaSurface` block with:

```qml
    CelestinaSurface {
        width: section.width
        implicitHeight: rowHolder.implicitHeight + CelestinaTheme.spaceCardInset * 2
        height: implicitHeight
        role: CelestinaSurface.Grouped

        // Rows sit spaceCardInset inside the radiusLg card, so a radiusMd row
        // is concentric with it (12 + 8 = 20) and nothing reaches the corner.
        Column {
            id: rowHolder
            x: CelestinaTheme.spaceCardInset
            y: CelestinaTheme.spaceCardInset
            width: parent.width - CelestinaTheme.spaceCardInset * 2
        }
    }
```

and update the header comment's "26-radius card" to "radiusLg card".

- [ ] **Step 4: Run to verify they pass**

Same command as Step 2. Expected: ctest 1/1, all cases green.

- [ ] **Step 5: Commit**

```bash
git add celestina-style/GlassCard.qml celestina-style/ListSection.qml celestina-style/tests/tst_listsection.qml celestina-style/tests/tst_textfield.qml
git commit -m "celestina-style-milestone: Put the module's own consumers on the desktop ladder"
```

### Task 7: The σ = 12 reference beside the live glass

**Files:**
- Create: `celestina-style/scripts/glass-reference.py`, `celestina-style/gallery/reference/backdrop.png`, `celestina-style/gallery/reference/haze.png`
- Modify: `celestina-style/gallery/Gallery.qml` (the `GLASS — L2 ELEVATION` section, lines ~377-418)

**Interfaces:**
- Produces: two PNGs, 300×130, that `Gallery.qml` shows with `Image`; the script is run by hand and its output is committed.

- [ ] **Step 1: Write the script**

```python
#!/usr/bin/env python3
"""Render the Haze reference the gallery shows beside the live GlassSurface.

Haze (1.6.10, the phone's glass) composes: a Gaussian blur whose sigma for a
20 dp radius is 0.57735 * 20 + 0.5 = 12.05 px, then its grain texture with
alpha 0.15, then the background colour at alpha 0.70. This script does exactly
that with Pillow over a backdrop that carries the gallery's own three colours
(accent, success, warning) as a horizontal gradient, so a person can hold the
live surface against the reference and see whether MultiEffect's blurMax is
calibrated. Run by hand; the PNGs are committed; no runtime reads them.

    python3 scripts/glass-reference.py
"""

from __future__ import annotations

import pathlib
import re
import sys

from PIL import Image, ImageFilter

HERE = pathlib.Path(__file__).resolve().parent
STYLE = HERE.parent
THEME = STYLE / "CelestinaTheme.qml"
OUT = STYLE / "gallery" / "reference"
SIZE = (300, 130)
SIGMA = 0.57735 * 20 + 0.5
NOISE_ALPHA = 0.15
TINT_ALPHA = 0.70


def literal(name: str) -> tuple[int, int, int]:
    text = THEME.read_text(encoding="utf-8")
    match = re.search(rf'readonly property color {name}: "#([0-9a-fA-F]{{6}})"', text)
    if match is None:
        match = re.search(rf'{name}: "#([0-9a-fA-F]{{6}})"', text)
    if match is None:
        sys.exit(f"{THEME}: no six-digit literal for {name}")
    value = match.group(1)
    return tuple(int(value[index : index + 2], 16) for index in (0, 2, 4))


def gradient(colours: list[tuple[int, int, int]]) -> Image.Image:
    width, height = SIZE
    image = Image.new("RGB", SIZE)
    pixels = image.load()
    stops = len(colours) - 1
    for x in range(width):
        position = x / (width - 1) * stops
        index = min(int(position), stops - 1)
        amount = position - index
        start, end = colours[index], colours[index + 1]
        colour = tuple(round(start[c] + (end[c] - start[c]) * amount) for c in range(3))
        for y in range(height):
            pixels[x, y] = colour
    return image


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    backdrop = gradient([literal("accent"), literal("success"), literal("warning")])
    backdrop.save(OUT / "backdrop.png", optimize=True)

    blurred = backdrop.filter(ImageFilter.GaussianBlur(radius=SIGMA)).convert("RGBA")

    noise = Image.open(STYLE / "icons" / "haze-noise.png").convert("RGBA")
    tiled = Image.new("RGBA", SIZE)
    for x in range(0, SIZE[0], noise.width):
        for y in range(0, SIZE[1], noise.height):
            tiled.paste(noise, (x, y))
    alpha = tiled.getchannel("A").point(lambda value: round(value * NOISE_ALPHA))
    tiled.putalpha(alpha)
    grained = Image.alpha_composite(blurred, tiled)

    canvas = literal("night")
    tint = Image.new("RGBA", SIZE, canvas + (round(255 * TINT_ALPHA),))
    tinted = Image.alpha_composite(grained, tint).convert("RGB")
    tinted.save(OUT / "haze.png", optimize=True)
    print(f"wrote {OUT / 'backdrop.png'} and {OUT / 'haze.png'} (sigma {SIGMA:.2f})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

`night` is the `ref` literal for the canvas (`#050608`); `accent`, `success` and `warning` resolve through the same `ref` literals the contrast script reads.

- [ ] **Step 2: Run it**

```bash
python3 celestina-style/scripts/glass-reference.py && python3 -c "from PIL import Image; print(Image.open('celestina-style/gallery/reference/haze.png').size)"
```

Expected: `wrote … (sigma 12.05)` and `(300, 130)`.

- [ ] **Step 3: Show the reference in the gallery**

Replace the `GLASS — L2 ELEVATION` section with:

```qml
            // ── Glass (needs a real GPU session) ───────────────────────────
            Section {
                heading: "GLASS — THE HAZE RECIPE (real session only)"
                Item {
                    width: sheet.width; height: 180
                    // The same three colours the reference backdrop carries.
                    Rectangle {
                        id: glassBackdrop
                        anchors.fill: parent
                        radius: CelestinaTheme.radiusLg
                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0; color: CelestinaTheme.accent }
                            GradientStop { position: 0.5; color: CelestinaTheme.success }
                            GradientStop { position: 1; color: CelestinaTheme.warning }
                        }
                    }
                    Row {
                        anchors.centerIn: parent
                        spacing: CelestinaTheme.space2xl

                        GlassSurface {
                            width: 300; height: 130
                            backdropSource: glassBackdrop
                            liveCapture: true
                            elevation: 2
                            cornerRadius: CelestinaTheme.radiusLg
                            Text { anchors.centerIn: parent; text: "Live GlassSurface"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.iconSm; font.weight: CelestinaTheme.weightDemiBold }
                        }

                        // scripts/glass-reference.py: Pillow, σ = 12.05, grain
                        // 0.15, canvas at 0.70. Calibrate glassBlurMax until
                        // the two panels match at arm's length.
                        Item {
                            width: 300; height: 130
                            Image {
                                anchors.fill: parent
                                source: Qt.resolvedUrl("reference/haze.png")
                                fillMode: Image.Stretch
                                layer.enabled: true
                                layer.effect: null
                            }
                            Rectangle {
                                anchors.fill: parent
                                radius: CelestinaTheme.radiusLg
                                color: CelestinaTheme.clear
                                // The reference is a flat PNG; clip it to the
                                // card shape with the same mask trick the glass uses.
                            }
                            Text { anchors.centerIn: parent; text: "σ = 12 reference"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.iconSm; font.weight: CelestinaTheme.weightDemiBold }
                        }
                    }
                }
            }
```

Then simplify the reference `Item` so that it actually clips to the card: replace the `Image` + empty `Rectangle` pair with:

```qml
                            Rectangle {
                                anchors.fill: parent
                                radius: CelestinaTheme.radiusLg
                                color: CelestinaTheme.glassFallback
                                clip: true
                                layer.enabled: true
                                Image {
                                    anchors.fill: parent
                                    source: Qt.resolvedUrl("reference/haze.png")
                                    fillMode: Image.Stretch
                                }
                            }
```

(`clip` on a rounded `Rectangle` clips to the bounding box, not the corners; `layer.enabled` with the rectangle's own radius is not a mask either. The rounded corners of the reference are cosmetic here, and the comparison is made in the panel's middle. State that in the evidence.)

- [ ] **Step 4: Construct the gallery offscreen**

```bash
QT_QPA_PLATFORM=offscreen timeout 8 celestina-style/gallery/run.sh --offscreen; echo "exit $?"
```

Expected: exit 124 with no QML warnings on stderr (an unresolved image prints `QML Image: Cannot open`; there must be none).

- [ ] **Step 5: Calibrate on the real session and record it**

From the Niri session (this is the author's machine; the gallery opens a real window):

```bash
(setsid celestina-style/gallery/run.sh >/tmp/gallery.log 2>&1 &); sleep 4; id=$(niri msg windows | awk '/^Window ID/{id=$3} /CelestinaStyle/{sub(":","",id); print id; exit}'); niri msg action focus-window --id "$id"; sleep 1; niri msg action screenshot-window --id "$id"; sleep 1; ls -t "$HOME"/Im*genes/Screenshots/*.png | head -1
```

Open the screenshot, crop the two 300×130 panels, and compare the middle 100×60 of each numerically:

```bash
python3 - "$(ls -t "$HOME"/Im*genes/Screenshots/*.png | head -1)" <<'PY'
import sys
from PIL import Image, ImageStat
shot = Image.open(sys.argv[1]).convert("RGB")
print("size", shot.size, "— locate the two panels in the glass section and set the boxes below")
PY
```

Set the two crop boxes by hand from the screenshot (the panels are side by side in the glass section), then compute the mean absolute channel difference and the standard deviation of a horizontal profile through each middle row. Accept when the mean difference is under 6/255 and the two profiles' standard deviations differ by less than 15 %; otherwise change `glassBlurMax` in `CelestinaTheme.qml` (try 20, then 28) and repeat. Record every tried value, both crops and the final numbers in the evidence record's Procedure and Result. Close the gallery window afterwards (`niri msg action close-window --id "$id"`).

- [ ] **Step 6: Commit**

```bash
git add celestina-style/scripts/glass-reference.py celestina-style/gallery/reference celestina-style/gallery/Gallery.qml celestina-style/CelestinaTheme.qml
git commit -m "celestina-style-milestone: Show the σ = 12 Haze reference beside the live glass"
```

### Task 8: Contract text, status, validation, evidence and the unit's guards

**Files:**
- Modify: `celestina-style/DESIGN.md` (§2 shape lines, §5.1, §5.2 table, §5.3, §6.1 `GlassSurface`/`GlassCard` rows), `celestina-style/STATUS.md`, `celestina-style/ROADMAP.md` (STYLE-G7 text), `celestina-style/VALIDATION.md`, `celestina-style/docs/evidence/2026-09-30-haze-recipe-and-desktop-scale.md`

- [ ] **Step 1: DESIGN.md**

§2 "Shape and structure": replace the first two bullets with:

```markdown
- Master radius: 20 for dialogs, menus, sidebars and grouped list cards.
- Rows, tiles and content surfaces: 12; chips and thumbnails: 8; text buttons:
  10; search and text fields are pills.
```

§2 "Typography, glass, depth and motion", replace the Glass bullet with:

```markdown
- Glass: the phone's Haze recipe, exactly — a σ ≈ 12 px blur, Haze's grain at
  0.15 under the tint, the canvas colour at 0.70 over it. No desaturation, no
  outline, no lit edge. Frosted and matte, not a refractive lens.
```

§5.1 "Shape": replace the paragraph with:

```markdown
`radiusLg 20` · `radiusMd 12` · `radiusSm 8` · `radiusButton 10` ·
`radiusInput = radiusPill` · `radiusXs 3` · `radiusPill 9999`. Radius scales
down with element size. Nested corners are concentric: an outer radius equals
the inner radius plus the inset between them, so a `radiusLg` card holding
`radiusMd` rows insets them `spaceCardInset` (12 + 8 = 20) and a `radiusMd`
tile holding a `radiusSm` thumbnail insets it `spaceXs` (8 + 4 = 12). Text or
a glyph that touches a corner sits at least `cornerInset(radius)` from it,
`ceil(0.3 × radius)`, or the arc clips it. Only a focus ring may enter that
inset, and nothing crosses the panel edge. The radius guard
(`scripts/radius_contract.py`) refuses the three violations: an inset below
`cornerInset`, a non-concentric nested radius, and a numeric margin literal
inside a rounded surface.
```

§5.2 table: change the L2 row to `Regular glass (the Haze recipe) plus soft shadow` and the L3 row to `The same glass plus scrim; no simultaneous depth shadow`.

§5.3 "Glass": replace the first paragraph (from "For `InSceneCapture`, the accepted order…" to "…degrades to a readable translucent tint.") with:

```markdown
For `InSceneCapture`, the order is Haze's: bounded capture at full resolution,
a blur calibrated to a Gaussian of σ ≈ 12 px (the 20 dp Haze default),
Haze's grain texture at 0.15, then the canvas colour at 0.70. `ExternalBackdrop`
omits only the capture and blur passes because the compositor supplies them;
it retains grain and tint before a semantic role applies its strength. There
is one density: `Strong` is a compatible name that paints the same material.
Failure to capture or supply an external backdrop degrades to the opaque
canvas, which is what Haze paints when it cannot blur.
```

and replace "`StandardMaterial` preserves that existing full-strength recipe. `ContentSurface` applies the reference-derived `0.64` strength to the complete decorative stack…" with "`StandardMaterial` is the full-strength recipe. `ContentSurface` applies `0.64` to grain and tint…"; delete "and disables outline and lit-edge layers" and "because its normal highlight tint is itself translucent, the usual visible tint is approximately two percent" is kept. In the membrane paragraph, delete the sentence "The matching compositor polygon…" only if it mentions outline; otherwise leave it.

§6.1: change the `GlassSurface` row's contract to "One Haze material over bounded in-scene capture or an explicit compositor-supplied backdrop, with an opaque canvas fallback"; the `GlassCard` row stays.

§8: no change (the guard is a root script; §5.1 names it).

- [ ] **Step 2: STATUS.md**

Set `Updated: 2026-09-30`, change the `Implementation` bullet to say "at 1.9.2 with `STYLE-G7-O` active", and add as the first item under "Current checkout truth":

```markdown
- `STYLE-G7-O` paints the phone's Haze recipe in `GlassSurface` — σ ≈ 12 px
  blur, Haze's grain at 0.15 under the canvas tint at 0.70, an opaque canvas
  fallback, no outline and no lit edge — and moves the theme to the desktop
  ladder behind the existing token names: `radiusLg` 20, `radiusMd` 12,
  `radiusSm` 8, `radiusButton` 10, `radiusInput` = pill, `rowHeight` 40,
  `rowHeightLg` 52, `windowMargin` 16, `iconMd` 20, with `iconLg`,
  `spaceCardGap`, `spaceCardInset`, `topBarHeight`, `compSegmentHeight` and
  `cornerInset()` new. Every application inherits the values through its
  symlinks and carries its layout debt in the radius ratchet until its own
  unit. See [the record](docs/evidence/2026-09-30-haze-recipe-and-desktop-scale.md)
  and [the design](../docs/superpowers/specs/2026-09-30-desktop-glass-design.md).
```

- [ ] **Step 3: ROADMAP.md**

After the paragraph that starts "`STYLE-G7-K` follows `STYLE-G7-J`…", add:

```markdown
`STYLE-G7-O` and `STYLE-G7-P` carry the 2026-09-30 design: the phone's Haze
recipe as the one glass material, the desktop token ladder, and the two
components every application's fixed top bar needs, `CelestinaTopBar` and
`CelestinaSegmentedControl`. The radius guard that verifies the ladder's
concentric rule is the suite's `AUD-1-I`, because its baseline is a shared
ratchet every application prefix lowers.
```

- [ ] **Step 4: VALIDATION.md**

Add before `VAL-STYLE-06`:

```markdown
## VAL-STYLE-07 — The same glass as the phone

- **Status:** pending
- **Related implementation:** `STYLE-G7-O`, recorded in
  [the evidence](docs/evidence/2026-09-30-haze-recipe-and-desktop-scale.md)
- **Requires:** the deployed Siderita and Fluorita on the real session, the
  phone with Magnetita open on its tab pill, the gallery's glass page
- **Procedure:** open a context menu over a folder grid in Siderita and the
  floating dock over a bright photograph in Fluorita; hold the phone's pill
  beside them; then compare the gallery's live panel with its σ = 12 reference
- **Pass condition:** menu, dock and pill read as one material — the same
  softness of blur, the same visible grain, the same darkness of tint — with no
  hairline and no lit top edge on the desktop; the gallery's two panels match
  at arm's length
- **Result:** not run by hand
- **Evidence:** which of blur, grain or tint differs, if any, and the
  `glassBlurMax` the author prefers
```

- [ ] **Step 5: Fill the evidence record**

Procedure: the exact commands of Tasks 2 to 7 (fetch and conversion, cmake/ctest, the contrast script, the style guard, the offscreen gallery, the real-session screenshot and the crop comparison with the tried `glassBlurMax` values). Result: ctest count, guard outputs, the final `glassBlurMax` and the two crops' numbers. Limits: offscreen QPA proves construction only; the σ match is a real-session comparison by the session, the perceptual match against the phone is `VAL-STYLE-07`; the reference PNG's corners are not rounded. Follow-up: `VAL-STYLE-07`.

- [ ] **Step 6: Run every guard the unit touches**

```bash
bash scripts/check-architecture-contract.sh && bash scripts/test-architecture-scanners.sh && python3 scripts/check-language-contract.py && bash scripts/check-documentation-contract.sh && cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```

Expected: every guard prints its OK line; ctest 1/1. If the architecture guard reports the applications' QML naming a deleted token, it is `glassBorder`/`glassOutline`/`glassEdge*` only in `celestina/qml` (halted, excluded from the QML roots by its halt) — `grep -rn "glassBorder\|glassOutline\|glassEdge" --include='*.qml' siderita magnetita grafita fluorita hematita` must be empty; if it is not, that consumer line is in scope of this unit as a one-line token rename (`glassBorder` → `glassHighlight`).

- [ ] **Step 7: Commit**

```bash
git add celestina-style/DESIGN.md celestina-style/STATUS.md celestina-style/ROADMAP.md celestina-style/VALIDATION.md celestina-style/docs/evidence/2026-09-30-haze-recipe-and-desktop-scale.md
git commit -m "celestina-style-milestone: Record the Haze recipe and the desktop ladder"
```

The unit is ready for the author to land: `python3 scripts/land-unit.py unit/celestina-style/STYLE-G7-O --kind milestone` from the canonical checkout (MINOR: 1.9.2 → 1.10.0; the landing appends the history row).

---

## Unit AUD-1-I — the radius guard

### Task 9: Open the unit and its ledger row

**Files:**
- Modify: `docs/plans/active/2026-09-26-monorepo-hardening.md` (ledger table, after `AUD-1-H`)
- Create: `docs/evidence/2026-09-30-radius-guard.md`

- [ ] **Step 1: Open the worktree stacked on the style unit**

```bash
cd /home/toni/CODIGO/CELESTINA && scripts/worktree.sh open suite AUD-1-I --from unit/celestina-style/STYLE-G7-O && cd /home/toni/CODIGO/CELESTINA.worktrees/suite-AUD-1-I
```

- [ ] **Step 2: Ledger row**

Insert after the `AUD-1-H` row:

```markdown
| AUD-1-I | `suite:` | active | `scripts/radius_contract.py`; `scripts/test-radius-contract.py`; `scripts/radius-baseline.tsv`; `scripts/check-architecture-contract.sh`; `docs/projects.toml` | — | Add the radius guard the 2026-09-30 design specifies: a bounded QML scanner that refuses text or a glyph closer to a rounded corner than `cornerInset`, a nested token radius that is not concentric with its parent, and a numeric margin literal inside a rounded surface, with a shrink-only per-project baseline registered as a shared ratchet so each application lowers it in the unit that pays the debt | [evidence](../../evidence/2026-09-30-radius-guard.md) | None |
```

- [ ] **Step 3: Evidence stub** (same template as Task 1, scope `AUD-1-I — suite`, Procedure/Result/Limits filled in Task 12).

- [ ] **Step 4: Commit**

```bash
git add docs/plans/active/2026-09-26-monorepo-hardening.md docs/evidence/2026-09-30-radius-guard.md
git commit -m "suite-maintenance: Open AUD-1-I for the radius guard"
```

### Task 10: The scanner, test-first

**Files:**
- Create: `scripts/radius_contract.py`, `scripts/test-radius-contract.py`

**Interfaces:**
- Produces the CLI `python3 scripts/radius_contract.py --theme <CelestinaTheme.qml> --baseline <tsv> [--write-baseline] <label>=<qml-root> ...`. Exit 0 when every project's findings equal its baseline row, 1 when a project exceeds its row or a row exceeds reality (ratchet must fall), 2 on usage. Findings print as `path:line: rule: detail`.
- Produces the functions `scan_file(path: Path, tokens: dict[str, int]) -> list[Finding]`, `theme_tokens(theme: Path) -> dict[str, int]`, `corner_inset(radius: int) -> int`.

- [ ] **Step 1: Write the failing tests**

```python
#!/usr/bin/env python3
"""Hermetic fixtures for the radius guard: each rule positive and negative, and the ratchet."""

from __future__ import annotations

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import radius_contract as guard  # noqa: E402

THEME = textwrap.dedent(
    """
    pragma Singleton
    import QtQuick
    QtObject {
        readonly property int radiusXs: 3
        readonly property int radiusSm: 8
        readonly property int radiusMd: 12
        readonly property int radiusButton: 10
        readonly property int radiusLg: 20
        readonly property int radiusPill: 9999
        readonly property int radiusInput: radiusPill
        readonly property int spaceXs: 4
        readonly property int spaceSm: 8
        readonly property int spaceMd: 12
        readonly property int spaceCardInset: 8
        readonly property int windowMargin: 16
        function cornerInset(radius) { return Math.ceil(radius * 0.3) }
    }
    """
)


class RuleTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "CelestinaTheme.qml").write_text(THEME, encoding="utf-8")
        self.tokens = guard.theme_tokens(self.root / "CelestinaTheme.qml")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def scan(self, body: str) -> list[str]:
        path = self.root / "Sample.qml"
        path.write_text("import QtQuick\n" + textwrap.dedent(body), encoding="utf-8")
        return [f"{f.rule}: {f.detail}" for f in guard.scan_file(path, self.tokens)]

    def test_corner_inset_matches_the_theme(self) -> None:
        self.assertEqual(guard.corner_inset(20), 6)
        self.assertEqual(guard.corner_inset(12), 4)
        self.assertEqual(guard.corner_inset(8), 3)

    def test_theme_tokens_resolve_aliases(self) -> None:
        self.assertEqual(self.tokens["radiusInput"], 9999)
        self.assertEqual(self.tokens["radiusLg"], 20)

    def test_text_anchored_to_a_rounded_parent_without_inset_fails(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusMd
                Text { anchors.left: parent.left; anchors.bottom: parent.bottom; text: "name" }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertTrue(findings[0].startswith("inset: Text at 0 px needs cornerInset(radiusMd) = 4"))

    def test_text_with_a_token_inset_below_the_corner_inset_fails(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusLg
                Text { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceXs; text: "x" }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertIn("spaceXs (4) is below cornerInset(radiusLg) = 6", findings[0])

    def test_text_with_enough_inset_passes(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Text { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceSm; text: "x" }
                }
                """
            ),
            [],
        )

    def test_parent_padding_counts_as_the_inset(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Pane {
                    padding: CelestinaTheme.spaceSm
                    background: Rectangle { radius: CelestinaTheme.radiusLg }
                    Text { anchors.fill: parent; text: "x" }
                }
                """
            ),
            [],
        )

    def test_vertically_centred_child_is_not_a_corner_contact(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Text { anchors.verticalCenter: parent.verticalCenter; anchors.left: parent.left; anchors.leftMargin: CelestinaTheme.spaceXs; text: "x" }
                }
                """
            ),
            [],
        )

    def test_focus_ring_may_enter_the_inset(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    CelestinaFocusRing { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceXs }
                }
                """
            ),
            [],
        )

    def test_non_concentric_child_radius_fails(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusLg
                Rectangle { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceXs; radius: CelestinaTheme.radiusMd }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertIn("concentric: radiusLg (20) != radiusMd (12) + spaceXs (4)", findings[0])

    def test_concentric_child_radius_passes(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Rectangle { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceCardInset; radius: CelestinaTheme.radiusMd }
                }
                """
            ),
            [],
        )

    def test_a_pill_inside_anything_passes(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    Rectangle { anchors.fill: parent; anchors.margins: CelestinaTheme.spaceXs; radius: CelestinaTheme.radiusPill }
                }
                """
            ),
            [],
        )

    def test_numeric_margin_inside_a_rounded_surface_fails(self) -> None:
        findings = self.scan(
            """
            Rectangle {
                radius: CelestinaTheme.radiusLg
                Text { anchors.fill: parent; anchors.margins: 10; text: "x" }
            }
            """
        )
        self.assertEqual(len(findings), 1)
        self.assertTrue(findings[0].startswith("literal: anchors.margins: 10"))

    def test_a_non_token_radius_is_not_inspected(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: height / 2
                    Text { anchors.fill: parent; text: "x" }
                }
                """
            ),
            [],
        )

    def test_comments_are_ignored(self) -> None:
        self.assertEqual(
            self.scan(
                """
                Rectangle {
                    radius: CelestinaTheme.radiusLg
                    // Text { anchors.fill: parent; text: "x" }
                    /* Rectangle { anchors.margins: 3 } */
                }
                """
            ),
            [],
        )


class RatchetTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "CelestinaTheme.qml").write_text(THEME, encoding="utf-8")
        (self.root / "app").mkdir()
        (self.root / "app" / "Bad.qml").write_text(
            'import QtQuick\nRectangle { radius: CelestinaTheme.radiusLg; Text { anchors.fill: parent; text: "x" } }\n',
            encoding="utf-8",
        )
        self.baseline = self.root / "radius-baseline.tsv"

    def tearDown(self) -> None:
        self.temp.cleanup()

    def run_guard(self, *extra: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(HERE / "radius_contract.py"),
                "--theme",
                str(self.root / "CelestinaTheme.qml"),
                "--baseline",
                str(self.baseline),
                *extra,
                f"app={self.root / 'app'}",
            ],
            text=True,
            capture_output=True,
        )

    def test_missing_baseline_row_is_zero_and_fails(self) -> None:
        self.baseline.write_text("# findings\tproject\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("app: 1 finding(s) over the baseline 0", result.stderr)
        self.assertIn("Bad.qml:2: inset:", result.stdout)

    def test_matching_baseline_passes(self) -> None:
        self.baseline.write_text("# findings\tproject\n1\tapp\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Radius contract: OK", result.stdout)

    def test_stale_baseline_must_fall(self) -> None:
        self.baseline.write_text("# findings\tproject\n3\tapp\n", encoding="utf-8")
        result = self.run_guard()
        self.assertEqual(result.returncode, 1)
        self.assertIn("app: baseline 3 exceeds the 1 finding(s) found; lower it in this commit", result.stderr)

    def test_write_baseline_records_reality(self) -> None:
        self.baseline.write_text("# findings\tproject\n", encoding="utf-8")
        result = self.run_guard("--write-baseline")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("1\tapp", self.baseline.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run to verify they fail**

```bash
python3 scripts/test-radius-contract.py
```

Expected: `ModuleNotFoundError: No module named 'radius_contract'`.

- [ ] **Step 3: Write the scanner**

```python
#!/usr/bin/env python3
"""The radius guard: text out of the corner's curve, concentric nesting, no margin literals.

Reads CelestinaTheme.qml for the token values, walks every QML file under the
given roots, and inspects each object whose `radius`/`cornerRadius` is a theme
token (`CelestinaTheme.radius*`). For that object's direct children it applies:

  inset      a glyph-bearing child (Text, Label, CelestinaIcon, CelestinaSectionLabel,
             Image) touching a corner — anchored to the parent's left/right/top/
             bottom/fill/centerIn, or positioned with x/y — sits at least
             cornerInset(radius) = ceil(0.3 * radius) from it; the inset is the
             child's own token margin, or the parent's token padding, or 0.
             A child anchored to verticalCenter/horizontalCenter with a token
             side margin is not a corner contact. CelestinaFocusRing may enter.
  concentric a child with its own token radius satisfies
             parent = child + inset, unless either radius is radiusPill or the
             child radius is radiusNone.
  literal    a numeric literal bound to padding, leftPadding, rightPadding,
             topPadding, bottomPadding, anchors.margins, anchors.leftMargin,
             anchors.rightMargin, anchors.topMargin, anchors.bottomMargin, x or y
             of a direct child.

Findings count per project against scripts/radius-baseline.tsv, a shrink-only
ratchet: a project over its row fails, and a row over reality fails too, so the
commit that pays the debt is the commit that lowers the floor.

    radius_contract.py --theme THEME --baseline TSV [--write-baseline] LABEL=ROOT ...
"""

from __future__ import annotations

import argparse
import dataclasses
import math
import pathlib
import re
import sys
from typing import Iterable, Iterator

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from architecture_scanners import qml_files, strip_qml_comments  # noqa: E402

GLYPH_TYPES = {"Text", "Label", "CelestinaIcon", "CelestinaSectionLabel", "Image"}
EXEMPT_TYPES = {"CelestinaFocusRing"}
INSET_PROPERTIES = (
    "padding", "leftPadding", "rightPadding", "topPadding", "bottomPadding",
    "anchors.margins", "anchors.leftMargin", "anchors.rightMargin",
    "anchors.topMargin", "anchors.bottomMargin", "x", "y",
)
CORNER_ANCHORS = {
    "anchors.fill", "anchors.centerIn", "anchors.left", "anchors.right",
    "anchors.top", "anchors.bottom",
}
CENTRE_ANCHORS = {"anchors.verticalCenter", "anchors.horizontalCenter"}
TOKEN = re.compile(r"^CelestinaTheme\.([A-Za-z0-9_]+)$")
NUMBER = re.compile(r"^-?[0-9]+(?:\.[0-9]+)?$")
OBJECT_START = re.compile(r"(?<![\w.])([A-Z][A-Za-z0-9_]*)\s*\{")
BINDING = re.compile(r"(?m)^\s*([A-Za-z_][\w.]*)\s*:\s*")


def corner_inset(radius: int) -> int:
    return math.ceil(radius * 0.3)


@dataclasses.dataclass(frozen=True)
class Finding:
    path: pathlib.Path
    line: int
    rule: str
    detail: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.rule}: {self.detail}"


@dataclasses.dataclass
class Node:
    type_name: str
    start: int
    end: int
    bindings: dict[str, tuple[str, int]] = dataclasses.field(default_factory=dict)
    children: list["Node"] = dataclasses.field(default_factory=list)


def theme_tokens(theme: pathlib.Path) -> dict[str, int]:
    text = strip_qml_comments(theme.read_text(encoding="utf-8"))
    tokens: dict[str, int] = {}
    aliases: dict[str, str] = {}
    for match in re.finditer(
        r"readonly property int (\w+):\s*([A-Za-z_]\w*|-?[0-9]+)", text
    ):
        name, value = match.group(1), match.group(2)
        if NUMBER.match(value):
            tokens[name] = int(value)
        else:
            aliases[name] = value
    for name, target in aliases.items():
        if target in tokens:
            tokens[name] = tokens[target]
    return tokens


def parse(text: str) -> list[Node]:
    """A bounded object tree: type names, their brace span, and the bindings
    written directly inside them (value text up to the newline or `;`, with
    braces, brackets and parentheses balanced)."""
    roots: list[Node] = []
    stack: list[Node] = []
    index = 0
    length = len(text)
    while index < length:
        match = OBJECT_START.search(text, index)
        if not match:
            break
        # Skip an object start that sits inside a binding value of the current
        # node (an object-valued binding like `background: Rectangle {`): it is
        # still a child for nesting purposes, which is what the rules need.
        node = Node(match.group(1), match.start(), -1)
        (stack[-1].children if stack else roots).append(node)
        stack.append(node)
        index = match.end()
        depth = 1
        while index < length and depth:
            character = text[index]
            if character in "\"'`":
                quote = character
                index += 1
                while index < length and text[index] != quote:
                    index += 2 if text[index] == "\\" else 1
                index += 1
                continue
            if character == "{":
                inner = OBJECT_START.match(text, _word_start(text, index))
                if inner and inner.end() == index + 1:
                    break  # a nested object: the outer loop parses it
                depth += 1
            elif character == "}":
                depth -= 1
                if depth == 0:
                    node.end = index
                    stack.pop()
            index += 1
        else:
            continue
        # Broke out for a nested object: leave `node` open on the stack and let
        # the outer loop find the nested start at `index`.
        index = _word_start(text, index)
    for node in list(stack):
        node.end = length
    _collect_bindings(text, roots)
    return roots


def _word_start(text: str, brace: int) -> int:
    cursor = brace - 1
    while cursor >= 0 and text[cursor].isspace():
        cursor -= 1
    while cursor >= 0 and (text[cursor].isalnum() or text[cursor] == "_"):
        cursor -= 1
    return cursor + 1


def _collect_bindings(text: str, nodes: list[Node]) -> None:
    for node in nodes:
        body_start = text.index("{", node.start) + 1
        segments = [(body_start, node.end)]
        for child in node.children:
            last_start, last_end = segments.pop()
            segments.append((last_start, child.start))
            segments.append((child.end + 1, last_end))
        for start, end in segments:
            for match in BINDING.finditer(text, start, end):
                name = match.group(1)
                value_end = _value_end(text, match.end(), end)
                value = " ".join(text[match.end():value_end].split())
                line = text.count("\n", 0, match.start()) + 1
                node.bindings[name] = (value, line)
        _collect_bindings(text, node.children)


def _value_end(text: str, start: int, limit: int) -> int:
    depth = 0
    index = start
    while index < limit:
        character = text[index]
        if character in "([{":
            depth += 1
        elif character in ")]}":
            depth -= 1
        elif character in ";\n" and depth <= 0:
            return index
        index += 1
    return limit


def token_value(value: str, tokens: dict[str, int]) -> tuple[str, int] | None:
    match = TOKEN.match(value)
    if not match or match.group(1) not in tokens:
        return None
    return match.group(1), tokens[match.group(1)]


def radius_of(node: Node, tokens: dict[str, int]) -> tuple[str, int] | None:
    for name in ("radius", "cornerRadius"):
        if name in node.bindings:
            return token_value(node.bindings[name][0], tokens)
    return None


def own_inset(node: Node, tokens: dict[str, int]) -> tuple[str, int] | None:
    """The smallest token inset the child declares on a side, or (name, 0) when
    it touches the parent with no margin at all."""
    smallest: tuple[str, int] | None = None
    for name in INSET_PROPERTIES:
        if name in node.bindings:
            resolved = token_value(node.bindings[name][0], tokens)
            if resolved and (smallest is None or resolved[1] < smallest[1]):
                smallest = resolved
    return smallest


def touches_corner(node: Node) -> bool:
    keys = set(node.bindings)
    if keys & CENTRE_ANCHORS and not (keys & {"anchors.fill", "anchors.centerIn"}):
        return False
    return bool(keys & CORNER_ANCHORS) or "x" in keys or "y" in keys


def parent_padding(node: Node, tokens: dict[str, int]) -> int:
    smallest: int | None = None
    for name in ("padding", "leftPadding", "rightPadding", "topPadding", "bottomPadding"):
        if name in node.bindings:
            resolved = token_value(node.bindings[name][0], tokens)
            if resolved and (smallest is None or resolved[1] < smallest):
                smallest = resolved[1]
    return smallest or 0


def rounded_nodes(nodes: Iterable[Node], tokens: dict[str, int]) -> Iterator[tuple[Node, Node, str, int]]:
    """Yield (owner, rounded, token, radius): `rounded` carries the radius and
    `owner` is the node whose children the rule inspects — the rounded node
    itself, or the control whose `background:` it is."""
    for node in nodes:
        radius = radius_of(node, tokens)
        if radius:
            yield node, node, radius[0], radius[1]
        for child in node.children:
            child_radius = radius_of(child, tokens)
            if child_radius and _is_background(node, child):
                yield node, child, child_radius[0], child_radius[1]
        yield from rounded_nodes(node.children, tokens)


def _is_background(parent: Node, child: Node) -> bool:
    value = parent.bindings.get("background")
    return bool(value) and value[0].startswith(child.type_name + " {")


def scan_file(path: pathlib.Path, tokens: dict[str, int]) -> list[Finding]:
    text = strip_qml_comments(path.read_text(encoding="utf-8"))
    findings: list[Finding] = []
    for owner, rounded, token, radius in rounded_nodes(parse(text), tokens):
        needed = corner_inset(radius)
        padding = parent_padding(owner, tokens)
        for child in owner.children:
            if child is rounded or child.type_name in EXEMPT_TYPES:
                continue
            line = text.count("\n", 0, child.start) + 1
            for name in INSET_PROPERTIES:
                if name in child.bindings and NUMBER.match(child.bindings[name][0]):
                    findings.append(Finding(path, child.bindings[name][1], "literal",
                                            f"{name}: {child.bindings[name][0]}; use a space* token"))
            inset = own_inset(child, tokens)
            effective = (inset[1] if inset else 0) + padding
            child_radius = radius_of(child, tokens)
            if child_radius and token != "radiusPill" and child_radius[0] not in ("radiusPill", "radiusNone"):
                if radius != child_radius[1] + effective:
                    inset_text = f"{inset[0]} ({inset[1]})" if inset else f"no inset ({padding})"
                    findings.append(Finding(path, line, "concentric",
                                            f"{token} ({radius}) != {child_radius[0]} ({child_radius[1]}) + {inset_text}"))
            if child.type_name in GLYPH_TYPES and touches_corner(child) and effective < needed:
                if inset:
                    detail = f"{child.type_name} inset {inset[0]} ({inset[1]}) is below cornerInset({token}) = {needed}"
                else:
                    detail = f"{child.type_name} at {effective} px needs cornerInset({token}) = {needed}"
                findings.append(Finding(path, line, "inset", detail))
    return findings


def read_baseline(path: pathlib.Path) -> dict[str, int]:
    rows: dict[str, int] = {}
    if not path.is_file():
        return rows
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        count, project = line.split("\t", 1)
        rows[project.strip()] = int(count)
    return rows


def write_baseline(path: pathlib.Path, counts: dict[str, int]) -> None:
    lines = [
        "# Radius-contract debt ratchet. A row may only fall, and it falls in the",
        "# same commit that earns the reduction. Registered in docs/projects.toml as",
        "# a shared ratchet file so every project prefix may lower its own row.",
        "# findings<TAB>project",
    ]
    lines += [f"{count}\t{project}" for project, count in sorted(counts.items())]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--theme", required=True, type=pathlib.Path)
    parser.add_argument("--baseline", required=True, type=pathlib.Path)
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("roots", nargs="+", metavar="LABEL=ROOT")
    arguments = parser.parse_args(argv)

    if not arguments.theme.is_file():
        print(f"missing theme: {arguments.theme}", file=sys.stderr)
        return 2
    tokens = theme_tokens(arguments.theme)
    if "radiusLg" not in tokens:
        print(f"{arguments.theme}: no radius tokens found", file=sys.stderr)
        return 2

    counts: dict[str, int] = {}
    for spec in arguments.roots:
        label, _, root = spec.partition("=")
        if not label or not root:
            print(f"expected LABEL=ROOT, got {spec}", file=sys.stderr)
            return 2
        findings = []
        for path in qml_files([root]):
            if path.resolve() == arguments.theme.resolve():
                continue
            findings.extend(scan_file(path, tokens))
        for finding in sorted(findings, key=lambda f: (str(f.path), f.line)):
            print(finding)
        counts[label] = len(findings)

    if arguments.write_baseline:
        write_baseline(arguments.baseline, counts)
        print(f"Radius contract: baseline written to {arguments.baseline}")
        return 0

    baseline = read_baseline(arguments.baseline)
    status = 0
    for label, count in counts.items():
        floor = baseline.get(label, 0)
        if count > floor:
            print(f"{label}: {count} finding(s) over the baseline {floor}", file=sys.stderr)
            status = 1
        elif count < floor:
            print(f"{label}: baseline {floor} exceeds the {count} finding(s) found; lower it in this commit", file=sys.stderr)
            status = 1
    if status == 0:
        print("Radius contract: OK")
    return status


if __name__ == "__main__":
    sys.exit(main())
```

The parser is deliberately bounded: it treats every `TypeName {` as an object (including object-valued bindings such as `background: Rectangle {`, which `rounded_nodes` recognises through `_is_background`), reads single-line bindings, and never evaluates expressions. Anything it cannot read as a token is not inspected, and the tests pin that.

- [ ] **Step 4: Run the tests until green**

```bash
python3 scripts/test-radius-contract.py -v
```

Expected: every case in `RuleTests` and `RatchetTests` passes. If `parse` mis-nests on the `Pane { background: Rectangle {…} Text {…} }` fixture, fix the nested-start handling in `parse` before touching the rules: the fixture's expected structure is `Pane` with children `[Rectangle, Text]`.

- [ ] **Step 5: Commit**

```bash
git add scripts/radius_contract.py scripts/test-radius-contract.py
git commit -m "suite-maintenance: Add the radius guard scanner with hermetic fixtures"
```

### Task 11: Wire the guard in with its ratchet

**Files:**
- Create: `scripts/radius-baseline.tsv`
- Modify: `scripts/check-architecture-contract.sh` (after the `check-style-contract.sh` call, line ~477), `docs/projects.toml` (`shared_ratchet_files`, line ~43)

- [ ] **Step 1: Add the guard call**

In `scripts/check-architecture-contract.sh`, directly after the block that runs `celestina-style/scripts/check-style-contract.sh` (keep its `failures=1` shape), add:

```bash
    # The radius contract (spec 2026-09-30 §3.3): glyphs out of the corner's
    # curve, concentric nesting and no margin literals inside rounded surfaces,
    # against the per-project ratchet in scripts/radius-baseline.tsv.
    radius_roots=()
    while IFS=$'\t' read -r role identifier _path qml_root; do
        case $role in
            application | shell | style) radius_roots+=("$identifier=$qml_root") ;;
        esac
    done < <(python3 scripts/architecture_scanners.py registry-qml-projects "$registry_file")
    if ! python3 scripts/radius_contract.py \
        --theme celestina-style/CelestinaTheme.qml \
        --baseline scripts/radius-baseline.tsv \
        "${radius_roots[@]}"; then
        fail "the radius contract failed"
    fi
```

`registry_file` is the read-only variable the script derives at line 24 (`ARCHITECTURE_REGISTRY_FILE` override, `docs/projects.toml` default) and `fail` is its existing reporting function, the same one the style guard call above uses. `registry-qml-projects` lists the halted shell (`shell celestina celestina celestina/qml`); the style guard scans it through the same rows, so the radius guard scans it too and its findings count under a `celestina` baseline row that no unit may lower while the shell is halted. State that in the evidence.

- [ ] **Step 2: Register the ratchet**

In `docs/projects.toml`, after `"scripts/qmllint-baseline.tsv",` inside `shared_ratchet_files`, add:

```toml
  # The radius ratchet moves for the same reason: the application unit that
  # takes its rows off the corner's curve is the one that lowers its floor.
  "scripts/radius-baseline.tsv",
```

- [ ] **Step 3: Write the baseline from reality**

```bash
python3 scripts/radius_contract.py --theme celestina-style/CelestinaTheme.qml --baseline scripts/radius-baseline.tsv --write-baseline $(python3 scripts/architecture_scanners.py registry-qml-projects docs/projects.toml | awk -F'\t' '$1=="application"||$1=="style"{print $2"="$4}') | tail -1 && cat scripts/radius-baseline.tsv
```

Expected: one row per application plus `celestina-style`; the style row is `0` (Task 6 made `ListSection` concentric and the module has no glyph in a corner; if the row is not 0, the printed findings are the module's own debt and belong to this unit: fix them, re-run).

- [ ] **Step 4: Run the full guard and the scanner tests**

```bash
bash scripts/check-architecture-contract.sh && python3 scripts/test-radius-contract.py && bash scripts/test-documentation-contract.sh && bash scripts/test-commit-scope.sh
```

Expected: `Radius contract: OK` inside the architecture guard's output, every test script green (the commit-scope tests read the registry and must accept the new ratchet path).

- [ ] **Step 5: Evidence and commit**

Fill `docs/evidence/2026-09-30-radius-guard.md`: Procedure (the commands above), Result (the baseline rows as written, the guard outputs), Limits (the scanner reads single-line token bindings and `background:` objects only; an inset computed through an expression, a `Layout.margins`, or a radius taken from a property alias is not inspected; the baseline counts findings, not files).

```bash
git add scripts/radius-baseline.tsv scripts/check-architecture-contract.sh docs/projects.toml docs/evidence/2026-09-30-radius-guard.md
git commit -m "suite-maintenance: Run the radius guard with a shrink-only per-project ratchet"
```

Ready to land: `python3 scripts/land-unit.py unit/suite/AUD-1-I --kind maintenance` after `STYLE-G7-O` has landed.

---

## Unit STYLE-G7-P — the top bar and the segmented control

### Task 12: Open the unit and its ledger row

- [ ] **Step 1: Open the worktree stacked on the guard**

```bash
cd /home/toni/CODIGO/CELESTINA && scripts/worktree.sh open celestina-style STYLE-G7-P --from unit/suite/AUD-1-I && cd /home/toni/CODIGO/CELESTINA.worktrees/celestina-style-STYLE-G7-P
```

- [ ] **Step 2: Ledger row** (under `STYLE-G7-O`):

```markdown
| STYLE-G7-P | `celestina-style:` | active | `CelestinaTopBar.qml`; `CelestinaSegmentedControl.qml`; `tests/tst_topbar.qml`; `tests/tst_segmented.qml`; `qmldir`; `CMakeLists.txt`; `scripts/check-style-contract.sh`; `gallery/Gallery.qml`; `DESIGN.md`; `STATUS.md`; `VALIDATION.md` | — | Publish the two components every application's fixed top bar needs: `CelestinaTopBar` (a `topBarHeight` canvas strip spanning sidebar and content with a leading slot, left-aligned eliding title and subtitle, and an icon-only trailing slot the style guard keeps free of text buttons) and `CelestinaSegmentedControl` (peer destinations in one `radiusButton` plate, concentric selected segment, one Tab stop with arrow keys, `PageTabList`/`PageTab` roles) | [evidence](../../evidence/2026-09-30-top-bar-and-segments.md) | `VAL-STYLE-08` |
```

- [ ] **Step 3: Evidence stub** (template, scope `STYLE-G7-P`), then commit:

```bash
git add celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md celestina-style/docs/evidence/2026-09-30-top-bar-and-segments.md
git commit -m "celestina-style-milestone: Open STYLE-G7-P for the top bar and the segmented control"
```

### Task 13: `CelestinaSegmentedControl`

**Files:**
- Create: `celestina-style/CelestinaSegmentedControl.qml`, `celestina-style/tests/tst_segmented.qml`
- Modify: `celestina-style/qmldir`, `celestina-style/CMakeLists.txt` (QML_FILES)

**Interfaces:**
- Produces: `CelestinaSegmentedControl { required property var model /* [{key, icon, label}] */; property int currentIndex; property bool iconOnly; property string helpText; signal activated(int index) }`; `implicitHeight == CelestinaTheme.compSegmentHeight`; child buttons have `objectName: "segment-<index>"`.

- [ ] **Step 1: Write the failing test**

```qml
import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// A row of peer destinations. What is asserted: the plate is radiusButton
// tall compSegmentHeight; the selected segment's plate is concentric with it;
// one Tab stop lands on the current segment; Left/Right/Home/End emit
// `activated` and the control never changes `currentIndex` itself; the roles
// are a page tab list of page tabs; under reduced motion the fill lands at once.
TestCase {
    id: testCase

    name: "CelestinaSegmentedControl"
    when: testWindow.visible

    property var activations: []

    Window {
        id: testWindow
        width: 480
        height: 160
        visible: true

        Item { id: before; width: 10; height: 10; activeFocusOnTab: true }

        CelestinaSegmentedControl {
            id: control
            x: 20
            y: 40
            helpText: "Secciones"
            model: [
                { key: "performance", icon: "gauge", label: "Rendimiento" },
                { key: "processes", icon: "view-list", label: "Procesos" },
                { key: "sensors", icon: "cpu", label: "Sensores" }
            ]
            currentIndex: 1
            onActivated: function(index) { testCase.activations.push(index) }
        }

        Item { id: after; x: 400; width: 10; height: 10; activeFocusOnTab: true }
    }

    function init() {
        testCase.activations = []
        control.currentIndex = 1
        control.iconOnly = false
        before.forceActiveFocus()
    }

    function segment(index) {
        return findChild(control, "segment-" + index)
    }

    function test_geometry_is_the_bar_slot() {
        compare(control.implicitHeight, CelestinaTheme.compSegmentHeight)
        const plate = findChild(control, "segmentedPlate")
        verify(plate)
        compare(plate.radius, CelestinaTheme.radiusButton)
        const selected = findChild(segment(1), "segmentPlate")
        verify(selected)
        compare(selected.radius, CelestinaTheme.radiusButton - CelestinaTheme.spaceXs)
        compare(segment(1).x, CelestinaTheme.spaceXs + segment(0).width + CelestinaTheme.spaceXs)
    }

    function test_one_tab_stop_lands_on_the_current_segment() {
        keyClick(Qt.Key_Tab)
        verify(segment(1).activeFocus)
        keyClick(Qt.Key_Tab)
        verify(after.activeFocus)
    }

    function test_arrows_emit_and_never_move_the_index_themselves() {
        keyClick(Qt.Key_Tab)
        keyClick(Qt.Key_Right)
        compare(testCase.activations, [2])
        compare(control.currentIndex, 1)
        keyClick(Qt.Key_Left)
        keyClick(Qt.Key_Left)
        compare(testCase.activations, [2, 0, 2])
        keyClick(Qt.Key_Home)
        keyClick(Qt.Key_End)
        compare(testCase.activations, [2, 0, 2, 0, 2])
    }

    function test_click_activates() {
        mouseClick(segment(2))
        compare(testCase.activations, [2])
    }

    function test_roles_and_names() {
        compare(control.Accessible.role, Accessible.PageTabList)
        compare(control.Accessible.name, "Secciones")
        compare(segment(0).Accessible.role, Accessible.PageTab)
        compare(segment(0).Accessible.name, "Rendimiento")
        verify(segment(1).checked)
        verify(!segment(0).checked)
    }

    function test_icon_only_hides_the_labels_and_keeps_the_names() {
        control.iconOnly = true
        const label = findChild(segment(0), "segmentLabel")
        verify(label)
        verify(!label.visible)
        compare(segment(0).Accessible.name, "Rendimiento")
        verify(segment(0).width < 2 * CelestinaTheme.compSegmentHeight + CelestinaTheme.spaceMd)
    }

    function test_reduced_motion_lands_the_fill_at_once() {
        const plate = findChild(segment(0), "segmentPlate")
        verify(plate)
        compare(findChild(plate, "segmentFill").duration,
                CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast)
    }
}
```

- [ ] **Step 2: Register the test and run to verify it fails**

Qt Quick Test runs every `tst_*.qml` in `tests/`; no registration is needed. Run:

```bash
cmake -S celestina-style -B celestina-style/build -DBUILD_TESTING=ON && cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```

Expected: FAIL: `CelestinaSegmentedControl is not a type`.

- [ ] **Step 3: Write the component**

```qml
import QtQuick
import QtQuick.Controls

// ─── CelestinaSegmentedControl ────────────────────────────────────────────────
// Peer destinations in one control-shaped plate: the sections of a resource
// monitor, the views of a library, the documents of an editor. It replaces the
// floating tab pill of the phone with the segmented control a desktop bar
// carries at its left. The plate is `radiusButton` on `card`; the current
// segment wears `surfaceSelected` at `radiusButton - spaceXs`, concentric with
// the plate because it sits `spaceXs` inside it.
//
// Navigation, not choice: activating is immediate and the control never moves
// `currentIndex` itself — it asks through `activated(index)` and the host, who
// owns the page, answers by setting `currentIndex`. One Tab stop: Tab lands on
// the current segment, Left/Right/Home/End walk the rest, Tab leaves.
//
// Model: [{ key: string, icon: string, label: string }]. `iconOnly` hides the
// words and keeps them as the segments' accessible names.
// ──────────────────────────────────────────────────────────────────────────────
FocusScope {
    id: control

    required property var model
    property int currentIndex: 0
    property bool iconOnly: false
    // The name a screen reader hears for the whole list; product copy.
    property string helpText: ""

    signal activated(int index)

    implicitWidth: row.implicitWidth + CelestinaTheme.spaceXs * 2
    implicitHeight: CelestinaTheme.compSegmentHeight

    Accessible.role: Accessible.PageTabList
    Accessible.name: control.helpText

    function move(delta) {
        const count = control.model.length
        if (count === 0)
            return
        control.activated((control.currentIndex + delta + count) % count)
    }

    Keys.onLeftPressed: function(event) { control.move(-1); event.accepted = true }
    Keys.onRightPressed: function(event) { control.move(1); event.accepted = true }
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Home && control.model.length > 0) {
            control.activated(0)
            event.accepted = true
        } else if (event.key === Qt.Key_End && control.model.length > 0) {
            control.activated(control.model.length - 1)
            event.accepted = true
        }
    }

    Rectangle {
        id: plate
        objectName: "segmentedPlate"
        anchors.fill: parent
        radius: CelestinaTheme.radiusButton
        color: CelestinaTheme.card
    }

    Row {
        id: row
        anchors.fill: parent
        anchors.margins: CelestinaTheme.spaceXs
        spacing: CelestinaTheme.spaceXs

        Repeater {
            model: control.model

            AbstractButton {
                id: segment

                required property int index
                required property var modelData
                readonly property bool current: index === control.currentIndex

                objectName: "segment-" + index
                height: row.height
                implicitWidth: content.implicitWidth + CelestinaTheme.spaceMd * 2
                hoverEnabled: true
                checkable: false
                checked: current
                focusPolicy: Qt.TabFocus
                // Only the current segment is in the Tab order.
                focus: current

                Accessible.role: Accessible.PageTab
                Accessible.name: modelData.label
                Accessible.checked: current

                onClicked: control.activated(index)

                background: Rectangle {
                    objectName: "segmentPlate"
                    radius: CelestinaTheme.radiusButton - CelestinaTheme.spaceXs
                    color: segment.current
                           ? CelestinaTheme.surfaceSelected
                           : segment.down
                             ? CelestinaTheme.pressedWash
                             : segment.hovered
                               ? CelestinaTheme.surfaceHover
                               : CelestinaTheme.clear
                    Behavior on color {
                        ColorAnimation {
                            objectName: "segmentFill"
                            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
                        }
                    }
                }

                CelestinaFocusRing {
                    target: segment
                    cornerRadius: CelestinaTheme.radiusButton - CelestinaTheme.spaceXs
                    shown: segment.visualFocus
                }

                contentItem: Item {
                    implicitWidth: content.implicitWidth
                    implicitHeight: content.implicitHeight

                    Row {
                        id: content
                        anchors.centerIn: parent
                        spacing: CelestinaTheme.spaceSm

                        CelestinaIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: segment.modelData.icon
                            width: CelestinaTheme.iconMd
                            height: width
                            tone: segment.current ? CelestinaIcon.Primary : CelestinaIcon.Secondary
                        }

                        Text {
                            objectName: "segmentLabel"
                            anchors.verticalCenter: parent.verticalCenter
                            visible: !control.iconOnly
                            text: segment.modelData.label
                            textFormat: Text.PlainText
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontBody
                            font.weight: segment.current ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
                            color: segment.current ? CelestinaTheme.text : CelestinaTheme.textMuted
                        }
                    }
                }
            }
        }
    }
}
```

`checked: current` with `checkable: false` is what NavItem does through `Accessible.checked`; if `AbstractButton` refuses `checked` while not checkable (Qt resets it), replace the two lines with `checkable: true; checked: current; onToggled: checked = current` and keep `onClicked` as the single activation path — the test's `verify(segment(1).checked)` is the contract.

Register it: add `CelestinaSegmentedControl 1.0 CelestinaSegmentedControl.qml` after the `CelestinaSwitch` line in `qmldir`, and `        CelestinaSegmentedControl.qml` after `CelestinaSwitch.qml` in `CMakeLists.txt` `QML_FILES`.

- [ ] **Step 4: Run the tests until green**

```bash
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure && bash scripts/check-architecture-contract.sh
```

Expected: ctest 1/1; the architecture guard's `cmake-qml-registration` parity and the radius guard both OK (the plate holds a `radiusButton - spaceXs` child at `spaceXs`: `10 = 6 + 4`, concentric — but the child radius is an expression, not a token, so the guard does not inspect it; that is acceptable and stated in the evidence).

- [ ] **Step 5: Commit**

```bash
git add celestina-style/CelestinaSegmentedControl.qml celestina-style/tests/tst_segmented.qml celestina-style/qmldir celestina-style/CMakeLists.txt
git commit -m "celestina-style-milestone: Publish CelestinaSegmentedControl"
```

### Task 14: `CelestinaTopBar`

**Files:**
- Create: `celestina-style/CelestinaTopBar.qml`, `celestina-style/tests/tst_topbar.qml`
- Modify: `celestina-style/qmldir`, `celestina-style/CMakeLists.txt`, `celestina-style/scripts/check-style-contract.sh` (after the literal-alpha check)

**Interfaces:**
- Produces: `CelestinaTopBar { property string title; property string subtitle; property alias leadingData; property alias trailingData }`; `implicitHeight == CelestinaTheme.topBarHeight`; children named `topBarLeading`, `topBarTitle`, `topBarSubtitle`, `topBarTrailing`.

- [ ] **Step 1: Write the failing test**

```qml
import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// The fixed bar every application wears. Asserted: its height and canvas
// fill; the title left-aligned right after the leading slot and eliding; the
// trailing slot right-aligned; the tool bar role; that it takes no focus of
// its own while its buttons keep theirs.
TestCase {
    id: testCase

    name: "CelestinaTopBar"
    when: testWindow.visible

    Window {
        id: testWindow
        width: 640
        height: 200
        visible: true

        CelestinaTopBar {
            id: bar
            width: 640
            title: "Un título tan largo que no cabe en la anchura que le queda entre los dos huecos de la barra"
            subtitle: "AMD Ryzen 7"
            leadingData: [
                CelestinaSegmentedControl {
                    id: segments
                    helpText: "Secciones"
                    model: [ { key: "a", icon: "gauge", label: "Rendimiento" }, { key: "b", icon: "view-list", label: "Procesos" } ]
                }
            ]
            trailingData: [
                CelestinaIconButton { id: firstAction; iconName: "search"; helpText: "Buscar"; role: CelestinaButton.Ghost },
                CelestinaIconButton { id: secondAction; iconName: "view-grid"; helpText: "Vista"; role: CelestinaButton.Ghost }
            ]
        }
    }

    function test_height_and_fill() {
        compare(bar.implicitHeight, CelestinaTheme.topBarHeight)
        compare(bar.height, CelestinaTheme.topBarHeight)
        compare(bar.background.color, CelestinaTheme.canvas)
        compare(bar.leftPadding, CelestinaTheme.windowMargin)
        compare(bar.rightPadding, CelestinaTheme.windowMargin)
    }

    function test_title_sits_left_after_the_leading_slot_and_elides() {
        const leading = findChild(bar, "topBarLeading")
        const title = findChild(bar, "topBarTitle")
        const subtitle = findChild(bar, "topBarSubtitle")
        verify(leading && title && subtitle)
        compare(leading.x, 0)
        compare(title.horizontalAlignment, Text.AlignLeft)
        compare(title.elide, Text.ElideRight)
        verify(title.truncated)
        compare(title.font.pixelSize, CelestinaTheme.fontTitle)
        compare(subtitle.font.pixelSize, CelestinaTheme.fontRowSecondary)
        verify(title.mapToItem(bar, 0, 0).x >= leading.width + CelestinaTheme.windowMargin)
    }

    function test_trailing_sits_right() {
        const trailing = findChild(bar, "topBarTrailing")
        verify(trailing)
        compare(trailing.mapToItem(bar, trailing.width, 0).x, bar.width - CelestinaTheme.windowMargin)
        compare(secondAction.x, firstAction.x + firstAction.width + CelestinaTheme.spaceXs)
        compare(firstAction.iconSize, CelestinaTheme.iconMd)
    }

    function test_role_and_focus() {
        compare(bar.Accessible.role, Accessible.ToolBar)
        compare(bar.Accessible.name, bar.title)
        verify(!bar.activeFocusOnTab)
        keyClick(Qt.Key_Tab)
        verify(segments.activeFocus || findChild(segments, "segment-0").activeFocus)
        keyClick(Qt.Key_Tab)
        verify(firstAction.activeFocus)
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Same ctest command. Expected: `CelestinaTopBar is not a type`.

- [ ] **Step 3: Write the component**

```qml
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// ─── CelestinaTopBar ──────────────────────────────────────────────────────────
// The one fixed bar an application wears, spanning the whole window above its
// sidebar and content. It is window chrome, not a floating layer: canvas fill,
// no card, no glass, `topBarHeight` tall. Three slots, left to right:
//
//   leading   navigation — a CelestinaSegmentedControl, a back button, or nothing
//   title     the page, left-aligned right after `leading`, eliding; never
//             centred, never uppercased; `subtitle` beneath it in textMuted
//   trailing  icon-only actions, right-aligned, spaceXs apart
//
// `trailing` holds CelestinaIconButton children only. A text button in it
// fails the style guard: on a desktop bar the verbs are glyphs and their names
// travel through helpText; words belong to dialogs. The bar takes no keyboard
// focus of its own; each button and the segmented control keep theirs, in
// reading order.
// ──────────────────────────────────────────────────────────────────────────────
Pane {
    id: bar

    property string title: ""
    property string subtitle: ""
    property alias leadingData: leadingSlot.data
    property alias trailingData: trailingSlot.data

    implicitHeight: CelestinaTheme.topBarHeight
    padding: 0
    leftPadding: CelestinaTheme.windowMargin
    rightPadding: CelestinaTheme.windowMargin
    activeFocusOnTab: false

    Accessible.role: Accessible.ToolBar
    Accessible.name: bar.title

    background: Rectangle {
        color: CelestinaTheme.canvas
    }

    contentItem: RowLayout {
        spacing: CelestinaTheme.spaceMd

        Row {
            id: leadingSlot
            objectName: "topBarLeading"
            Layout.alignment: Qt.AlignVCenter
            spacing: CelestinaTheme.spaceXs
        }

        Column {
            objectName: "topBarTitles"
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            spacing: 0

            Text {
                objectName: "topBarTitle"
                width: parent.width
                visible: bar.title.length > 0
                text: bar.title
                textFormat: Text.PlainText
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignLeft
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightDemiBold
                color: CelestinaTheme.text
            }

            Text {
                objectName: "topBarSubtitle"
                width: parent.width
                visible: bar.subtitle.length > 0
                text: bar.subtitle
                textFormat: Text.PlainText
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignLeft
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                color: CelestinaTheme.textMuted
            }
        }

        Row {
            id: trailingSlot
            objectName: "topBarTrailing"
            Layout.alignment: Qt.AlignVCenter
            spacing: CelestinaTheme.spaceXs
        }
    }
}
```

The test expects `firstAction.iconSize == iconMd`: `CelestinaIconButton` defaults to `iconSm`, so add to the bar, after `background`, a binding that the buttons read — simplest is to require it of consumers and assert it in the test through a consumer-set value. Change the test's trailing buttons to `iconSize: CelestinaTheme.iconMd` and keep the assertion; DESIGN.md §6.1 states the bar's buttons take `iconMd`. The style guard rule below enforces the button *type*, not the size.

Register: `CelestinaTopBar 1.0 CelestinaTopBar.qml` in `qmldir` after `CelestinaSegmentedControl`, and `        CelestinaTopBar.qml` in `CMakeLists.txt` after `CelestinaSegmentedControl.qml`.

- [ ] **Step 4: The guard rule for the trailing slot**

In `celestina-style/scripts/check-style-contract.sh`, after the literal-alpha `if literal_alpha_hits=$(python3 - … PY ); then … fi` block, add:

```bash
# A top bar's trailing slot carries glyphs, never words (DESIGN §6.1). This
# reads each `trailingData: [ ... ]` list as a whole and refuses a
# CelestinaButton inside it; CelestinaIconButton is the accepted child.
if trailing_hits=$(python3 - "${contract_files[@]}" <<'PY'
import re
import sys

SLOT = re.compile(r"\btrailingData\s*:\s*\[")
TEXT_BUTTON = re.compile(r"(?<![\w.])CelestinaButton\s*\{")
OPENERS = {"(": ")", "[": "]", "{": "}"}


def list_body(text, start):
    depth = 1
    quote = ""
    index = start
    while index < len(text):
        char = text[index]
        if quote:
            if char == "\\":
                index += 2
                continue
            if char == quote:
                quote = ""
        elif char in "\"'`":
            quote = char
        elif char in OPENERS:
            depth += 1
        elif char in ")]}":
            depth -= 1
            if depth == 0:
                return text[start:index]
        index += 1
    return text[start:]


status = 0
for path in sys.argv[1:]:
    try:
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
    except (OSError, UnicodeDecodeError) as error:
        print(f"{path}: {error}", file=sys.stderr)
        status = 2
        continue
    for slot in SLOT.finditer(text):
        body = list_body(text, slot.end())
        for hit in TEXT_BUTTON.finditer(body):
            line = text.count("\n", 0, slot.end() + hit.start()) + 1
            print(f"{path}:{line}:CelestinaButton inside a top bar's trailing slot")
sys.exit(status)
PY
); then
    if [[ -n $trailing_hits ]]; then
        printf '%s\n' "$trailing_hits"
        printf 'ERROR: a top bar carries icon-only actions; move the words to a dialog\n\n' >&2
        failures=1
    fi
else
    printf 'ERROR: could not complete the trailing-slot check.\n\n' >&2
    failures=1
fi
```

Add a negative fixture check to the unit's evidence by running the guard once against a scratch file placed under `celestina-style/` that contains `trailingData: [ CelestinaButton { text: "Guardar" } ]`, confirming the ERROR line, then deleting the file (the style guard has no fixture harness of its own; `test-architecture-scanners.sh` covers the root scanners only, and this rule lives in the style script by commit scope).

- [ ] **Step 5: Run the tests and guards until green**

```bash
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure && bash scripts/check-architecture-contract.sh
```

Expected: ctest 1/1; every guard OK.

- [ ] **Step 6: Commit**

```bash
git add celestina-style/CelestinaTopBar.qml celestina-style/tests/tst_topbar.qml celestina-style/qmldir celestina-style/CMakeLists.txt celestina-style/scripts/check-style-contract.sh
git commit -m "celestina-style-milestone: Publish CelestinaTopBar and keep its trailing slot icon-only"
```

### Task 15: Gallery page, contract text, evidence

**Files:**
- Modify: `celestina-style/gallery/Gallery.qml`, `celestina-style/DESIGN.md` (§6.1 table, §6.2 table), `celestina-style/STATUS.md`, `celestina-style/VALIDATION.md`, `celestina-style/docs/evidence/2026-09-30-top-bar-and-segments.md`

- [ ] **Step 1: Gallery section**

After the `LIST SECTION — THE SIGNATURE` section, add:

```qml
            Section {
                heading: "TOP BAR AND SEGMENTS — THE DESKTOP FRAME"
                Column {
                    width: sheet.width
                    spacing: CelestinaTheme.spaceMd

                    CelestinaTopBar {
                        width: parent.width
                        title: "Procesador"
                        subtitle: "AMD Ryzen 7 9800X3D 8-Core Processor"
                        leadingData: [
                            CelestinaSegmentedControl {
                                helpText: "Secciones"
                                currentIndex: 0
                                model: [
                                    { key: "performance", icon: "gauge", label: "Rendimiento" },
                                    { key: "processes", icon: "view-list", label: "Procesos" },
                                    { key: "sensors", icon: "cpu", label: "Sensores" }
                                ]
                                onActivated: function(index) { currentIndex = index }
                            }
                        ]
                        trailingData: [
                            CelestinaIconButton { iconName: "search"; helpText: "Buscar"; role: CelestinaButton.Ghost; iconSize: CelestinaTheme.iconMd },
                            CelestinaIconButton { iconName: "view-grid"; helpText: "Vista"; role: CelestinaButton.Ghost; iconSize: CelestinaTheme.iconMd },
                            CelestinaIconButton { iconName: "view-refresh"; helpText: "Actualizar"; role: CelestinaButton.Ghost; iconSize: CelestinaTheme.iconMd }
                        ]
                    }

                    CelestinaTopBar {
                        width: parent.width
                        title: "Descargas"
                        leadingData: [
                            CelestinaIconButton { iconName: "go-previous"; helpText: "Volver"; role: CelestinaButton.Ghost; iconSize: CelestinaTheme.iconMd }
                        ]
                        trailingData: [
                            CelestinaIconButton { iconName: "search"; helpText: "Buscar"; role: CelestinaButton.Ghost; iconSize: CelestinaTheme.iconMd }
                        ]
                    }

                    CelestinaTopBar {
                        width: parent.width
                        title: "Un título tan largo que la barra lo tiene que recortar por la derecha para que las acciones sigan cabiendo"
                        subtitle: "sin navegación a la izquierda"
                        trailingData: [
                            CelestinaIconButton { iconName: "x"; helpText: "Cerrar"; role: CelestinaButton.Ghost; iconSize: CelestinaTheme.iconMd }
                        ]
                    }

                    CelestinaSegmentedControl {
                        helpText: "Vistas"
                        iconOnly: true
                        model: [
                            { key: "grid", icon: "view-grid", label: "Cuadrícula" },
                            { key: "list", icon: "view-list", label: "Lista" },
                            { key: "details", icon: "view-details", label: "Detalles" }
                        ]
                        onActivated: function(index) { currentIndex = index }
                    }
                }
            }
```

Every icon name above exists in `icons/` (`gauge`, `cpu`, `search`, `view-grid`, `view-list`, `view-details`, `view-refresh`, `go-previous`, `x`); `list` must be checked with `ls celestina-style/icons | grep -x list.svg` and replaced by `view-list` if absent.

- [ ] **Step 2: DESIGN.md**

§6.1 add two rows after `ListSection`:

```markdown
| `CelestinaTopBar` | The fixed application bar: `topBarHeight` canvas strip spanning sidebar and content; `leadingData` (navigation), left-aligned eliding `title`/`subtitle` at `fontTitle`/`fontRowSecondary`, `trailingData` of `CelestinaIconButton`s at `iconMd`; `ToolBar` role, no focus of its own; a `CelestinaButton` in `trailingData` fails the style guard |
| `CelestinaSegmentedControl` | Peer destinations in one `radiusButton` plate on `card`, `compSegmentHeight` tall; the current segment wears `surfaceSelected` at `radiusButton − spaceXs`, concentric; icon plus label or `iconOnly`; one Tab stop, Left/Right/Home/End emit `activated(index)`, the host owns `currentIndex`; `PageTabList` of `PageTab`s named by their labels |
```

§6.2: delete the `TabPills` row and add nothing.

§1 direction, after "Reachability patterns do not transfer blindly…": append the sentence "Floating pills, centred page titles and loose circular actions are not desktop defaults either; a window wears one fixed top bar." to rule 1.

- [ ] **Step 3: STATUS.md** — add above the `STYLE-G7-O` item:

```markdown
- `STYLE-G7-P` publishes `CelestinaTopBar` and `CelestinaSegmentedControl`,
  the two types every application's fixed top bar needs; the style guard keeps
  a bar's trailing slot free of text buttons. See
  [the record](docs/evidence/2026-09-30-top-bar-and-segments.md).
```

- [ ] **Step 4: VALIDATION.md** — add `VAL-STYLE-08 — The bar reads as one frame`: requires the gallery on the real session; procedure: Tab through the three bars, resize the window under 700 px; pass: the title elides before any action leaves the bar, focus visits segments then actions in reading order, the selected segment's plate is concentric with the control at 1× and 2× scale.

- [ ] **Step 5: Evidence, guards, commit**

Fill the evidence record with the commands of Tasks 13-14 (ctest, architecture guard, the trailing-slot negative fixture run and its ERROR line, the offscreen gallery smoke). Then:

```bash
bash scripts/check-architecture-contract.sh && python3 scripts/check-language-contract.py && bash scripts/check-documentation-contract.sh && cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure && QT_QPA_PLATFORM=offscreen timeout 8 celestina-style/gallery/run.sh --offscreen; echo "gallery exit $?"
git add celestina-style/gallery/Gallery.qml celestina-style/DESIGN.md celestina-style/STATUS.md celestina-style/VALIDATION.md celestina-style/docs/evidence/2026-09-30-top-bar-and-segments.md
git commit -m "celestina-style-milestone: Show the top bar and the segments in the gallery and record the contract"
```

Ready to land: `python3 scripts/land-unit.py unit/celestina-style/STYLE-G7-P --kind milestone` after `AUD-1-I` (MINOR: 1.10.0 → 1.11.0).

---

## Self-review against the spec

- **§3.1 recipe:** capture 1.0 (T3), blurMax 24 + reference calibration (T3, T7), no desaturation (T3, T5), one tint (T3), Haze grain at 0.15 under the tint (T2, T5), edges removed with tests (T4, T5), shadow unchanged, fallback opaque canvas (T3, T5), API kept, `materialEdgesVisible` removed (T5). The spec's deletion list is amended: `glassHighlight` stays (the veil's tint, used by the shell), `glassSaturation` and `glassBlurMultiplier` stay as tokens with the Haze values because Magnetita's `MediaCard` and Fluorita's `AmbientLight` read them for their own blurs.
- **§3.2 ladder:** every row of the token table has a value in T3; `compTopBarButtonSize` is dropped — the bar's buttons are `CelestinaIconButton` at their `controlHeightXs` circle with `iconMd`, which the top bar test asserts through `iconSize`.
- **§3.3 guard:** rules, message format, ratchet, registry, wiring (T10, T11). The spec's "baseline shrinks only; refuses a stale row" is the `stale_baseline_must_fall` test. The spec's per-file baseline became per-project counts, the shape the repository's other ratchets use.
- **§3.4 components:** slots, alignment, elision, roles, focus, guard rule (T13, T14). `Accessible.role: ToolBar` and `PageTabList` as specified.
- **§3.5 gallery, tests, docs:** T7, T15; DESIGN §2, §5.1, §5.2, §5.3, §6.1, §6.2 (T8, T15); STATUS, ROADMAP, VALIDATION rows (T8, T15). Version bumps are the landing's.
- **Placeholders:** none; every step carries its code or its exact command. The one deliberately open value is `glassBlurMax`, which T7 calibrates and records.
- **Type consistency:** `cornerInset` (theme) ↔ `corner_inset` (guard); `spaceCardInset` in T3, T6, T10; `compSegmentHeight`, `topBarHeight`, `iconMd`, `iconLg` in T3, T13, T14; object names `segmentedPlate`, `segment-N`, `segmentPlate`, `segmentFill`, `segmentLabel`, `topBarLeading`, `topBarTitle`, `topBarSubtitle`, `topBarTrailing` match between components and tests; `glassFallback` in T3, T5, T7.
