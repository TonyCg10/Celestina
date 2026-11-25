# Evidence: the wheel on the popups' sliders (CEL-I-008)

- **Date:** 2026-11-25
- **Scope:** ordinary `milestone` unit (179 lines outside documents); `CEL-I-008`: `celestina/src/osd/popup.rs` (a wheel turn over a slider of the level popups steps it by one percent per notch; the slider path and the wheel path share `ask_level`), `celestina/src/osd/audio_rows.rs` and `monitor_rows.rs` (level getters), the ledger row and `VAL-CEL-I-008`. Stacked on `unit/celestina/CEL-I-007`
- **Environment:** worktree `celestina-CEL-I-008`; pinned toolchain and stable 1.100
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** not needed: no look changes; the author asked for the behaviour on 2026-11-25

## Decisions

- A notch is the bar's own unit (`scroll_steps`, 15 px a step), so a smooth
  wheel steps once per notch and a half turn waits for the rest.
- Up raises, as the wheel over the bar's Volume and Brightness pieces does.
- Every slider of both popups takes the wheel: the output, the microphone,
  each application, the single brightness slider, All and each monitor. All
  starts from the mean shown and gives every monitor with a reading the new
  level. A slider with no reading takes none.
- The wheel over the bar's pieces is unchanged (its own step).

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

- **Exit:** every command exits 0; 2423 tests passed, 0 failed
- **Tests:** `the_wheel_moves_a_monitor_row_one_percent_per_notch`, `the_wheel_on_all_moves_every_monitor_with_a_reading`, `the_wheel_moves_the_output_one_percent_and_stops_at_the_ends`, `the_wheel_does_nothing_on_a_slider_with_no_reading`, `a_late_read_back_does_not_lose_a_wheel_notch`, `the_wheel_does_nothing_while_a_slider_is_dragged`
- **Review:** 1 round (Sonnet, high reasoning); fixed: a late read-back between notches could lose or reverse a step (a notch within 1.5 s of the last steps from the level the wheel set), the wheel acted during a drag (now ignored), a field named like a method; deferred: All steps from the rounded mean, so with monitors at 100 and 99 a notch up moves nothing; a half notch left long ago still completes a step

## Limits

- The tests were written with the code, not seen failing first.
- No live run: the live check is `VAL-CEL-I-008`.

## Follow-up

- All steps from the rounded mean: with monitors at 100 and 99, a notch up moves nothing.

## Landing

- **Base revision:** `88829f21516b1511e8541278f8b03422d46910e5`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:4e77bfeb9b02e8e3b50269f670b07f366061705033de020a7d637887c26e5290, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
