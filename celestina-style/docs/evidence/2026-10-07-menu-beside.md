# Evidence: 2026-10-07 a menu opens beside its button

- **Date:** 2026-10-07
- **Scope:** `STYLE-G7-X` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri with the blur rule for the org.celestina applications
- **Artifact:** not applicable in the session; the QuickTest binary runs offscreen

## Procedure

```sh
cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
cmake --build celestina-style/build --parallel
QT_QPA_PLATFORM=offscreen ./celestina-style/build/celestina-style-modal-test
```

`tst_contextmenu.qml` opens a fresh `GlassContextMenu` (three items) in a
480x320 window from a button at the top-left and one at the bottom-right.

## Result

- **Exit:** before the change four cases failed (`popupBeside` undefined);
  after it all 162 cases pass.
- **Observed:** below the top button and above the bottom one, on the very
  first open, with `spaceSm` between menu and button, no overlap, and the menu
  inside the window horizontally. The room is measured in the overlay (the
  window), with `margins` kept clear at its edges, so a menu declared inside a
  small bar or card places correctly; the chosen point is mapped back into the
  menu's parent because `popup(x, y)` is parent-relative. Added cases: a menu
  and button inside a small offset bar at the window bottom (above, both
  preferences), a mid-window nested button (below), a menu wider than its
  parent, and a Repeater whose model changes right before the open.

## Limits

- The test covers the placement arithmetic offscreen; no application calls
  `popupBeside` yet.

## Follow-up

Magnetita, Fluorita and Siderita adopt `popupBeside` in their own units.

## Landing

- **Base revision:** `b5ad309990e9da0e81bd58af77746089adeb68e6`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:41e10114cfeb99ceb39de701da895f64dbba295ae76247b9619556d0c83a3574, verification_fingerprint sha256:f256d6f02bb7785c574fca4fbf6abe1e38b41c27872d2cb8faa39fb4826af703
