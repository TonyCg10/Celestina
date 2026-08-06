# Evidence: 2026-08-05 destructive-operation guards and portal answers

- **Date:** 2026-08-05
- **Scope:** `SID-G7-C`; plan
  [shared-reading-surface](../plans/active/2026-08-04-shared-reading-surface.md);
  suite audit findings `SID-A1`, `SID-A3`, `SID-M1`–`SID-M5`, `SID-M7`,
  `SID-M8`, `SID-B4`–`SID-B6`, `SID-B9` from
  [`../../../docs/evidence/2026-08-05-static-suite-audit.md`](../../../docs/evidence/2026-08-05-static-suite-audit.md)
- **Environment:** source corrections with compilation and unit tests. No
  production build, no deployment, no portal request served from a real
  application, and no version transition — the author asked for the
  corrections, not the delivery
- **Artifact:** none; no production build ran

## What was wrong

The worst of it was one missing filter. `drop_uris` refused a source whose
parent was already the destination; `paste` did not. Pasting into the folder an
entry already lives in therefore produced a collision between a file and
itself, and answering the conflict with Replace — the primary button —
sent the original to the trash to make room for the copy that then failed with
`SourceMissing`. Nothing was lost beyond recall, but pasting into a folder
appeared to make the file
disappear.

The portal backend was answering for the requester rather than from it: every
`OpenFile` reply asserted `writable: true`, so an application that asked to read
a document received write access to it, and a save destination came back with no
overwrite confirmation at all.

## What changed

- `plan_paste` decides collisions by device and inode through `is_same_entry`,
  and an entry that collides with itself is planned as `KeepBoth` — a duplicate,
  which is what every other manager does with a paste into the same folder.
  Comparing paths textually would not have been enough; two names can reach one
  inode.
- `portal.rs` derives `writable` from the request: a `save` or `saves` is
  writable by definition, an `open` only when the option says so, defaulting to
  the `false` the interface documents. The picker asks before returning a
  destination that already exists, through the new `PickerOverwriteDialog`, and
  refuses `.` and `..` as names.
- `spawn_trash` takes the same running-operation guard `paste` already had, so a
  trash started from the context menu can no longer overwrite the cancellation
  token of a copy in flight or interleave its progress with it.
- A directory entry that vanishes between `read_dir` and its own
  `symlink_metadata` is skipped instead of failing the whole listing, and
  `handle_scan_result` leaves the error and status text alone when the rescan
  was quiet. Watching a folder while something writes into it no longer flashes
  the folder-unreadable banner over a listing that is fine.
- A symlink that resolves to a directory is navigable, a drop target, and offers
  the folder verbs, without losing its own kind.
- A trash entry is restored and purged by its own info path, checked to still
  exist, instead of by its position in a list that reloads under it. Permanent
  deletion no longer resolves through an index.
- Dropped URIs cross to Rust as bytes and are decoded with
  `celestina_core::percent`, so one malformed `%XX` from another manager can no
  longer throw a `URIError` in QML and silently drop the whole batch.
- `settings.rs`, `bookmarks.rs`, `favorites.rs` and `folder_views.rs` write
  through the atomic replace the icon store already used; `recent.rs` unescapes
  XML entities; the outgoing drag percent-encodes by segment; and a consumed cut
  clears the system clipboard only when it still holds its own URIs.

Deciding and performing one paste moved out of `controller.rs` into
`controller/paste.rs`: `PastePlan`, `plan_paste`, `is_same_entry`,
`holds_exactly`, `paste_one` and `place_into`. The corrections had pushed the
coordinator from 1223 to 1357 lines, and inventoried architecture debt may not
grow. The extraction is not cosmetic — planning a paste is a named
responsibility with its own testable boundary, and it is where the
collides-with-itself rule now lives — and it leaves the file at 1171 lines, so
its baseline row falls to that number in the same change rather than being
raised to accommodate the growth.

## Procedure

```sh
cargo check                                   # in siderita/
cargo test -p siderita-core -p siderita-ops   # in celestina-rs/
cargo fmt
```

## Result

| Command | Result |
|---|---|
| `cargo check` in `siderita/` | passes |
| `cargo test -p siderita-core -p siderita-ops` | 98 + 45 + 64 pass, 0 fail |
| `cargo fmt` | clean on the edited files |

## Limits

No real application requested a file through the portal during this work, so the
`writable` answer, the overwrite confirmation and the save flow are unproven
against a genuine requester — the failure mode this project has already been
bitten by, where a picker looks broken because the requester never waited for
`Response`. That observation is `VAL-SID-05` in
[`../../VALIDATION.md`](../../VALIDATION.md).

## Not in this unit

`SID-A2`, the lossy Qt seam that lists non-UTF-8 names but cannot operate on
them, is untouched: Fluorita carries the identical defect at the identical
boundary and one shared decision should fix both. `SID-M6`, the cross-device
move that verifies only type and length before removing the source, is also
untouched; closing it properly means renaming the source aside before copying,
which is a design choice rather than a patch.
