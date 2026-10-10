# Evidence: the pages on opaque paper over the glass canvas

- **Date:** 2026-10-10
- **Scope:** `CAL-1-D` of
  [CAL-1's plan](../plans/active/2026-10-09-cal-1-foundation.md), a bug
  unit: `qml/components/PageView.qml` (the `paper` token and the paper
  rectangle under every page's image), `tests/qml/tst_paper.qml` (new),
  STATUS, VALIDATION (VAL-CAL-OPEN) and the plan's ledger
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0 with
  QtQuick.Pdf and `qsb` from `/usr/lib/qt6/bin`, offscreen only
- **Artifact:** the `calcita` release binary in the shared Cargo target, not
  installed; the version stays 1.0.0 in the tree and the landing
  (`--kind bug`) bumps the patch

## Bug

The author's screenshot of the deployed 1.0.0 showed black text with no
contrast: QtPdf's `PdfPageImage` renders a page with a clear background,
and the page delegate held only that image and the hit and selection
shapes, so the ink was drawn straight on the window's dark translucent
glass. The canvas is glass, but a document page is content and must be
opaque paper.

## Procedure

Run from the repository root of the unit's worktree, in this order:

```sh
(cd calcita && cargo fmt --all --check)                          # exit 0
(cd calcita && cargo clippy --all-targets --locked -- -D warnings) # exit 0
(cd calcita && cargo test)                                       # exit 0
(cd calcita && cargo build --release --locked)                   # exit 0
sh calcita/scripts/qml-tests.sh                                  # exit 0, 3 runs
sh calcita/scripts/smoke.sh --binary <release binary>            # exit 0
bash scripts/qmllint-cxxqt.sh calcita                            # exit 0
bash scripts/check-architecture-contract.sh                      # exit 0
python3 scripts/check-language-contract.py                       # exit 0
bash scripts/check-documentation-contract.sh                     # exit 0
python3 scripts/commit_scope.py --check "calcita-maintenance: Keep every page on opaque paper over the glass canvas"  # exit 0
```

## Result

- **Exit:** 0 for every command.
- **Rust:** unchanged; the application's tests pass.
- **QML:** 89 passed, 0 failed, 3 consecutive clean runs (84 before;
  `tst_paper` adds 3 tests and its two case hooks). `tst_paper` (new): the
  first page's delegate holds a `paper` rectangle whose colour has alpha 1,
  is the view's `paper` token and the suite's `iconSheet`, and fills the
  page, with no layer while reading as drawn; the paper is the delegate's
  lowest child, the `PdfPageImage` is drawn over it and the hit overlay is
  above it; in the reading mode the sheet stays opaque and lies inside the
  view the layer effect is applied to, so the layer covers it.
- **Smoke:** `calcita-smoke: empty=true pageCount=0 …` and, with the
  fixture, `empty=false pageCount=3 …`, no QML error, no auto-binding.
- **qmllint:** 1 warning, the baseline row, unchanged.
- **Architecture:** radius, glass-canvas and activation contracts OK; the
  paper rectangle takes `radiusNone`.

## Decisions

- `celestina-style` has no paper or white token. Its one sheet of paper is
  `iconSheet` (`#fbf7ee`, the near-white warm leaf of the folder icon), so
  `PageView.paper` is a documented local alias of it rather than a literal
  or a second white; changing the suite's sheet changes the pages with it.
- The paper is square and casts no shadow: DESIGN's L1 content surfaces
  are opaque and shadowless, and a rounded sheet under a square page image
  would show its corners under a full-bleed page.
- The rectangle is the delegate's first child, so the image, the hits
  (`accentSoft`), the current hit and the selection (`selectionMarquee`)
  and the link items stay above it; the `LinkPill` and the cards belong to
  the window, above the whole view.
- The reading mode's layer is the page view's, so it covers the paper with
  the ink: `reading.frag` un-premultiplies, inverts and turns the hue, so
  the near-white sheet reads near-black and the gaps between pages, which
  have no paper, stay clear over the glass. The translucent hits and the
  selection keep their alpha over the inverted sheet.

## Limits

- Offscreen only: the shader's output is not observed (the offscreen
  platform does not render the layer effect to anything inspected), so the
  dark paper under the reading mode and the hit and selection contrast in
  both modes rest on the shader's arithmetic and are confirmed by the
  author in VAL-CAL-OPEN and VAL-CAL-DARK.
- No screenshot was taken; the fix is asserted by the tree's structure and
  the token's alpha.

## Follow-up

VAL-CAL-OPEN (the page is opaque paper with black text readable over the
glass) and VAL-CAL-DARK (dark paper, light text, glass gaps) on the real
session.

## Landing

- **Base revision:** `b825a96921998efea1bc2ff88511893ba150950d`
- **Check:** `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:513ee6076352162307a6ed80989b89878c3f8cdc38325ab53a8359f30b07a136, verification_fingerprint sha256:0f5538769d977295b37aab7ecd9a0aaf54febbe0d41a948e32589c2b5c53621e
- **Deploy:** after the push: calcita: deploy-production.sh, status-production.sh
