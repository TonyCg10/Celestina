# Evidence: 2026-10-11 wheel notches that carry pixel deltas

- **Date:** 2026-10-11
- **Scope:** `STYLE-G7-AD` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.12.0; the QuickTest binary runs offscreen
- **Artifact:** not applicable in the session; the landing builds and verifies the registered module and its consumers

## Procedure

The author still felt a brake on the wheel in every application. A trace of
`CelestinaWheelScroll.onWheel` in Siderita on the author's session
(2026-10-11, 779 events) showed what Qt 6.12 delivers for a mouse wheel:

- one notch arrives with `angleDelta.y` = ±120 **and** `pixelDelta.y` = ±15;
  two notches folded into one event carry ±240 and ±30;
- `phase` is 0 (`Qt.NoScrollPhase`) on every notch;
- `event.device.type` is 4 (`PointerDevice.TouchPad`) and its name is
  "touchpad" even for the mouse, so the device cannot tell them apart.

The scroller decided `pixel = |pixelDelta.y| >= 0.01`, so every notch took
the touchpad path: 15 px applied straight, no glide, instead of
`angleDelta.y / 120 * compWheelStep` (two rows, 80 px) followed by the
glide. Each notch moved less than a fifth of what it should, which is the
brake.

The rule is now: pixels are applied straight only for a real touchpad
stream, that is an event with a scroll phase (a touchpad on Wayland sends
`ScrollBegin`, `ScrollUpdate`, `ScrollEnd` and `ScrollMomentum`) or one that
carries pixels and no angle at all. Everything else is a notch and moves
`angleDelta.y / 120` steps with the glide, so a high-resolution wheel's
fraction of a notch moves the same fraction of a step. The sideways pass-on
compares the angles for a notch and the pixels for a touchpad stream. The
whole decision moved from `onWheel` into
`take(angleX, angleY, pixelX, pixelY, phase, modifiers)`, which returns
whether the event was used; `onWheel` calls it with the event's fields.
Bounds, Ctrl+wheel, reduced motion, `retarget()` and `stepped(delta, notch)`
are unchanged.

`mouseWheel` in QtTest cannot set a pixel delta or a phase, so the new cases
in `tests/tst_wheelscroll.qml` call `take` directly:

- `test_i`: a notch with pixel 15 and no phase does not move at once and
  glides to exactly one `compWheelStep`; `stepped(-step, true)`.
- `test_j`: ten such notches 15 ms apart add up to ten steps.
- `test_k`: a phased stream (`Qt.ScrollUpdate`) applies its 15 px straight,
  `stepped(-15, false)`.
- `test_l`: pixels with no angle and no phase apply straight.
- `test_m`: a half notch (angle 60, pixel 7.5, no phase) glides to half a
  step.
- `test_n`: on a view that scrolls sideways, a tilt notch and a mostly
  sideways phased swipe are passed on, and a notch whose angles are mostly
  vertical is taken even though its pixels are mostly sideways.
- `test_o`, `test_p`: Ctrl, the top bound and reduced motion behave as
  before through `take`.

```sh
cmake -S celestina-style -B <scratch>/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build <scratch>/build --parallel
QT_QPA_PLATFORM=offscreen <scratch>/build/celestina-style-modal-test   # three runs
cmake --build <scratch>/build --target all_qmllint
sh celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
bash scripts/qmllint-cxxqt.sh siderita    # and calcita, grafita, fluorita, magnetita, selenita, cuprita
```

The failing-first run copied the style into a scratch `CelestinaStyle/`
directory and ran `qmltestrunner -import <scratch> -input
celestina-style/tests/tst_wheelscroll.qml`, once against the old decision
wrapped as `take` in the scratch copy only.

## Result

- **Exit:** before the change, with the old decision behind `take`, 13 passed
  and 5 failed: `test_i` (the notch jumped 15 px), `test_j` (ten notches
  reached 150 px, expected 800), `test_m` (the half notch jumped 7.5 px),
  `test_n` (the mostly vertical notch was passed on) and `test_p` (15 px
  instead of 80). After it, `tst_wheelscroll.qml` passes 18 of 18 and the
  whole QuickTest binary 191 of 191, three runs in a row; `all_qmllint`
  exit 0; style, architecture, language and documentation contracts exit 0;
  the qmllint ratchets are unchanged (Siderita 222, Calcita 1, Grafita 38,
  Fluorita 56, Magnetita 38, Selenita 1, Cuprita 9).
- **Observed:** `CelestinaWheelScroll.qml` lints clean against the scratch
  build's module.

## Limits

- The trace numbers come from the author's session; the session that made
  the change ran no window on it and turned no wheel.
- The ratchets were measured against the release modules already in the
  shared Cargo target, not rebuilt for this branch; the change touches no
  qmldir or type, only a function body. Hematita has no release module in
  the shared target and was not measured.
- Whether the brake is gone is the author's check, `VAL-STYLE-09`.

## Landing

- **Base revision:** `1f6d3725cf7209810e5e8d2a51a4a1639d95c918`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:bb2e41530cf20c29aabc71f3e71565f89f3233dc83ff45f8334492bd02b697a3, verification_fingerprint sha256:e398fc45e25928598f00485a52c779c135f2deca4cd09a276dfad6bf414edf5c
