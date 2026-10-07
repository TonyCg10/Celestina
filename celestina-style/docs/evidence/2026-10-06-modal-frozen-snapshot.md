# Evidence: 2026-10-06 the modal fades its last frame

- **Date:** 2026-10-06
- **Scope:** `STYLE-G7-V` — `celestina-style`
- **Environment:** the author's CachyOS, Qt 6.11.2
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys every consumer

## Procedure

The author's recording after `STYLE-G7-U`, read frame by frame at 60 fps:
on the first frame of the close the scrim was already gone, the properties
card had lost its usage section and shrunk (the controller clears its fields
and the usage hub closes on `shown` dropping), and over the next twelve
frames the card's tint thinned while its text stayed readable. Fading live
items cannot compose first; a layer toggled on at the start of the fade did
not help because the content itself changed under it.

The layer's stage — scrim, content host and input shield — now renders only
through a `ShaderEffectSource` with `hideSource`. It is `live` while `shown`
and freezes the moment `shown` drops, so what fades is the last complete
picture of the dialog. Input still reaches the stage (the pointer-blocking
and drag-claim tests pass unchanged). `snapshotLive` is a read-only alias
for the test.

```sh
cmake -S celestina-style -B celestina-style/build -DCMAKE_BUILD_TYPE=Release
cmake --build celestina-style/build --parallel --target celestina-style-modal-test
QT_QPA_PLATFORM=offscreen celestina-style/build/celestina-style-modal-test
bash celestina-style/scripts/check-style-contract.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** modal test 151 passed, 0 failed; every guard OK.
- **Observed:** `test_exit_fade_is_a_frozen_snapshot` sees the snapshot
  live while shown and frozen from the first instant of the close; the
  focus, Escape, pointer-blocking and exit-fade-blocking tests pass unchanged.

## Limits

- The look of the fade is the author's judgement on the session; the test
  proves the freeze, not the picture.

## Follow-up

None.

## Landing

- **Base revision:** `bb21e966d4053e28bc2866fabb2951fd4bf213f1`
- **Check:** `production_artifact.py check celestina-style --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** celestina-style build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:94bcca376dca5a8ddd31636a2ed2da5b190a2935de54294e46b7e3bdfcb9d71a, verification_fingerprint sha256:3cd596258875c06b0c82cc70dc16cd347a319ebdcac82730c2add0d7d0e139ec
