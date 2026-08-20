# Evidence: 2026-08-19 documents Grafita used to refuse

- **Date:** 2026-08-19
- **Scope:** checkpoints `G9` through `G13`, unit `G9-A`; plan
  [g9-imported-document](../plans/archive/2026-08-19-g9-imported-document.md)
- **Environment:** Arch Linux, `rustc 1.97.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.2`. Independent readers used for validation: `qpdf 12.4.0`,
  `pdftotext` (poppler), and the `zip` crate as a third-party archive writer
- **Artifact:** `grafita/target/release/grafita`,
  deployed to `/home/toni/.local/bin/grafita`; `siderita/target/release/siderita`
  likewise. Both manifests record `verified = true`

## What changed

Grafita opens five kinds of document it used to refuse, edits the text in them,
and writes each one back with everything the author did not touch untouched.

| Module | What it is |
|---|---|
| `grafita-core/src/container.rs` | a ZIP container parsed and rebuilt rather than repacked: untouched members are copied as the exact bytes they occupy |
| `import/part.rs` | text located by byte span inside XML, driven by per-format rules rather than a parser per format |
| `import/epub.rs` | the package document and spine, so chapters read in the order the book declares |
| `import/rtf.rs` | rich text: its code page, its escapes, its `\uc` fallbacks |
| `import/pdf/` | objects, cross-references (table, stream and object stream), text extraction through `ToUnicode`, in-place correction, form fields, and the incremental update that appends them |
| `import/gzip.rs` | text inside a gzip wrapper, classified by the same probe every other file goes through |
| `import.rs` | the imported document itself: one text, one save, five formats |

The contract is written at
[document import](../contracts/document-import.md).

## Procedure

```sh
cargo test -p grafita-core                                           # in celestina-rs/
cargo fmt --all --check
cargo clippy -p grafita-core --all-targets --offline -- -D warnings
cargo clippy --manifest-path grafita/Cargo.toml --all-targets --offline -- -D warnings
cargo clippy --manifest-path siderita/Cargo.toml --all-targets --offline -- -D warnings
bash scripts/qmllint-cxxqt.sh grafita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash grafita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

## Result

- **Exit:** 0 for every command.
- **Observed:** 165 tests passed, 0 failed (92 unit, 4 container, 27 document,
  14 imported, 30 session). Format, Clippy, QML lint at its 62-warning
  baseline, architecture, colour, contrast and language contracts all OK. Both
  hosts built once, verified as those exact bytes, and deployed.

### What was proved, and against what

| Claim | How |
|---|---|
| A rewritten container reproduces its input byte for byte | `rewriting_nothing_reproduces_the_file_byte_for_byte`, over an archive written by the `zip` crate |
| Replacing one member leaves every other member's bytes and compression as they were | `replacing_one_part_leaves_every_other_part_exactly_as_it_was`, re-read with the `zip` crate |
| A `.docx` corrected keeps its styles | `word/styles.xml` compared byte for byte after a save |
| An `.odt` keeps its outline level, paragraph style and spans | asserted on the saved `content.xml` |
| An `.epub` reads in spine order | its chapters are stored in the archive in reverse of the reading order |
| Rich text keeps its font table, colour table, ignorable destinations and `\pard` | asserted on the saved file |
| An accented letter goes back as an escape, not a raw byte | `\u231?` asserted after a save |
| Adding a paragraph is refused before anything is written | the file on disk compared with the original after the refusal |
| A PDF reads | every PDF on the machine: 11 of 12 read, the twelfth correctly reported as a scan with no text |
| A PDF correction lands where it should | the placement invariant — every span holds the string that produced its text — checked over 114 949 placements in three documents |
| A corrected PDF is valid and says the new thing | `qpdf --check` passes and `pdftotext` reads the correction, on three documents from `/usr/share/doc` |
| A PDF correction appends rather than rewrites | the original file is the literal byte prefix of the saved one |
| A font that cannot draw a character refuses | `WavPack5FileFormat.pdf`: "the font this text is drawn with has no 'R'" |
| A form field is shown and takes a value back | a one-page `AcroForm` document assembled in the test |
| Compressed text round-trips its text exactly | decompressed with `flate2` and compared |

## What a PDF's text now says, measured

The first PDF reader this checkpoint delivered lost characters silently. A font
with no `ToUnicode` map had its codes read as raw bytes, so `Specification`
came out as `Specication`: the ligature's code is 12, which is a form feed, and
it vanished. Words ran together for a second reason — a PDF moves the pen
between words instead of writing a space, and the gap this crate called a word
break was too wide to catch most of them.

Both are fixed by reading what the font itself declares: its `/Encoding`, its
base encoding and its `/Differences` array, which names the glyph each code
draws. The glyph names are the Adobe list for Latin text; the 246 accented ones
are derived by asking Unicode whether "base letter plus accent" exists rather
than being typed out, and a name with no precomposed character is left out
instead of guessed at.

The word-break gap is not a convention either. It was measured: for every PDF
on the author's machine, the text this crate reads was compared word by word
with the text `pdftotext` reads from the same file.

| Document | Before | After |
|---|---|---|
| `speexdsp/manual.pdf` | 63.1% | 89.5% |
| `ghostscript/Ghostscript.pdf` | 86.3% | 98.6% |
| `ijs/ijs_spec.pdf` | 91.7% | 97.8% |
| `ghostscript/GS9_Color_Management.pdf` | 85.2% | 89.8% |
| `lirc/PCB.pdf` | 6.0% | 44.7% |
| `wavpack/*` (three) | 97-99% | unchanged |
| **mean over 11 documents** | **73.0%** | **83.4%** |

No document got worse. The threshold peaks at 150 thousandths of an em; below
about 120 the kerning inside ordinary words starts reading as spaces and
agreement falls away again.

A ligature is kept as the one character the document draws — `U+FB01`, not
`fi` — where `pdftotext` splits it. That is deliberate and it is an editor's
reason: a document nobody edited has to write back the code it came from, and
two characters cannot go back into one code.

## Limits

- **Nothing here creates structure.** Adding or removing a paragraph is
  refused; a character the font has no code for is refused; a PDF is never
  re-laid-out, so replacement text longer than what it replaces runs past where
  the old text ended.
- **A `.gz` is imported, not native.** The text round-trips exactly and the
  compression does not: the same text compressed at another setting is other
  bytes. `.zst`, `.xz` and `.bz2` are absent because their codecs are not
  vendored in this repository.
- **PDF text extraction is as good as the fonts allow.** A font with no
  `ToUnicode` map is read as Latin, which is right for the simple fonts that
  omit it and a guess for the rest; a ligature with no map comes out missing.
  A scanned page holds no text at all and says so.
- **A PDF correction that spans two content streams is refused**, because one
  half would have to move to the other's page.
- **Form appearances are not drawn.** The document is asked to have them
  rebuilt through `NeedAppearances`, which every viewer honours; a viewer that
  ignored it would show the old value until something redrew it.
- **`.pptx` and `.xlsx` are reachable by the same path and deliberately out.**
- This is automated evidence. Whether these documents look right in the
  author's own session is `VAL-G9`.

## Follow-up

`VAL-G9` in [VALIDATION.md](../../VALIDATION.md). No checkpoint is open.
