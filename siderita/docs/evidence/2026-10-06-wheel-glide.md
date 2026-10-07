# Evidence: 2026-10-06 a fast wheel scroll is one motion

- **Date:** 2026-10-06
- **Scope:** `SID-H1-I` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

The author recorded the folder heading glitching through its state changes
on a fast wheel scroll while a slow one was clean. Each notch restarted a
200 ms eased `NumberAnimation` on the heading's `travel` and another on the
listing's `contentY`, so a burst of notches accelerated and braked once per
notch and the two tweens drifted apart. Both glides are now a `Behavior`
with a `SmoothedAnimation` at the shared `wheelVelocity`: a notch that lands
mid-glide re-aims the running animation and carries its speed over.

`Immediate` reversing, not `Sync`: `Sync` snaps to the target when the
direction reverses, and the velocity it compares against survives a stopped
glide, so the first notch after an instant move (a folder change) landed at
once. The `tst_heading_scroll` suite caught that in order (`test_g` leaves
a glide in flight, `test_h` then saw no tween).

```sh
sh siderita/scripts/qml-tests.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** `qml-tests.sh`: 176 passed, 0 failed; every guard OK.
- **Observed:** the heading and listing tests still pass unchanged: an
  unsmoothed advance lands at once, a notch tweens both, four notches lose
  no distance, geometry moving does not stop the gesture.

## Limits

- The feel of a fast scroll is the author's judgement on the session; the
  tests prove continuity of the target, not the absence of a visible brake.

## Follow-up

None.

## Landing

- **Base revision:** `3af07002f771341e5761bc4ce6ba5ff41860d297`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e9fa7511bcfdba5231b907c604263839851433b53c6cb116b74306de22568982, verification_fingerprint sha256:d0567b067e64fe81068f5c030913de269cc252b1dc3b73b09b9c2c0721c91671
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
