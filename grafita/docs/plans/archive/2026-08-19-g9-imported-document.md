# G9-G13 — the imported document

- **Opened:** 2026-08-19
- **Closed:** 2026-08-19
- **Plan ID:** g9-imported-document
- **Status:** done
- **Successor:** none; the authorised sequence ends here
- **Scope:** grafita
- **Implementation checkpoint:** G9, and G10-G13 which it grew to cover
- **Author-validation checkpoint:** `VAL-G9` in [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

Correcting one word in a `.docx` means leaving the session for a dedicated
application, because Grafita has one kind of document — bytes it can reproduce
exactly — and a container is not that. A second kind, whose contract is that
every part the author did not edit is written back unchanged, carries the text
of a container in and out without Grafita ever creating structure. That is the
difference between an editor that reaches these files and a word processor.

## Tangible outcome

A real `.docx` opens, one word is corrected, and the saved file opens in its
original application with every style, image and property intact. Nothing else
in the file differs from the original: the bytes of every part the author did
not touch are the bytes that were there.

## What this plan ended up covering

It opened as G9 — the imported document, demonstrated on `.docx` — and closed
as the whole authorised sequence. G10 through G13 are the same model applied to
more formats and one commit's worth of files; splitting them into four plans
would have described an order that no inventory could divide, since every one
of them edits `import.rs`, the open path and the same tests.

| Checkpoint | Delivered |
|---|---|
| G9 | the imported contract, the ZIP container that reproduces a file, the `word/document.xml` anchors, `.docx` |
| G10 | `.odt` and `.epub`, which forced the anchor model to grow a second rule; `.pptx` and `.xlsx` decided out |
| G11 | `.rtf`, which has no container at all |
| G12 | PDF: reading, correcting text in place, and form fields |
| G13 | text inside a gzip wrapper |

## Scope

- `grafita/docs/contracts/document-import.md`, the canonical statement of the imported
  contract that the roadmap currently carries in a table.
- A ZIP container in `grafita-core` that reproduces a file rather than
  repacking it: untouched entries are copied as the exact bytes they occupy,
  headers and order included, and only a replaced entry is written afresh.
- A `word/document.xml` reader that locates the text-carrying spans by byte
  offset, so the flat text an author edits maps back into the XML that was
  already there, and everything between the spans stays untouched.
- The imported document kind: a flat text projection, its anchors, its edit
  and its save, refusing rather than guessing when a replacement cannot be
  placed.
- The host presentation that says a tab is imported, and the save path that
  writes the container atomically like any other file.
- The round-trip harness the later container checkpoints reuse: open a real
  document, save it, and compare bytes.

## Exclusions

Out of scope: `.odt`, `.epub`, `.rtf`, `.pdf` and compressed text, which are
G10 onwards and consume this one's model; creating or editing formatting,
styles, tables, fonts or page layout; `.pptx` and `.xlsx`; and any change to
the native document's byte-preserving contract, which this checkpoint adds
beside rather than modifies.

## Build order

1. The container: parse, read one entry, and rewrite with a replacement,
   proving an untouched rewrite is byte-identical.
2. The `word/document.xml` anchors: flat text out, spliced text back in, with
   the XML entities the format uses.
3. The imported document kind over those two, with its edit and save.
4. The host: an imported tab that says what it is, and the save that writes it.

## Implementation exit

- `cargo test -p grafita-core` covers a rewritten archive that changed nothing
  being byte-identical to its input; a replaced entry leaving every other
  entry's bytes untouched; the anchors round-tripping a document with entities,
  several runs and non-ASCII text; and an edit that cannot be placed being
  refused rather than approximated.
- Rust format and Clippy for the crate and both hosts; QML lint at its
  baseline; `bash scripts/check-architecture-contract.sh`.
- `grafita/scripts/complete-production.sh` and, because `grafita-core`
  changes, `siderita/scripts/complete-production.sh`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| G9-A | `grafita:` | done | [inventory](../../inventories/2026-08-19-g9-imported-document/G9-A.numstat.tsv) | 40 files, +6830/-120 | The whole sequence: the imported contract and its written form, a ZIP container that reproduces a file rather than repacking it, the anchored text model behind `.docx`, `.odt` and `.epub`, `.rtf`, a PDF reader that reads a font's own encoding rather than its raw bytes, an in-place corrector with form fields, gzip-wrapped text, the host presentation of an imported tab, and the crate-wide plumbing (`Document::save_request` becoming `SaveIntent`, the fallible `to_bytes`, `open`/`open_with` sharing one read) that both this and `G8-A` need to compile — the two checkpoints were written as one continuous change and land in the same commit as a result | [documents Grafita used to refuse](../../evidence/2026-08-19-documents-grafita-used-to-refuse.md) | `VAL-G9` |

## Decisions and rollback

The container is written here rather than taken from the `zip` crate, which
Siderita already uses. That crate creates and extracts archives; it does not
promise that reading a file and writing it back produces the same bytes, and
that promise is the entire imported contract. `flate2` is taken, for the
deflate codec alone: a compression format is not something to reimplement, and
the crate is already vendored in this repository for Siderita's archives.

An untouched save is byte-identical for a reason worth stating: the save path
already refuses to rewrite a clean document, so the strong case is the edited
one, where every entry except the edited part is copied as the bytes it was.

Text is located by byte offset in the XML rather than parsed into a tree. A
tree would have to be serialised again, and no serialiser reproduces someone
else's whitespace, attribute order and namespace prefixes. Offsets leave every
byte between the spans exactly where it was, which is what the contract asks
for.

Rollback is per slice: nothing here changes how a native document opens, edits
or saves, so reverting any unit returns the previous refusal of a container.
