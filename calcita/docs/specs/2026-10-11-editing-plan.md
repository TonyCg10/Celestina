# Calcita Editing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let Calcita add and edit text boxes, highlights and freehand drawings as standard PDF annotations, fill existing forms, and show every program's annotations, saving through the suite's two outcomes.

**Architecture:** QtPdf stays the renderer, through a C++ image provider with annotations on; `calcita-core` owns the annotation model, the coordinate conversion and the writing (`lopdf` incremental updates); the CXX-Qt adapter runs reads and writes on workers; QML draws editable annotations and form controls over the page.

**Tech Stack:** Rust, cxx-qt 0.9.1, Qt 6.12 QML + QtPdf (C++ image provider), `lopdf =0.45.0` (`default-features = false`), `siderita_ops` (Trash), celestina-style.

**Spec:** `calcita/docs/specs/2026-10-11-editing-design.md`

## Global Constraints

- Checkpoint `CAL-2`; ledger `calcita/docs/plans/archive/2026-10-11-cal-2-editing.md` (rows CAL-2-A, CAL-2-B, CAL-2-C), created by Task 1; prefix `calcita:` (session commits `calcita-maintenance:`); each unit lands `--kind milestone`.
- One new dependency: `lopdf = { version = "=0.45.0", default-features = false }` in `calcita-core`, justified in its `Cargo.toml`. No system package.
- Writing is always an incremental update: the original bytes are a prefix of the result (tests `cmp` it). Encrypted or unparseable documents refuse edit mode with a notice.
- Save outcomes: «Guardar ambas» (copy beside, default) and «Guardar solo la editada» (replace; original to the Trash via `siderita_ops`, never deleted); close with changes adds «Descartar».
- Coordinates go through one module (PDF user space ⇄ displayed points; `/CropBox` else `/MediaBox`; `/Rotate` 0/90/180/270), with the spike's formula: view to PDF `(u,v)` = 0:`(X,Y)`; 90:`(Y,H−X)`; 180:`(W−X,H−Y)`; 270:`(W−Y,X)`, then `x=x0+u`, `y=y0+H−v`; QtPdf rectangles are `normalized()`.
- Style: tokens only, glass grammar, motion 100–500 ms honouring `reducedMotion`, Spanish only in `qsTr()`/`message_es`, English development text, no blocking IO on the Qt thread, typed errors with `message_es`, no production `unwrap`/`expect`, every QML file in `build.rs`. Calcita's qmllint row 1 never grows; no `for` loops in QML handlers to dodge it.
- Before every commit: in `calcita/` `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test`, `cargo build --release --locked`; in `celestina-rs/` `cargo test -p calcita-core` + clippy; from the root `sh calcita/scripts/qml-tests.sh` ×3, `sh calcita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/calcita`, `bash scripts/qmllint-cxxqt.sh calcita`, `bash scripts/check-architecture-contract.sh`, `python3 scripts/check-language-contract.py`, `bash scripts/check-documentation-contract.sh`, `python3 scripts/commit_scope.py --check "<subject>"`. Never `git stash`.
- Verification offscreen with fixtures; the author's live checks are VAL entries; the throwaway spike at the session scratchpad `calcita-spike/` may be read for reference, never copied verbatim.

---

### Task 1: CAL-2-A — every annotation shown; the model and the coordinates

**Files:**
- Create: `calcita/cpp/page_provider.{h,cpp}` (a `QQuickImageProvider` keyed by document and page that renders with `RenderFlag::Annotations` at the requested size; registered from the CXX-Qt side like the clipboard shim), `celestina-rs/crates/calcita-core/src/annot/{mod.rs, model.rs, read.rs, coords.rs}` + `tests/{annot_read.rs, coords.rs}`, fixtures `calcita/tests/fixtures/annotated.pdf` (FreeText + Highlight + Ink + a Link, made by a checked-in generator script or a tiny Rust test helper) and `calcita/tests/fixtures/rotated.pdf` (four pages with `/Rotate` 0/90/180/270 and an offset box), `calcita/docs/plans/active/2026-10-11-cal-2-editing.md`, `calcita/docs/evidence/2026-10-11-annotations-shown.md`
- Modify: `calcita/qml/components/PageView.qml` (the provider's `Image` instead of `PdfPageImage`, keeping the paper, hits, selection, links, reading-mode layer), `calcita/build.rs`, `calcita-core/Cargo.toml` (`lopdf`), the controller (an `annotations(page)` read on a worker, unused by the UI until Task 2 except a count for the smoke), README/STATUS/ROADMAP (checkpoint CAL-2)/VALIDATION (VAL-CAL-ANNOT: another program's highlight now visible)

**Interfaces (Produces):** `calcita_core::annot::{Annotation { id: AnnotId, page: u32, kind: Kind, rect: Rect, color: Rgb, author: Option<String> }, Kind::{Text { contents, size_pt }, Highlight { quads: Vec<Quad> }, Ink { strokes: Vec<Vec<Point>> }, Other { subtype }}, AnnotId(ObjectId or /NM), read_annotations(path) -> Result<Vec<Annotation>, AnnotError>, Field { name, kind: FieldKind::{Text, Checkbox, Other}, value, rect, page }, read_fields(path)}`; `calcita_core::annot::coords::{PageBox { x0, y0, w, h, rotate }, to_view(PageBox, Point) -> Point, to_pdf(PageBox, Point) -> Point}`; `AnnotError` with `message_es` (encrypted, unparseable, io).

- [ ] Step 1: failing coords tests (four rotations, offset box, round trip) → implement → pass.
- [ ] Step 2: failing read tests over `annotated.pdf` (three kinds + one Other, colours, contents) and over `outline.pdf` (two Links as Other) → implement with `lopdf` → pass; an encrypted fixture (or a synthetic `/Encrypt` trailer) → `AnnotError::Encrypted`.
- [ ] Step 3: failing QML/smoke check that `annotated.pdf`'s highlight region is not white on the rendered page (sample the image provider offscreen) → the provider + PageView change → pass; reading mode, search, selection and links tests still green.
- [ ] Step 4: ledger, evidence, docs; checks; commit `calcita-maintenance: Show every annotation and read the annotation model`.

### Task 2: CAL-2-B — edit mode, the three tools, saving

**Files:**
- Create: `celestina-rs/crates/calcita-core/src/annot/{write.rs, appearance.rs, edit.rs}` + tests (`write_*`), `calcita/src/editor.rs` (QObject: edit session, undo stack, save worker), `calcita/qml/edit/{EditToolbar.qml, AnnotationLayer.qml, TextBoxItem.qml, InkItem.qml, HighlightItem.qml, SaveQuestion.qml}`, `calcita/tests/qml/tst_edit*.qml`, evidence `calcita/docs/evidence/2026-10-11-editing.md`
- Modify: the page provider (render from an in-memory copy with editable annotations `/F` Hidden while editing), `DocumentWindow.qml` (Ctrl+E, «Editar», reading mode off while editing), controller, `Cargo.toml` (`siderita-ops` path dependency, justified), docs (VAL-CAL-EDIT)

**Interfaces (Produces):** `calcita_core::annot::edit::{Edit::{Add(Annotation), Move(AnnotId, Rect), Recolor(AnnotId, Rgb), SetText(AnnotId, String), Delete(AnnotId)}, EditSet}`; `write::apply(original: &Path, edits: &EditSet, out: &Path) -> Result<(), AnnotError>` (incremental; prefix preserved); `appearance::{free_text_ap, highlight_ap, ink_ap}`; `write::hidden_copy(original, ids) -> Vec<u8>` for the edit-mode render; text limited to WinAnsi (`AnnotError::Unencodable(char)`).

- [ ] Steps: failing writer tests per kind (prefix `cmp`, reread equals the edit, a second edit appends again) → implement; failing editor/undo tests → implement; QML tests (tools, handles, delete, undo/redo, save question with the two outcomes and «Descartar») → implement; smoke: add a text box offscreen through the controller, save «Guardar ambas», reread finds it; checks; commit `calcita-maintenance: Add text boxes, highlights and drawing saved as annotations`.

### Task 3: CAL-2-C — forms

**Files:**
- Create: `calcita-core/src/annot/form.rs` + tests (stamped view copy; fill writer with `/V`, regenerated `/AP` for text, `/AS` for checkboxes), `calcita/qml/edit/FormLayer.qml`, fixture `calcita/tests/fixtures/form.pdf` (a text field and a checkbox), `tst_form.qml`, evidence `calcita/docs/evidence/2026-10-11-forms.md`
- Modify: the provider (show the stamped copy for a document with an AcroForm), editor (field edits join the same `EditSet` and save), docs (VAL-CAL-FORM)

**Interfaces (Produces):** `form::{stamped_view(original) -> Vec<u8>, Edit::Fill(FieldName, FieldValue)}`; radio buttons and choice lists read-only with a notice.

- [ ] Steps: failing tests (stamped copy renders the value through QtPdf offscreen; fill writes `/V`+`/AP`/`/AS`, prefix preserved, reread) → implement; QML (a text field and a checkbox over their widgets; values flow into the edit set) → implement; checks; commit `calcita-maintenance: Fill text fields and checkboxes in forms`.
