# Evidence: 2026-10-06 the heading without its expanded state

- **Date:** 2026-10-06
- **Scope:** `SID-H1-K` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

The author's recordings after 1.9.2 and 1.9.3 showed a fast wheel scroll
jumping: frame by frame, the listing went back about 20 px for one frame and
returned. In the frames the heading was in or near its expanded state (the
metadata block open), where the content frame under the rows moves with the
heading's progress. The author asked for that state to go: it is never
reached for.

The expanded state is removed end to end: `HeadingScroll` has no
`expandSpan` and no `compactProgress`, and its travel is clamped at zero;
`FolderWheelHandler` no longer has an expanding branch nor pins the listing
to a moving origin; `FolderHeading` draws the title (and the watch warning
and the phone button) with no section label or metadata lines; `FolderView`
places the content frame once, behind the path bar, instead of interpolating
it on the heading's progress. The wheel glide keeps the `Behavior` with a
`SmoothedAnimation` from 1.9.2 and reads its velocity at each re-aim from
what is pending (`glideVelocity`), with `duration` unset so the Behavior
carries a running glide over instead of stopping it.

```sh
sh siderita/scripts/qml-tests.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/qmllint-cxxqt.sh siderita
```

The author then felt the scroll freeze for as long as the heading returned.
A trace of the deployed build showed why: the grid's top inset was a
difference of two positions that both move with the path bar, and on every
frame of the return it passed through a wrong value before settling (one
side updated before the other); each flicker re-aimed the wheel glide, and
a `SmoothedAnimation` re-aimed every frame never advances. The inset is a
sum of heights now, which does not move with the bar at all.

## Result

- **Exit:** `qml-tests.sh`: 171 passed, 0 failed; every guard OK; qmllint
  at the baseline.
- **Observed:** the expansion tests (`test_d`, `test_j`, `test_p` to
  `test_s`) are gone with the state; `test_t` asserts that a push up at the
  top moves neither the heading nor the listing. The architecture baseline
  for `FolderView.qml` ratchets to 739 lines and `FolderHeading.qml` leaves
  the language baseline (its literal Spanish labels went with the block).

## Limits

- Whether the jump is gone is the author's judgement on the session; the
  frame-by-frame reading of the recordings is the diagnosis, not a proof.

## Follow-up

None.

## Landing

- **Base revision:** `b59adc637b724bcbba37f0f57b21c9f3869e60b8`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8ace36eca4142ed132301fb503cc5e10f6a279309a3e2f3af914236433beaac5, verification_fingerprint sha256:a7e955a929c1172fbf4e396955bbf3fce3279a37dbf23137aaed932720f1622a
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
