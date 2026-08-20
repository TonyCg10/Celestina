# Evidence: 2026-08-19 the encodings a file cannot declare

- **Date:** 2026-08-19
- **Scope:** checkpoint `G8`, unit `G8-A`; plan
  [g8-text-already-refused](../plans/archive/2026-08-19-g8-text-already-refused.md)
- **Environment:** Arch Linux, `rustc 1.97.1`, `c++ (GCC) 16.2.1`,
  `cmake 4.4.2`. Source work with compilation, lint, unit tests and the
  canonical production flow of both hosts
- **Artifact:** `grafita/target/release/grafita` at version 1.2.0,
  `sha256:f6ebbe8f095218cca908f774d0680a1fd0efc191124399845226f93380ec1837`;
  `siderita/target/release/siderita`,
  `sha256:de5e527d519ceadfdb142084247641a05c279ee333291f1f1b8f1a66773ec9bc`.
  Both manifests record `verified = true`

## What changed

Grafita reads thirty-four encodings it used to refuse, and the file itself no
longer has to prove which one it is.

- `grafita-core/src/encoding/tables.rs` — thirty single-byte encodings
  (`windows-1250`…`1258`, `ISO-8859-1`…`16`, `KOI8-R`/`U`, `IBM-437`/`850`/
  `866`, `Macintosh`), generated from CPython's codecs by
  `tools/generate-encoding-tables.py single`. The generator refuses a table
  whose low half is not ASCII, whose bytes map to a surrogate or outside the
  BMP, or whose distinct bytes share a character, so reversibility is checked
  rather than reviewed.
- `grafita-core/src/encoding/multibyte.rs` — `Shift-JIS`, `GBK`, `EUC-KR` and
  `Big5`, generated the same way. These are *not* bijective, and the module
  says so: a character can have two encodings and a pair can have none.
- `Encoding` gains `SingleByte`, `MultiByte`, unmarked `UTF-16` LE/BE and
  `UTF-32` LE/BE. The mark reader is untouched, so nothing new is ever
  concluded from bytes. `UTF-32` is named-only for a stated reason: its
  little-endian mark begins with the `UTF-16 LE` mark, and this crate does not
  guess between them.
- `open_with` reads a file as the encoding a caller names, then re-encodes what
  it decoded and compares it with the bytes it read. That single check is what
  admits a non-bijective encoding without weakening the contract.
- `Encoding::encode` returns `Result`. A character the encoding has no byte for
  is `SaveRefusal::Unrepresentable`, carried by the new `SaveIntent` and
  presented by both hosts. Nothing is written.
- `DocumentSession::open_with` / `reopen_with`, `Job::OpenWith`, and in the
  application `encodingNames`, `encodingIndex`, `encodingRetry`,
  `encodingPrompt`, `requestEncodingChooser`, `cancelEncodingChooser` and
  `chooseEncoding`.
- `qml/components/EncodingDialog.qml`, opened by `Ctrl + E` or by the footer
  button that now shows the document's encoding — the label G7 removed when it
  was only a statement, back because it asks for something.

## Procedure

```sh
cargo test -p grafita-core                                          # in celestina-rs/
cargo fmt --all --check
cargo clippy -p grafita-core --all-targets --locked -- -D warnings
cargo clippy --manifest-path grafita/Cargo.toml --all-targets --locked -- -D warnings
cargo clippy --manifest-path siderita/Cargo.toml --all-targets --offline -- -D warnings
bash scripts/qmllint-cxxqt.sh grafita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
python3 scripts/version_tool.py bump grafita milestone --unit G8-A \
  --summary "Add the encodings a file cannot declare"
bash grafita/scripts/complete-production.sh
bash siderita/scripts/complete-production.sh
```

## Result

- **Exit:** 0 for every command.
- **Observed:**

| Command | Result |
|---|---|
| `cargo test -p grafita-core` | 92 unit, 27 document and 30 session tests passed, 0 failed |
| `cargo fmt --all --check` | clean, crate and both hosts |
| `cargo clippy … -D warnings` | no Rust diagnostic in the crate or either host |
| `scripts/qmllint-cxxqt.sh grafita` | OK at the 62-warning baseline, which this work did not raise |
| `scripts/check-architecture-contract.sh` | sealed colour, contrast, QML visual and architecture all OK |
| `scripts/check-language-contract.py` | OK, 157 ratcheted files, none of them new |
| `version_tool.py bump grafita milestone` | 1.1.3 → 1.2.0 with its `docs/version-history.tsv` row |
| `grafita/scripts/complete-production.sh` | built once at 1.2.0, verified, smoke OK with the binary alive 8 s and no QML error, deployed to `/home/toni/.local/bin/grafita`, status `current and verified` |
| `siderita/scripts/complete-production.sh` | built once, verified, QML test runner 72 passed / 0 failed, smoke OK, deployed to `/home/toni/.local/bin/siderita`, status `current and verified` |

The tests are the argument rather than a sample of it:

| Test | What it proves |
|---|---|
| `every_table_maps_every_byte_it_assigns_back_to_itself` | All 256 bytes of all thirty single-byte tables: each either decodes to one character that re-encodes to that byte, or is refused as unassigned |
| `a_table_refuses_the_character_it_has_no_byte_for` | An emoji typed into `windows-1252` is a refusal, and the same character is fine in UTF-8 |
| `an_unassigned_byte_is_refused_where_it_sits` | `ISO-8859-7`'s `0xAE`, which the standard assigns nothing |
| `a_table_is_never_concluded_from_the_bytes` | The mark reader knows none of the tables |
| `a_multi_byte_encoding_reads_and_writes_the_text_it_carries` | Shift-JIS and GBK over the same text, with their different bytes |
| `a_multi_byte_stream_that_is_not_this_encoding_is_refused` | A lead byte at end of file, an unassigned pair, and a character with no sequence |
| `a_named_encoding_opens_what_the_bytes_cannot_prove` | A `latin-1` file: refused alone, opened when named, saved back identically |
| `a_named_encoding_that_would_not_write_the_file_back_is_refused` | An unassigned byte and an odd-length UTF-16 file |
| `unmarked_wide_text_opens_only_when_it_is_named` | Unmarked UTF-16 LE/BE and UTF-32 LE/BE: binary to the probe, exact once named |
| `a_multi_byte_file_opens_named_and_saves_back_identically` | A Shift-JIS file end to end |
| `naming_an_encoding_opens_what_the_probe_refused` | The session flow the window drives: decline, name, open |
| `a_dirty_document_does_not_get_reread_in_another_encoding` | Unsaved work is never re-read away |
| `a_terminal_capture_full_of_escapes_is_text_and_a_program_is_not` | The heuristic this checkpoint wrongly accused |

## Limits

- **Naming an encoding cannot detect a wrong choice.** The same bytes are often
  valid in two encodings: a Shift-JIS file opens as GBK, shows different
  characters, and writes back unchanged. The byte comparison cannot see that
  and does not pretend to. What is guaranteed is that no byte is lost, not that
  the author picked the language the file was written in. This is pinned by
  `a_multi_byte_file_opens_named_and_saves_back_identically`.
- **A file that fails verification is refused, not shown.** Reading it would
  still be useful; a document that cannot be saved is what G9's imported
  document introduces, and this checkpoint has no such kind.
- Seven of the thirty single-byte tables — `windows-1253`, `windows-1255`,
  `windows-1257`, `ISO-8859-3`, `-6`, `-8` and `-11` — have bytes the standard
  assigns nothing to; a file carrying one is refused rather than opened. In the
  `windows-125x` family the vendor-undefined bytes in `0x80..=0x9F` decode as
  the C1 control of the same value, which is what the web platform does.
- The multi-byte encodings are CPython's `cp932`, `gbk`, `cp949` and `big5` —
  the supersets in actual use rather than the narrow historical registrations.
  The generated module names each one.
- Both reverse maps are built per save rather than generated, so the two
  directions cannot drift. That is 128 entries sorted for a table and up to
  about 22 000 for a multi-byte encoding, once per save. It was not measured;
  it rides on a save that already reads and writes a file.
- `VAL-G8` is not this record. Whether the chooser is reachable and legible on
  the author's own session, and whether `Ctrl + E` survives the compositor, are
  the author's lane.

## Follow-up

`VAL-G8` in [VALIDATION.md](../../VALIDATION.md). G9 opens the imported
document when the author asks for it.
