# Evidence: search, the outline, text copy and links

- **Date:** 2026-10-09
- **Scope:** `CAL-1-B` of
  [CAL-1's plan](../plans/active/2026-10-09-cal-1-foundation.md):
  `calcita-core::search` (`SearchRequest`, `next_hit`), the controller's
  `copySelection` (through the `cpp/` clipboard shim) and `openExternal`,
  `CalcitaDocument`'s `searchQuery` and `nextHit`, Calcita's own continuous
  `PageView` over QtPdf, the search card, the outline card, the link
  confirmation and their keys
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0 with
  QtQuick.Pdf
- **Artifact:** the `calcita` release binary in the shared Cargo target, not
  installed

## Procedure

Run from the repository root of the unit's worktree, in this order:

```sh
(cd celestina-rs && cargo test -p calcita-core)                  # exit 0
(cd calcita && cargo fmt --all --check)                          # exit 0
(cd calcita && cargo clippy --all-targets --locked -- -D warnings) # exit 0
(cd calcita && cargo test --locked)                              # exit 0
(cd calcita && cargo build --release --locked)                   # exit 0
sh calcita/scripts/qml-tests.sh                                  # exit 0, 20 runs
sh calcita/scripts/smoke.sh --binary <release binary>            # exit 0
bash scripts/qmllint-cxxqt.sh calcita                            # exit 0
bash scripts/check-architecture-contract.sh                      # exit 0
python3 scripts/check-language-contract.py                       # exit 0
bash scripts/check-documentation-contract.sh                     # exit 0
python3 scripts/commit_scope.py --check "calcita-maintenance: Add search, the outline, text copy and links"  # exit 0
```

The new fixture `calcita/tests/fixtures/outline.pdf` (three A4 pages reading
«Page one», «Page two», «Page three» in Helvetica; an outline Introduction →
page 1, Chapter two → page 2; on page 1 an internal link to page 3 and a
link to https://example.org/guide) is written by
`calcita/scripts/make-fixture-pdf.py`, byte-identical on every run, which
still writes `three-pages.pdf` unchanged; `pdfinfo` reads it as a valid
three-page PDF and `pdftotext` finds the three headings.

## Result

- **Exit:** 0 for every command.
- **Rust:** `calcita-core` 25 tests, 7 of them new in `tests/search.rs`
  (no hits; the first step lands on the first or the last hit; one step each
  way; wrapping round both ends; the ends staying put without wrap; a
  current hit past a shrunk list; a request trims its query and blank text
  searches nothing). The application 13 (2 new: only `http`, `https` and
  `mailto` links reach `xdg-open`, never `file:`, `javascript:`, a bare host
  or a link with a blank or a control character; `nextHit` in the
  property's `-1` terms).
- **QML:** 62 passed, 0 failed, 20 consecutive clean runs of
  `qml-tests.sh`. `tst_search` (new): Ctrl+F opens the card and «page» finds
  3 hits; the first hit is current at once («1 de 3»); Enter, Shift+Enter,
  F3 and Shift+F3 walk the hits round both ends and the view and the page
  field follow the current hit; page two's hit is marked and the current
  one painted; a word not in the document reads «Sin resultados»; Escape
  closes the card, clears the search and gives the pages the focus; the
  bar's «Buscar» button toggles the card. `tst_outline` (new): F9 shows the
  two bookmarks with their accessible names; Down and Enter go to page 2,
  Up and Enter back to page 1; a click on a bookmark navigates; Escape
  closes the card; the internal link, named for page 3, goes to page 3
  with no pill; the external link shows «Abrir enlace externo
  example.org» and opens nothing until «Abrir», then hands
  `https://example.org/guide` to the controller; Escape refuses it; a
  pointer drag over «Page one» selects it and Ctrl+C gives the selection to
  `copySelection`. `tst_document` gains a zoom after a scroll keeping the
  page. The CAL-1-A tests run unchanged over the new view except for
  reading its `contentY` directly.
- **Smoke:** `calcita-smoke: empty=true pageCount=0 …` and, with the
  fixture, `empty=false pageCount=3 …`, no QML error, no auto-binding.
- **qmllint:** 1 warning, the baseline row; Calcita's own QML lints clean.

## Decisions

- QtPdf's `PdfMultiPageView` opens an external link at once
  (`Qt.openUrlExternally` in its link delegate, so no confirmation can come
  first) and paints hits and selection in its own colours, not theme
  tokens, so the window now lays out its pages itself in
  `qml/components/PageView.qml`: a Flickable with one item
  per page, whose image, hits, selection and links are built only near the
  viewport. Every page's place is exact at any scale (a `ListView` shifted
  its origin when the scale changed). The same `PdfDocument` pooling and
  the close rules of CAL-1-A are unchanged. This also closes CAL-1-A's
  deferral: the pages scroll with `CelestinaWheelScroll` and
  `CelestinaScrollBar`.
- The hits are `accentSoft`; the current hit and the selection are the
  theme's `selectionMarquee`.
- The outline's key is F9 (other readers' side pane): the design's keyboard
  map names none.
- QtPdf makes the first hit of a new search current without a change
  signal, so the view shows it as soon as the hits arrive and the counter
  reads «1 de N».
- The selection keeps both ends set by the drag itself, so it stays drawn
  after the release; it lives on one page at a time.
- `copySelection` is Qt-thread clipboard work through a one-function C++
  shim (`cpp/clipboard.cpp`), because cxx-qt-lib has no `QClipboard`;
  `openExternal` starts `xdg-open` on a worker that also reaps it.

## Limits

- The stand-ins re-implement the hit order in JavaScript; the Rust side is
  covered by the crate's tests.
- A real clipboard paste into another application, a real browser opening
  and a real document's outline and links are `VAL-CAL-SEARCH` in
  [VALIDATION.md](../../VALIDATION.md).
- QtPdf's search ignores case; `SearchRequest::case_sensitive` is false.
- Selection is per page: a drag does not run across two pages.
- Known noise, unchanged: emptying a document logs QtPdf's `PdfDocument:
  Cannot open:` once per closed window.

## Review fix round 1

The review accepted the own page view (for the reason above) and F9 for the
outline, and asked for these changes, all in the fix commit:

- **Scaling.** `PageView` computes every page's top once per document and
  scale into `tops` (also the content height); `pageAt` is a binary search
  over it; no `forceLayout` anywhere. The pages' content is built only in
  the near range `firstNear`..`lastNear` (a viewport above, two below),
  two properties a `contentY` or `height` change updates, which the
  Loaders' `active` reads instead of a per-page geometry binding. Every
  page keeps an empty shell item (no image, no handlers). New fixture
  `tests/fixtures/many-pages.pdf` (300 empty A4 pages, 30 KB, from the same
  generator) and `tst_long.qml`: open, scroll to the end in 60 steps and
  back in 60 within 2 s (measured 118 ms offscreen) with at most 6 pages
  built (2 at the end); a jump to the last page lands on it.
- **Search jumps.** The view moves for a hit only when a search's hits go
  from none to some, and on an explicit step (`showHit`, used by Enter,
  F3 and the arrows); `onCurrentResultChanged` no longer scrolls. Typing
  waits 150 ms before the search string changes. Tests: a later `count`
  change leaves `contentY` alone; the search string stays empty while
  typing and follows after the pause.
- **Selection.** Chosen: the view does not flick on a pointer drag
  (`interactive: false`); it scrolls with the wheel (`CelestinaWheelScroll`),
  the scroll bar and the keyboard, so a drag on a page is always a
  selection by construction. Test at zoom 1.5: a downward drag from «Page
  one» selects across the lines down to «Go to the end» and does not
  scroll. (Offscreen, QtTest's drag was not stolen even with flicking on,
  so the test documents the behaviour; the guarantee is the
  non-interactive Flickable.) Touch flicking is not available as a result.
- **Zoom keeps the place on every page,** the first included: the view
  keeps the point at its top at the same place in its page (tests on page
  1 and page 2).
- `pageAt`'s comment names the item; «No se pudo abrir el enlace.» is
  `LinkError::LaunchFailed` (with `NotWebOrMail`, `message_es`);
  `SearchRequest` holds only `query` (QtPdf has no case or direction
  option; `next_hit` keeps direction and wrap); `external_target` refuses a
  bare scheme (`https:`, `https://`, `mailto:`).
- STATUS and README list the full keyboard, F9 included.

Rerun after the fix, every command exit 0: `cargo test -p calcita-core`
(25), in `calcita/` fmt, clippy `-D warnings`, `cargo test` (13), the
release build; `qml-tests.sh` (70 passed, 20 consecutive clean runs); the smoke (`pageCount=0`, then
`pageCount=3`); `qmllint-cxxqt.sh calcita` (1, the baseline); the
architecture, language and documentation contracts; the scope check.

## Review fix round 2

- Closing the search card stops the 150 ms typing pause and clears the
  field (`SearchCard.reset`, from `closeSearch`), so a query typed just
  before Escape never reaches QtPdf. Test: type «page», Escape, wait 200 ms
  → the search string and the field are empty and `contentY` is unchanged.
- The multi-line drag test now also asserts the view is not interactive,
  the property that makes a drag a selection.
- `qml-tests.sh`: 71 passed, 6 consecutive clean runs; qmllint 1 (the
  baseline).

## Follow-up

`CAL-1-C`: reading mode, recents, the `application/pdf` entry, 1.0.0.

## Landing

- **Base revision:** `bc9b04326018b6a8b9ffbfcaabcca623f79140e2`
- **Check:** `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a8cf91cefe549544f8388bc1007719fd3de821d44d37a080a9857b4e2b82a5b7, verification_fingerprint sha256:0f5538769d977295b37aab7ecd9a0aaf54febbe0d41a948e32589c2b5c53621e; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:59a1ac22418f01bc19f67afdaff2268bae48bce99fd4f4e5ceec7af28b4f7d05, verification_fingerprint sha256:f3e63d44eead06d3b38e276f9e0460e046333e90dd82421e06653df28f4e21a9
- **Deploy:** after the push: calcita: deploy-production.sh, status-production.sh
