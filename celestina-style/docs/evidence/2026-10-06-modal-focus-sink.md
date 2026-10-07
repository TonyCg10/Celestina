# Evidence: 2026-10-06 the modal parks its focus

- **Date:** 2026-10-06
- **Scope:** `STYLE-G7-W` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys every consumer

## Procedure

After `STYLE-G7-U` the author still saw, on every open of the properties
dialog, a highlight on a row of the usage section that hopped at once to the
close button. Reproduced in the modal test with a `CelestinaUsageList`
whose rows arrive after the open: giving the layer scope the focus handed it
on to the list's cursor row, and the second batch of rows destroyed that
delegate, so `keepFocusInside` saw no owned focus item and wrapped to the
last control with a keyboard reason, which is what lights a ring.

The layer now parks the focus on a plain sink item (`parkFocus`) on show,
when the focused item vanishes (no active focus item at all), and when
there is nothing focusable; a wrap with a keyboard reason is kept only for
real Tab travel that left the layer. Tab from the sink reaches the first
control.

```sh
cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release
cmake --build celestina-style/build --parallel --target celestina-style-modal-test
QT_QPA_PLATFORM=offscreen celestina-style/build/celestina-style-modal-test
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** modal test 152 passed, 0 failed; every guard OK.
- **Observed:** `test_rows_arriving_after_the_open_do_not_move_the_focus`
  keeps the focus on the layer with no ring on the close button through two
  row batches; the Tab cycle now includes the list and still stays inside.

## Limits

- The look on the real dialog is the author's judgement on the session.

## Follow-up

None.

## Landing

- **Base revision:** `f53bebaa76c8b1c7d25f744a0afcbabc24fc4345`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ac43f8a613923423eb865726608098755da4bf7a6e1c13d3b3a5464fc1296ffc, verification_fingerprint sha256:b0f7eb04650c257f5fcba29a6f9832c6b747dee108a567e606009f52befcddc8
