# Evidence: brightness gaps after CEL-I-007 and CEL-I-008 (CEL-I-009)

- **Date:** 2026-11-25
- **Scope:** light unit (`bug`); `CEL-I-009`: `celestina/src/osd/popup.rs` (the single brightness slider holds its asked level until read back or `ASK_HOLD`; the popup asks for a frame when that hold ends; the wheel over All moves rows that differ from the target even when the rounded mean does not change), the ledger row and `VAL-CEL-I-009`
- **Environment:** worktree `celestina-CEL-I-009` from main `3c87039b`; pinned toolchain and stable 1.100
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** not needed: the fix restores the approved look

## Procedure

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** every command exits 0; 2426 tests passed, 0 failed; 117 lines outside documents (light)
- **Tests:** `the_single_brightness_slider_keeps_its_level_until_the_monitor_reads_it_back`, `the_single_brightness_slider_shows_the_reading_after_the_hold`, `the_wheel_on_all_brings_close_monitors_together`; `a_reading_does_not_move_a_slider_under_the_pointer_but_does_afterwards` updated to the hold (it pinned the old adoption)
- **Review:** 1 round (Sonnet, high reasoning); fixed: the single slider recorded an ask even with no reading; deferred: at the cap, a notch over All re-sends the level to a row already at it (the throttle bounds it)

## Limits

- The tests were written with the fix, not seen failing first.
- No live DDC run; the live check is `VAL-CEL-I-009`.

## Follow-up

None.

## Landing

- **Base revision:** `3c87039b8f73d200d82eca7c5bbe3d37c6f6743d`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2c343ca17993998bcdb791c6541f4c51e0cc8634b1e3367fc2eca042077e78aa, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
