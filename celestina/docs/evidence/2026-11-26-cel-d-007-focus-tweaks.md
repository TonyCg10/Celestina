# Evidence: full opacity around an open sheet, one pill on the open piece (CEL-D-007)

- **Date:** 2026-11-26
- **Scope:** light unit; `CEL-D-007`: `celestina/src/sheet` (`Indicator`, `Sheets::quiet_piece`), `celestina/src/island` (`BarTree::paint_row`), `celestina/crates/celestina-ui/src/paint.rs` (`Tree::paint_children_quiet`), their tests, the ledger row and `VAL-CEL-D-007`
- **Environment:** worktree `celestina-CEL-D-007` from main at `2fa7cdfe`; pinned toolchain and stable 1.100
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** variants O1 and P1 of "Celestina · ajustes del foco", approved by the author on 2026-11-26 (an artifact private to the author)

## Procedure

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100 clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
scripts/smoke.sh --binary PATH --only bar,osd,notify
```

## Result

- **Nested look:** [open-volume](2026-11-26-cel-d-007-focus-tweaks/open-volume.jpg) the volume sheet open: the other pieces at full opacity, one grey pill under the volume piece; [hover-stack](2026-11-26-cel-d-007-focus-tweaks/hover-stack.jpg) top to bottom: hover at rest (clipboard), pointer over the open piece (indicator pill only), pointer over another piece (its hover pill), the frame right after the press (the hover pill already gone), settled on the new piece (one pill); [switched](2026-11-26-cel-d-007-focus-tweaks/switched.jpg). Nested niri, fake wpctl graph, not the live session
- **Exit:** every command exits 0; 2560 tests passed, 0 failed; smoke: OK (only: notify,bar,osd)

- **Review:** 1 round (light unit), by the controller (Opus; the implementer was Sonnet): the diff mutes only the open piece's subtree highlight (hover and press; keyboard focus kept) through the scoped tree walk, the dim and its clock are gone, the cut-edge blur is untouched; the nested shots match the approved O1 and P1. Nothing to fix; the shots were converted to JPEG.

## Limits

- The cut edge of the row (the blur band) was not on screen in these nested frames: the capsule sits at the screen edge, so the window cuts nothing. That code is unchanged.
- No frame of the indicator mid-flight was caught; the settled frames and the frame right after the press show one pill each.
- The return of the hover after the close is covered by a test, not by a nested frame.

## Follow-up

None.

## Landing

- **Base revision:** `e8d111faa32205874eda0225f92fd7f7bada7c8c`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar,osd,notify only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:9d16e9486cf56f0c0f04704b7c8efbdfbf50f89d10b3842d100c4b638db401d6, verification_fingerprint sha256:2bed5d99aa6712fca5ea937cca7ca6b9aa6dba3cbece5757dafe808f66585102
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
