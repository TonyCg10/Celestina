# Grafita implementation roadmap

- **Status:** planned
- **Active implementation checkpoint:** none
- **Authorised sequence:** G8-G13, opened by author decision on 2026-08-19 and
  closed the same day. Nothing is in flight; a new checkpoint needs a new
  falsifiable problem and the author's word
- **Related author validation:** `VAL-G7`, `VAL-G8` and `VAL-G9`, all pending
  and none blocking; earlier completed observations and deliberate exclusions
  are in [VALIDATION.md](VALIDATION.md)

## G7 — reading comfort, closed 2026-08-19

Its falsifiable problem was a surface that showed an encoding label nobody
could act on while withholding a line to refer to, a scroll position, room
against the frame and any way to change the text size. The delivered result is
the numbered, inset editing surface whose text size and wrap mode are chosen
with `Ctrl +` / `Ctrl −`, the `Ctrl` wheel, `F10` and `Alt + Z`, and survive a
relaunch.

Units `G7-A` through `G7-D` are in the archived
[plan](docs/plans/archive/2026-08-04-g7-reading-comfort.md). The checkpoint's
implementation exit ran on 2026-08-19 and both consumers of `grafita-core` now
carry the verified bytes: the
[delivery record](docs/evidence/2026-08-19-g7-production-completion.md).
`VAL-G7` stays pending in the author's lane and did not block this closure.

## The second document contract

G8 stays inside the contract Grafita has always had. G9 onwards cannot, and the
answer is not to weaken it but to name a second one. From G9 a document is one
of two kinds, and every checkpoint below states which kind it produces.

| | Native document | Imported document |
|---|---|---|
| What it is | a text file | a container: docx, odt, epub, rtf, pdf, compressed text |
| What is edited | the bytes | the text inside the structure |
| Save contract | untouched content reproduces the original bytes | every part the author did not edit is written back unchanged: styles, images, metadata, parts Grafita does not understand |
| Refusal | a save that cannot reproduce the bytes is refused | a save that cannot write a part back unchanged is refused |

The imported contract is what keeps Grafita an editor. It never creates
structure: no styles, no tables, no layout, no fonts. It carries text in and
out of a structure somebody else authored. An imported document is never
presented as a native one, and the two never share a save path.

Because the imported contract can destroy a file where the native one cannot,
every container checkpoint below carries the same non-negotiable evidence: open
a real document, save it without editing anything, and prove the result is
byte-identical to the input. A checkpoint whose formats do not all pass that
round trip does not close.

## G8 — text Grafita already refuses, closed 2026-08-19

Its falsifiable problem was that files which are unambiguously plain text were
rejected: a `windows-1252` note, a `latin-1` subtitle, a UTF-16 file with no
mark. The delivered result is thirty single-byte encodings, four multi-byte
ones, unmarked UTF-16 and UTF-32, opened by naming them and verified to write
the file back byte for byte before anything may be edited. A save that would
lose a character the encoding cannot carry is refused instead.

Nothing is detected: the mark reader is unchanged and no byte pattern concludes
an encoding. The author names one with `Ctrl + E` or the footer button that
shows the document's current encoding.

The unit is in the archived
[plan](docs/plans/archive/2026-08-19-g8-text-already-refused.md); the
[record](docs/evidence/2026-08-19-encodings-a-file-cannot-declare.md) carries
the tests, the production flow and the limits — chiefly that naming an encoding
cannot detect a wrong choice, only a lossy one. `VAL-G8` is pending.

G8 and G9-G13 below were written in one continuous pass and landed in a single
verified delivery — Grafita 1.2.0 — rather than as two separable builds; the
falsifiable problems and the exit evidence are still tracked as two checkpoints
because that is the honest shape of what each one proves, independent of how
many commits carried it.

## G9-G13 — the imported document, closed 2026-08-19

The five checkpoints were authorised as one sequence and delivered as one: each
is the same model applied to another format, over the same files. Their plan is
[G9-G13 — the imported document](docs/plans/archive/2026-08-19-g9-imported-document.md)
and their record is
[documents Grafita used to refuse](docs/evidence/2026-08-19-documents-grafita-used-to-refuse.md).
The contract they introduced is written at
[document import](docs/contracts/document-import.md).

| Checkpoint | Falsifiable problem | Delivered |
|---|---|---|
| G9 | correcting one word in a `.docx` meant leaving the session | the imported contract, a ZIP container that reproduces a file rather than repacking it, the anchored text of `word/document.xml`, and the save that writes a container atomically |
| G10 | the same model, on formats that do not put text where WordprocessingML does | `.odt` and `.epub`; the anchors grew a second rule — *all character data except what these elements hold* — and an `.epub`'s chapters read in spine order as one document |
| G11 | `.rtf` is text with brace markup and was refused anyway | rich text, its code page read from the file itself and its `\par` understood as ending a paragraph rather than starting one |
| G12 | fixing a date in a PDF sent the author to another application | reading (validated against every PDF on the author's machine), correcting text in place through the font's own map, and form fields |
| G13 | a `.txt.gz` was refused | text inside a gzip wrapper, imported rather than native because compression is not reproducible |

Grafita 1.2.0 carries G8 and G9-G13 together, and so does the current Siderita.
`VAL-G9` is pending.

What none of it does is create structure. Adding or removing a paragraph is
refused, a character the font has no code for is refused, and a PDF is never
re-laid-out. Those are the limits the contract states, and each one is a
refusal that leaves the file untouched.

`.pptx` and `.xlsx` were decided out in G10: the same path reaches them, and
editing a spreadsheet as flat text is a bad answer. `.zst`, `.xz` and `.bz2`
are out of G13 for a plainer reason — their codecs are not vendored here, and
G13's shape would take them the day they are.

## Permanently out of scope

These are decisions, not gaps, and they do not reopen without the author
saying so:

- binary `.doc` — the OLE compound format, an effort out of proportion to the
  need;
- OCR of scanned PDFs;
- creating or editing formatting, styles, tables, fonts or page layout in any
  imported document;
- the IDE features the root contract already excludes: project trees, build
  runners, debuggers, LSP, terminals, plugin platforms.

## Implementation exit

Close implementation for a checkpoint when its code and focused domain tests
pass and every affected deployable host completes its registered production
flow. Grafita-only work uses `grafita/scripts/complete-production.sh`; a shared
`grafita-core` change also uses `siderita/scripts/complete-production.sh`.
Each command builds its release once, verifies those exact bytes and deploys
them to the author's normal test destination. Every checkpoint from G9 onwards
changes `grafita-core`, so every one of them completes both hosts.

A perceptible or manual result goes to `VALIDATION.md` and never keeps the
implementation checkpoint open.

## Closed evidence

The completed G0-G6 implementation, measurements, fixes and real-session
observations are preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
