# G8 — text Grafita already refuses

- **Opened:** 2026-08-19
- **Plan ID:** g8-text-already-refused
- **Closed:** 2026-08-19
- **Status:** done
- **Scope:** grafita
- **Implementation checkpoint:** G8
- **Author-validation checkpoint:** `VAL-G8` in [`../../../VALIDATION.md`](../../../VALIDATION.md)
- **Successor:** none; G9 opens when the author asks for it

## Hypothesis

Grafita refuses files that are unambiguously plain text. A `windows-1252` note
and a `latin-1` subtitle are rejected although a single-byte table is bijective
and therefore satisfies the existing byte-preserving contract exactly as UTF-8
does, and a UTF-16 file without a byte-order mark is rejected although the
author can name its encoding. Carrying the encodings that are provably
reversible, and letting the author name the ones the bytes cannot prove, opens
those files as ordinary native documents without weakening a single rule.

## Tangible outcome

A `windows-1252` file opens by itself, is edited and saved, and every byte the
author did not touch is identical. A UTF-16 file with no mark opens through an
explicit "reopen with encoding" choice and saves the same way. The encoding a document was opened with is visible and
changeable, and a file whose chosen encoding cannot reproduce its bytes says so
instead of being edited.

## Scope

In scope, all of it in `grafita-core` except the last item:

- a catalogue of single-byte encodings — `windows-1252`, `ISO-8859-1`,
  `ISO-8859-15`, `KOI8-R` and the rest of the `windows-125x` family — written
  as tables in the crate, with no new dependency, because the property that
  makes them acceptable is bijectivity and a table is what proves it;
- `Encoding` gaining those members, so decode/encode, the save path, the probe
  and the session inherit them with no second concept of "encoding";
- opening with an encoding the caller names, distinct from opening with the
  encoding the bytes prove, and refusing when the named one cannot reproduce
  the bytes;
- UTF-16 without a mark and UTF-32, reachable only through that named path,
  never through detection;
- the multi-byte encodings `Shift-JIS`, `GBK`, `EUC-KR` and `Big5`, each
  verified by re-encoding at open time and degraded to read-only when the bytes
  do not reproduce;
- the application's gesture for naming an encoding and showing the current one.

## Exclusions

Out of scope: every container format, which is G9 onwards and a different save
contract; statistical or frequency-based detection of any kind — an encoding is
either proved by the bytes or named by the author; a preferences surface;
per-tab encoding defaults; transcoding a document from one encoding to another,
which is a conversion and not this checkpoint's subject; and Siderita's
embedded surface, which consumes whatever the core decides but grows no gesture
of its own here.

## Build order

1. Add the single-byte catalogue and its bijectivity tests to `grafita-core`,
   with `Encoding` extended and every existing consumer still passing.
2. Add "open with a named encoding" to the open path and the session, with the
   refusal that fires when the named encoding cannot reproduce the bytes.
3. Add unmarked UTF-16 and UTF-32 behind that named path only.
4. Add the multi-byte encodings with their open-time round-trip verification
   and the read-only degradation.
5. Publish the document's encoding to the host and add the gesture that names
   one, then re-reads the document through it.

## Implementation exit

- `cargo test -p grafita-core` covers, for every catalogued single-byte
  encoding, that all 256 byte values decode and re-encode to themselves; that a
  named encoding which cannot reproduce the bytes is refused rather than
  applied; and that unmarked UTF-16 is never concluded without being
  named.
- Rust format and Clippy pass for the crate and the application; QML lint and
  `bash scripts/check-architecture-contract.sh` pass.
- `grafita/scripts/complete-production.sh` and, because `grafita-core` changes,
  `siderita/scripts/complete-production.sh`.

Whether the gesture is reachable and legible on the author's own session is
`VAL-G8` and does not block this checkpoint.

## Change and commit ledger

Update before editing a slice and again when its diff is ready.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| G8-A | `grafita:` | done | [inventory](../../inventories/2026-08-19-g8-text-already-refused/G8-A.numstat.tsv) | 11 files, +64648/-17 | Thirty single-byte tables and four multi-byte encodings generated from the standards' own mappings, and the chooser (`Ctrl + E`, the footer button) that lets the author name one Grafita cannot conclude from the bytes. The fallible encode this checkpoint needs, and the crate-wide plumbing that carries it, land with `G9-A` in the same commit — the two were written as one continuous change and are not separable into two buildable states | [the encodings a file cannot declare](../../evidence/2026-08-19-encodings-a-file-cannot-declare.md) | `VAL-G8` |

The build order was written as four slices and delivered as one unit. Every
slice edits `encoding.rs`, the open path and the same tests, and an inventory
claims whole paths, so four inventories could not divide these files between
them. One commit that builds is worth more than four that describe an order
nobody can check afterwards; the order itself is still visible in the code,
which added the catalogue before the path that reaches it.

G8-A also settled a claim this plan opened with. The control-byte heuristic was
said to call an ANSI-escaped log binary; it does not, because `ESC` was already
exempt. Nothing was changed, the real behaviour is now pinned by a test, and the
item was struck from the scope and the build order rather than left as work that
would have had nothing to do.

## Decisions and rollback

The tables are written in the crate rather than taken from `encoding_rs`. The
property that lets a single-byte encoding into the native contract is that
decoding and re-encoding reproduce every one of the 256 byte values, and a
table in the repository is what a test can prove exhaustively. A general
transcoding library also brings replacement-character semantics, which is
precisely the silent loss the contract exists to prevent.

Detection stays refused. `probe.rs` opens by stating that it consults no name,
extension or MIME value, and the same reasoning rejects guessing an encoding
from byte frequencies: a wrong guess shows plausible text that saves as
different bytes. The bytes prove the encoding, or the author names it.

The multi-byte encodings are verified per document rather than trusted per
table, because they are not bijective the way the single-byte ones are: some
byte sequences have no character and some characters have two encodings. The
open-time round trip is what keeps them inside the contract, and a document
that fails it is refused. Showing it read-only would be better, and it is not
possible yet: every document this crate has can be saved, and a kind that
cannot is exactly what G9's imported document introduces.

That round trip is also the limit of what naming an encoding can promise, which
a test pins. The same bytes are often valid in two encodings — a Shift-JIS file
reads as GBK and writes back unchanged — so the byte comparison cannot tell
which one the author meant. What the contract guarantees is that no byte is
lost, not that the choice was right.

Refusals reuse `OpenRefusal::UnsupportedEncoding` rather than adding a variant,
and the encoding chooser's state is a session property beside `closePrompt`
rather than a new event. Both hosts match these enums exhaustively, so every new
variant is a second project's commit; the detail string carries the specific
reason and no meaning is lost.

Adding a variant to `SaveRefusal` stops Siderita compiling until its presenter
shows it, and `siderita/src/editor.rs` is outside Grafita's commit scope. The
author chose two consecutive commits over widening this unit: `G8-A` carries
the core and Grafita, and the five presenter lines follow immediately under
Siderita's own prefix. The cost is one revision in between where Siderita does
not build, which is accepted rather than hidden.

Rollback is per slice. Every unit adds members and paths rather than changing
the existing ones, so reverting a slice returns the previous acceptance
behaviour without touching how a UTF-8 document opens or saves.
