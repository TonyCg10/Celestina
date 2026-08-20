# After the archive verbs

- **Opened:** 2026-08-19
- **Closed:** 2026-08-19
- **Plan ID:** after-the-archive-verbs
- **Status:** done
- **Scope:** siderita
- **Implementation checkpoint:** SID-A2
- **Author-validation checkpoint:** `VAL-SID-08` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)
- **Successor:** [Pause and global scope](2026-08-19-pause-and-global-scope.md)

## Hypothesis

Everything here came from the author using the archive verbs on their own
session and finding what a test could not: a Trash that copied gigabytes to the
wrong disk, a sidebar lighting two rows at once, a fold that forgot itself
overnight, a dialog nobody could read, and a folder of files that all looked the
same. None of it is speculative work — each unit answers something seen.

## Tangible outcome

Deleting from another disk is instant and reversible from that disk's own
Trash; the sidebar marks one place; folds survive a restart; the unsaved-changes
question is legible; a folder tells its files apart by family, by language and
by the picture each file carries inside itself.

## Scope

- The freedesktop per-volume Trash, and the no-replace rename that concurrent
  writes needed.
- `siderita-embedded`: the image inside a program, a song, a package, a book.
- Icon families by extension, with colour where the family draws one page for
  several languages.
- The sidebar's marked location, remembered folds, a legible guard dialog, a
  concentric content radius and a quieter route reveal.

## Exclusions

- Windows executables are read for their icon, not for anything else.
- The author's own run of all of it on the live session, tracked as
  `VAL-SID-08`.

## Build order

1. The domain: per-volume Trash, and the parsers that read an embedded image.
2. The application: what the folder draws, and what the sidebar remembers.

## What the author found, and what it was

- **Deleting on another disk filled the home disk.** `trash()` always used
  `$XDG_DATA_HOME/Trash`, so trashing from an external drive *copied every
  byte* onto the system disk. It now finds the volume the entry lives on and
  uses that volume's own Trash, which makes the same delete a rename — and the
  Trash view lists every mounted volume's Trash, or the file would have looked
  like it simply vanished.
- **Two rows lit at once.** Every sidebar row compared its path against
  `current_path_key`, which keeps naming the folder underneath while Papelera
  or Recientes are open. One published property now answers "what is marked",
  and the rule lives in one place instead of in five QML files.
- **A fold forgot itself on every launch.** It lived in the session on purpose;
  for someone who keeps a section shut that is a setting that does not work.
- **The unsaved-changes question was unreadable.** It was text over a scrim,
  and a scrim is a veil, not a background.
- **Every file looked the same.** One generic page for everything that was not
  a folder or media. Families by extension now, a page per language where the
  icon family draws one, and colour where it does not — never a glyph borrowed
  from a second family.
- **A `.dll` drew nothing at all.** A file that carries no picture answers with
  an image that is *ready and empty*; "loaded" was taken for "has pixels", so
  the cell went blank instead of falling back to its family glyph.

## Implementation exit

Close `SID-A2` when the per-volume Trash round-trips (delete, list, restore) on
a second filesystem, an embedded image is read for each format the crate claims,
and `scripts/complete-production.sh` builds, verifies and deploys those exact
bytes. The author's own pass on the live session belongs to `VAL-SID-08`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-A2-A | `siderita:` | done | [inventory](../../inventories/2026-08-19-after-the-archive-verbs/SID-A2-A.numstat.tsv) | 14 files, +1361/-21 | The domain: the freedesktop per-volume Trash with its listing across every mounted volume, and `siderita-embedded`, which reads the image inside a program, a song, a package or a book | [evidence](../../evidence/2026-08-19-volume-trash-and-embedded-images.md) | `None` |
| SID-A2-B | `siderita:` | done | [inventory](../../inventories/2026-08-19-after-the-archive-verbs/SID-A2-B.numstat.tsv) | 41 files, +1122/-217 | The application: icon families and their tints, embedded pictures through the thumbnail provider, one marked sidebar row, remembered folds, a legible guard dialog, a concentric content radius and a quieter route reveal | [evidence](../../evidence/2026-08-19-what-the-folder-shows.md) | `VAL-SID-08` |
| SID-A2-Z | `siderita:` | done | [inventory](../../inventories/2026-08-19-after-the-archive-verbs/SID-A2-Z.numstat.tsv) | 4 files, +131/-86 | Archive this plan, closed the same day with every unit already `done`, alongside `SID-G7` and `SID-A1` in the same administrative move | [archival evidence](../../evidence/2026-08-19-after-the-archive-verbs-archival.md) | None |

Like every plan in this repository, this one records intent and grants no
authority.
