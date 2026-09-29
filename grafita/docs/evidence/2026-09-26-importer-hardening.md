# Evidence: the document importer refuses hostile input

- **Date:** 2026-09-26
- **Scope:** unit `GRA-H1-A` (program unit P-5) of the
  [hardening plan](../plans/active/2026-09-26-hardening.md); it closes GRA-1,
  GRA-2, GRA-3 and GRA-7 of
  [the Hematita and Grafita audit record](../../../docs/evidence/2026-09-26-monorepo-audit-hematita-grafita.md).
  Code: `celestina-rs/crates/grafita-core` (`src/inflate.rs`,
  `src/container.rs`, `src/open.rs`, `src/import.rs`, `src/import/gzip.rs`,
  `src/import/epub.rs`, `src/import/pdf/{object,file,text,form,update}.rs`,
  `examples/pdfprobe.rs`) and its tests (`tests/hostile.rs`,
  `tests/containers.rs`, `tests/documents.rs`); the
  [document import contract](../contracts/document-import.md)
- **Environment:** session worktree `grafita-GRA-H1-A` on branch
  `unit/grafita/GRA-H1-A`, based on `main` at `2a3c74f`; Linux container
  (kernel 6.18) running as uid 0; `rustc 1.97.1`, `cargo 1.97.1`; Cargo
  `--offline` with the shared worktree target directory. No Qt 6 SDK, CXX-Qt
  build, Wayland session or AT-SPI bus, so neither Grafita nor Siderita could
  be compiled here
- **Artifact:** the landing builds it (`grafita/scripts/complete-production.sh`
  and `siderita/scripts/complete-production.sh`, because Siderita links
  `grafita-core`)

## What changed

| Finding | Change |
|---|---|
| GRA-1 (Critical) | `Lexer` counts nesting and refuses past `MAX_NESTING` (64) with `PdfError::Malformed`, so 20 000 `[` or `<<` is a refusal instead of a stack overflow. Every unchecked slice in the lexer and the file reader is a checked `get` with a typed error: `eat`, the dictionary and `stream` probes, the number and generation reads, the cross-reference entry (only an entry in use is read as an offset, and its ten bytes must be digits), `startxref` past the end of the file, a stream's `start + length` (`checked_add`, falling back to `endstream`), and the cross-reference stream's rows (`/W` widths above 8 or summing to 0 are refused; rows come from `chunks_exact`, and object numbers stop at `u32::MAX`). The predictor's `/Columns` must fit its data before anything is allocated. The update writer's `size + 1` is checked. ZIP header arithmetic goes through one checked `position` helper and every span is read with `get`. |
| GRA-2 (Critical) | One owner for bounded decoding, `grafita_core::inflate::Budget`: every decoder is read through `take(remaining + 1)`, an overrun is `TooLarge`, and nothing is charged for content that does not fit. gzip, every ZIP member (including the EPUB container and package descriptions) and every PDF `FlateDecode` stage draw on it. A document's ZIP members share one budget; a stream's filter chain shares one; a file's cross-reference streams share one. `uncompressed_size` bounds the read and is compared with the result (a mismatch is `Corrupt`), but never sizes an allocation. `Imported::open` takes the ceiling; `open` reads the file itself with `take(max_bytes + 1)` and maps an import's ceiling to `OpenRefusal::TooLarge`, which both hosts already present. |
| GRA-3 (Important) | `Pdf` remembers each object stream once read (or the reason it could not be), behind a `Mutex` so the document stays `Send + Sync`; the retained total draws on its own budget of the ceiling. A stream met again while it is being read is a cycle and is refused; a chain of object streams deeper than 8 is refused. `/N` must be a count the stream's content can hold, and each object is lexed only up to the next object's offset, once per distinct offset, so an object stream is read in one pass. The per-section copy of the whole file that `read_stream_section` made is gone: sections are read through the file itself. |
| GRA-7 (Minor) | `tests/hostile.rs` is the negative table: 19 hostile cases and 5 controls, each opened the way both hosts open a file and required to answer within 30 s. `tests/documents.rs`'s mode-0500 test now checks its own precondition and skips when the process can write into the sealed directory (root, `CAP_DAC_OVERRIDE`). |

Two bounds beyond the literal finding text, in the same class as GRA-1: a page
tree or field tree that names a node twice is walked once (forty levels of
"both kids are the next node" was 2^40 pages from a few hundred bytes, and the
unfixed code ran out of memory), and a walk over the document has a work
budget, so thousands of pages drawing one large stream are refused instead of
lexed thousands of times. Both were extended by the two review rounds below,
which describe the code as it now stands.

Canonical owner of the decoding bound: `celestina-rs/crates/grafita-core/src/inflate.rs`.
Equivalent recipes searched with `rg "GzDecoder|DeflateDecoder|ZlibDecoder"`
across the monorepo: the three decoders in `grafita-core` now delegate to it.
The only other user is `siderita-archive` (`src/read.rs`, `src/extract.rs`),
which extracts archives to disk under its own contract and was not touched; a
shared owner would need a second consumer with the same semantics, which it is
not. Dependency direction is unchanged and no crate was added.

## Procedure

```sh
cd celestina-rs
cargo test -p grafita-core --offline --no-fail-fast        # baseline, before any change
cargo test -p grafita-core --offline --test hostile --no-run
# RED: each hostile test run alone on the unfixed library, 6 GB address-space
# limit, 100 s timeout, so an abort or a hang cannot take the others down
for t in $(hostile --list); do (ulimit -v 6000000; timeout 100 hostile --exact $t); done
cargo test -p grafita-core --offline --no-fail-fast        # GREEN
cargo fmt --check
cargo clippy -p grafita-core --all-targets --offline -- -D warnings
cargo run -q -p grafita-core --offline --example pdfprobe -- check \
  /usr/lib/libreoffice/share/xpdfimport/xpdfimport_err.pdf  # new, and base from `git archive HEAD`
cd ..
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Baseline:** `cargo test -p grafita-core --no-fail-fast` failed one test,
  `a_save_that_cannot_create_its_temporary_leaves_the_original_intact`, as
  root (GRA-7); everything else passed.
- **RED** (unfixed library, each test alone): all 19 hostile cases failed and
  the 5 controls passed.
  - Aborts (exit 134): `twenty_thousand_nested_arrays…` and
    `nested_dictionaries_in_the_trailer…` ("has overflowed its stack");
    `an_object_stream_that_claims_a_huge_count…` ("memory allocation of
    34359738360 bytes failed"); `a_page_tree_that_revisits_its_nodes…` (out of
    memory after 68 s).
  - Panics (exit 101): `file.rs:318` on the 0xFF entry; `object.rs:161` on
    `startxref` past the end; `attempt to add with overflow` on the wrapping
    `/Length` (`file.rs:211`), on `/Columns` (`file.rs:451`) and on `/W`.
  - Accepted instead of refused (exit 101 on the assertion): the gzip bomb,
    the PDF Flate bomb, the cross-reference stream bomb, the filter chain, the
    lying ZIP header, the ZIP members over the total, the small-ceiling gzip;
    the two ZIP bombs also took 62 s and 65 s.
  - Timeouts (exit 124, 100 s): the 3 000-page object-stream document and the
    4 096 pages drawing one stream.
  - Three fixtures were tightened after the RED run: the 0xFF entry gained an
    `n`-kind variant, the shared-stream case runs under 4 MiB instead of
    16 MiB, and the fitting gzip became an honest file with its checksum.
- **GREEN:** `cargo test -p grafita-core --offline --no-fail-fast` exit 0:
  101 unit tests (6 new in `inflate`), `containers` 4, `documents` 28 (the
  mode-0500 case prints its skip as root), `hostile` 24 in 4.4 s, `imported`
  16, `sessions` 31, doc-tests 0.
- `cargo fmt --check` exit 0; `cargo clippy -p grafita-core --all-targets
  --offline -- -D warnings` exit 0.
- `pdfprobe check` on the one real PDF in the container: "37 placements, 0
  with a span that is not a string" from both the base and the changed library.
- The three guards printed `Architecture contract: OK`,
  `Language contract: OK (148 legacy file(s) ratcheted)` and
  `Documentation contract: OK`.

## Observed facts

- The audit's 1 042 933-byte gzip is reproduced by `gzip_bomb(1024)` (a sync
  flushed mebibyte repeated 1 024 times); it is now refused as `TooLarge`
  after inflating at most 64 MiB + 1 byte.
- `OpenRefusal::TooLarge.size` is the file's length when the `stat` refuses it,
  and `limit + 1` when unpacking stopped at the ceiling; its display now says
  "at least". Grafita (`grafita/src/session.rs`) and Siderita
  (`siderita/src/editor.rs`) match `TooLarge { .. }` and already show "too
  large", so no host changes.
- No host uses the changed signatures (`Imported::open`, `Container::read`,
  `Pdf::parse`) or matches the error enums that gained a `TooLarge` variant
  (`rg` over `grafita/src` and `siderita/src`).

## Review round (same day)

The unit's review found that the literal probes were closed but the text
layer still aborted or hung on small PDFs. Its probe crate's cases are now
fixtures in `tests/hostile.rs` (33 tests there in all), each within the 30 s
bound.

| Review finding | Change |
|---|---|
| Critical 1: `ToUnicode` `bfrange` with four-byte codes (1 928 bytes aborted) | `read_to_unicode` stores only codes the font can look up (`0xFF`, or `0xFFFF` for a two-byte font), a line writes at most 65 536 codes, a map stops at `MAX_MAP_WRITES` (131 072 writes, overwrites included) with `Malformed`, and each write is charged to the work budget |
| Critical 2: `Tf` cloned the font map per operator (721 bytes aborted) | `Extraction.fonts` is `Vec<Arc<Font>>`; a stream adds a font once per name and selects it again by index; an unknown name draws with the fallback at index 0, as before |
| Important 1: time | `Pdf::work()` counts bytes lexed, decoded and copied plus 16 per lookup, and extraction was refused past 8 × the ceiling of that plus 48 per font map entry, checked after each decoded stream (round 2 moved the check into every lookup). Fonts are read once per object and font sets once per `/Resources` or `/Font` reference (keyed by number in this round, by the bytes they read since round 2); inherited resources are shared (`Arc`), not copied per page. Classic cross-reference entries across all `/Prev` sections share one count of file length / 18. EPUB manifest and members are maps, and a repeated spine entry is read once. `Imported::open` takes the host's `CancellationToken` and checks it per ZIP member and, through the PDF walk, at every lookup (`ImportError::Cancelled` → `OpenRefusal::Cancelled`) |
| Important 2: a legitimate 896 KB, 3 000-page PDF with two shared CJK fonts refused under 8 MiB | fixed by the font memo; kept as a fixture that must open under 8 MiB |
| Important 3: `low + 0xFFFF`, `low + offset` | `saturating_add` and `checked_add` |
| Minor: a child field listed in `/Fields` before its parent | roots with `/Parent` are walked after every true root, so the child keeps its full name |
| Minor: ledger "passes as root" | the row now says the test checks its precondition and skips as root |

RED, the new fixtures on the previous commit `928a53d` (each alone, 6 GB
address-space limit, 120 s timeout): four-byte ranges, repeated range and
shared font dictionary timed out (exit 124); repeated `Tf` aborted
("memory allocation of 3 bytes failed"); the top-of-range map, the repeated
spine (over 5 s against the fixture's text), the CJK document (`TooLarge`
under 8 MiB) and the field order failed their assertions; the nested
cross-reference tables took 44.5 s against the 30 s bound. GREEN: all pass.

The review's probe crate on the previous commit (debug) and on this one:

| Case | Before | After |
|---|---|---|
| `tounicode-mem 100` | 14.9 s, Ok, VmHWM 1 683 132 kB | 59 ms, Ok, 9 656 kB |
| `tounicode-mem 4096` | not run | 70 ms, Ok, 10 620 kB |
| `tounicode-time 2000` | 64.5 s, Ok | 99 ms, refused (map too large), 9 908 kB |
| `tounicode-overflow` | panic (add overflow) | Ok, 4 380 kB |
| `tf-clone 1000` | 36.1 s, Ok, VmHWM 5 612 116 kB | 55 ms, Ok, 9 740 kB |
| `tf-clone 100000` | not run | 127 ms, Ok, 10 700 kB |
| `pages-fonts 9000` | over 120 s | 59 ms, refused (no text), 28 064 kB |
| `xref-nest 2000` / `20000` | 0.45 s / 44.5 s (fixture) | 0.4 ms / 3 ms, refused |
| `legit-cjk 3000`, 8 MiB | 14.8 s, `TooLarge` | 259 ms, Ok, 13 776 kB |
| `epub-spine 5000` | 5.4 s, Ok | 260 ms, Ok, 6 728 kB |

Peak memory is `VmHWM` from `/proc/self/status`, printed by the probe;
`/usr/bin/time` is not installed in the container.

## Second review round (same day)

The re-review of `6bf4b7f` confirmed each item of the first round for the
case it named, and found the same class elsewhere: one large object reached
many times (through aliased numbers or plain references) aborted the process
in one case and was quadratic in three, and nested `/Prev` sections were
quadratic; the Limits above claimed they were bounded.

| Review finding | Change |
|---|---|
| N1 (Critical): thousands of cross-reference numbers at one large page's offset; each kid was lexed again and its dictionary kept, 4.7 GB for 250 KB, an abort at 3 000 pages | `Pdf::key(number)` names the bytes a number reads. The page-tree walk refuses one object under two numbers (`Malformed`), still skips a number met twice, and keeps a page that is an object of its own as its number: extraction reads each page when its turn comes and drops it after, so what is held is one page at a time plus inline pages (which are bytes of their parent) |
| N2: pages whose `/Contents` is one large array and never a stream went unchecked | The work budget is no longer checked by the caller after a decode. `Pdf::within(budget, token, run)` opens one walk; while it runs, every `Pdf::object` lookup, every `stream_data`, every `object_at` and every `Pdf::spend` checks the budget (`WORK_FACTOR` = 8 × the ceiling) and the host's token. `text::extract` and `form::fields` each run as one walk under `Pdf::walk_budget()` |
| N3: each `/Fields` reference looked up twice, no check, no token | `form::fields` takes the token and runs as one walk; roots are deduplicated by the bytes they read before they are read, and the visited set is keyed by bytes, so aliases of one large object are read once. In this round each root and kid was read up front and held; round 3 reads them in turn instead |
| N4: sections nested inside the previous trailer's string lexed the rest of the file each | `Pdf::parse` takes the token and reads its sections as one walk whose budget is 4 × the file length plus the ceiling; each classic section is charged everything it lexed, trailer included, and a cross-reference stream is charged through the ordinary lookup and decode |
| N5: the Limits overstated what was bounded | rewritten below; each sentence described the code of `45b5590`; round 3 corrects the ones it made false |
| N6 | The dispatch named N6 without describing it. The one reviewer probe not covered by N1–N4 is `pagetree_self` (every kid aliases the `/Pages` node itself). I treated that as N6, and it is now refused by the same rule as N1. That is my decision; no other change was made for it |

RED, the round-2 fixtures on `6bf4b7f` (each alone, 8 GB address-space limit,
120 s timeout): the 3 000-alias page aborted ("memory allocation of 4800000
bytes failed", 64 s); the shared `/Contents` array, the distinct fields with
one large kid and the nested trailer strings timed out (exit 124); the
10 000 aliased fields took 93.8 s against the 30 s bound. GREEN: all pass
(`hostile` is 38 tests, 6.4 s).

The reviewer's files, before (the review's release-build figures) and after
(release build of its probe crate against this code):

| File | Before | After |
|---|---|---|
| `pd-1k.pdf` (250 KB) | accepted, 9.9 s, VmHWM 4.7 GB | refused (one object under two numbers), 8 ms, 8 064 kB |
| `pd-3k.pdf` (307 KB), 8 GB limit | allocation failure, exit 134 | refused, 10 ms, 8 424 kB |
| `ca-2k.pdf` (478 KB) | 25.5 s, then no text | refused, 2 ms, 3 680 kB |
| `f-10k.pdf` (311 KB) | 17.0 s, accepted | accepted, 6 ms, 5 044 kB |
| `f-20k.pdf` (611 KB) | 55.9 s, accepted | accepted, 25 ms, 7 408 kB |
| `pt-10k.pdf` (311 KB, N6) | not reported | refused, 7 ms, 4 820 kB |
| `xs-5k.pdf` (205 KB) | 1.1 s | refused as too large, 177 ms, 3 460 kB |
| `xs-20k.pdf` (820 KB) | 22.3 s | refused as too large, 239 ms, 5 892 kB |

The legitimate 3 000-page CJK fixture still opens under 8 MiB.

## Third review round (same day)

The re-review of `45b5590` confirmed N2, N3 and N4, and legitimate shapes
opened fine: shared page-tree nodes, two-section cross-reference streams, and
a 500-page report sharing one 1 MiB font. N1 was fixed for file offsets but
still bypassed through object streams.

| Review finding | Change |
|---|---|
| R1 (Critical): an object stream header that repeats one offset N times kept N copies of the object (888 bytes: 26 s, 4.7 GB; 1.4 KB and a 34 KB 3 000-page file aborted), and in-stream objects were keyed by index, so the page tree's alias rule never fired | Unpacking lexes each distinct offset once and holds it once. The header maps entries to distinct objects (`Unpack { objects, slots }`), and `ObjectKey` for an in-stream object is its container and distinct object, so aliases collapse and the page tree refuses them. Unpacking charges the decoded content and each distinct object's bytes to the walk, and checks the budget and token every 4 096 header entries and after each distinct object. Two paths around it stayed uncharged in this round and are closed in round 4: the per-stream count of streams in progress, and `Pdf::key`, which swallowed errors |
| R2 (Minor, a round-2 regression): all `/Fields` roots were read up front and held, and each kid was read again for every parent before the visited check | The field walk (`Walker`) deduplicates roots by key before reading them and holds none of them. What a node is (named, has a parent) is read once per object and remembered. Each node is read again only when its turn in the walk comes. So a kid shared by 2 000 fields is read twice in all. The round-2 fixture that expected such a file to run out of budget now expects it to open (`distinct_fields_that_all_list_one_large_kid_read_it_once`) |
| R3 (Minor): walk time at the budget | stated in Limits |
| R4 (Minor): false sentences | corrected in Limits, the round-2 table and the report |
| Nit: nested `within` replaced the outer ceiling | an inner walk ends at the nearer of both ceilings and answers to both tokens |

RED, the round-3 fixtures on `45b5590` (each alone, 8 GB address-space
limit, 120 s timeout):
- `an_object_stream_header_that_repeats_one_offset_keeps_one_copy` failed its 5 s bound at 1 000 entries.
- `pages_named_at_one_object_stream_offset_are_refused` aborted at 18 s ("memory allocation of 4800000 bytes failed").
- `distinct_fields_that_all_list_one_large_kid_read_it_once` was refused as too large.
- `many_form_roots_are_not_all_held_at_once` passes on both commits. It checks the behaviour; the memory figure is the review's measurement below.

GREEN: all pass. The `hostile` suite has 41 tests and runs in 4.9 s: the
slowest of three debug runs, which took 4.5, 4.8 and 4.9 s.

The reviewer's files (release build of its probe crate against this code;
before figures are the review's):

| File | Before | After |
|---|---|---|
| `oc-1k.pdf` (888 B) | 26 s, accepted, VmHWM 4.7 GB | 16 ms, accepted, 16 952 kB |
| `oc-100k.pdf` (1.4 KB) | abort, exit 134 | 28 ms, accepted, 18 356 kB |
| `op-3k.pdf` (34 KB) | abort | refused (one object under two numbers), 12 ms, 12 616 kB |
| `roots-5k.pdf` (17 MB) | VmHWM 38 MB → 415 MB in round 2 | 0.70 s, accepted, 38 924 kB |
| `ovf-3k.pdf` | 534 MB, then too large | too large, 0.69 s, 4 636 kB |
| `fk-2k.pdf` | too large after 13.2 s | 17 ms, accepted, 13 572 kB |
| `oh-2k.pdf` / `oh-4k.pdf` (legitimate) | — | accepted, 0.28 s / 0.50 s, 127 836 kB / 252 612 kB |
| `sf0/1/2-500.pdf` (legitimate) | — | accepted, 29 ms each, about 10 MB |

## Fourth review round (same day)

The re-review of `89c4ce8` confirmed R1, R2, R4 and the nest fix: `oc-1k`
opens in 23 ms at 16.9 MB, `roots-5k` peaks at 38.9 MB, and overlapping
offsets and object-stream length cycles are bounded. It found one more path
in the same class.

| Review finding | Change |
|---|---|
| N7 (Critical): each new object stream counted the streams in progress by iterating every stream read so far, quadratic, uncharged and unchecked; and `Pdf::key` for a root in a missing object stream continued with no charge or check while adding a memo entry. 100 000 missing containers took 41 s and a million over 120 s; 100 000 small streams (14 MB) took 88 s | The streams in progress are a counter. `Pdf::key` costs a lookup and checks the walk's budget and token, found or not |
| N8: `Pdf::key` turned every unpack error, including a walk's `TooLarge` and `Cancelled`, into "no such object", and the failure stayed remembered | `Pdf::key` returns `Result<Option<ObjectKey>, PdfError>`. It is an error exactly when reading the object would be, and `None` only for an object the file does not locate. Every caller propagates the error. In this round every `TooLarge` or `Cancelled` unpack was forgotten, including a stream's own failure; round 5 forgets only an unpack during which the walk itself stopped |
| N9: evidence | the token and loop sentences above are exact. Added: the Siderita figure, and the Form XObject gap |

RED, the round-4 fixtures on `89c4ce8` (each alone, 120 s timeout): both
100 000-field files timed out (exit 124). The three new unit tests in
`pdf::file` (missing stream is an error, `key` checks the token, a walk's
`TooLarge` is not remembered) test the new `Result` signature and do not
compile against the old one. GREEN: all pass. The `hostile` suite is 43 tests
in 9.3 s, and the unit tests 104.

The reviewer's generators against this code (release build):

| File | Before | After |
|---|---|---|
| `mc-100k` (1.2 MB) | 41 s | refused (missing object stream), 32 ms, 18 MB |
| `mc-1m` (13 MB) | over 120 s | refused, 0.33 s, 188 MB (the cross-reference itself) |
| `ms-100k` (14 MB) | 88 s | opens, 0.65 s, 193 MB |
| `oc-1k` | 23 ms (review) | 15 ms, 17 MB |

## Fifth review round (same day)

The re-review of `413d33d` confirmed N7, N8 and N9 (`mc-1m` 0.34 s) and kept
the refusal of a `/Fields` root in a missing object stream.

| Review finding | Change |
|---|---|
| N10 (Important, a round-4 regression): every `TooLarge` unpack was forgotten, including a stream's own inflate past the ceiling, so a stream every page reached through an indirect `/Length` was decoded again for each page, and a failed inflate charged only its raw bytes (the review's `bl-500`, 111 KB: 26.6 s) | An unpack's failure is remembered unless the walk around it has stopped (over its budget, or cancelled), which is asked of the walk after the failure. A stream's own `TooLarge`, from its inflate or the retained total, is therefore decoded once. A `FlateDecode` stage that stops past the ceiling now charges what it produced, the room it had plus one byte |
| Test margin | `many_small_object_streams_open_in_linear_time` asserts the suite's 30 s bound instead of its own 20 s |

With the reviewer's generator (`gen10.py`, release build, files deleted
after): `bl-50` opens in 52 ms and `bl-500` in 61 ms, both at 68 MB peak,
where round 4 took 2.58 s and 26.6 s. A follow-up commit added the fixture
`an_object_stream_that_fails_on_its_own_is_unpacked_once`. It builds
gen10's shape in the test: 500 pages whose `/Length` is packed in a
double-Flate object stream that unpacks past a 2 MiB ceiling. It opens in
0.06 s in a debug build. RED: with the caching arm put back to round 4's
(every `TooLarge` forgotten), the repeated unpacks exhaust the walk and
the file is refused as too large. GREEN after restoring it, with
`hostile` at 44 tests in 9.0 s.

## Limits

- Grafita and Siderita were not compiled or run: the container has no Qt 6
  SDK. The landing's `complete-production.sh` for both is the first build of
  the hosts against this core.
- The container had one real PDF and no writer (LibreOffice lacks its Writer
  and Draw modules), so real-world PDF 1.5 files with object streams were
  exercised only through the hand-built fixtures; the audit's system manuals
  (`/usr/share/doc/ijs/ijs_spec.pdf`, `/usr/share/doc/glm/manual.pdf`) are
  absent, so `imported.rs`'s real-PDF round trip returned early as it always
  does without them. Running `cargo test -p grafita-core` where they exist
  would cover it.
- Timing bounds are for a debug build on this container; release is faster.
- `Budget::inflate` bounds content, not allocator slack: `read_to_end` may
  hold up to about twice the ceiling in capacity while growing.
- Not closed here: the in-memory `Object` tree of one lexed PDF object can be
  about twenty times its bytes (a 64 MiB array of `0`s is roughly 1.5 GB
  while it is held).
- What the other bounds are, exactly:
  - Time: a document walk (text extraction; form fields) is bounded by its
    work budget. The budget is 8 × the ceiling in bytes lexed, decoded and
    copied, plus 16 per lookup and 48 per font map entry. Every lookup,
    every `Pdf::key` query (found or not) and every decode, including one
    that fails past the ceiling (charged the ceiling plus one byte), is
    charged and
    checks it. Unpacking an object stream checks it every 4 096 header
    entries and after each distinct object. Counting the object streams in
    progress costs nothing. One lexing or decoding
    step is not interrupted, so a walk can overshoot by one step, and one
    step is at most the file's length or the ceiling. Reading the
    cross-reference sections is bounded by 4 × the file length plus the
    ceiling.
  - What that time is: a walk may lex up to 8 × 64 MiB at the default
    ceiling, and this lexer reads about 27 MB/s of dense numbers in the
    review's measurement, so a walk that runs to its budget takes 13 to 20 s
    before it is refused (the review's `cd-2k` 19.2 s and a combined
    pages-and-fields file 23.6 s; `cd-2k` 13.4 s in this container's release
    build). The same checks read the host's token, so closing the document
    stops a walk within one uninterrupted step: one lex of at most the
    file, one decode of at most the ceiling, or 4 096 header entries.
  - What that budget refuses: a legitimate-shaped document that draws one
    large stream on every page, once the pages times the stream pass
    8 × the ceiling. At Grafita's default 64 MiB ceiling that is about
    500 pages of a 1 MiB stream (the review's `sc-600-1024`: 600 pages,
    refused after 7.1 s; `sc-500-1024` opens in 7.4 s). At Siderita's 8 MiB
    preview ceiling it is about 250 pages of a 256 KiB stream: the review's
    `sc-500-256` (500 pages) is refused as too large in 0.93 s there.
  - Memory: extraction holds one page's dictionary at a time and the fonts it
    has read. Fonts are read once per object, and their maps count against the
    same budget. It also holds the text and placements it produced, the file
    itself, and the object streams it unpacked. Their decoded bytes total at
    most the ceiling, but they are held as `Object` trees, which are the
    ~20× amplification above: the review's legitimate object-stream-heavy
    `oh-2k` (1.45 MB) peaks at 128 MB and `oh-4k` (2.9 MB) at 253 MB. Each
    distinct object of a stream is held once, however many header entries
    name it. Inline pages are kept as the bytes of their parent node, which
    is read once.
- Outside a walk, lookups count but nothing checks the count. The save path
  runs outside one: it rewrites the streams and fields that were edited, and
  its reads are bounded by the edits it makes.
- `Pdf`'s object-stream memory marks a stream as being read while one caller
  reads it. A second thread reading the same `Pdf` at that moment would see
  the mark and get a spurious "needs itself" refusal. Both hosts open and
  save a document on one worker thread, so this cannot happen today; the lock
  keeps it memory-safe regardless.
- Top-level objects are not memoised. Each lookup is charged by the bytes it
  lexes and checked against the walk's budget. Since round 3, aliased
  numbers of one object — at one file offset, or at one offset inside an
  object stream — are refused in the page tree and read once in the field
  tree, and extraction reads each font and each font set once. A file that names one
  large, distinct object from many places is therefore refused as too large
  once the walk has lexed 8 × the ceiling, rather than read quickly. Real
  documents read their objects a small number of times.

- Text extraction does not read Form XObjects (content drawn through `Do`).
  This gap predates this unit; text inside them is not shown or edited.

## Follow-up

- A later unit can bound the `Object` tree's amplification with an item
  budget per lexed object.
- Reading text drawn through Form XObjects is a separate feature, with its
  own budget.
- Author validation: none; nothing here is visible except a refusal message
  both hosts already had.

## Landing

- **Base revision:** `f7d80aa47f240b5f831b70fd177363efc880b865`
- **Check:** `production_artifact.py check grafita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh
- **Build:** grafita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:8f1326ea4e4dd55f21f80bf317596dcf60f1c6b8fc1700ee42dc120fbba7345a, verification_fingerprint sha256:2f76c9b2c38df2ed0bd97f5ff4c03e4d241583c8717747f29d0608271e4a415d; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:958c652e4a449c47436967606a8850aa42c23648fa591fa4e1e7cae156931546, verification_fingerprint sha256:5186dd5531f76f4416649288a783cc08e04043f19a7103cf6a0b5b086c9f9bb1; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:d2c73dbe830ec992594412229b4a03cc7c8de8f2f217e8a839cb4cf8d5cddd44, verification_fingerprint sha256:7d31ed8dfa9c5c7a2b29aea59826d8462f7f9ad91e5309901b6f6673d52257d9
- **Deploy:** after the push: grafita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh
