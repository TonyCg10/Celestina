# Evidence: brightness rows stay where they are released (CEL-I-007)

- **Date:** 2026-11-25
- **Scope:** light unit (`bug`); `CEL-I-007`: `celestina/src/osd/monitor_rows.rs` (`MonitorMirror` holds each row's asked level), `celestina/src/osd/popup.rs` (passes `now`), the ledger row and `VAL-CEL-I-007`
- **Environment:** worktree `celestina-CEL-I-007` from main `3b5dbc53`; pinned toolchain and stable 1.100
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** not needed: the fix restores the approved look of CEL-I-003 (variant C, approved 2026-11-25)

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

- **Exit:** every command exits 0; 2417 tests passed, 0 failed; 138 lines outside documents (light)
- **Cause:** a drag sends several levels; each DDC monitor reads each back 1 to 2 s later, after the release, and every read-back was adopted as a new reading (the author's recording of 2026-11-25: 14 % released, rows showing 19 %, 12 % and 15 % before settling)
- **Tests:** `a_row_keeps_the_level_it_was_set_to_until_the_monitor_reads_it_back`, `a_row_shows_the_reading_once_the_hold_passes_without_a_read_back`; `a_late_reading_of_one_row_does_not_move_another` rewritten to the hold rule (it pinned the old behaviour)
- **Review:** 1 round (Sonnet, high reasoning); fixed: no frame was asked for when a held row's hold ended (the popup now schedules one), dead logic in `adopt`; deferred: a stale read-back equal to the final ask ends the hold early, a row that loses its reading keeps the asked level for up to 6 s, the hold is lost when the popup is reopened within it, the single brightness slider has no hold

## Limits

- The new tests were written with the fix, not seen failing first.
- A monitor that reads back a slightly different level than it was set to keeps the asked level shown for one DDC settle (6 s); the popup then redraws with the reading.
- No live DDC run; the live check is `VAL-CEL-I-007`.

## Follow-up

- The single brightness slider (one monitor) has no hold; a slow DDC monitor can still jump back there.
- The hold does not survive closing and reopening the popup within 6 s.

## Landing

- **Base revision:** `3b5dbc53a2a6a1a62cdfb95c59fc2243fd319cd2`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases osd only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:81d7e37f0ea553a3a7c01684643636ae4f6e5dcabd67e477d6e843a4b007ce5e, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
