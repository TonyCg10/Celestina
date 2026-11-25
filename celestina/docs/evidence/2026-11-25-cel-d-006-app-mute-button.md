# Evidence: the speaker button that mutes one application (CEL-D-006)

- **Date:** 2026-11-25
- **Scope:** light unit; `CEL-D-006`: `celestina/src/osd/audio_rows.rs` (`stream_row`), its two new tests, the ledger row and `VAL-CEL-D-006`
- **Environment:** worktree `celestina-CEL-D-006` from main at `101f2da8`; pinned toolchain and stable 1.100
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** variant A of the per-app mute board, approved by the author on 2026-11-25 (a design canvas private to the author)

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

- **Nested look:** [apps](2026-11-25-cel-d-006-app-mute-button/apps.png) — the volume popup with the applications section unfolded: Zen (unmuted, speaker button) and Spotify (muted, dimmed name, crossed-out speaker button) (full frame: [apps-full](2026-11-25-cel-d-006-app-mute-button/apps-full.png)); nested niri, fake wpctl graph, not the live session
- **Exit:** every command exits 0; 2379 tests passed, 0 failed; 117 lines outside documents (light)
- **Review:** 1 round (Sonnet, high reasoning); fixed: stale "mute switch" wording in VAL-CEL-I-004, two doc comments and a test comment, a new test for the disabled button; deferred: a hovered muted button shows the hover fill instead of the muted one (glyph and ink still say muted)

## Limits

- No nested screenshot yet; the live look is `VAL-CEL-D-006`.

## Follow-up

None.

## Landing

- **Base revision:** `63ab8e710d88dcb108e7b1e24ae992965593e210`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e015f7e43cff71b99379ef255b7fe27125529b320379cfc83f269913f3ef7a76, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
