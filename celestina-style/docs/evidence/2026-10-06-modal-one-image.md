# Evidence: 2026-10-06 the modal fades as one image, and takes its own focus

- **Date:** 2026-10-06
- **Scope:** `STYLE-G7-U` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys every consumer

## Procedure

After `STYLE-G7-T` the author still saw a modal closing in parts: the scrim
above and below the card went on one clock and the card's centre, the glass
capture, on another, and the whole thing read as a cut rather than a fade.
On open, a control inside lit its focus ring and the ring then hopped to the
close button.

The modal layer now renders through a `layer` as a whole, scrim and dialog,
for as long as its opacity is strictly between 0 and 1, so the fade thins one
image; the content host's own layer is gone. The fade runs over
`motionNormal`. On show the layer itself takes the focus with
`PopupFocusReason`; no control is entered, Tab travel starts from the layer
and reaches the first control, Escape still dismisses.

```sh
cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release
cmake --build celestina-style/build --parallel --target celestina-style-modal-test
QT_QPA_PLATFORM=offscreen celestina-style/build/celestina-style-modal-test
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** modal test 150 passed, 0 failed; every guard OK.
- **Observed:** `test_takes_focus_itself_without_entering_a_control` holds
  the layer's focus on show and reaches the first field on Tab; the Tab and
  Backtab cycle, Escape, the pointer blocking and the exit-fade blocking pass
  unchanged.

## Limits

- The look of the fade is the author's judgement on the session.

## Follow-up

None.

## Landing

- **Base revision:** `99b9ddd65170d00d3df3ade003067fe805480025`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:465af51fb3aad95621e1556a994ec28b3b48adefafa23aa8d822c6e37a2e7591, verification_fingerprint sha256:fcc7bf5b0b99827a36e63c080d81629f669fc4b9e04528b5c1249a116140d812
