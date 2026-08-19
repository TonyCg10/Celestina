# Evidence: 2026-08-19 the volume's own Trash, and the picture inside a file

- **Date:** 2026-08-19
- **Scope:** `SID-A2-A`; plan
  [after-the-archive-verbs](../plans/archive/2026-08-19-after-the-archive-verbs.md)
- **Environment:** Arch-derived Linux, `cargo` stable. One filesystem for the
  checkout and the home Trash, which is what the volume test asserts
- **Artifact:** none built here; the application's build and deployment are
  `SID-A2-B`'s

## What was wrong

**The Trash was always the home Trash.** `trash()` resolved
`$XDG_DATA_HOME/Trash` and sent everything there, whatever disk the entry lived
on. On the same filesystem that is a rename; from another disk it is a copy of
every byte onto the system disk, followed by a delete. A 40 GB folder deleted
from an external drive would have filled the home partition and taken as long as
the copy it really was — and the freedesktop specification has said for years
that each volume keeps its own Trash for exactly this reason.

**And nothing else could carry an image.** Every program, song, package and book
drew the same generic page, while each of them already contains the picture a
person recognises it by.

## Procedure

| Check | Result |
|---|---|
| `cargo test -p siderita-ops` | 39 tests pass, including the volume rules |
| `cargo test -p siderita-embedded` | 10 tests pass |
| `cargo clippy --workspace --all-targets` | no warnings |
| Author's `SEXOPHOBIA.exe` | 270 398 bytes out, a valid 256×256 icon `file(1)` identifies as `MS Windows icon resource` |
| Author's EPUB (`Sigue lloviendo`) | its cover, a 650×944 JPEG |
| Three `.dll`s on this machine | no image, answered as "none" rather than as a broken one |
| A file that is not a PE at all | `None`, no panic |
| An Android package built in the test | the hinted launcher icon, not the larger unhinted image beside it |

## Result

The Trash a file goes to is now the Trash of the volume it lived on —
`.Trash-$uid`, or a sticky shared `.Trash/$uid` when the administrator made one
— with the home Trash kept for the home filesystem and as the fallback when a
volume's Trash cannot be created. Listing reads every mounted volume, so an
entry deleted on a drive is still recoverable from the Trash view.

`siderita-embedded` reads: the icon in a PE's resource section (largest size,
returned as a `.ico` assembled around it), `APIC` in ID3v2, `PICTURE` in FLAC,
`covr` in MP4, the base64 picture in an Ogg comment, the launcher art in an
Android package, and the cover in an EPUB.

## Limits

- Every parser is bounds-checked and capped, and the crate forbids `unsafe`; a
  malformed file answers `None` rather than panicking or allocating what its
  headers claim.
- Format chosen by extension, not by sniffing: running seven parsers over every
  file in a folder would cost more than the picture is worth.
- The per-volume Trash has not been exercised on a second physical filesystem
  here — no removable volume was mounted during this work. That pass is
  `VAL-SID-08`'s.
