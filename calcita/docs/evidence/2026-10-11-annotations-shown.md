# Evidence: every annotation shown; the annotation model and the coordinates

- **Date:** 2026-10-11
- **Scope:** `CAL-2-A` of
  [CAL-2's plan](../plans/archive/2026-10-11-cal-2-editing.md):
  `cpp/page_provider.cpp` and `cpp/calcita/page_provider.h` (new),
  `src/pages.rs` (new), `src/main.rs`, `src/document.rs`
  (`annotationCount`, `annotationsOnPage`), `src/controller.rs`
  (`smokeGrab`), `build.rs`, `qml/components/PageView.qml`,
  `qml/DocumentWindow.qml`, `qml/Main.qml`, the QML stand-ins,
  `tests/qml/tst_paper.qml`, `scripts/make-fixture-pdf.py` with
  `tests/fixtures/annotated.pdf` and `tests/fixtures/rotated.pdf` (new),
  `scripts/smoke.sh`; `celestina-rs/crates/calcita-core` (`annot/`, the
  `lopdf` dependency, `tests/annot_read.rs`, `tests/coords.rs`); the
  [spec](../specs/2026-10-11-editing-design.md) and
  [plan](../specs/2026-10-11-editing-plan.md); README, STATUS, ROADMAP,
  VALIDATION (VAL-CAL-ANNOT), AGENTS (the local boundary) and the plan
  index
- **Environment:** CachyOS, Rust 1.98.1, cxx-qt 0.9.1, Qt 6.12.0 with QtPdf
  and QtQuick.Pdf, `lopdf` 0.45.0 without default features, offscreen only
  (`QT_QPA_PLATFORM=offscreen`), base `24fa39a0`
- **Artifact:** the `calcita` release binary in the shared Cargo target
  (`/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/calcita`),
  not installed; the version stays 1.0.1 in the tree

## Procedure

Run from the repository root of the unit's worktree:

```sh
(cd calcita && cargo fmt --all --check)                            # exit 0
(cd calcita && cargo clippy --all-targets --locked -- -D warnings) # exit 0
(cd calcita && cargo test --locked)                                # exit 0
(cd calcita && cargo build --release --locked)                     # exit 0
(cd celestina-rs && cargo fmt --all --check)                       # exit 0
(cd celestina-rs && cargo test -p calcita-core)                    # exit 0
(cd celestina-rs && cargo clippy -p calcita-core --all-targets -- -D warnings) # exit 0
sh calcita/scripts/qml-tests.sh                                    # exit 0, 3 runs
sh calcita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/calcita # exit 0
bash scripts/qmllint-cxxqt.sh calcita                              # exit 0
bash scripts/check-architecture-contract.sh                        # exit 0
python3 scripts/check-language-contract.py                         # exit 0
bash scripts/check-documentation-contract.sh                       # exit 0
git diff --cached --name-only | python3 scripts/commit_scope.py --check "calcita-maintenance: Make every annotation visible and read the annotation model" # exit 0
```

The smoke's new check was run first against the binary with the smoke
hooks but still on `PdfPageImage`, and failed as intended:

```text
smoke: the annotated page does not show its annotations: 880x1245 highlight=0.00 ink=0.000 paper=1.00
```

## Result

- **Exit:** 0 for every command above.
- **calcita-core:** 48 tests pass, 20 of them new.
  - `tests/coords.rs` (9): the four rotations against the view boxes the
    spike measured on QtPdf's renders; the offset box both shifted with the
    annotation and alone (the spike's measured `(20, 156, 130, 190)`,
    `(652, 20, 686, 130)`, `(465, 652, 575, 686)`, `(156, 465, 190, 575)`);
    the displayed size swapping on a quarter turn; the round trip
    view ⇄ PDF; `/Rotate` read as whole quarter turns as PDFium does (100
    and 179 → 90°, 45 and −45 → 0°, −100 → 270°) and a real `/Rotate`
    truncated (90.0 and 100.0 → 90°, 269.9 → 180°, NaN → 0°); rectangle
    normalization.
  - `tests/annot_read.rs` (10): `annotated.pdf` reads as a FreeText
    «Hello Calcita» at 16 pt in the `/DA` colour (0, 0, 153), a yellow
    Highlight with its quad, a red Ink stroke of five points and a Link as
    `Other`, each with its author and object id; `outline.pdf`'s two links
    as `Other`; `rotated.pdf`'s four boxes with the `/CropBox` winning on
    page 4 and each page's highlight landing at the corner its turn carries
    it to; a document encrypted with RC4 by `lopdf` itself, with a user
    password and with an empty one, is `Encrypted` for both readers, with
    its Spanish message; a broken file is `Unparseable`, a missing one
    `Io`, a sparse file past 256 MiB `TooLarge`; a form made with `lopdf`
    gives its text field, checkbox and radio group with names, kinds,
    values, rects and pages. Review round 1 added four over a five-page
    document made with `lopdf`: `/Rotate 90.0`, `100` and `-90` read as
    90°, 90° and 270°, a `/CropBox [0 0 700 900]` over the A4
    `/MediaBox` is clipped to it and one off the `/MediaBox` leaves the
    `/MediaBox`; with `/Annots [null <<…>> 999 0 R <<…>>]` the two inline
    annotations are at places 1 and 3; text strings lose a UTF-8 mark,
    decode UTF-16 big- and little-endian, and keep what decodes of a
    UTF-16 string with an odd last byte (`Hi\u{FFFD}`) or a lone
    surrogate (`\u{FFFD}A`), PDFDocEncoding giving `"café"`; the annotated
    fixture saved with object streams and a cross-reference stream reads
    as the same four annotations through them.
  - `annot::read` unit test (1): the object streams of one read share one
    inflation budget: exactly their inflated size loads, one byte less is
    `InflatesTooMuch` with that limit and the Spanish message.
- **calcita:** 16 tests pass, 3 new: the per-page annotation count over
  both new fixtures; only the newest waiting model read starts; the model
  reader reads one request at a time, publishes each (4 annotations for
  `annotated.pdf`, -1 for a missing file) and ends with its sender.
- **QML:** 90 passed, 0 failed, in each of 3 consecutive runs (89 before;
  `tst_paper` adds `test_each_page_image_comes_from_the_page_provider`:
  the page's image is an `Image` at `image://calcitapage/1/<key>`,
  asynchronous, with the source width the page is shown at). Every other
  test, the paper, the reading mode, search, the outline and links, keyboard
  and long documents among them, passes unchanged. Under qmltestrunner the
  provider is not registered (the binary registers it), so each built page
  logs `Invalid image provider` and shows no image; no test depends on the
  image's pixels.
- **Smoke:** `annotations=-1` on the empty window, `pageCount=3 …
  annotations=0` on `three-pages.pdf`, `pageCount=1 … annotations=4` on
  `annotated.pdf`; the grabbed first page (880 × 1245 px) is yellow over
  75 % of the highlight's inset rectangle, red over 7.7 % of the ink's
  (a 4-point zigzag) and paper over 100 % of an empty region. The grab is
  taken once the page's `Image` reports `Ready`, or after 3 s at most. No
  QML error, no auto-binding, no thread warning in the log.
- **qmllint:** 1 warning, the baseline row, unchanged (a first draft's
  `smokeGrab.length` was a second warning and was replaced with a
  comparison).
- **Guards:** architecture (radius, glass canvas, activation), language
  and documentation contracts OK.

## Decisions

- **Threading.** The provider is a `QQuickAsyncImageProvider` whose renders
  run on a `QThreadPool` of exactly one thread that never expires. That
  thread opens, uses and closes the provider's own `QPdfDocument`s (at most
  four, the most recently used kept, reopened when the file's size or
  modification time changes), so no `QPdfDocument` is ever used from two
  threads and none crosses threads: the window's `PdfDocument` stays the
  Qt thread's, for search, selection and links. A per-request load was
  rejected (it reparses the cross-reference table for every page and zoom
  step), and so was `QPdfPageRenderer` (a QObject answering through Qt
  signals, a poor fit for the provider's response objects). A larger pool
  buys nothing: QtPdf serializes every PDFium call of the process behind
  one recursive mutex (`QPdfMutexLocker`, in QtPdf's private
  `qpdfdocument_p.h`), so renders run one at a time anyway. Requests the
  view no longer needs are taken back out of the queue (`cancel`); quitting
  drops queued renders, closes the documents on their thread and waits at
  most 1 s.
- **Memory.** A render is bounded to 32 megapixels and 16 384 px a side,
  keeping the page's proportions (an A4 page at the ladder's top, 4×, on a
  2× screen fits; a poster is rendered smaller and scaled up by the Image
  item). The near-range building of pages and the image cache are
  unchanged.
- **The reading mode** is still the page view's layer effect, so it covers
  the provider's image with the paper and the annotations; the image keeps
  QtPdf's clear background over the paper sheet.
- **The model lives in `calcita-core`** (`annot::{model, read, coords}`),
  read with `lopdf =0.45.0` without default features (no async runtime,
  image decoder, font parser, date crates or thread pool), justified in
  the crate's manifest; it adds 44 crates to `calcita/Cargo.lock` and 29 to
  `celestina-rs/Cargo.lock`, and upgrades none. The file is read whole
  but bounded to 256 MiB; each stream `lopdf` inflates while loading to
  64 MiB, and the object streams of one read to 256 MiB together (an
  object filter inflates each one within what the read has left, so
  `lopdf` finds it plain and inflates nothing twice); a page's `/Parent`
  chain (for inherited attributes) is followed to 32 links and the form's
  `/Kids` tree to depth 32 with a visited set, while the page tree itself
  is `lopdf`'s own walk; a damaged annotation costs only itself. An
  encrypted document is refused even when it opens with the empty
  password, since Calcita never edits one. `AnnotError` gains `TooLarge`
  and `InflatesTooMuch` beside the plan's three causes. Pages are 0-based,
  as QtPdf counts them; an `AnnotId` is the object id, else `/NM`, else
  the place in `/Annots`, counting every entry. Page boxes and rotations
  follow PDFium: the `/MediaBox` (US Letter without an area), the
  `/CropBox` clipped to it when an area remains, `/Rotate` as whole
  quarter turns with a real number truncated. Text strings drop their
  byte-order mark and decode lossily rather than empty.
- **The window reads the model on a reader of its own** (`CalcitaDocument`,
  one thread per window, started on the first document): one read at a
  time, and of the requests waiting only the newest starts, so a document
  replaced before its read began is never read; results are
  generation-tagged so a read overtaken while it ran is dropped; the
  reader ends when the window's object goes. It publishes only
  `annotationCount`, which the smoke reports; `annotationsOnPage` reads the
  stored model. Nothing draws or edits it before CAL-2-B.
- **Fixtures** are written by `scripts/make-fixture-pdf.py` (byte-for-byte
  reproducible; the existing three come out identical), checked with
  Ghostscript and poppler; `calcita-core`'s tests read them from
  `calcita/tests/fixtures`, where the plan puts them.
- **The smoke's pixel check** goes through the real chain (binary,
  provider, Image, scene graph): `CALCITA_SMOKE_GRAB` makes the front
  document save its first page as drawn (`PageView.grabPage`, a signal
  each built page answers once its image is `Ready`, or after
  `grabWait`, 3 s), and the smoke reads the PPM with Python's standard
  library.
- The spec's redo key, written with the Spanish name of the Shift key,
  became `Ctrl+Shift+Z` (the same key sequence, as Qt spells it) so the
  spec passes the language contract.

## Limits

- Offscreen only: the grab proves QtPdf drew the annotations into the page
  image in the window; real-session sharpness, scrolling speed, rotated
  real documents and other programs' annotations are VAL-CAL-ANNOT.
- QtPdf draws no form widget (`/Widget`), with or without the flag (spike);
  forms are CAL-2-C.
- A document whose name is not valid UTF-8 opens in neither the window
  (QtPdf takes a URL) nor the provider (QtPdf takes a `QString` name),
  as before this unit; the provider's address keeps the bytes.
- The shutdown's bounded wait was not exercised (the smoke ends the
  process with `timeout`).
- The inflation budget goes through `lopdf`'s object filter, which
  `lopdf` does not call for an encrypted document; such a document is
  refused once loaded, and only the per-stream bound applies while it
  loads. Cross-reference streams are inflated while `lopdf` parses the
  cross-reference table, before any filter, each bounded alone and freed
  once parsed.
- A page already in the image cache keeps the file version it was
  rendered from when the file is replaced outside Calcita; CAL-2-B's
  revision in the page address covers it.

## Follow-up

VAL-CAL-ANNOT on the real session; CAL-2-B adds a revision to the page
address so a saved document, or one replaced outside Calcita, renders
again past the image cache.

## Landing

- **Base revision:** `38206e8b785bdd5a079f5906f4433213261ed207`
- **Check:** `production_artifact.py check calcita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita-android --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check hematita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check cuprita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check selenita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** calcita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:e72211740156ad25a70b5ab5f2b23ae0f2f18f6909f3fbc7996867964cf2ecef, verification_fingerprint sha256:f9d20b0e8c79b0f44510f0d69b76100379706156532663163313203591407b48; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:c1fcede975f5b54291234d537c30b8e5fe61376098b2aeb3dfbce46ffec9dfc1, verification_fingerprint sha256:8fe09da2a149a4273cd6ddb42fa9d3cd298b8b6576be4210c3caf9edebd79756; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:401fb7902a26d35568631a1a33f55c12aa3ad078dbe2e62c691d914c8453bc0f, verification_fingerprint sha256:8ba69713fe6c03d8752a130f181abb479727ba67aeda498eb7b24f90d5996006; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:f0328bc468cace838f45bfc89dd19513527e62bbb351715972809b3cbdc4053c, verification_fingerprint sha256:147871ad792a5b53ee0f7c665ce91ce25872de9ae2ac451bedc6bdde13f290e9; magnetita-android build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3296799662a178152291004cff76abc5f721ddbb56588ccbe4324370e0b497ae, verification_fingerprint sha256:ad5a9a7d84e572a6f89188ddfa0b1d5d3c6e1d3fb9d892a8930df0847d5ffcff; grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:fd629aee7cb7d1cce906741a26ba7bfe61d60b9ae07413e67ed770b4a4684c82, verification_fingerprint sha256:a0ae233ecf3c7e8e31ae8a81422875600a4fc516e5f7cf743920aa144f202ad8; fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:7ed666a34926ad99011d18d1d7516a5b40a2aa8247d9117024237bbf101de0d3, verification_fingerprint sha256:486daf1d3cb7a8c5aa967a456acbca05c06482c9d4e9d679a427858596a39806; hematita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:2b106b829abdd49519874d28cb602e0dbbace0b9d7dd3361a3fda8216e2c3d8f, verification_fingerprint sha256:daee35677d75e773025c27c23abffae482c9721a36f86ccaaab3fc9aaa047985; cuprita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:5277cfa9a5bec3ce3f7e5c81662d7c654c0f68287264c3c62299aad0c9ebe089, verification_fingerprint sha256:b2cdbda340eec5b42466a3720d8925e7541a52675bd29e80cf4a2414dacd6606; selenita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d3ac80447fbc4a3c421ed3770f8e16a2acc4a3808211f98628fb8b3e2231a694, verification_fingerprint sha256:c8ba469b7be4a5e67e02b153c9ef91270b75e25078150a99f4e639fb5281039b
- **Deploy:** after the push: calcita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
