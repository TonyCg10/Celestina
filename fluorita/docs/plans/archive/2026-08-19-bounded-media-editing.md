# Bounded media editing

- **Opened:** 2026-08-19
- **Plan ID:** bounded-media-editing
- **Status:** done
- **Closed:** 2026-08-20
- **Successor:** none; the roadmap names no active checkpoint, and the work
  this plan's single unit carried beyond F7 is described in the roadmap's
  F8-F15 sections
- **Scope:** fluorita
- **Implementation checkpoint:** F7
- **Author-validation checkpoint:** VAL-FLU-EDIT

## Hypothesis

The edits the author actually performs on the pictures this library shows —
turning one the right way up, cutting it down, writing a word on it, covering
something that should not be seen — can be delivered inside the viewer F6
already built, with the toolkit that already reads these files, without adding
an encoder to the dependency closure and without the application becoming an
editing suite.

## Tangible outcome

A picture opens in the immersive viewer and enters an edit mode. It can be
rotated, flipped, cropped and resized; text, freehand strokes, lines, arrows,
rectangles, ellipses, a highlighter and a redaction can be placed on it, and
each of those remains an object that can be selected, moved, resized, deleted
and undone. Saving offers two outcomes and asks nothing else: a copy beside the
original, after which reopening the item restores the objects for further
editing, or a replacement, which flattens the result, writes it atomically and
sends the original to the desktop Trash.

## Scope

- The decision record that redefines the product limit, and the README change
  it rules: `tag editor` leaves the exclusion list and *editing suite* enters
  it.
- The capability matrix in `fluorita-core`: what a given item admits, and
  whether each operation is *lossless* — reordering the original bytes — or
  *raster* — producing a new image.
- The edit stack in `fluorita-core`: canvas transformations, then annotation
  objects in the coordinates of the resulting canvas; undo, redo, validation
  and a bounded persisted form.
- The write path in `fluorita-engine`: the fixed output-format rule, the
  budgets checked before allocation, atomic replacement, the Trash path for a
  replaced original, and the catalogue and thumbnail reconciliation that
  follows only a confirmed write. The engine drives it through a narrow
  rasteriser seam it is handed, so the crate stays pure and testable against a
  fake.
- The rasteriser itself in the application's `cpp/` seam: the toolkit draws the
  composition, because the toolkit is what can read these formats and lay out
  text. `fluorita-core` and `fluorita-engine` never see a `QImage`.
- The stack's persistence beside the catalogue, keyed by media identity, and
  its invalidation when the file it describes changes underneath it.
- The edit mode in `qml/`: tool selection, live preview, direct manipulation of
  objects, keyboard reach, and confirm or discard.

## Exclusions

- Any encoder. Nothing here re-encodes video or audio, and nothing links a new
  media library. The image writer is the toolkit that already reads these
  files.
- Layers, masks, blend modes, per-layer opacity, configurable brushes,
  gradients, cloning, non-rectangular selection, curves and levels, and rich
  text with more than one style inside one box.
- Annotating video. A frame extracted from a video is an image and inherits
  everything here; the moving surface is not annotated.
- Metadata editing, stream-copy trimming, frame extraction, track removal and
  batch application. Each is authorised in intent by the roadmap and opens as
  its own checkpoint once this stack and writer exist.
- Any change to Siderita's embedded surface. Editing belongs to the standalone
  application.
- A format dialogue. The output format is a fixed rule, not a question.

## Build order

1. **The contract.** The decision record and the README's replaced limit, with
   F6 closed and its plan archived. Documentation only; no code and no UI.
2. **The model.** `fluorita-core` gains the capability matrix and the
   composable stack: what an item admits and in which class, transformations,
   annotation objects, the canonical order between them, undo and redo, and the
   bounded serialised form. The matrix answers before anything is read, so an
   item that cannot be edited says so without allocating. Pure, tested, and
   with no writer behind it yet.
3. **The writer.** The seam first: a narrow rasteriser contract the engine
   calls and the application implements over the toolkit, because a pure crate
   cannot hold a `QImage` and the toolkit is what reads these formats. Then
   `fluorita-engine` lands the result: the fixed format rule, the existing byte
   and pixel budgets checked before allocation, atomic replacement,
   `siderita-ops` for a replaced original, and the two outcomes as distinct
   confirmed results. Copy preserves the stack; replace flattens it and records
   that the item is no longer reopenable, because the base it would rebuild
   from is gone. The engine's own tests run against a fake rasteriser, so the
   write path is proved without a display.
4. **Transformations.** Rotate, flip, crop and resize over the stack, with
   rotation and flip taken as orientation metadata when the file can carry them,
   so the most common edit reorders bytes rather than pixels.
5. **Annotation.** Text, freehand, line and arrow, rectangle and ellipse,
   highlighter and redaction, as objects in image coordinates. Selection,
   movement, resizing and deletion go through the same undo the transformations
   use.
6. **The surface.** Edit mode inside the immersive viewer: the icon-first
   anatomy with the uniform hover circle, live preview of the stack, every tool
   reachable and operable by keyboard with role, name and state exposed, and
   confirm or discard as the only exits. Colours, radii and motion come from
   `CelestinaTheme`; the product copy is Spanish under ADR 0007.

## Implementation exit

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
bash scripts/qmllint-cxxqt.sh fluorita
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked   # fluorita-core, fluorita-engine, fluorita
bash fluorita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

`fluorita-core` and `fluorita-engine` change, so Siderita completes too even
though its surface gains nothing: it carries the same verified bytes.

The engine tests must cover the write path against the filesystem, not only the
model: a copy that lands beside the original, a replace whose original is in the
Trash and whose destination is confirmed before the source moves, a write that
fails partway and leaves the original untouched, a stack applied to an image
whose name is not valid UTF-8, and an item over each budget refused before
allocation. The offscreen smoke constructs edit mode in both the normal and the
reduced-motion configuration. Perceived drawing, pointer precision, real display
scale and the desktop's own Trash belong to `VAL-FLU-EDIT`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| F7-A | `fluorita:` | done | [inventory](../../inventories/2026-08-19-bounded-media-editing/F7-A.numstat.tsv) | 61 files, +14987/-120 | Rule what editing means in Fluorita and replace the `tag editor` limit with the editing-suite limit; answer what an item admits and in which class; model the composable stack with undo, redo and the preview mapping; rasterise through a toolkit seam and land the result with copy and replace as its two confirmed outcomes; remember the recipe behind a copy beside the catalogue; and compose the edit mode inside the immersive viewer | [evidence](../../evidence/2026-08-19-bounded-media-editing.md) | `VAL-FLU-EDIT` |
| F7-B | `fluorita:` | done | [inventory](../../inventories/2026-08-19-bounded-media-editing/F7-B.numstat.tsv) | 11 files, +539/-270 | Archive this plan and make the project's documents describe what its commit actually delivered, rather than only the editing surface it was written for | [evidence](../../evidence/2026-08-20-what-shipped-and-what-nothing-could-reach.md) | `None` |

`F7-B` is an administrative unit added after `F7-A` landed: it owns the archive
transition this plan could not carry itself, and the documents that understated
what its commit delivered. A corrective unit follows it for the defect that
shipping revealed — two capabilities reached production with nothing in the
interface able to call them.

One unit because it was one worktree. The plan opened with four — contract,
model, writer, surface — and they never became four deliveries: the seam's shape
changed while the surface was being built (the geometry crossing it became one
`x,y,…` spelling because the surface needed to read its own rows back), the
engine gained its `Bin` seam only when the ordering had to be *proved* rather
than described, and the toolkit's real API decided how much of the drawing could
live in Rust. No part of it was ever going to be committed alone, and four
inventories over one indivisible diff would be bookkeeping rather than history.

It closes as `fluorita-milestone` with the exact MINOR transition, `1.2.4` →
`1.3.0`.

**This unit's inventory was computed when the author asked for the commit**, with `HEAD` at that moment as its base. An inventory is compared against the worktree until the commit lands, so any earlier one written while the same crates — `fluorita-core` and `fluorita-engine` above all — were still moving was stale before it could be staged. Writing one early was tried and withdrawn for exactly that reason.

**Two paths this batch changed will stay outside it**, because neither belongs
to the `fluorita:` prefix.

[ADR 0009](../../../../docs/decisions/0009-editing-without-an-encoder.md) and the
decisions index are suite documentation and land under `suite:`, exactly as
ADR 0006 did for F5: a decision that redefines a product's contract is not
carried inside that product's own delivery. It has no product version and no
production build of its own.

**The version-history row is in this inventory.** `docs/version-history.tsv`
is shared by the six products, and when this unit was first measured another
session had a row of its own staged beside this one. Those deliveries landed
first, so the file could be claimed here without claiming their work: the row
for `1.3.0` is the last one in the file and the only one this commit adds.

The boundary between the crates is the architecture direction and it survived
the consolidation: what an edit *is* has no filesystem and no toolkit in it, and
what an edit *costs* — bytes, pixels, a worker, a Trash entry — has no domain
rules in it. One commit does not mean one module.
