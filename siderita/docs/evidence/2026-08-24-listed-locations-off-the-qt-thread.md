# Evidence: 2026-08-24 the listed locations read off the Qt thread

- **Date:** 2026-08-24
- **Scope:** `SID-A4-B`; plan
  [what-the-window-costs-and-shows](../plans/active/2026-08-20-what-the-window-costs-and-shows.md)
- **Environment:** Arch-derived Linux, Qt 6.11.1, `cargo` stable. The machine
  had 40 mount points, among them one `fuse.sshfs` (a phone through Magnetita),
  `fuse.gvfsd-fuse` and `fuse.portal`, and a 37 KB `recently-used.xbel` with 59
  bookmarks
- **Artifact:** `siderita/target/release/siderita`

## The defect

The author reported that opening Recientes or the Papelera sometimes froze the
whole window. Both locations did every filesystem question they needed on the
Qt thread:

- `siderita_ops::list_trash()` reads `/proc/self/mounts` and asks each mount
  whether it holds a per-user trash — two `stat` calls per mount — then reads
  every `.trashinfo` record and stats every body.
- `publish_trash` and `open_recent` then asked `is_dir()` for the row kind,
  again for the size branch, again for the row lookup, and `metadata()` for the
  size: four round trips per row to learn two facts.
- `recent::load` probed *every* bookmark in `recently-used.xbel` with
  `path.exists()` and only then kept the newest 100, so entries nobody would
  ever see were paid for in full.

None of that is expensive when every filesystem answers. The freeze is what
happens when one does not: a phone that went to sleep, a share that stopped
answering, a drive pulled out. A single blocking `stat` on such a mount holds
the Qt thread for as long as that filesystem takes to give up — and both
locations reach exactly those mounts, one through the volume trashes and the
other through files the desktop opened on them. "A veces" is the phone being
asleep.

## What was measured

| Check | Before | After |
|---|---|---|
| Trash listing, mount probe alone, everything healthy | 80 `stat`, 25 ms **on the Qt thread** | the same 80, on a worker |
| Filesystem questions per published row | 4 | 1 |
| `recently-used.xbel` with 500 records, 100 shown | 500 existence probes | 100 |
| One unresponsive mount while listing | window frozen until it answers | window live; Back leaves at once |

The 25 ms is the healthy case and only the mount probe — before a single
`.trashinfo` is read. It is paid on every open of the Papelera and on every
refresh after a restore, purge or empty.

## What changed

**Both locations read on a worker and publish when they land.** `open_trash`
and `open_recent` enter the location immediately — the click reaches a window
that is already showing Papelera or Recientes — and hand the reading to a
thread that queues its result back, the same shape `search_recursive` already
used.

**An answer nobody is waiting for is dropped.** The location's own `active`
flag is the test: leaving clears it, and a listing that lands afterwards
returns without touching the window. That is also what cancels a slow listing,
so leaving is instant rather than a wait.

**Leaving is never gated on `loading`.** `canGoBackOrLeave` held back every way
out while the window was reading, including the way out of the location being
read. Now `loading` holds back only the history move, the one step that races a
scan. Without this the fix would have traded a frozen window for a locked one.

**One question per row instead of four**, and `recent::load` orders the list
first and asks about one entry at a time until it is full, so a bookmark it
will not show is never probed.

**The Papelera pill says nothing rather than calling the Trash empty while it
is still reading.** Empty is an answer, and it did not have one yet.

## Procedure

| Check | Result |
|---|---|
| `cargo test` | 122 tests pass (4 new, on the bounded probing) |
| `cargo clippy --all-targets` | no warnings of ours |
| `cargo fmt` | clean |
| `scripts/qml-tests.sh` | 90 tests pass |
| `scripts/qmllint-cxxqt.sh` | OK, no new warnings against the baseline |
| `scripts/smoke.sh --binary target/release/siderita` | binary alive 8 s, no QML errors |
| `scripts/check-language-contract.py` | OK, 157 files ratcheted |

## Result

- **Exit:** 0 for every command in the table above.
- **Observed:** the mount-probe numbers are what `/proc/self/mounts` and 80
  `stat` calls cost on this machine, timed in the shell against the same
  candidate paths `all_trash_roots` builds. The probe counts are what the new
  unit tests assert.

## Limits

- `scripts/check-architecture-contract.sh` could not be run to completion: its
  `celestina-style` visual guard fails on `celestina/qml/MenuSection.qml`, an
  uncommitted change belonging to another unit. Every rule that names a file of
  this unit passes; `FolderView.qml` is back at its baseline 745 lines.
- The freeze itself is reasoned from the blocking calls and reproduced only in
  its healthy form: making a mounted phone hang on demand is the author's
  session, not this lane. `VAL-SID-11` is that pass.
- Listing the Trash still asks all 40 mounts. Off the Qt thread that costs
  latency rather than a freeze; narrowing the probe to mounts that can hold a
  trash is a separate question and is not in this unit.
- No production run and no version transition: this records the fix, not a
  release.
