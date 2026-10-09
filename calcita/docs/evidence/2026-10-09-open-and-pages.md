# Evidence: a PDF opens with continuous pages, zoom and navigation

- **Date:** 2026-10-09
- **Scope:** `CAL-1-A` of
  [CAL-1's plan](../plans/active/2026-10-09-cal-1-foundation.md):
  `calcita-core` (zoom ladder, page grammar, recent store, reading position),
  the controller's document set and recents, the per-window
  `CalcitaDocument`, the activation adapter's `Open`, the document window
  over QtPdf with its bar, the drop, the file chooser and the keyboard
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
sh calcita/scripts/qml-tests.sh                                  # exit 0
sh calcita/scripts/smoke.sh --binary <release binary>            # exit 0
bash scripts/qmllint-cxxqt.sh calcita                            # exit 0
bash scripts/check-architecture-contract.sh                      # exit 0
python3 scripts/check-language-contract.py                       # exit 0
bash scripts/check-documentation-contract.sh                     # exit 0
python3 scripts/commit_scope.py --check "calcita-maintenance: Add opening, continuous pages, zoom and navigation"  # exit 0
```

The fixture `calcita/tests/fixtures/three-pages.pdf` (three empty A4 pages)
is written by `calcita/scripts/make-fixture-pdf.py`, byte-identical on every
run; `pdfinfo` reads it as a valid three-page PDF.

## Result

- **Exit:** 0 for every command.
- **Rust:** 25 tests at the first commit (29 after the fix round). `calcita-core` (18): the ladder (`zoom_in(1.0) == 1.1`,
  `zoom_in(4.0) == 4.0`, `zoom_out(0.5) == 0.5`, factors between rungs), the
  page grammar (`12`, `+3` from 5 → 8, `-2` from 1 refused, `fin`, `inicio`,
  `0` refused, words refused), the recent store (one entry per path, latest
  first, capped at 50, a non-UTF-8 path round-tripping through the file,
  damaged lines skipped). The application (7): `Activate` raises, `Open`
  hands every path over in order through the fake queue, keys are byte-exact,
  admission by name (`.pdf`, local `file://` only), the shown name.
- **QML:** 36 passed, 0 failed, over the real `Main.qml`,
  `DocumentWindow.qml` and QtPdf with the fixture, the Rust types replaced by
  the stand-ins in `tests/qml/stubs`. `tst_document`: the fixture loads with
  3 pages; `goTo("3")` scrolls the view to page 3; a page past the end is a
  notice; the zoom buttons walk 1.0 → 1.1 → 1.25 → 1.1; fit page and fit
  width fit; a scroll moves the page field to «2 / 3»; the window remembers
  its page and zoom on close. `tst_main`: `Open` with one path replaces the
  empty state; with two, two windows; an open document is raised, not opened
  twice; closing a window drops it; a dropped non-PDF is a notice and no
  document; a dropped PDF opens; a recent document reopens at its page.
  `tst_bar`: «n / N», typing a page, the zoom controls, accessible names.
  `tst_keyboard`: PageDown/Space/PageUp/Shift+Space, Home/End, Ctrl+G then a
  page and Enter, Ctrl+0/Plus/=/Minus/1/2, Ctrl+O.
- **Smoke:** two 8 s offscreen starts with no QML error: without arguments
  `calcita-smoke: empty=true pageCount=0 textScale=1 fontBody=13`; with the
  fixture on the command line
  `calcita-smoke: empty=false pageCount=3 textScale=1 fontBody=13`.
- **qmllint:** 1 warning, the baseline row (the shared `CelestinaIcons.qml`);
  Calcita's own QML lints clean.

## Limits

- The stand-ins re-implement the page grammar and the ladder in JavaScript;
  the Rust side is covered by the crate's tests, not by the QML run.
- The drop is exercised through `openDropped`, not a real drag; the
  FileChooser portal, the compositor's raise and a real PDF on the session
  are `VAL-CAL-OPEN` in [VALIDATION.md](../../VALIDATION.md).
- QtPdf's view moves its own `currentPage` only when a jump or the scroll bar
  ends; the window follows every scroll from the view's Flickable instead.

- Deferred: the page view keeps QtPdf's own scroll bar; the suite's
  scroller and scroll bar are not wrapped around `PdfMultiPageView` yet.

## Review fix round 1

The review of the first commit asked for these changes, all in the fix
commit:

- A refusal goes to the window that asked: `openPath(key, origin)` and
  `openDropped(uris, origin)` carry the asking window (a document's key or
  `main`) and `notice(kind, text, origin)` returns it, so a non-PDF dropped
  on an inactive window shows that window's pill; a notice no window asked
  for (an `Open` from another launch) shows on the front window.
- The front window is the document window last focused (else the newest,
  else the empty window): `Activate` raises it, and the file chooser is
  parented to it when it opens (set on open, not bound, and cleared when
  that window closes).
- Admission runs on a worker: a folder named `x.pdf` is refused like a
  non-PDF (`admit(request, is_dir)`, tested with a fake stat).
- `components/WindowChrome.qml` holds the drop area, the notice pill, the
  notice routing and Ctrl+O for both windows.
- `document.rs` tests its pure parts (the mode names a restored word gives,
  the zoom word to remember, which reported scales are taken).
- The gap comment now says QtPdf's `rowSpacing` is 6 logical pixels at any
  scale.
- New QML tests: a refused drop on an inactive document window shows its own
  pill; a notice no window asked for shows on the front window; `Activate`
  targets the last focused window, which parents the chooser; Space typed
  in the page field types a space and does not scroll.
- Once in seven runs, a QtPdf render still queued on Qt's image reader
  thread crashed that thread after its document was destroyed at the end
  of a test file. That is a risk in the application too (a window closed
  mid-render), so round 2 makes closing safe by design: `onClosing`
  empties the `PdfDocument` (`source = ""`, the view stays on a document
  that still exists) and tells the owner on the next event-loop turn
  (`Qt.callLater`); the window, and its Instantiator row, are destroyed only
  then. The tests close their windows that way in `cleanup()`. Round 1's
  150 ms test pause was removed: without it, 20 consecutive runs of the
  QML tests were clean (41 passed each), including a new test that closes
  the fixture's window as soon as `pageCount` reports 3, twenty times in
  one process.

Rerun after the fix, every command exit 0: `cargo test -p calcita-core`
(18), in `calcita/` fmt, clippy `-D warnings`, `cargo test` (11), the
release build; `qml-tests.sh` (41 passed after round 2, 20 runs); the smoke (`pageCount=0`, then
`pageCount=3`); `qmllint-cxxqt.sh calcita` (1, the baseline); the
architecture, language and documentation contracts; the scope check.

## Review fix round 3

- `onClosing` now makes the controller forget the document at once
  (`closeDocument`), marks the window as closing and drops it from `front`
  and the chooser's parent; only its destruction waits a turn. An `Open`
  of the same path in between opens a fresh window (new test: close the
  fixture's window and open it again at once → a new window with 3 pages).
- Round 2's deferral alone was not enough: in ten runs after this round's
  first change, two still crashed on the image reader thread (a document
  freed while a page was being read). The `PdfDocument`s now belong to the
  first window, one per window ever open at once, and are never destroyed
  while Calcita runs: a closing window empties its document, and the next
  window reuses it. The tests own their `PdfDocument` the same way.
- After that, 30 consecutive `qml-tests.sh` runs were clean (42 passed
  each), with no pause in the tests. The smoke passes (`pageCount=0`, then
  `pageCount=3`) and qmllint stays at 1, the baseline.
- Known noise: emptying a document logs QtPdf's `PdfDocument: Cannot
  open:` once per closed window.

## Follow-up

`CAL-1-B`: search, the outline, text selection and links.

## Landing

- **Base revision:** `4502c26f0c01f20ea16054759695b974096746ba`
- **Check:** `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:ad54af385e0f19dd752ba03ada09960c55ec00c09d1fc809c55b161a70c6ca4d, verification_fingerprint sha256:0f5538769d977295b37aab7ecd9a0aaf54febbe0d41a948e32589c2b5c53621e; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e1506a652ffa745c981734ed6658ca1ce3d2f8977415dd5969d9342b72fcb8c0, verification_fingerprint sha256:39cf2a4bf85903814c7ffacdd1566a6385a205ee21fb02e555eb07ecdfde491a; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7b3725970d3d6dc09182bc6049ea28510e8b0cb2dd133abf2f068404ed172eae, verification_fingerprint sha256:4e4c9f4bbfe41cd41690fa242748446d59112ca070feb53af29f48f8dc373f81; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:1fa1c5b17754f2f5aaf2223ba8727edf8bb100984258ef5dec4f7fda2e5ac00f, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:948c2058ea1bcb5641fef2e15d15374802ef2f86b2ebfedd33aef4d9ae090d28, verification_fingerprint sha256:ad5a9a7d84e572a6f89188ddfa0b1d5d3c6e1d3fb9d892a8930df0847d5ffcff; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:cd6b8dc299c5ea7d3ace59636805d4dfe21cfa9940f1e2334b9031e4c485857a, verification_fingerprint sha256:a0ae233ecf3c7e8e31ae8a81422875600a4fc516e5f7cf743920aa144f202ad8; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2ee5a40a0b02ae13b921be7b122579ea017b4a112ac26e68eff5d6856cbc8905, verification_fingerprint sha256:bb4e7203c261ba3c77e61be97c966f04e13b0db0546428e91cc64671d6a8e1ea; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:340e0a2987743b915617ba420ff1f23857b850ab9cf4d451f491dd62af837dd5, verification_fingerprint sha256:daee35677d75e773025c27c23abffae482c9721a36f86ccaaab3fc9aaa047985; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2341cd0ab2c168ab0e489af5b5fda2e0d239f49921289bbe2ceff364556986e5, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606
- **Deploy:** after the push: calcita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh
