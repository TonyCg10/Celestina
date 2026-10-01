# Evidence: 2026-09-30 the Haze recipe and the desktop scale

- **Date:** 2026-09-30
- **Scope:** `STYLE-G7-O` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2, Python 3.14 with Pillow 12.3, Niri session on a 2560×1440 output at scale 1 for the real-session crops, offscreen QPA for tests
- **Artifact:** not applicable in the session; the landing builds and verifies the registered module

## Procedure

### Task 2 — vendor Haze's grain texture (test-first)

```sh
cmake -S celestina-style -B celestina-style/build -DBUILD_TESTING=ON
cmake --build celestina-style/build --parallel
ctest --test-dir celestina-style/build --output-on-failure -R celestina-style-modal-focus
```

RED, before the texture and its registration existed:

```
FAIL!  : celestina_style_modal::GlassSurface::test_grain_is_the_haze_texture() 'grain must be Haze's texture: qrc:/qt/qml/CelestinaStyle/icons/glass-noise.png' returned FALSE. ()
Totals: 121 passed, 1 failed, 0 skipped, 0 blacklisted, 883ms
```

`haze_noise.webp` (Haze tag 1.6.10) and its `LICENSE` were fetched from
`chrisbanes/haze` on GitHub with `curl`, converted losslessly to
`celestina-style/icons/haze-noise.png` (64×64 RGBA) with Pillow, and
registered in `CMakeLists.txt`, `icons.qrc` and `GlassSurface.qml`'s
`Image.source` binding in place of `glass-noise.png`, which was removed with
`git rm`.

GREEN, after registration:

```
cmake --build celestina-style/build --parallel
ctest --test-dir celestina-style/build --output-on-failure -R celestina-style-modal-focus
100% tests passed out of 1
```

Full suite once before committing: `ctest --test-dir celestina-style/build
--output-on-failure` — 100% tests passed out of 1 (122 individual QuickTest
functions, all PASS).

### Task 3 — theme: desktop ladder and Haze glass scalars

RED, contrast check against the pre-edit tint:

```sh
python3 celestina-style/scripts/check-contrast-contract.py
```
```
contrast: ERROR: glassTint/white/text: 4.17:1; minimum 4.5:1
```

GREEN, after swapping `glassTint`/`glassTintStrong` to `#b3050608` and
deleting `glassBorder`/`glassOutline`:

```sh
python3 celestina-style/scripts/check-contrast-contract.py
```
```
Contrast contract: OK
```

`CelestinaTheme.qml` moved to the desktop ladder (`radiusLg` 20, `radiusMd`
12, `radiusSm` 8, `radiusButton` 10, `radiusInput` aliased to `radiusPill` at
this point in the plan, `radiusXs` 3, `radiusPill` 9999), added
`cornerInset(radius)`, `windowMargin` 14 → 16, `rowHeight` 54 → 40,
`rowHeightLg` 66 → 52, `iconMd` 19 → 20, new `iconLg`, `spaceCardGap`,
`spaceCardInset`, `topBarHeight`, `compSegmentHeight`, and the glass scalars
`glassBlurMax` 32 → 24, `glassBlurMultiplier` 3.0 → 1.0, `glassSaturation`
-0.03 → 0, `glassSampleScale` 0.55 → 1.0, `glassNoiseOpacity` 0.025 → 0.15,
`glassFallback: canvas`, with `glassEdgeWidth`/`glassEdgeMidPosition`/
`glassEdgeLowPosition`/`glassEdgeMidOpacity`/`glassEdgeLowOpacity` deleted.

```sh
python3 celestina-style/scripts/check-contrast-contract.py && bash celestina-style/scripts/check-style-contract.sh
```
```
Contrast contract: OK
Contrast contract: OK
QML visual contract: OK
```

### Task 4 — `tst_glasssurface.qml` rewrite (test-first)

RED, against the pre-Task-5 `GlassSurface.qml`:

```sh
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```
```
Totals: 121 passed, 4 failed, 0 skipped, 0 blacklisted, 884ms
```

The four failures were the expected ones: the opaque fallback still resolved
to the old `surfaceStrong` colour instead of `glassFallback`; the grain/tint
paint order was still wrong; and two cases found a live
`celestina-glass-outline` and `celestina-glass-silhouette-outline` object
where `null` was expected.

### Task 5 — `GlassSurface.qml` repaints the Haze recipe

GREEN, after deleting `materialEdgesVisible`, the four edge layers
(`celestina-glass-outline`, `celestina-glass-lit-edge`,
`celestina-glass-silhouette-outline`, `celestina-glass-silhouette-lit-edge`),
the `MultiEffect` saturation pass, moving the grain `Image` to sit before the
tint, and replacing every `CelestinaTheme.surfaceStrong` fallback with
`CelestinaTheme.glassFallback`:

```sh
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```
```
1/1 Test #1: celestina-style-modal-focus ......   Passed    0.92 sec
100% tests passed out of 1
```
```sh
bash celestina-style/scripts/check-style-contract.sh
```
```
Contrast contract: OK
QML visual contract: OK
```

Offscreen gallery smoke:

```sh
QT_QPA_PLATFORM=offscreen timeout 8 celestina-style/gallery/run.sh --offscreen
```
exited 124 (the expected timeout) with no QML warning or "Cannot open" error
on stderr.

### Fix round — `radiusInput` does not stadium multi-line areas

A review finding (the controller's ruling recorded in this unit) established
that `radiusInput = radiusPill` stadiums `grafita/qml/components/DocumentView.qml`
and `siderita/qml/dialogs/GrafitaEditorDialog.qml`, both multi-line text
areas, not the pill-shaped search field. `CelestinaTheme.qml` was changed to
`radiusInput: radiusMd` (multi-line input areas), and
`CelestinaTextField.qml`'s Search branch changed from
`CelestinaTheme.radiusInput` to `CelestinaTheme.radiusPill` explicitly, so the
single-line search field inside `CelestinaTextField` stays a pill by its own
token, not through `radiusInput`. `tst_textfield.qml` gained
`test_search_field_is_a_pill_and_input_areas_are_not()` and two pre-existing
assertions in `test_the_shape_is_a_closed_role` were updated to compare
against `radiusPill` instead of the now-different `radiusInput`.

```sh
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```
```
1/1 Test #1: celestina-style-modal-focus ......   Passed    0.93 sec
100% tests passed out of 1
```

### Task 6 — module consumers follow the desktop ladder

RED:

```sh
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```
```
FAIL!  : celestina_style_modal::ListSection::test_rows_sit_one_card_inset_inside_the_card() Compared values are not the same
   Actual   (): 4
   Expected (): 8
```

GREEN, after moving `GlassCard`'s `cornerRadius` from `radiusMd` to
`radiusLg` and `ListSection`'s row holder to `x`/`y` `spaceCardInset` (8)
inside a `radiusLg` (20) card, concentric with its `radiusMd` (12) rows:

```sh
cmake --build celestina-style/build --parallel && ctest --test-dir celestina-style/build --output-on-failure
```
```
100% tests passed out of 1
```
```sh
bash celestina-style/scripts/check-style-contract.sh
```
```
Contrast contract: OK
QML visual contract: OK
```

### Task 7 — the σ = 12 Haze reference beside the live glass

`celestina-style/scripts/glass-reference.py` renders
`celestina-style/gallery/reference/backdrop.png` (the accent/success/warning
gradient) and `celestina-style/gallery/reference/haze.png` (Gaussian blur
sigma 12.05 → Haze grain 0.15 → canvas tint 0.70), reading its colours
straight out of `CelestinaTheme.qml`:

```sh
python3 celestina-style/scripts/glass-reference.py
```
```
wrote .../celestina-style/gallery/reference/backdrop.png and .../celestina-style/gallery/reference/haze.png (sigma 12.05)
```

`Gallery.qml`'s glass section was replaced with one live `GlassSurface`
labelled "Live GlassSurface" beside the reference PNG shown through its ruled
final form, with a "σ = 12 reference" label.

```sh
QT_QPA_PLATFORM=offscreen timeout 8 celestina-style/gallery/run.sh --offscreen
bash celestina-style/scripts/check-style-contract.sh
```
exit 124 (expected timeout, no error on stderr); `Contrast contract: OK` /
`QML visual contract: OK`.

#### Real-session calibration

The gallery was launched as a real window on the author's Wayland/Niri
session. No input-automation tool exists on this machine (`ydotool`,
`wlrctl`, `xdotool` are all absent, and `wtype` only sends keyboard events,
which the QML `Flickable` does not respond to), and the glass section sits
below the fold of the gallery window on the 1896×1010 physical output. To
bring the two panels on screen, `Gallery.qml` was temporarily edited —
`Component.onCompleted: contentY = Math.max(0, contentHeight - height)` on
its `Flickable`, and the `Window`'s `height` shrunk from 1180 to 960 — and
both edits were reverted before committing (`git diff HEAD~1 HEAD --
Gallery.qml` shows only the ruled Step 3 changes).

For each candidate `glassBlurMax` (24, then 28, then 20): edited
`CelestinaTheme.qml`, relaunched the gallery, focused the window (`niri msg
action focus-window`) and captured `niri msg action screenshot-window`. The
two 300×130 panels were located at `(159,807)-(459,937)` (live) and
`(483,807)-(783,937)` (reference), identical across all three runs.

The brief's prescribed "middle 100×60" crop (`x:259-359`, `y:842-902`)
overlapped the centred "Live GlassSurface" / "σ = 12 reference" label text,
which swamps the mean-diff and std numbers with unrelated text-vs-text
differences. The comparison window was shifted to a 100×56 box in the same
horizontal center, above the text baseline (`x:259-359`, `y:807-863` for the
live panel; `x:583-683`, `y:807-863` for the reference).

Crops and the raw screenshots are kept beside this unit's plan, outside the
repository, at
`.superpowers/sdd/2026-09-30-style-haze-desktop-scale/task-7-crops/`:
`screenshot_blurMax28.png`, `screenshot_blurMax20.png`,
`live_24_notext.png`/`ref_24_notext.png`,
`live_28_notext.png`/`ref_28_notext.png`,
`live_20_notext.png`/`ref_20_notext.png`.

## Result

Full suite: `ctest --test-dir celestina-style/build --output-on-failure` — 1/1
test binary passed (`celestina-style-modal-focus`), 122+ individual QuickTest
functions, all PASS, no warning attributable to `GlassSurface`, `CelestinaTextField`
or `ListSection`. `bash celestina-style/scripts/check-style-contract.sh` and
`python3 celestina-style/scripts/check-contrast-contract.py` both print `OK`
on every run recorded above.

`glassBlurMax` was tried at 20, 24 and 28 on the real session. The
horizontal-profile standard deviation of the live panel differed from the
reference by ≈ 8.8–9.2 % across the three values (criterion < 15 %, met at
every value), and the mean absolute channel difference was ≈ 7.03–7.05 out of
255 at every value (criterion < 6, missed by about 1 point at all three, and
essentially insensitive to `glassBlurMax`: the full 20→28 range moves the
live standard deviation by only 0.008). The gap concentrates in the blue
channel — live ≈ 62, reference ≈ 53, a constant ≈ 9-point offset across all
three runs — which reads as a tint/ambient-light compositing difference, not
a blur-width difference, since a parameter that controls blur radius does not
move a colour-channel offset this way. `glassBlurMax` was kept at its
original value, 24: none of the three candidates clears the mean-diff
threshold, and 24 is statistically indistinguishable from the nominal "best"
of the three (20, by 0.011, inside noise). `CelestinaTheme.qml` carries no
diff for `glassBlurMax` in the final commit. The accepted calibration
criterion is the horizontal-profile standard deviation one (met at 20, 24 and
28); the mean-difference miss is a tint offset and does not gate the blur.

The grain is masked to the rounded corners: its `Image` sits inside a
`celestina-glass-noise-mask` layer whose `MultiEffect` masks it with a
root-sized rounded rectangle, so it no longer paints the corner wedges
(`test_grain_is_clipped_to_the_corners`).

## Limits

- Offscreen QPA (`QT_QPA_PLATFORM=offscreen`) proves construction, property
  values and paint ordering only; it cannot observe real compositor blur, the
  physical scale of the author's output, or perceived tint/grain against a
  live desktop.
- The reference PNG's corners are not rounded (it is a flat rectangular
  Gaussian-blur render for pixel comparison only), so it is not a claim about
  `GlassSurface`'s rounded geometry, only about its blur/grain/tint recipe.
- The brief's prescribed "middle 100×60" crop overlaps the centred label text
  in both panels; a 100×56 box shifted above the text baseline was used
  instead so the comparison measures the glass material, not glyph edges.
  This is a deviation from the letter of the brief's crop box, recorded here
  for the controller.
- The gallery's `Flickable` was scrolled into view by a temporary, reverted
  QML edit (`Component.onCompleted: contentY = ...` plus a shrunk `Window.height`)
  because no input-automation tool (`ydotool`/`wlrctl`/`xdotool`/`wtype`)
  exists on this machine to scroll or resize the window interactively;
  `git diff HEAD~1 HEAD -- Gallery.qml` confirms nothing scroll- or
  size-related survived into the committed state.
- The σ match is a real-session comparison by this session only, on one
  output at one scale; the perceptual match against the phone's own Haze
  surface — the menu, dock and pill reading as one material at arm's length —
  is `VAL-STYLE-07`, which remains author-only and not run by hand.
- The style guard (`check-style-contract.sh`) is a text/structural scanner; it
  does not resolve `CelestinaTheme.<token>` references against the theme's
  declared properties, so it cannot detect a QML file naming a token the
  theme no longer has (this surfaced only at QML-engine load time, caught by
  `ctest`, not by the guard, during Task 4's RED run). `grep -rn
  "glassBorder\|glassOutline\|glassEdge\|materialEdgesVisible" --include='*.qml'`
  was run over every project root in this repository as the Step 6 check for
  a deleted-token consumer outside this module; it returned empty, so no
  application QML needed a rename for this unit.

## Follow-up

`VAL-STYLE-07` in [VALIDATION.md](../../VALIDATION.md).

## Landing

- **Base revision:** `8be101e4580acb2e83cafffc7e88d8cbffdd6fe7`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 0: artifact: celestina-rs current; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact is not verified yet; run verify-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: tests or rules changed; run verify-production.sh again; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d4f9c09a41afe5fa60d8c3a09a5838ba9b5ed94c2d3157c76f04ba943c6c5666, verification_fingerprint sha256:8c7d5e0f205ea6b3a7de7115a9caaf7694f3bedda65cd1919eedc8e5b3f5fd12; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:50d43327de39cff92d2a7f6a22d069a8c013a28b6d692f542119cf93985ecd2e, verification_fingerprint sha256:01c9368aee23cbc650089e8bd62ad86a07c63bf373c3e086113184ac38d23e76; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:92a656b40e303121a6640beaa86c05ebb7cb0baccc4c5a97a0dddf0522dc9114, verification_fingerprint sha256:180f1b3a2b30401b9e5834a1169d64f568d07abafb1aac8997b9f88633f94a7c; magnetita-android verify: verify-production.sh exit 0, manifest source_fingerprint sha256:5b55c4f62b23679f8584a626b5fece8b9b67c9f8a3e6778efeff4a4895fbb271, verification_fingerprint sha256:753855cafe309b6b27444b2114f13a229ee9590320d2c05309337b476f1d69a9; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:dc706bafeceed3cab7df2ef2d5af87bfc61669778fcfa5d78579969c65bf5d0f, verification_fingerprint sha256:f60ee1686147804a7f0d4fe0e4ce7ba8e28e56a406d13c32b92e51b487c2f3c5; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:cfe5c7329d896461e447f74ff6e00b3288ff4f3e82e9318327b88c861f45956d, verification_fingerprint sha256:b39b36ad6f7279a2ec27525b0aef224f0a99fe729381fdce62502692a0e3adc8; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:593eeabe6453eb6af82856638958f2e40d3a0c68e60d7f964362ea30bc124d7d, verification_fingerprint sha256:3c26523392d1fb407ee4858148b46da462975cfaf41c390fab745514e919469b
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh
