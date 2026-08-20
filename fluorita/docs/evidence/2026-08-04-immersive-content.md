# Evidence: F6 immersive content and honest catalogue

- **Date:** 2026-08-04
- **Scope:** F6-A; plan
  [immersive-content](../plans/archive/2026-08-04-immersive-content.md)
- **Environment:** Arch-based Linux, niri, Qt 6.9, libmpv; author's checkout
- **Artifact:** `fluorita/target/release/fluorita`, `siderita/target/release/siderita`

## Procedure

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
cargo fmt --check
cargo clippy --all-targets
cargo test -p fluorita-core -p fluorita-engine
cargo test --bins                       # fluorita
bash scripts/qmllint-cxxqt.sh fluorita
bash fluorita/scripts/smoke.sh --binary fluorita/target/release/fluorita
CELESTINA_REDUCED_MOTION=1 bash fluorita/scripts/smoke.sh \
    --binary fluorita/target/release/fluorita
bash fluorita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

## Result

- **Exit:** 0 for each command.
- `Architecture contract: OK`, `Language contract: OK (160 legacy file(s)
  ratcheted)`, `Documentation contract: OK`, `qmllint-production: OK`.
- Clippy reports no error and no unused-code warning; `cargo fmt --check` is
  clean.
- Tests: 75 in `fluorita-core`, 75 in `fluorita-engine`, 31 in the application.
  New coverage is the forgetting rule — a deleted file under a root that
  answered goes, and a file under a root that never answered stays and stays
  marked missing.
- Both smokes pass, in the normal and the reduced-motion configuration. The
  second matters on its own: with motion off there is no animation to wait for,
  and the close path that ends the playback session had to end it anyway.

## Observed while building it

- The opening transition showed black until the handoff waited for a picture to
  be *presented* rather than merely requested. `showsPicture` means the player
  has a source; between that and a drawn pixel there is a decode, and handing
  over at the first only moves the black from before the animation to after it.
- The ambient light was invisible for video because `MpvVideo` filled the
  surface and painted its own letterbox over it. Sizing the item to the film's
  shape leaves those bands unpainted, which is what lets the light through.
- Stepping from one video to another crashed. `close` documents a two-phase
  teardown — hand the render handle back, wait for the surface, then stop the
  worker — and `open` went straight to `stop_worker`, destroying the mpv
  instance under a live surface. Replacing an item now goes through the same
  handshake.

## Limits

- No pointer or keyboard interaction was exercised by an agent. Earlier in this
  session, synthetic input against the author's live session trashed two of
  their files and unmapped a configured folder; both were restored, and driving
  the running application that way was abandoned. Everything interactive here is
  therefore unverified by the agent lane and belongs to `VAL-FLU-IMMERSIVE`.
- A build proves compilation and a smoke proves startup. Neither proves the
  transition's appearance, the ambient light's strength, the dock's reveal or
  the real behaviour of the file chooser.
- The ambient light and the growth both depend on a cached thumbnail. An item
  without one opens unlit and grows showing the player, which is the honest
  fallback and not a defect to hide.
- The video's shape is read from its thumbnail's aspect. A wrongly generated
  thumbnail would leave a thin band inside the picture.

## Follow-up

`VAL-FLU-IMMERSIVE` in [VALIDATION.md](../../VALIDATION.md).
