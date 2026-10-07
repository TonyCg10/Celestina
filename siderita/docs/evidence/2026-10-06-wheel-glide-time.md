# Evidence: 2026-10-06 the wheel glide runs on time, not speed

- **Date:** 2026-10-06
- **Scope:** `SID-H1-J` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2
- **Artifact:** not applicable in the session; the landing builds, verifies and deploys the registered binary

## Procedure

After `SID-H1-I` the author recorded the listing moving like a belt: a burst
of notches advanced at one fixed speed instead of answering the wheel. The
glides kept their `Behavior` + `SmoothedAnimation` shape (one continuous
motion across notches) but run on a fixed time now, `velocity: -1` with
`duration: wheelGlide` (`motionNormal`): whatever distance is pending is
covered in that time, so more notches mean a faster glide.

```sh
sh siderita/scripts/qml-tests.sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** `qml-tests.sh`: 176 passed, 0 failed; every guard OK.
- **Observed:** the heading and listing tests pass unchanged.

## Limits

- The feel of the scroll is the author's judgement on the session.

## Follow-up

None.

## Landing

- **Base revision:** `32edc760f538bad870351835f3f4077695aab919`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5be33c4968b08180dd023202064337e28a7d6891214b13b3ef1a5cbb24233160, verification_fingerprint sha256:d0567b067e64fe81068f5c030913de269cc252b1dc3b73b09b9c2c0721c91671
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
