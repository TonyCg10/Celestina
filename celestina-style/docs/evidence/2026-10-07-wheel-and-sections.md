# Evidence: 2026-10-07 free wheel scrolling and menu sections

- **Date:** 2026-10-07
- **Scope:** `STYLE-G7-Y` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the QuickTest binary runs offscreen

## Procedure

```sh
cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build celestina-style/build --parallel
QT_QPA_PLATFORM=offscreen ./celestina-style/build/celestina-style-modal-test
bash celestina-style/scripts/check-style-contract.sh
(cd siderita && cargo build --release)
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

`tst_wheelscroll.qml` scrolls a 600 px `ListView` of 2000 rows of 40 px
with `CelestinaWheelScroll { view: list }` inside an outer wheel catcher.
`tst_contextmenu.qml` adds a menu built like Fluorita's stream menu: two
`GlassMenuSection` headers, each followed by rows an `Instantiator` inserts
after it, a hidden header and a hidden row, opened with
`popupBeside(button, true)` from a button at the bottom of a 900x600 window
right after its model changes.

## Result

- **Exit:** before the change both files failed to compile (`GlassMenuSection`
  and `CelestinaWheelScroll` are not types). With the new types and the old
  `GlassContextMenu`, the stream menu's bottom sat at 576 against a button top
  of 554 (22 px over the button). The wheel cases run against Qt's own wheel
  handling (the type stubbed out) fail four of seven: a notch lands at 72 px,
  not 80; ten notches 15 ms apart reach 255 px, not 800. The same follower
  written as a `WheelHandler` failed only the bound case: no notch reached
  the outer catcher. After the change all 172 cases pass, five runs in a row;
  the style contract, the Siderita release build, the Siderita qmllint
  ratchet (239, unchanged), the architecture, language and documentation
  contracts exit 0.
- **Observed:** offscreen, three runs, against Qt's default on the same list:

  | | first frame | 90 % | settled | ten notches 15 ms apart |
  |---|---|---|---|---|
  | Qt default | 27 px | 103 ms | 72 px at ~305 ms | 253-257 px, settled ~300 ms after the last notch |
  | `CelestinaWheelScroll` | 44 px | 71-73 ms | 80 px at 145-153 ms | 800 px, 90 % 31 ms and settled 153-174 ms after the last notch |

  With variable row heights (the list's `originY` moved to -26 while rows
  loaded), thirty notches down and thirty up each landed exactly (3200 px,
  then 800 px): the "something else moved the view" guard did not misfire.
  At the top an upward notch and at the bottom a downward one reach the outer
  catcher and the list stays put; a direct `contentY` write mid-glide wins;
  Ctrl+wheel is not taken; with `reducedMotion` a notch jumps one step at once;
  `retarget()` re-clamps after the content shrinks. The stream menu's rows
  read, in content order and on screen, each header directly above its group,
  the hidden header and row take 0 px, and the settled menu's bottom clears
  the button by `spaceSm`.

### Fix round 1

- **Exit:** a hidden `GlassMenuItem` in an open three-row menu kept 38 px
  (`test_hidden_item_takes_no_room`: actual 38, expected 0). `GlassMenuItem`
  now collapses when hidden (`implicitHeight: visible ? controlHeight : 0`),
  because Qt's Menu ListView keeps an invisible item's height; the row after
  the hidden one then sits right under the visible one. All 173 cases pass,
  five runs in a row; the style contract, the Siderita release build, the
  Siderita qmllint ratchet (239, unchanged), the architecture, language and
  documentation contracts exit 0.
- **Observed:** the Ctrl+wheel case now also checks that the event reaches a
  Ctrl zoom `WheelHandler` declared inside the view (non-blocking, so the
  scroller's area sees the event too and must leave it alone); with the area
  set to accept Ctrl it fails (one step taken). Behind a ListView's content
  Qt delivered no Ctrl+wheel to anything in these probes, with or without the
  scroller, so the zoom cannot be checked further out. The scroller passes on
  a wheel when it has no `view`, stops its glide if the view goes away, and
  leaves a mostly horizontal touchpad swipe (|x| > |y| pixel delta) to
  whatever scrolls sideways; QtTest's `mouseWheel` sends no pixel deltas, so
  that last guard has no automated case.

## Limits

- `CelestinaWheelScroll` is a `MouseArea` placed in the view's `contentItem`
  over the visible part, not a `WheelHandler`: a `WheelHandler` decides
  whether the event goes on (`blocking`) before its `wheel` signal runs, and
  setting `accepted = false` in `onWheel` was measured to have no effect, so
  it could never pass a notch on at a bound.
- `CelestinaUsageList` is not wired yet: Siderita and Hematita compile the
  shared file into their own modules, which do not list
  `CelestinaWheelScroll.qml`, so using it there would fail to load at run
  time (qmllint resolves the symlink to this directory and cannot see it).
- Timings are offscreen frames (~16-20 ms); the feel is judged in the
  applications.

## Follow-up

Each application adopts `CelestinaWheelScroll` and `GlassMenuSection` in its
own unit; the unit that registers `CelestinaWheelScroll.qml` in Siderita and
Hematita attaches it to `CelestinaUsageList`.

## Landing

- **Base revision:** `00ca996edf033f72d780588fb68d1d90233c0fe1`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d60378652892405210c7e73e98cff50a9d5a3e6538e4f6db647001ddc9db016f, verification_fingerprint sha256:6c2697c30e563dd67490168b0e142095be986e3cd90a86e7db3df34b7e38c2aa
