# Evidence: icons in the Island's tab row (CEL-D-001)

- **Date:** 2026-11-25
- **Scope:** ordinary `maintenance` unit (313 lines outside documents, most of
  them tests of the text labels that are gone); `CEL-D-001`:
  `PageView::icon` on every page, `page_icon` in `island/view.rs`, the tab row
  in `island/pages/tabs.rs`, the two abbreviated tab labels removed from
  `copy.rs`, the ledger row and `VAL-CEL-D-001`. Stacked on
  `unit/celestina-style/STYLE-ICONS-1`, which adds `calendar`
- **Environment:** worktree `celestina-CEL-D-001`; pinned toolchain and stable
  1.100
- **Artifact:** not applicable in the session (the landing builds and verifies)
- **Preview:** variant A of the tab-row board, approved by the author on
  2026-11-25 (a design canvas private to the author); the current row was
  captured in a nested niri on 2026-11-25 for the comparison

## Mapping

| Page | Icon |
|---|---|
| activity | `view-grid` |
| notifications | `bell` |
| tray | `system-tray` |
| sysmon | `gauge` |
| clock | `calendar` |
| clipboard | `clipboard-paste` |
| network | `wifi` |
| bluetooth | `bluetooth` |
| power | `zap` |
| phone | `phone` |
| media | `music` |
| absent | `app-window` |

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

- **Nested look:** [tabs](2026-11-25-cel-d-001-tab-icons/tabs.png) — the Island open on the clock page: the tab row shows one icon per page, the clock icon active in the accent colour (full frame: [tabs-full](2026-11-25-cel-d-001-tab-icons/tabs-full.png)); nested niri, not the live session
- **Exit:** every command exits 0; 2376 tests passed, 0 failed
- **Tests:** `every_page_icon_has_a_file` (the mapping, and each SVG exists),
  `tabs_draw_the_page_icon_not_its_title`,
  `the_open_tab_tints_its_icon_accent_ink`,
  `icon_tabs_share_the_row_equally_and_fit_388`; the two label tests are
  removed
- **Review:** 1 round (Sonnet, high reasoning); fixed: orphaned and stale doc comments in `tabs.rs`, a test still naming the old label key, a note on `VAL-CEL2-CLIPBOARD-LOOK`; deferred: none

## Limits

- No nested screenshot of the new row yet; the live look is `VAL-CEL-D-001`.
- A page added later must name an icon: the trait method has no default.
- The toolkit has no accessible-name API, so the tabs, like every node, carry
  no name for assistive technology; the visible title was their only label.
  Accessible names are a toolkit gap for a later entry.

## Follow-up

None.

## Landing

- **Base revision:** `1b1c610ecc4f27cf1ecf5ad4d7becf4c98d0882a`
- **Check:** `production_artifact.py check celestina --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** celestina build: build-production.sh exit 0, verify-production.sh exit 0, smoke phases bar only; the last landing of the series runs the full smoke, manifest source_fingerprint sha256:af9e4ce36c5f34f2d4066f7fb1c6f162d02d41be6a3af51438635cff9b918017, verification_fingerprint sha256:30943d5f01b58336ac6b95f3fc10ed78b35b459f81361951517fb8b1f587c2e9
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh
