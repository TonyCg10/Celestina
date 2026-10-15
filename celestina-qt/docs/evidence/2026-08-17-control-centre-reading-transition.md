# Control-centre reading transition

- **Date:** 2026-08-17
- **Scope:** Celestina unit `R8-P-Q` control-centre follow-up
- **Environment:** the author's 1920x1080, 60 fps recording; Qt 6.11.1;
  focused QML and registered production verification
- **Artifact:** Celestina 0.29.12 delivery batch, built, verified and deployed
  to the normal test prefix without activating the main session
- **Unit:** `R8-P-Q` control-centre follow-up
- **Recording:** `recording_20260816_235057.mp4`, 1920 x 1080 at 60 fps
- **Defect:** provider-owned option text was replaced in one compositor frame
  while the adjacent switch continued its own transition.

## Procedure

Two independent changes reproduce the same cut. In the caffeine row the
reading is wholly `apagado` in the frame at approximately 1.837 s and wholly
`encendido` in the next frame at approximately 1.853 s. In the night-light row
the equivalent `apagada` to `encendida` replacement occurs between the frames
at approximately 7.317 s and 7.333 s. Neither sequence contains an intermediate
text frame, although the switch thumb continues moving afterwards.

`ControlRow` owned one `Text` item whose `text` binding read the latest provider
value. A provider snapshot therefore replaced every glyph atomically. The row
now owns a local two-layer reading transition: the last reported value fades
out while the new provider value fades in over `motionFast`. A further snapshot
is queued until that transition finishes, and reduced motion settles the same
authoritative value with zero duration.

## Result

- `tst_controlcentre.qml`: `7 passed, 0 failed`; proves a reported option keeps
  the outgoing reading during the fade, settles the new reading, and removes
  the duration under reduced motion.
- `cmake --build celestina/build --target celestina celestina_qmllint -j2`:
  passed; qmllint reports only the three pre-existing diagnostics in
  `BrightnessLevel.qml`, `CalendarMenu.qml`, and `SoftCard.qml`.
- `bash scripts/check-architecture-contract.sh`: passed.
- `ctest --test-dir celestina/build -R
  '^(celestina-output-chooser|celestina-indicator-menu|celestina-overlay-contract)$'
  --output-on-failure`: 3 of 3 passed.
- `celestina/scripts/verify-production.sh`: passed outside the restricted
  sandbox against the already-built production manifest; all 23 CTest targets,
  the Rust suites, qmllint, guards, and both production smokes passed.

## Limits

`VAL-PANEL-1` must repeat the caffeine, power-profile, and night-light changes
in the real nested Wayland session and confirm that the cross-fade reads as one
continuous option change at the output refresh rate.

A later authorized `celestina/scripts/complete-production.sh` run stopped the
development host first, passed the complete registered verification, and
deployed the current Celestina 0.29.12 bundle to `~/.local` without activating
the main session.
