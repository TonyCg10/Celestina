# Evidence: the Siderita audit

- **Date:** 2026-09-26
- **Scope:** `AUD-1-A` of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md): Siderita (`siderita/`) and its crates under `celestina-rs/crates/` (`siderita-ops`, `siderita-core`, `siderita-archive`, `siderita-embedded`), on `main` at `9d022dd`; one of the seven area records the [monorepo audit](2026-09-26-monorepo-audit.md) consolidates
- **Environment:** read-only audit in a Linux container (kernel 6.18) running as uid 0; rustc and cargo 1.94.1, Python 3.11, Git 2.43.0; no Qt 6 SDK or CXX-Qt build, no libmpv, no Android SDK, NDK or Gradle, no Wayland session, no AT-SPI bus and no real device; Cargo ran `--offline` with its target directory in the session scratchpad, so no production target or cache was touched
- **Artifact:** not applicable

The auditor's report was titled "Audit SID: Siderita (file manager) and its `siderita-*` crates".

## Procedure

The auditor worked read-only from a common brief: no tracked file was
edited, nothing was committed, no production entry ran and no
subagent was spawned. Every finding was verified by reading the code at
the cited path.

```sh
python3 scripts/agent-context.py siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
cargo test -p siderita-ops -p siderita-core -p siderita-archive --offline
```

Three proofs of concept were built in a scratch crate outside the
repository (`scratchpad/poc`) to confirm SID-1 (a crafted `.tar`),
SID-2 (200 concurrent copy rounds) and SID-4 (a relative `Path=`
record).

## Result

- **Exit:** the guards printed OK (148 language files ratcheted); 124 tests passed (9+13 archive, 39+1 core, 39+23 ops); the three proofs reproduced SID-1, SID-2 (160 of 200 rounds) and SID-4.
- **Observed:** 31 findings: 1 Critical, 11 Important, 19 Minor. The auditor's summary, the findings table and every finding follow unchanged, with their original IDs; the consolidated record merges a few of them across areas without renaming them.

### Auditor's notes

Checkout: `main` at `9d022dd`, clean worktree. Read only; no tracked file changed.

### Executive summary

1. **Healthy:** the pure crates are clean. They forbid `unsafe` and have no production `unwrap`. Their errors are typed, and 124 domain tests pass (`cargo test -p siderita-ops -p siderita-core -p siderita-archive --offline`). Directory scans carry generations. The native model diffs rows instead of resetting. Paths cross the Qt seam as byte-exact keys (ADR 0008).
2. **Guards:** all three repository guards pass. The ratcheted coordinators have not grown: `controller.rs` is at 754 and `FolderView.qml` at 744, both equal to their baselines.
3. **Critical risk (SID-1):** archive extraction can write outside its root. The symlink check only looks at the text of a target, so a chain of links that each look inside escapes. Reproduced: a crafted `.tar` wrote `into/pwned.txt` outside `into/evil/`, and `extract` returned `Ok`.
4. **Loss-free guarantee broken in three places:**
   - A copy that loses a same-name race deletes the winner's file. Reproduced 160 times in 200: one copy reports `Ok`, but its file no longer exists.
   - A move across filesystems deletes the source without calling `fsync` on the copy first.
   - That same move runs `remove_dir_all` on the source, which also deletes files that appeared in the source during the copy.
5. **Restore bug (SID-4):** restoring from a volume Trash ignores the relative `Path=` records that GIO writes. The entry lands relative to the process's working directory. Reproduced.
6. **Lifecycle:** write workers are detached threads that are never joined. Closing the window kills a copy halfway and leaves a truncated file under its final name. Closing the tab that started a job leaves that job in the process-wide register forever.
7. **Concurrency:** while any job runs anywhere, every tab ignores folder changes and never catches up afterwards. A folder-change refresh also cancels a navigation the user just started. A search that was closed can still land and reopen the results.
8. **Qt thread blocked:** undo (which can copy across filesystems), restore, purge, empty Trash, and the UDisks/Magnetita D-Bus listings all run on the Qt thread.
9. **Unbounded input:** the PE icon reader loads a whole `.exe` into memory to draw a thumbnail, so multi-GB installers mean multi-GB reads on the thread pool.
10. **Architecture and docs:** paste planning and execution, undo and the search walk are file-domain logic sitting in the adapter. `STATUS.md` still describes eight committed units as "uncommitted", and several comments promise guarantees the code does not keep.

### Findings

| ID | Severity | Category | path:line | Summary | Effort | Prefix |
|---|---|---|---|---|---|---|
| SID-1 | Critical | Security | `celestina-rs/crates/siderita-archive/src/member.rs:40-66`, `extract.rs:447-451` | Chained symlinks escape the extraction root (zip-slip bypass); reproduced | M | `siderita-archive:` |
| SID-2 | Important | Correctness | `celestina-rs/crates/siderita-ops/src/copy.rs:83-87,111-121` | A copy's rollback deletes a destination another writer created; a racing copy reports `Ok` but its file is gone (160/200) | S | `siderita-ops:` |
| SID-3 | Important | Correctness | `celestina-rs/crates/siderita-ops/src/relocate.rs:134-141,184-185`; `copy.rs:243-246` | A cross-device move deletes the source without `fsync`, and `remove_dir_all` deletes entries added during the copy | M | `siderita-ops:` |
| SID-4 | Important | Correctness | `celestina-rs/crates/siderita-ops/src/restore.rs:52-56,78` | Restore ignores relative `Path=` records and restores relative to the CWD; reproduced | S | `siderita-ops:` |
| SID-5 | Important | Correctness | `siderita/src/main.rs:92-94`; `controller/fileops.rs:160,573`; `controller/archive.rs:220,367` | Write workers are detached; quitting kills them and leaves truncated files under their final names | M | `siderita:` |
| SID-6 | Important | Correctness | `siderita/src/controller/jobs.rs:254-260`; `fileops.rs:196-199,626-629` | A job never ends if its tab closes; its ring stays forever and `op_running` stays true | S | `siderita:` |
| SID-7 | Important | Correctness | `siderita/src/controller/scan.rs:515` | Any running job silences folder changes in every tab, and the dropped change is never replayed | S | `siderita:` |
| SID-8 | Important | Correctness | `siderita/src/controller/scan.rs:74-80,536` | A folder-change refresh of the old folder cancels a navigation in flight | S | `siderita:` |
| SID-9 | Important | Correctness | `siderita/src/controller/find.rs:66-73,79` | A cancelled or superseded search still publishes and reopens the results after close | S | `siderita:` |
| SID-10 | Important | Correctness (thread) | `siderita/src/controller/fileops.rs:706-750`; `controller/trash.rs:365-470` | Undo, restore, restore-all, purge and empty Trash run whole copies and deletes on the Qt thread | M | `siderita:` |
| SID-11 | Important | Correctness (thread) | `siderita/src/controller/mounts.rs:50-51,117-118`; `devices.rs:87-100` | Blocking D-Bus (UDisks2, Magnetita with activation) on the Qt thread, with a new connection per call and per-tab watchers | M | `siderita:` |
| SID-12 | Important | Performance / unbounded input | `celestina-rs/crates/siderita-embedded/src/pe.rs:27` | `std::fs::read` loads the whole executable just to find its icon | S | `siderita-embedded:` |
| SID-13 | Minor | Correctness (thread) | `fileops.rs:45,60,76,106`; `paste.rs:91,125`; `selection.rs:318-340`; `marks.rs:39`; `trash.rs:360`; `archive.rs:107` | Remaining small filesystem syscalls on the Qt thread (they block on hung mounts) | M | `siderita:` |
| SID-14 | Minor | Correctness | `siderita-ops/src/trash.rs:103`; `restore.rs:74-78` | Trash and restore use a plain `rename` that can replace, bypassing the crate's own `reserve` | S | `siderita-ops:` |
| SID-15 | Minor | Correctness | `siderita/src/controller/paste.rs:180-193` | "Reemplazar" trashes the old entry before the new one is confirmed; replacing an ancestor trashes the source | M | `siderita:` |
| SID-16 | Minor | Security | `siderita-ops/src/volume.rs:41-42,77-78,151-161` | Trash choice follows a symlinked source; `.Trash-$uid` accepted without symlink/owner checks; uid falls back to 0 | S | `siderita-ops:` |
| SID-17 | Minor | Security / unbounded | `siderita-archive/src/tool.rs:120,126,222,227,150-156` | Archive password passed on argv (readable in `/proc/*/cmdline`); tool output kept unbounded | S | `siderita-archive:` |
| SID-18 | Minor | Security | `siderita/src/dbus.rs:137-150` | `uri_to_path` treats `file://anyhost/...` as local; query and fragment are not stripped | S | `siderita:` |
| SID-19 | Minor | Correctness | `siderita/src/portal.rs:597-621` | Caller-supplied filter names/patterns are joined with `\t` and `\|` unescaped; `expect` in a fallback | S | `siderita:` |
| SID-20 | Minor | Correctness | `siderita-ops/src/trash.rs:185-201` | `DeletionDate` written in UTC (the spec says local); the listing sorts mixed-zone strings | S | `siderita-ops:` |
| SID-21 | Minor | Architecture (unsafe) | `siderita/src/localzone.rs:28-30`; `properties.rs:134-136,157-159` | Three copies of `unsafe localtime_r` with no recorded exception | S | `siderita:` |
| SID-22 | Minor | Architecture (reuse) | `siderita-ops/src/trash.rs:204`; `siderita-archive/src/stamp.rs:133`; `fluorita/src/library/detail.rs:105`; `siderita/src/properties.rs:112` | Calendar arithmetic implemented three times; passwd lookup duplicates `hematita-core::passwd` | S | `suite:` / `siderita:` |
| SID-23 | Minor | Architecture | `siderita/src/controller/paste.rs`, `actions.rs`, `fileops.rs:706-750`, `src/search.rs`, `portal.rs:526-560,633-668` | Paste planning/execution, conflict, undo, search walk and save-name composition are domain logic in the adapter | L | `siderita:` |
| SID-24 | Minor | Correctness | `siderita/src/controller/trash.rs:145-150,153-163,215-220` | Trash and Recientes listings have no generation; stale listings can overwrite newer ones; threads pile up | S | `siderita:` |
| SID-25 | Minor | Correctness | `siderita-core/src/executor.rs:65-68,121-150` | `expect` on worker spawn; a scan stuck on a hung mount blocks every later navigation in that tab | M | `siderita-core:` |
| SID-26 | Minor | Performance | `siderita/cpp/thumbnailprovider.cpp:276-300` | Thumbnails run on the global `QThreadPool` with no `cancel()` and no bound | S | `siderita:` |
| SID-27 | Minor | QML / accessibility | `siderita/qml/dialogs/NamePromptDialog.qml:7-12`; `CompressDialog.qml` | Two modal dialogs expose no `Accessible.Dialog` role or name; untyped `property var` injection | S | `siderita:` |
| SID-28 | Minor | QML / motion | `OperationRing.qml:92-96,186-187`; `SidebarChevron.qml:23-25`; `FolderWheelHandler.qml:106-110` | Spatial motion ignores `CelestinaTheme.reducedMotion` (known planned debt, not ratcheted) | S | `siderita:` |
| SID-29 | Minor | Tests | see section | No tests for the failure modes above (race, relative restore, replace/keep-both execution, stale search, symlink chain) | M | per unit |
| SID-30 | Minor | Documentation truth | `siderita/STATUS.md:161,192,197,215,235,241,262,279,165`; `README.md:15-17`; `controller/trash.rs:19`; `fileops.rs:147`; `siderita-ops/src/purge.rs:13,22`; `volume.rs:153` | Stale "uncommitted" claims and comments promising guarantees the code does not keep | S | `siderita:` |
| SID-31 | Minor | Quick win | `siderita-ops/src/volume.rs:242-250` | Dead `unwritable` kept alive with `#[allow(dead_code)]` | S | `siderita-ops:` |

---

### SID-1: Chained symlinks escape the extraction root (Critical, security)

**Evidence.** `target_stays_inside` checks only the text of a target. Every `Normal` component counts as one level down, even when that component is itself a symlink:

```rust
// member.rs:54-56
Component::Normal(_) => depth += 1,
Component::ParentDir => { depth -= 1; if depth < 0 { return false; } }
```

Members are then written through whatever links already exist:

```rust
// extract.rs:447-450
let target = self.root.join(name);
if let Some(parent) = target.parent() {
    fs::create_dir_all(parent)...
```

**Reproduction.** A tar with four members:
- `d/` (directory)
- `d/up -> ..`
- `esc -> d/up/..`
- `esc/pwned.txt`

Extracting it with `siderita_archive::extract` into `work/into` returned `Ok(Extracted { root: "work/into/evil", written: 4, .. })` and created `work/into/pwned.txt`, outside the root. Adding more `..` hops reaches any depth, for example `~/.bashrc` or `~/.config/autostart/*.desktop`, which means code execution at the next login.

The PoC is at `scratchpad/poc/` (`src/main.rs`, `work/evil.tar`). The post-check `tool::no_symlink_escapes` (`tool.rs:477-505`) uses the same text-only test, so RAR/7z output is not protected either.

**Why it matters.** Opening a downloaded archive is enough to overwrite files outside the destination. The crate documents (`extract.rs:9-10`) that this cannot happen.

**Fix.**
- Never write through a link: before placing a member, `symlink_metadata` every ancestor below the root and refuse any that is a symlink.
- Create all symlink members last, after every regular member is written, as GNU tar does.
- For the tool path, check each link by resolving `canonicalize(link.parent()).join(target)` against `canonicalize(root)`.
- Add the chained case to `tests/archives.rs`.

**Effort:** M. **Prefix:** `siderita-archive:` (a `siderita:` bug unit if a ledger closes it).

### SID-2: A copy's rollback deletes a destination it did not create (Important, correctness)

**Evidence.** `copy_to` rolls back on any error, including the `AlreadyExists` that `File::create_new` or `fs::create_dir` return when a racing writer won the name:

```rust
// copy.rs:83-87
match copy_tree(&mut context, source, destination) {
    Ok(()) => Ok(()),
    Err(error) => { rollback(destination); Err(error) }
```

`rollback` then removes whatever is at that path (`copy.rs:111-121`, `remove_dir_all` for a directory).

**Reproduction.** Two threads call `copy_as` on the same 8 MB file to the same destination, started together by a barrier. In 160 of 200 rounds one call returned `Ok(())`, the other returned `AlreadyExists`, and the destination no longer existed (`scratchpad/poc`, `race` mode).

**Why it matters.**
- Siderita runs write jobs concurrently (`fileops.rs:144-151`). Two "keep both" pastes that compute the same `(copia)` name, or a sync client creating the name, lose data.
- A file created by another process is deleted outright.
- The success message is false.

**Fix.** Track whether this call created the top-level destination. On a create-time `AlreadyExists`, return that error without rolling back, and roll back only what was created. Add the race test.

**Effort:** S. **Prefix:** `siderita-ops:`.

### SID-3: A cross-device move deletes the source without durability, and deletes newcomers (Important, correctness)

**Evidence.**
- `copy_file` ends with `writer.flush()` (`copy.rs:243-246`). `flush` is a no-op on `File`, and there is no `sync_all`.
- `relocate_by_copy` then calls `remove_source(source)` (`relocate.rs:141`), which runs `fs::remove_dir_all(source)` (`relocate.rs:185`).
- `verify` checks only the kind and, for a single top-level file, its length (`relocate.rs:145-171`).

**Why it matters.**
- Moving to a USB stick and pulling it, or losing power, after the source is gone loses data that was still in the page cache.
- A file created inside a source folder during a long cross-device move (for example an active download) is deleted permanently without ever being copied.
- Trash-by-copy and restore-by-copy share this path.

**Fix.**
- Call `sync_all` on every copied file and on each destination directory before removing anything.
- Remove only the entries the copy recorded, bottom-up with `remove_file`/`remove_dir`. Report, rather than delete, anything left behind.

**Effort:** M. **Prefix:** `siderita-ops:`.

### SID-4: Restore ignores relative `Path=` records (Important, correctness)

**Evidence.** `restore.rs:52` uses `parse_original_path(&content)` and then `fs::rename(&trashed, &original)` (`:78`). It never calls `volume::resolve_original`, which the listing uses (`trashinfo.rs:79`). GIO (Nautilus, Thunar and others) writes relative paths in `$topdir/.Trash-$uid`.

**Reproduction.** A `.trashinfo` with `Path=fotos/uno.jpg` under `top/.Trash-0/` was restored to `cwd/fotos/uno.jpg`, and `restore_from_trash` returned `Ok(Restored { to: "fotos/uno.jpg" })`. The record was then deleted.

**Why it matters.** The listing shows the right origin, but "Restaurar" puts the entry somewhere else, or fails when the relative parent is missing. The controller calls this from `trash.rs:385,409` and `fileops.rs:743`.

**Fix.** Derive the volume top from the Trash root, or pass the listed `TrashEntry.original` into restore. Reject a path that is still relative. Add the test.

**Effort:** S. **Prefix:** `siderita-ops:`.

### SID-5: Write workers are detached and killed on quit (Important, correctness)

**Evidence.**
- Paste, trash and archive workers are started with `std::thread::spawn` and no handle is kept (`fileops.rs:160,573`, `archive.rs:220,367`); there are about 20 such spawns in `src/`.
- `main.rs:92-94` returns straight from `app.exec()`.
- `Main.qml` has no `onClosing` guard.

**Why it matters.** Quitting in the middle of a copy leaves `movie.mkv` truncated under its final name. That breaks the documented invariant that "a half copy is never left behind claiming to be complete" (`copy.rs:27-28`). A trash-by-copy leaves a `.trashinfo` pointing at a partial body, and an extraction leaves a partial visible folder. The rule "workers shut down deterministically" is not met.

**Fix.** Keep the `JoinHandle`s in the job register. On close with running jobs, ask the user or cancel; then join, so that each worker's rollback runs before `exec()` returns.

**Effort:** M. **Prefix:** `siderita:`.

### SID-6: A job never ends if its tab closes (Important, correctness)

**Evidence.**
- The register is process-wide (`jobs.rs:14-18`).
- A job leaves it only through `end_job` (`jobs.rs:254-260`), which is reached only from `finish_paste`, `finish_trash` or the archive finish, each queued to the originating controller (`fileops.rs:196-199,626-629`).
- `closeTab` (`Main.qml:121-128`) destroys that tab's `SideritaController`, so `qt.queue` fails and the job is never removed.

**Why it matters.** Every other tab shows a ring that never finishes. `op_running` stays `true` for the rest of the process (`jobs.rs:326-330`), and because of SID-7 that also turns off folder refresh everywhere.

**Fix.** End the job in the register from the worker itself: an off-thread `finish(id)` followed by `wake_listeners`. Deliver the outcome to any live listener rather than only the originator.

**Effort:** S. **Prefix:** `siderita:`.

### SID-7: A running job silences folder changes in every tab (Important, correctness)

**Evidence.** `scan.rs:515`: `if !degraded && *self.op_running() { return; }`. `op_running` reflects any job in the process. The early return comes before `watch.observe_change`, so the change is never recorded, and at the end only the originating controller refreshes (`finish_batch`).

**Why it matters.** During an hour-long extraction, no tab shows new downloads. A tab showing the paste destination stays stale after the paste finishes, until some unrelated event arrives.

**Fix.** Always call `observe_change` and defer the rescan while a job runs. On `end_job`, rescan every stale listener. Alternatively, only suppress refresh in the folders the job writes.

**Effort:** S. **Prefix:** `siderita:`.

### SID-8: A folder-change refresh cancels a navigation in flight (Important, correctness)

**Evidence.**
- `refresh_quiet` sets `pending_nav = None` and rescans `history.current()` (`scan.rs:74-80`), and `coordinator.begin` cancels the active scan.
- The watch still points at the old folder until the new scan lands (`update_watch` at `scan.rs:199`).

**Why it matters.** Navigating away from a busy folder (Downloads during a download) toward a slow one can be silently undone: the path bar snaps back and the click did nothing. If events keep arriving faster than the slow scan finishes, the navigation can never complete.

**Fix.** In `refresh_quiet`, return when `pending_nav.is_some()` or a non-quiet scan is in flight.

**Effort:** S. **Prefix:** `siderita:`.

### SID-9: A cancelled search still publishes (Important, correctness)

**Evidence.** `find.rs:68`: `if token.is_cancelled() && outcome.hits.is_empty() { return; }`. A search cancelled with partial hits is still queued. `publish_search` (`:79`) checks no generation, sets `search_active(true)` and replaces the rows.

**Why it matters.** Closing a search while it is still walking brings the results back a moment later. Retyping a query also flashes stale results. This breaks the rule "async results carry generation; stale results are discarded".

**Fix.** Tag each search with a generation, or compare it with the current `search_cancel`, and drop any outcome that is not current. Test it.

**Effort:** S. **Prefix:** `siderita:`.

### SID-10: Heavy writes on the Qt thread: undo and the Trash verbs (Important, thread affinity)

**Evidence.**
- `undo` (`fileops.rs:706-750`) calls `siderita_ops::move_entry` and `restore_from_trash` synchronously. After a cross-device move, both perform a full copy (`relocate_by_copy`).
- `purge_trash`, `restore_trash`, `restore_all_trash` and `empty_trash` (`trash.rs:365-470`) run `remove_dir_all` or restore-by-copy over every entry, on every volume, synchronously.
- The module doc says the opposite at `trash.rs:19`: "Nothing here reads the filesystem on the Qt thread".

**Why it matters.** Undoing a 10 GB move to an external disk, or emptying a large Trash, freezes the whole application with no progress and no cancel. This is the class of freeze the author already reported (`SID-A4-B`).

**Fix.** Route these through the job worker used by paste and trash, which already has a register, progress and cancellation.

**Effort:** M. **Prefix:** `siderita:`.

### SID-11: Blocking D-Bus on the Qt thread (Important, thread affinity)

**Evidence.**
- `load_volumes` calls `volumes::list_volumes()` and `load_phones` calls `devices::list_devices()` inline (`mounts.rs:50-51,117-118`); the comments say "quick — runs inline".
- Each call opens a new blocking `Connection::system()` or `Connection::session()` and makes synchronous calls (`devices.rs:87-100`, `volumes.rs:34-38`).
- Each controller (tab) starts its own watcher threads (`mounts.rs:94-110,146-160`), so one hotplug triggers N synchronous reloads on the Qt thread.

**Why it matters.** If Magnetita is activatable but slow, D-Bus activation blocks the call for up to the default 25 s. A stalled UDisks2 freezes the window. The rule "D-Bus failures degrade best-effort functionality instead of freezing" is not met.

**Fix.** Keep one process-wide device model on a worker with a persistent connection, and publish snapshots through `qt.queue` to every listener, like `watchreg`.

**Effort:** M. **Prefix:** `siderita:`.

### SID-12: The PE icon reader loads the whole executable (Important, unbounded input)

**Evidence.** `pe.rs:27`: `let bytes = std::fs::read(path).ok()?;`. The thumbnail provider calls `siderita_embedded_image` for any `.exe`, `.dll`, `.scr`, `.mun` or `.cpl` with no size gate (`thumbnailprovider.cpp:218`, `src/embedded.rs:22-30`).

**Why it matters.** A folder of game installers (2 to 8 GB `.exe` files) makes the thumbnail pool read gigabytes into memory in parallel. That means out-of-memory or swap for a 64-px tile. The audio readers are capped (`HEAD`), but this one is not.

**Fix.** Read the DOS and PE headers and section table with bounded reads, then seek to the resource section and bound each read with `MAX_IMAGE`. As a stopgap, refuse files larger than a fixed limit.

**Effort:** S. **Prefix:** `siderita-embedded:`.

### SID-13: Remaining filesystem syscalls on the Qt thread (Minor, thread affinity)

**Evidence.**
- `new_folder`, `new_file`, `rename_path` and the batch `rename_paths` loop (`fileops.rs:45,60,76,106`).
- `plan_paste`'s per-source `symlink_metadata` and `is_same_entry`, run from `begin_paste` on the Qt thread (`paste.rs:91,125`).
- `preview_text` reads 128 KiB inline; the comment admits it (`selection.rs:318-340`).
- `favorite_entry_list` runs `metadata` on every favourite (`marks.rs:39`).
- `trash_record` calls `info.exists()` (`trash.rs:360`).
- `archive.rs:107`.

**Why it matters.** On an unanswering phone, share or disk, each of these hangs the window. A favourite on an unplugged NFS share blocks every refresh of favourites.

**Fix.** Move them to workers: renames into the job path, the paste plan onto the worker (the conflict prompt can be driven from the result), the preview onto a reader, and favourite kinds into a background refresh.

**Effort:** M. **Prefix:** `siderita:`.

### SID-14: Trash and restore bypass the crate's no-replace rename (Minor, correctness)

**Evidence.**
- `trash.rs:103`: `match fs::rename(source, &destination)`. Only `info/<name>.trashinfo` is reserved, so an existing `files/<name>` (an orphan body, or one left by another tool) is silently replaced.
- `restore.rs:74-78` checks, then calls a plain `fs::rename`, the look-then-rename window that `reserve.rs:3-18` documents as "exactly the one the whole crate exists to prevent".

**Why it matters.** A trashed body can be destroyed, and concurrent restores to one origin can overwrite each other.

**Fix.** Use `reserve::rename_without_replacing` in both places. In `reserve_name`, also skip candidates whose `files/` name is taken.

**Effort:** S. **Prefix:** `siderita-ops:`.

### SID-15: "Reemplazar" removes the old entry before the new one is confirmed (Minor, correctness)

**Evidence.** `paste.rs:180-193` calls `siderita_ops::trash(&target, …)` and then `place_into(...)`. If the placement fails or is cancelled, neither version is in place, and the failure text never says that the old one went to Trash. If the target is an ancestor of the source (pasting `a/x/x` into `a`), trashing `a/x` also trashes the source.

**Why it matters.** It weakens the "confirm destination first" rule. The data is recoverable from Trash, but only if the user guesses that it is there.

**Fix.** Place the new entry under a reserved temporary sibling, then trash the old one, then rename into place. Refuse "replace" when the target contains the source.

**Effort:** M. **Prefix:** `siderita:`.

### SID-16: Volume Trash selection and trust (Minor, security)

**Evidence.**
- `device_of` uses `fs::metadata`, which follows links (`volume.rs:141-144`), so a symlink on `/home` pointing to a USB drive is trashed into the drive's Trash.
- `.Trash-$uid` is `create_dir_all`-ed and used without checking that it is a real directory owned by the uid (`:77-78`). The shared `.Trash` is checked (`:87-101`).
- `uid()` reads the owner of `/proc/self` and falls back to `0` (`:151-161`). A non-dumpable process sees root, so `.Trash-0` is used. The comment "getuid never fails and needs no FFI here" is misleading.

**Why it matters.** Entries land in the wrong Trash. On a shared or world-writable volume, a pre-planted `.Trash-1000` symlink can redirect another user's deletions.

**Fix.** Use the entry's parent device through `symlink_metadata`. Check `lstat` of `.Trash-$uid` (directory, not a symlink, owned by the user). Get the uid safely, for example through a `rustix::process::getuid` wrapper in `celestina-core`; `hematita` already uses rustix.

**Effort:** S. **Prefix:** `siderita-ops:`.

### SID-17: Archive tool password on argv; unbounded output (Minor, security)

**Evidence.**
- `tool.rs:120,126,222,227` pass `-p<password>` as an argument, which is readable in `/proc/<pid>/cmdline` and `ps` by other local users while 7z or unrar runs.
- `said` accumulates every stdout line for the life of the process (`:150-156`).

**Fix.** Feed the password on stdin where the tool accepts it (7z does). Keep only a bounded tail of the output for error classification.

**Effort:** S. **Prefix:** `siderita-archive:`.

### SID-18: `uri_to_path` accepts foreign hosts (Minor, security)

**Evidence.** `dbus.rs:140-144` drops any authority, so `file://otherhost/etc/passwd` becomes `/etc/passwd`. `?` and `#` are not stripped. It is used for `FileManager1` calls from any session peer, for drops and for the clipboard.

**Fix.** Accept only an empty host or `localhost` (optionally the machine's hostname), and reject a query or fragment.

**Effort:** S. **Prefix:** `siderita:`.

### SID-19: Portal filters use an unescaped delimiter format (Minor, correctness)

**Evidence.** `portal.rs:613-620` builds `format!("{name}\t{joined}")` with patterns joined by `|`, all supplied by the requesting application. A tab or pipe in a name or glob shifts the fields the QML reads. `:606` uses `.expect("scalar value")` inside a fallback that a `let … else` would express directly.

**Fix.** Strip or escape `\t` and `|`, or publish two parallel lists. Replace the `expect`.

**Effort:** S. **Prefix:** `siderita:`.

### SID-20: Trash dates in UTC (Minor, correctness)

**Evidence.** `trash.rs:193-201` writes `DeletionDate` in UTC ("Local time would need a timezone database"). The freedesktop spec says local time, and GIO and KIO write local time. `sort_newest_first` (`trashinfo.rs:106-112`) compares the strings lexically across both zones. The application already has a zone seam (`siderita/src/localzone.rs`, `siderita_archive::Zone`).

**Fix.** Move the `Zone` trait to `celestina-core` and have `trash` take one, as `siderita-archive` does.

**Effort:** S. **Prefix:** `siderita-ops:`.

### SID-21: `unsafe localtime_r` without an exception, three copies (Minor, architecture)

**Evidence.**
- `localzone.rs:28-30`: `let mut tm: libc::tm = unsafe { std::mem::zeroed() }; … unsafe { libc::localtime_r(&stamp, &mut tm) }`.
- The same pattern twice more in `properties.rs:134-136,157-159`.
- The rule is "unsafe is forbidden unless a prior, isolated, documented exception exists". No exception is recorded in `docs/standards` or `siderita/`, and the guard does not scan for it.

**Fix.** Keep one isolated conversion (`localzone.rs` returning a small safe `LocalParts`), route `properties::format_time*` through it, and record the exception in `docs/standards/architecture.md`.

**Effort:** S. **Prefix:** `siderita:`.

### SID-22: Duplicated recipes (Minor, reuse)

**Evidence.**
- Howard Hinnant's `civil_from_days` exists three times: `siderita-ops/src/trash.rs:204`, `siderita-archive/src/stamp.rs:133` (with the inverse) and `fluorita/src/library/detail.rs:105`.
- `siderita/src/properties.rs:112` (`lookup_name`) parses `/etc/passwd` itself, although `hematita-core::passwd` already owns that parse and Siderita already depends on `hematita-core`.

**Fix.** Put one calendar module in `celestina-core` and reuse `hematita_core::passwd` (plus a group reader beside it).

**Effort:** S. **Prefix:** `suite:` for the shared calendar, `siderita:` for the passwd reuse.

### SID-23: Domain logic in the adapter; controller responsibilities to move out (Minor, architecture)

**Evidence.** The controller bridge (`controller.rs`, 754 lines, ratcheted) and its submodules hold these pure file-domain rules:
- Paste planning and same-entry detection: `controller/paste.rs:81-133`.
- Per-entry execution with Skip, Replace and KeepBoth and the outcome record: `paste.rs:158-248`.
- The conflict and undo vocabulary: `controller/actions.rs`.
- Undo execution: `fileops.rs:706-750`.
- The recursive search walk: `src/search.rs`. It also crosses filesystem boundaries (`/proc`, network mounts) because it never compares `st_dev`.
- SaveFiles name composition and the MIME-to-glob table: `portal.rs:526-560,633-668`.

None of it touches Qt.

**Why it matters.**
- The findings SID-2, SID-3, SID-14 and SID-15 are harder to test because half of each rule lives outside `siderita-ops`.
- The bridge keeps accumulating independent domains (navigation, clipboard, jobs, notices, trash, recents, marks, archive, conflicts, passwords).

**Fix.**
- Move `PastePlan`, `paste_one`, `ConflictStrategy` and `UndoAction` (with apply) into a `siderita-ops::paste` module with tests.
- Move `search` into `siderita-core`, with a same-device option.
- Move `compose_save_files` beside `next_available`.
- The controller keeps only marshalling.

**Effort:** L. **Prefix:** `siderita:`.

### SID-24: Listed locations lack a generation (Minor, correctness)

**Evidence.** `load_trash` and `load_recent` spawn a fresh thread each time (`trash.rs:145-150,215-220`). `trash_listed` checks only `trash_active` (`:153-163`). A slow listing can land after a newer one, for example after a purge, and repaint entries that no longer exist. Toggling Papelera against a hung mount spawns one stuck thread per toggle.

**Fix.** Use a generation counter, as the scan coordinator does, and at most one listing in flight.

**Effort:** S. **Prefix:** `siderita:`.

### SID-25: The scan executor cannot abandon a stuck scan (Minor, correctness)

**Evidence.**
- `executor.rs:68`: `.expect("failed to create Siderita scan worker")` is a production panic on resource exhaustion, and `panic = "abort"` is set.
- There is one worker per controller (`:121-150`). A `read_dir` or `stat` blocked on a dead mount cannot be cancelled, so a newer pending request waits behind it indefinitely.

**Fix.** Return a `Result` from `new`. When a request supersedes a scan that has been running longer than a bound, detach that worker and start a new one, capped in count.

**Effort:** M. **Prefix:** `siderita-core:`.

### SID-26: Thumbnail work is unbounded on the global pool (Minor, performance)

**Evidence.** `ThumbnailResponse` starts on `QThreadPool::globalInstance()` in its constructor (`thumbnailprovider.cpp:279-284`), with no `cancel()` override and no check before work. Scrolling past a folder of 5,000 images queues 5,000 decodes. Reads blocked on a slow mount occupy the pool that QML's own async loaders also use.

**Fix.** Use a dedicated `QThreadPool` with a bounded thread count, override `cancel()` to set a flag checked before `loadThumbnail`, and dequeue work that was cancelled.

**Effort:** S. **Prefix:** `siderita:`.

### SID-27: Two modal dialogs without dialog semantics (Minor, accessibility)

**Evidence.** `NamePromptDialog.qml` and `CompressDialog.qml` contain no `Accessible.role: Accessible.Dialog` and no name, while the other ten dialogs have them (for example `ConflictDialog.qml:23`). `NamePromptDialog` also injects `property var controller`, `owner` and `backdrop` (`:9-11`) and reads `owner.width` and `owner.focusView()`. There are 75 such `property var controller/owner/view/host` declarations in `qml/`.

**Fix.** Add the role and a name bound to the heading. Type the injected properties, or make them `required`.

**Effort:** S. **Prefix:** `siderita:`.

### SID-28: Spatial motion ignores reduced motion (Minor, motion)

**Evidence.** These durations use the constant tokens (`CelestinaTheme.motionFast` is always 100 ms, `CelestinaTheme.qml:776-778`):
- `OperationRing.qml:186-187`: scale.
- `OperationRing.qml:92-96,119-121`: arc.
- `SidebarChevron.qml:23-25`: rotation.
- `FolderWheelHandler.qml:106-110`: smooth scroll.

None consults `reducedMotion`. This is recorded as planned debt in `STATUS.md` ("After the shared style motion inventory exists…"), so it is not a regression.

**Fix.** Gate each on `CelestinaTheme.reducedMotion ? 0 : …` when that unit lands.

**Effort:** S. **Prefix:** `siderita:`.

### SID-29: Missing negative tests (Minor, tests)

**Evidence.**
- `paste.rs` tests only planning. Nothing exercises `paste_one` Replace or KeepBoth, undo, or cancellation mid-batch.
- `restore.rs` has no relative-record test.
- `copy.rs` has no concurrent-create test.
- `member.rs` and `tests/archives.rs` have no chained-symlink case.
- `find.rs` has no stale-publication test.

**Fix.** Add the reproduction for each finding to the unit that fixes it; the PoCs in `scratchpad/poc/src/main.rs` are ready to port.

**Effort:** M. **Prefix:** per unit.

### SID-30: Documentation truth (Minor, docs)

**Evidence.**
- `STATUS.md:161,192,197,215,235,241,262,279` describe `SID-A1` and `SID-G7-C` through `-I` as "Uncommitted in the checkout". `git status` is clean, and `siderita-archive` landed in `2e5065d`.
- `STATUS.md:165` says extraction "stages the whole extraction". `extract.rs:12-17` now writes the visible destination.
- `README.md:15-17` says archives are handled "entirely in process", yet RAR/7z are extracted by external tools when installed (`controller/archive.rs:61`, `siderita-archive/src/tool.rs`).
- `controller/trash.rs:19` says "Nothing here reads the filesystem on the Qt thread"; see SID-10.
- `fileops.rs:147` says "the domain reserves every destination name atomically, so two writers … cannot overwrite each other"; see SID-2.
- `purge.rs:13,22` link to a nonexistent `crate::list_home_trash`.
- `volume.rs:153` has the misleading `getuid` comment.

**Fix.** Condense the stale STATUS bullets into history, and correct the README and the four comments.

**Effort:** S. **Prefix:** `siderita:`.

### SID-31: Dead code kept with `#[allow]` (Minor, quick win)

**Evidence.** `volume.rs:242-243`: `/// Why a Trash could not be prepared, for the one caller that must say so.` followed by `#[allow(dead_code)] pub(crate) fn unwritable(...)`. It has no caller. AGENTS.md: "Do not hide debt with `#[allow]`".

**Fix.** Delete it, or use it where `trash_home_for` silently falls back to the home Trash.

**Effort:** S. **Prefix:** `siderita-ops:`.

## Limits

- The `siderita` app crate tests and the QML tests did not run: they
  need the Qt and CXX-Qt build, and production builds were outside the
  brief.
- Nothing that needs a real Wayland session, portal or AT-SPI ran.
- The audit did not re-check Siderita's in-process embedding of the
  Grafita and Fluorita cores (GRA-1, GRA-2, GRA-5, FLU-3) or the
  Qt-thread `open_with` scan (RS-4); the other area records carry them.

## Follow-up

Each finding closes in the program unit that carries it; the program
ids, the rulings and the dependency order are in the
[monorepo audit](2026-09-26-monorepo-audit.md) record, and the units are ledger rows of the plans named
below. The suite rows are in the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md).

| Program | Ledger unit | Plan | Findings of this area it closes |
|---|---|---|---|
| P-4 | `SID-H1-A` | `siderita/docs/plans/active/2026-09-26-hardening.md` | SID-1, SID-17, SID-29 |
| P-8 | `SID-H1-B` | `siderita/docs/plans/active/2026-09-26-hardening.md` | SID-2, SID-3, SID-4, SID-14, SID-15, SID-16, SID-20, SID-29, SID-31 |
| P-15 | `SID-H1-C` | `siderita/docs/plans/active/2026-09-26-hardening.md` | SID-5, SID-6, SID-7, SID-8, SID-9, SID-10, SID-11, SID-12, SID-13, SID-18, SID-19, SID-24, SID-25, SID-26, SID-27, SID-28, SID-29, SID-30 |

Unscheduled backlog (Minor; taken when the file is next touched): SID-21, SID-22, SID-23.

### Proposed units, as the auditor wrote them

The program replaces the component and `suite:` prefixes proposed here
with the owning product's primary prefix, as section 6 of the
[monorepo audit](2026-09-26-monorepo-audit.md) record explains; the grouping below is kept as the
auditor's reasoning.

Ordered by value divided by effort. Each is a `siderita:` bug unit with a ledger entry unless it is a pure component-code commit.

1. **`siderita-archive:` Contain extraction to its root.** Refuse writing through symlinked ancestors, create links last, and check tool output by resolving each link canonically. Add the chained-link and argv-password fixes plus tests. Covers SID-1, SID-17 and part of SID-29. (M)
2. **`siderita-ops:` Make the loss-free verbs keep their promise.** Roll back only what this call created, call `fsync` before removing a source, remove only copied entries, use `rename_without_replacing` for trash and restore, resolve relative `Path=` records, harden volume Trash selection, write local `DeletionDate`, and drop the dead `unwritable`. Add a test for each. Covers SID-2, SID-3, SID-4, SID-14, SID-16, SID-20, SID-31 and part of SID-29. (M)
3. **`siderita:` Make job lifecycle deterministic.** End jobs from the register when the worker finishes, keep and join handles on quit (cancel or ask first), and replay folder changes that were suppressed during jobs. Covers SID-5, SID-6 and SID-7. (M)
4. **`siderita:` Drop stale asynchronous answers.** Refresh must not cancel a navigation in flight; add generations to search, Trash and Recientes listings. Covers SID-8, SID-9 and SID-24. (S)
5. **`siderita:` Take the remaining IO off the Qt thread.** Run undo, restore, purge and empty through the job worker; keep one process-wide D-Bus device model on a worker; run renames, the paste plan, the preview and favourite kinds off-thread; use a bounded, cancellable thumbnail pool. Covers SID-10, SID-11, SID-13 and SID-26. (L)
6. **`siderita-embedded:` Bound the PE reader.** Covers SID-12. (S)
7. **`siderita:` Move file-domain rules out of the controller.** Move paste, conflict and undo into `siderita-ops`, search into `siderita-core` (same-device option), and save-name composition beside `next_available`. Keep one local-time owner with a recorded `unsafe` exception, reuse `hematita_core::passwd`, and share one calendar module (`suite:` part). Fix Replace ordering while the code is moving. Covers SID-15, SID-21, SID-22, SID-23 and SID-25. (L)
8. **`siderita:` Documentation and accessibility cleanup.** Update STATUS and README, correct the stale comments, add dialog roles and names, type the injected properties, escape portal filters, restrict foreign-host URIs, and add reduced-motion gating when the style inventory lands. Covers SID-18, SID-19, SID-27, SID-28 and SID-30. (S)
