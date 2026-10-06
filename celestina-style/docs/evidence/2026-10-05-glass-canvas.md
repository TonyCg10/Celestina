# Evidence: 2026-10-05 the glass canvas

- **Date:** 2026-10-05
- **Scope:** `STYLE-G7-Q` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with `background-effect` blur on the author's patched build, offscreen QPA for tests
- **Artifact:** not applicable in the session; the landing builds and verifies the registered module

## Procedure

The author asked, on the real session, for the black window canvas to carry the
same blur and transparency Niri gives an unfocused window, with the Haze
material of the repository; then for the boxes to take the bar's black without
transparency; then reported a focus ring on the properties dialog's crumbs, the
wheel blocked under an open context menu, and the sort pill without glass. Each
step was built as a spike on Siderita, relaunched on the session and judged by
the author by eye; the accepted values are the ones here.

```sh
cmake -S celestina-style -B celestina-style/build -DBUILD_TESTING=ON
cmake --build celestina-style/build --parallel
ctest --test-dir celestina-style/build --output-on-failure
bash scripts/check-architecture-contract.sh
QT_QPA_PLATFORM=offscreen timeout 8 celestina-style/gallery/run.sh --offscreen
```

## Result

- **Exit:** ctest `100% tests passed out of 1`; `Sealed colour contract: OK`,
  `Contrast contract: OK`, `QML visual contract: OK`, `Radius contract: OK`,
  `Architecture contract: OK`; the gallery constructs offscreen with no QML
  warning.
- **Observed:** `CelestinaBackdrop` paints `glassTint` with the grain at
  `glassCanvasNoiseOpacity` 0.05 (the author rejected 0.15 on a window-sized
  pane as visible texture). `card` is `#0b0c10`, the black the Haze-tinted
  location bar renders over the glass canvas, measured on the session; the
  author asked for the boxes opaque. `controlFill` is the Haze tint
  `#b3050608`, so pills over their own capture match the bar. The modal's
  opening focus placement uses `Qt.PopupFocusReason` in `focusInside`; the Tab
  wrap keeps its keyboard reasons. `GlassContextMenu` is `modal: false` with a
  `MouseArea` shield created in the overlay on open: presses close the menu and
  never reach the content, wheel events are left unaccepted and scroll the
  view.

## Limits

- The compositor blur is real only on the session; offscreen proves
  construction. The consumer side (a transparent window and the Niri rule
  `background-effect { blur true }` for `org.celestina.*`) is each
  application's and the author's configuration; Siderita lands it in
  `SID-H1-F`, the other applications in their design units.
- The grain and tint values are the author's judgement on this wallpaper;
  `VAL-STYLE-07` remains the perceptual check against the phone.
- The shield does not block hover; whether a hovered row under an open menu
  should stay lit is author validation.

## Follow-up

`VAL-STYLE-07` in [VALIDATION.md](../../VALIDATION.md).

## Landing

- **Base revision:** `12319f617cd80e12be8811e0d7e5f26cafcdb303`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:f7808066dabfd3dec495c6123aa6eb195666a9ed38e71991e35ddbfb0b696c5b, verification_fingerprint sha256:059e299969c40e316efbd1dedd2d5d68576c2062b1ab003d049ac3c63c504582
