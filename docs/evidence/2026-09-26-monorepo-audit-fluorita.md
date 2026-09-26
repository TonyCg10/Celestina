# Evidence: the Fluorita audit

- **Date:** 2026-09-26
- **Scope:** `AUD-1-A` of the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md): Fluorita (`fluorita/`) and `fluorita-core`, `fluorita-engine` and `fluorita-qt`, on `main` at `9d022dd`; one of the seven area records the [monorepo audit](2026-09-26-monorepo-audit.md) consolidates
- **Environment:** read-only audit in a Linux container (kernel 6.18) running as uid 0; rustc and cargo 1.94.1, Python 3.11, Git 2.43.0; no Qt 6 SDK or CXX-Qt build, no libmpv, no Android SDK, NDK or Gradle, no Wayland session, no AT-SPI bus and no real device; Cargo ran `--offline` with its target directory in the session scratchpad, so no production target or cache was touched
- **Artifact:** not applicable

The auditor's report was titled "Audit FLU — Fluorita (app, fluorita-core, fluorita-engine, fluorita-qt)".

## Procedure

The auditor worked read-only from a common brief: no tracked file was
edited, nothing was committed, no production entry ran and no
subagent was spawned. Every finding was verified by reading the code at
the cited path.

```sh
python3 scripts/agent-context.py fluorita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
cargo test -p fluorita-core --offline
cargo test -p fluorita-engine --offline
```

## Result

- **Exit:** the guards printed OK (148 legacy language files ratcheted, no Fluorita growth; the documentation guard printed only the historical `SID-G7-D` erratum notes); `fluorita-core`: 158 tests passed; `fluorita-engine` could not link because the container has no `libmpv`.
- **Observed:** 30 findings: 1 Critical, 19 Important, 10 Minor. The auditor's summary, the findings table and every finding follow unchanged, with their original IDs; the consolidated record merges a few of them across areas without renaming them.

### Auditor's notes

Checkout: `main` at `9d022dd`. Read-only audit. Guards run: `check-architecture-contract.sh` OK, `check-language-contract.py` OK (148 legacy files ratcheted, no Fluorita growth), `check-documentation-contract.sh` OK (only historical SID-G7-D erratum notes). `cargo test -p fluorita-core --offline`: 158 passed. `cargo test -p fluorita-engine` could not link (no `libmpv` in this container). The app crate (CXX-Qt, Qt 6) and the QML lint were not built.

### Executive summary

1. **Healthy:** `fluorita-core` is pure, `forbid(unsafe_code)` and well tested (158 tests pass). The hand-written FLAC/JPEG parsers check every length against the slice. Paths cross the Qt seam as keys as ADR 0008 requires. Hosts use cancellation tokens, and the QML follows the token and reduced-motion rules. Production Rust has no `unwrap`, `expect` or `panic!`.
2. **Top risk, data loss (FLU-1):** a "Replace" whose format does not change overwrites the original in place instead of sending it to the Trash. This covers the editor, every metadata write and batch runs. ADR 0009, the README and two VALIDATION cases all promise a recoverable original.
3. **Top risk, privacy (FLU-2, FLU-15, FLU-16):** edits land through `atomic_file::replace`, a helper written for suite state files. It resets a 0600 photo to umask-default (for example 0644). EXIF stripping leaves XMP GPS in place. Pixelated redaction of text can be reversed.
4. **Top risk, crash (FLU-3):** `Duration::from_secs_f64` is fed doubles that a file controls. Under `panic = "abort"`, a crafted duration kills Fluorita or Siderita.
5. **Library truth (FLU-8/9/10):** an in-place edit leaves a duplicate untagged row. A whole folder moved in or out is never noticed. One deep subtree stops the whole library from ever forgetting deleted files.
6. **Threading (FLU-5/6/7/14/19):** the GUI thread still stats every item on each sidebar click and each watch event. It can join workers that ignore cancellation: a resync scan, a lost cancel, or a portal dialog waiting up to 300 s.
7. **Duplication (FLU-12/13):** Siderita carries a copy of the player's session/surface handshake without Fluorita's two fixes. After a video fails to open, Siderita's preview stops working for the rest of the session.
8. **ADR drift (FLU-18):** the engine ships an unused libx264 encode path (`trailer.rs`), although ADR 0009 says no encoder enters the closure.
9. **Promised but missing (FLU-11):** edit recipes are written but never read, so "a copy stays reopenable" is not implemented.
10. **Docs (FLU-25):** `STATUS.md` still lists F11, F12 and F14 as conditional and a committed change as uncommitted. `projects.toml` omits `siderita-ops` from Fluorita's production inputs (FLU-21).

### Findings

| ID | Severity | Category | path:line | Summary | Effort | Prefix |
|---|---|---|---|---|---|---|
| FLU-1 | Critical | 1 Correctness | celestina-rs/crates/fluorita-engine/src/edit.rs:185 | "Replace" with an unchanged format overwrites the original; it never reaches the Trash (edit, metadata, batch) | M | fluorita-engine: |
| FLU-2 | Important | 2 Security | celestina-rs/crates/celestina-core/src/atomic_file.rs:13 | User media lands through the state-file helper: mode, owner and xattrs are reset (0600 becomes umask default), and a copy can clobber a name that appeared in the meantime | M | suite: |
| FLU-3 | Important | 1 Correctness | celestina-rs/crates/fluorita-engine/src/probe.rs:81 | `Duration::from_secs_f64` on file-controlled doubles panics; release uses `panic = "abort"` | S | fluorita-engine: |
| FLU-4 | Important | 2 Security | celestina-rs/crates/fluorita-engine/src/probe.rs:85 | Probed tags are unbounded and not stripped; an oversized catalogue is then dropped and overwritten | S | fluorita-engine: |
| FLU-5 | Important | 1 Correctness | fluorita/src/library/work.rs:424 | Watch resync scans with a fresh token; `close()` joins it on the GUI thread (up to 120 s) | S | fluorita: |
| FLU-6 | Important | 1 Correctness | celestina-rs/crates/fluorita-engine/src/worker.rs:212 | A cancel that arrives between `submit` and the token swap is lost; `Drop` then joins a full job | S | fluorita-engine: |
| FLU-7 | Important | 3 Performance | fluorita/src/library/project.rs:241 | Projection on the GUI thread stats every item and deep-clones the catalogue (sidebar, watch, trash, artwork) | M | fluorita: |
| FLU-8 | Important | 1 Correctness | fluorita/src/library/work.rs:499 | An in-place replace leaves a stale duplicate record and an untagged new one; the watch never probes | M | fluorita: |
| FLU-9 | Important | 1 Correctness | celestina-rs/crates/fluorita-engine/src/watch.rs:172 | Directory-level moves are dropped: folders moved in never appear, folders moved out stay "available" | S | fluorita-engine: |
| FLU-10 | Important | 1 Correctness | celestina-rs/crates/fluorita-engine/src/library.rs:130 | Truncation is global and permanent: one deep subtree turns off forgetting in every root | M | fluorita: |
| FLU-11 | Important | 1 Correctness | fluorita/src/editor.rs:929 | Edit recipes are written and never read; the store is unbounded and silently reset past 16 MiB | M | fluorita: |
| FLU-12 | Important | 1 Correctness | siderita/src/media.rs:294 | Siderita's copy of the close handshake sets `closing` after clearing the handle and has no stale-handle guard; preview stops working | S | siderita: |
| FLU-13 | Important | 4 Architecture | fluorita/src/player.rs:1083 | Session worker and surface handshake are duplicated in two apps; Fluorita's generation guard misses a plain close | M | suite: |
| FLU-14 | Important | 1 Correctness | fluorita/src/library.rs:831 | Portal dialogs (folder, cover) are joined on the GUI thread; window close hangs up to 300 s; listener thread leaks | M | fluorita: |
| FLU-15 | Important | 2 Security | celestina-rs/crates/fluorita-engine/src/metadata.rs:474 | "Remove location" strips only EXIF APP1; XMP GPS and MPF secondary images survive and are not reported | M | fluorita-engine: |
| FLU-16 | Important | 2 Security | fluorita/cpp/imagecanvas.cpp:57 | Redaction by 1/16 pixelate or 1/24 blur is recoverable for text (Depix class); the code calls it "genuinely irreversible" | S | fluorita: |
| FLU-17 | Important | 6 QML/a11y | fluorita/qml/components/EditObjectLayer.qml:138 | Existing annotations can only be selected with a pointer and expose no accessible role or name | M | fluorita: |
| FLU-18 | Important | 4 Architecture | celestina-rs/crates/fluorita-engine/src/trailer.rs:144 | Unused libx264 encode path in the engine contradicts ADR 0009 | M | fluorita: |
| FLU-19 | Important | 1 Correctness | fluorita/src/player.rs:487 | Opening a still (viewer and editor) runs `stat` and a header read on the GUI thread | M | fluorita: |
| FLU-20 | Important | 1 Correctness | celestina-rs/crates/fluorita-qt/cpp/mpvvideoitem.cpp:191 | Renderer and mpv callback use a raw item pointer outside `synchronize`; no lifetime guarantee | M | fluorita-qt: |
| FLU-21 | Minor | 7 Docs | docs/projects.toml:323 | Fluorita `production_inputs` omit `siderita-ops`, which app and engine link | S | suite: |
| FLU-22 | Minor | 4 Architecture | fluorita/src/library/work.rs:609 | First-run seed guesses folder names; Siderita already parses `user-dirs.dirs` | S | suite: |
| FLU-23 | Minor | 4 Architecture | fluorita/src/folders.rs:210 | Two `file://` decoders in Fluorita disagree on `localhost`; more copies elsewhere in the suite | S | suite: |
| FLU-24 | Minor | 1 Correctness | fluorita/src/mpris.rs:383 | MPRIS `Rate`/`MinimumRate`/`MaximumRate` hard-coded to 1.0 despite F11 speeds | S | fluorita: |
| FLU-25 | Minor | 7 Docs | fluorita/STATUS.md:38 | STATUS/README/C++ comments contradict the checkout | S | fluorita: |
| FLU-26 | Minor | 4 Architecture | celestina-rs/crates/fluorita-engine/src/artwork.rs:85 | Shared-cache PNGs lack `Thumb::URI`/`Thumb::MTime`; validity is not the spec rule | M | fluorita: |
| FLU-27 | Minor | 1 Correctness | celestina-rs/crates/fluorita-engine/src/watch.rs:173 | Watch rejects paths with any dotted ancestor, the scan does not: a root under a hidden directory is never live | S | fluorita-engine: |
| FLU-28 | Minor | 1 Correctness | fluorita/src/player.rs:838 | Six `std::thread::spawn` calls abort on spawn failure; frame extraction is uncancellable and joined in `Drop` | S | fluorita: |
| FLU-29 | Minor | 5 Tests | fluorita/src/player.rs:1399 | Tautological tests (a local helper; constants the C++ never uses) | S | fluorita: |
| FLU-30 | Minor | 4 Architecture | fluorita/src/rasteriser.rs:54 | `#[allow(clippy::…)]` hides debt (3× in the bridge, 1× in trailer) | S | fluorita: |

---

### FLU-1 — Replace overwrites the original instead of trashing it (Critical, correctness)

**Evidence**
- `celestina-rs/crates/fluorita-engine/src/edit.rs:285-286`: a same-extension replace returns `request.source.to_path_buf()` as the destination. Then `:174` runs `atomic_file::replace(&destination, &bytes)`, and `:185` skips the Trash:
  `SaveChoice::Replace if destination == request.source => None,`
- `celestina-rs/crates/fluorita-engine/src/metadata.rs:139-151`: `SaveChoice::Replace => request.source.to_path_buf()`, followed by the same guard. So the third arm (`bin.send`) can never run for metadata.
- Batch goes through the same `save`/`write` (ROADMAP F10: "every item goes through the same single-item save").
- The contract says the opposite. `docs/decisions/0009-editing-without-an-encoder.md:77-79`: "*Replace* writes the new bytes … and sends the original to the desktop Trash through `siderita-ops`. It is never an `unlink`". `fluorita/README.md:29` says the same. `MetadataPanel.qml:12-13` says "a replacement whose original goes to the Trash". `VAL-FLU-EDIT` and `VAL-FLU-METADATA` both expect the original in the Trash.

**Why it matters.** `rename(2)` over the original is an unlink of the old inode. Any same-format replace (JPEG→JPEG, PNG→PNG, every FLAC retag, every EXIF strip, every batch "replace") destroys the original with no recovery, although the published contract promises one. The engine tests assert this behaviour (`edit.rs:718`: "it replaced its own path"), so the drift is baked in.

**Fix.** Write the result to a hidden sibling and `fsync` it. Send the original to the Trash through `siderita-ops`. Then `rename` the sibling onto the original path. If the Trash step fails, delete the sibling and leave the original in place. Update the tests to assert that the original is in the bin. The destination is confirmed before the source moves, which keeps the ADR's ordering rule.

### FLU-2 — User media lands through the state-file primitive (Important, security)

**Evidence**
- `celestina-rs/crates/celestina-core/src/atomic_file.rs:1` "Lossless replacement of small suite-owned state files". Its temporary is created at `:39-43` with `OpenOptions::new().write(true).create_new(true)`, which gives mode `0666 & ~umask` with no `fchmod` and no ownership or xattr copy. `:22` then runs `fs::rename(&temporary, path)`, which overwrites whatever sits at `path`.
- It is used for user files: `fluorita-engine/src/edit.rs:174`, `metadata.rs:143`, `frame.rs:100`.

**Why it matters.** Stripping location "in place" from a private `0600` photo republishes it as `0644` (typical umask), readable by other local users. Group-shared libraries lose `g+w`, and xattrs and ACLs are dropped. For "Copy" outcomes, `next_available` picks a free name and `rename` then clobbers any file created at that name in between, which breaks the keep-both promise.

**Fix.** Add a media-landing function to `celestina-core` beside `atomic_file` (one owner). It should `fchmod` the temporary to the source's mode before the rename (best effort for owner and xattrs). For Copy it should publish with `link()`+`unlink` or `renameat2(RENAME_NOREPLACE)` so an existing name is refused, not replaced. Have the engine call it at all three sites.

### FLU-3 — Panic on backend doubles (Important, correctness/crash)

**Evidence**
- `fluorita-engine/src/probe.rs:78-81`: `.filter(|seconds| seconds.is_finite() && *seconds > 0.0).map(Duration::from_secs_f64)`
- `fluorita-engine/src/session.rs:165,172,175`: the same for `time-pos` and `duration`.
- `fluorita/src/player.rs:544` (seek from QML) and `:833` (`position_seconds`).
- `fluorita/Cargo.toml` release profile: `panic = "abort"`.

**Why it matters.** `Duration::from_secs_f64` panics for any finite value above `u64::MAX` seconds (about 1.8e19). The duration is container-controlled: Matroska's `Duration` element is an IEEE float scaled by `TimecodeScale`, and mpv's own demuxer passes it through. A crafted `.mkv`/`.mka` aborts the whole process when it is opened, or during the tag pass on launch. Siderita's embedded player shares the engine.

**Fix.** Use `Duration::try_from_secs_f64` and treat an error as "unknown". Put that in one helper in `fluorita-core` and use it at all five sites, with a test at `f64::MAX`.

### FLU-4 — Probed tags are unbounded (Important, security)

**Evidence.** `fluorita-engine/src/probe.rs:85-87` returns `instance.optional_string(&format!("metadata/by-key/{key}"))`, and `instance.rs:165-170` only trims it. Compare `fluorita-core/src/streams.rs:80-85`, which strips control characters and applies `.take(MAX_LABEL_CHARACTERS)`, and `metadata.rs:31`, which sets `MAX_TAG_CHARACTERS = 512` for writes.

**Why it matters.** The local contract says names, tags, dimensions and duration are hostile and must be bounded before allocation. A multi-megabyte ID3 or Vorbis title flows into the catalogue, into every `QStringList` publication and into the persisted TSV. Once the TSV passes `MAX_BYTES` (`catalogue_store.rs:45`), `load` errors and `run_scan` silently falls back (`work.rs`: `_ => Catalogue::new()`). The next save overwrites the file, so every learned tag is lost on each launch.

**Fix.** Apply one sanitizer to every probed tag: lift `streams::bounded` into a core helper and cap at `MAX_TAG_CHARACTERS`. Log and keep a stored catalogue that fails to load instead of overwriting it.

### FLU-5 — Resync scan cannot be cancelled (Important, correctness/threading)

**Evidence.** `fluorita/src/library/work.rs:421-425`:
`fluorita_engine::scan(sources, ScanLimits::conservative(), &celestina_core::CancellationToken::new())`. The host's `close()` (`library.rs:413-420`) cancels its own token and then calls `handle.join()` on the GUI thread.

**Why it matters.** A burst over 512 paths, or a watcher error, starts a scan of up to 120 s that ignores the host token. Adding or removing a folder, or closing the window, during that scan freezes the GUI until it ends. The file's own comment at `CANCEL_POLL` records this exact freeze as fixed.

**Fix.** Pass the watch loop's `cancellation` into the resync scan. On `Err(Cancelled)` return, don't `continue`.

### FLU-6 — Lost cancellation in `EngineWorker` (Important, correctness/race)

**Evidence.** `fluorita-engine/src/worker.rs:212-215` creates the token only when the thread dequeues the job (`let token = CancellationToken::new(); … *slot = token.clone();`). `cancel_current` (`:179-185`) cancels whatever token is in the slot at that moment.

**Why it matters.** A `cancel_current()` or `shutdown()` that runs after `submit` but before the worker thread swaps in the new token cancels the previous job's token. The new job then runs to completion: a scan for up to 120 s, a probe for 15 s. The host joins it from the GUI thread (`start_scan` → `close`), for example when a folder is removed right after launch.

**Fix.** Create the token in `submit` and send it inside `Message::Work`, or keep a shutdown `AtomicBool` that the worker checks before starting each job. Add a test that cancels immediately after `submit` against a blocking fake engine.

### FLU-7 — GUI-thread projection stats every item and deep-clones the catalogue (Important, performance/thread affinity)

**Evidence**
- `fluorita/src/library/project.rs:241`: `if !entry.is_file()` runs once per gallery row and once per track. `:162-165` calls `pending_artwork(…)`, which runs another `fs::metadata` per video or audio record (`artwork.rs:326`). `:185` does `catalogue: catalogue.clone()`, a deep clone.
- These run on the GUI thread from `select_source` (`library.rs:431-440`), `item_trashed`, `artwork_finished`, and every watch batch (`work.rs:449-455`: `library.rs`-side closure `project(&published, …)`).
- The `Arc<Catalogue>` introduced for FLU-P1 (`library.rs:291-296`) is then re-wrapped from a fresh clone in `apply`.

**Why it matters.** At the documented scale (50 000 records), each sidebar click or file change costs about 55 000 `stat` calls plus a full clone on the GUI thread. On an sshfs root, which `metadata.rs:202-205` names as a real case, the window freezes. This breaks "blocking IO never runs on the Qt thread" and undoes FLU-P1 for every path except search.

**Fix.** Route every projection through the worker path that search already uses, with a revision check. Carry `Arc<Catalogue>` in `LibrarySnapshot` instead of a clone. Resolve thumbnails once per publication on the worker.

### FLU-8 — In-place replace leaves a duplicate stale record and loses tags (Important, correctness)

**Evidence**
- `fluorita/src/library/work.rs:465-501`: `absorb_one` builds a record keyed by the new `(dev, ino)` and calls `catalogue.absorb([record], false)`. The old record, with the same path and the old inode, is never removed, because no `Removed` event arrives for a path that still exists.
- The watch loop never probes tags (`learn_tags` runs only in `run_scan`).
- ROADMAP F8 claims "the scan and the watch drop its extracted metadata and probe the file again", and README promises the library re-sorts under the corrected name.

**Why it matters.** Every rename-over write changes the inode: a metadata replace, an edit replace or a batch replace. After a tag correction, Music shows the old row with the old tags and a new row with no tags under "unknown artist" until the next launch. VAL-FLU-METADATA ("watch where it sorts") will fail.

**Fix.** In `absorb_one`, forget any record with the same path and a different id. From the watch thread, queue a bounded probe for touched audio files, reusing `learn_tags` with a small cap.

### FLU-9 — Directory moves are invisible to the watch (Important, correctness)

**Evidence.** `fluorita-engine/src/watch.rs:147-151` checks `is_library_item` per path, and `:172-180` returns `MediaKind::classify_path(path).is_some()`, which is false for a directory. `notify` reports a single event for `mv album/ <music folder>/` (the author's Music folder has a Spanish name), and the same for moving a folder out.

**Why it matters.** Moving a folder of photos into a root shows nothing until the next launch. Moving one out, or `mv`-ing it to another disk, leaves every record `Available`. Those items render and then fail on click ("no longer in the library" or backend errors).

**Fix.** In `interpret`, turn a create, rename or remove of a directory (no media extension, `is_dir()` true, or an absent path with no extension) into `Resync`. Alternatively do a bounded sub-walk of that directory.

### FLU-10 — Truncation disables forgetting for every root, forever (Important, correctness)

**Evidence.** `fluorita-engine/src/library.rs:130-132` (`if depth > limits.max_depth { outcome.truncated = true; … }`) and `:205-207` (file cap) set a single global flag. `fluorita/src/library/work.rs:329-331` and `:431` only call `forget_vanished` when `complete`.

**Why it matters.** The depth cap of 12 is a permanent property of a tree. One deep directory anywhere, such as a photo export tree or a git checkout inside Pictures, makes every scan "incomplete". The F6 fix ("deleted files kept appearing") then never applies to any root. The same happens once any library passes 50 000 files.

**Fix.** Record completeness per root (`reached_complete: BTreeSet<SourceId>`). Treat depth-capped subtrees as "not seen, not missing": keep records under capped directories and forget elsewhere. Add a test with one deep and one shallow root.

### FLU-11 — Edit recipes are write-only (Important, correctness/docs)

**Evidence**
- `fluorita/src/editor.rs:903-943` `remember` loads the store, inserts and saves.
- Nothing calls `EditStore::get` or `usable` outside `edit_store.rs` (repo-wide search). `open_item` (`editor.rs:319-373`) always starts `EditDocument::new(canvas, …)`.
- `edit_store.rs:74-76` never evicts. At `:43` `MAX_BYTES = 16 MiB`, and `remember` then does `.unwrap_or_default()`, which silently resets the store.
- README line 28 ("a copy … which stays reopenable") and `editor.rs:21-22` claim otherwise.

**Why it matters.** The first half of ADR 0009's save contract is not implemented. VAL-FLU-EDIT ("reopen the copy and check that the crop can still be undone") fails by construction. The store also grows without bound and is then wiped with no notice.

**Fix.** In `open_item`, off the GUI thread (see FLU-19), look up `usable(result_id, current_base_identity)` and rebuild the document from it. Bound the store (LRU by count), forget an entry when its file is trashed, and warn instead of resetting when a load fails.

### FLU-12 — Siderita's copy of the close handshake leaves preview stuck (Important, correctness; overlaps the SID audit)

**Evidence**
- `siderita/src/media.rs:293-296`: `self.as_mut().set_render_handle(0); self.as_mut().rust_mut().closing = true; return;`. The order is the reverse of Fluorita's fix (`fluorita/src/player.rs:706-711`: "Marked before the handle is cleared, not after").
- `siderita/src/media.rs:435-437` queues `set_render_handle(address)` with no generation guard (Fluorita added one in `fcc6fbf`).
- `siderita/qml/dialogs/MediaPreview.qml:37-39` only shows the `MpvVideo` after confirmed playback. An item that never rendered therefore has zero claims, and `setHandle(0)` emits `contextReleased` synchronously (`mpvvideoitem.cpp:238-240`).

**Why it matters.** Open a video that fails or is still "abriendo", then press Space on another file or close the modal. `surface_released` runs before `closing` is set, ignores the release, and then `closing = true` sticks. From then on `request_preview` parks every request (`pending_request`) and never resumes: embedded playback is dead until Siderita restarts. A stale queued handle for an already-destroyed `mpv_handle` can also be republished after `stop_worker`.

**Fix.** Port the ordering and the generation guard now (siderita:), and resolve the root cause with FLU-13.

### FLU-13 — The session/surface handshake has two owners (Important, architecture)

**Evidence.** `fluorita/src/player.rs:1083-1211` (`run_session`, `Command`, `close`/`surface_released`/`decide_open`/`pending_open`, `state_label`) and `siderita/src/media.rs:283-481` implement the same pure-Rust protocol independently. Fluorita's guard is also incomplete: `close()` never bumps `generation` (`player.rs:444-445` is the only writer). A plain close that runs before the queued `set_render_handle` closure (`:1136-1145`) still publishes a freed handle. The test at `:1399-1409` checks a local `fn publishes` and asserts "a close bumped past this session", which never happens.

**Why it matters.** This is the reuse rule's canonical case: one invariant, two owners that have already diverged (FLU-12). Under ADR 0005 the bridge should own the shared lifecycle intersection. The protocol has no Qt in it, so it belongs in `fluorita-core` (state machine) or `fluorita-engine` (worker loop).

**Fix.** Extract a `SurfaceHandshake` state machine (open / close / released / parked) and a session-worker loop that takes a publish callback, with generation bumped on every close. Both hosts then only marshal. Replace the tautological test with tests on the extracted type.

*Note on re-entrancy.* Both hosts depend on `setHandle(0)` emitting `contextReleased` synchronously, which calls back into another `Pin<&mut Self>` invokable while the outer `&mut` is live. The extracted state machine should make that call idempotent, or queue it, instead of relying on aliasing that Rust does not permit.

### FLU-14 — Portal waits are joined on the GUI thread (Important, correctness/threading)

**Evidence**
- `fluorita/src/folders.rs:36` sets `DEADLINE = 300 s`.
- `library.rs:827-833` (`Drop`: "Joining it is the only way…") joins the folder worker.
- `metadata.rs:301-302` (`close` → `cancel_worker`) and `:481-487` (`let _ = worker.join();`) join while `choose_cover` holds a worker blocked in the portal (`:381-383`).
- `folders.rs:120-130` spawns a detached `fluorita-portal` listener that only ends when a *later* signal arrives.

**Why it matters.** Closing the window while the folder or cover chooser is open freezes the GUI until the person answers the dialog or 300 s pass. The chooser is not really modal: no parent handle is passed (`folders.rs:157-160`). Each request also leaks a thread and a match rule. This contradicts "No detached thread outlives its host".

**Fix.** Keep the request handle and call `org.freedesktop.portal.Request.Close` on cancel or drop. Wait with the host token instead of a bare deadline. Stop the listener by dropping its connection. Do not join a portal worker from `Drop` without first closing its request.

### FLU-15 — "Remove location" leaves XMP GPS (Important, security/privacy)

**Evidence.** `fluorita-engine/src/metadata.rs:474`: `let is_exif = marker == 0xE1 && … == Some(b"Exif\0\0");`. Only EXIF APP1 is dropped, and everything after SOS is copied verbatim (`:481`). `private_facts` (`:408`) looks only at the EXIF IFDs.

**Why it matters.** Phone and editor exports often carry `exif:GPSLatitude`/`GPSLongitude` in an XMP APP1 packet, and MPF (APP2) secondary images carry their own EXIF after the first EOI. The panel then reports no location, and the file still gives the address away. That is the exact threat F8 names ("a photograph about to leave this machine").

**Fix.** Drop APP1 segments starting with `http://ns.adobe.com/xap/1.0/\0` (and extended XMP), and report XMP presence as `Location` when it contains GPS keys. Either strip MPF and truncate after the primary EOI, or refuse the file. Add fixtures for each case.

### FLU-16 — Redaction is reversible for text (Important, security)

**Evidence.** `fluorita/cpp/imagecanvas.cpp:53-72`: blur shrinks by `/24` and pixelate by `/16` and scales back. The comment claims "genuinely irreversible, which is the property a redaction needs".

**Why it matters.** Fixed-block pixelation of screenshot text is recoverable with public tools (the Depix class of attacks). A screenshot with an address or number is the use case the roadmap cites. The preview draws an opaque plate (`EditObjectLayer.qml:102-116`), so the user trusts a result that is weaker than what they saw.

**Fix.** Make a solid fill the default redaction. If pixelation stays, use a block size proportional to the region (at least 1/8 of its short side), add quantisation or noise, and word the option honestly.

### FLU-17 — Annotations are pointer-only (Important, QML/a11y)

**Evidence.** `fluorita/qml/components/EditObjectLayer.qml:138-140` has only a `TapHandler { onTapped: objectLayer.objectPicked(…) }` and no `Accessible.*`. `EditSurface.qml:543-583` moves or deletes only the *already selected* object, and there is no invokable that selects an existing object by keyboard (repo search for `selectObject`).

**Why it matters.** "Every action works with keyboard and assistive technology". A mark placed earlier cannot be reached, moved or deleted without a pointer, and screen readers get nothing for it.

**Fix.** Add `selectNextObject(forward)` to the editor, bind Tab or Shift+Tab (or `[`/`]`) inside the surface, and give each delegate `Accessible.role`/`name` (kind plus text) with `selected` state.

### FLU-18 — A dormant encoder contradicts ADR 0009 (Important, architecture)

**Evidence**
- `fluorita-engine/src/trailer.rs:125-147` builds an mpv instance with `("o", output)`, `("of", "mp4")`, `("ovc", "libx264")`, `("ovcopts", "preset=veryfast,crf=28")`.
- `produce_trailer` / `Job::Trailer` / `TrailerLease` have no consumer in `fluorita/src` or `siderita/src` (only `tests/real_media.rs`). F14 uses a live session instead.
- ADR 0009:18-20 says libmpv "does not encode", and `:64` says "No encoder enters the closure".

**Why it matters.** The engine carries an H.264 encode path, an extra hostile-input and CPU surface, that the accepted decision says does not exist. The core's preview types keep an unused API alive.

**Fix.** Remove `trailer.rs`, `Job::Trailer`, `TrailerJob`/`TrailerOutcome` and the unused core preview types, with their tests. Alternatively record an ADR exception. Correct the ADR's factual claim.

### FLU-19 — Stills are measured on the GUI thread (Important, thread affinity)

**Evidence.** `fluorita/src/player.rs:487-500` (`show_image`: `std::fs::metadata(path)` plus `probe_image`, which opens the file and reads its header) and `editor.rs:341-342` are called from invokables. `metadata.rs:202-205` already explains why this is wrong: "four megabytes across a link like that is a frozen window".

**Why it matters.** Every filmstrip step or editor open does file IO on the GUI thread, and it hangs on network roots.

**Fix.** Run the probe on a worker with a generation. Publish the still (or refusal) and the editor canvas when it returns.

### FLU-20 — Render seam uses the item outside `synchronize` (Important, correctness; fluorita-qt)

**Evidence.** `celestina-rs/crates/fluorita-qt/cpp/mpvvideoitem.cpp:191-192` (in `releaseContext`, also called from `~MpvRenderer`): `QMetaObject::invokeMethod(m_item, "notifyContextReleased", …); m_item->settleRenderContext();`. At `:33-34`, `onMpvUpdate` calls `invokeMethod(static_cast<MpvVideoItem *>(context), …)` from mpv's thread.

**Why it matters.** Qt documents that a `QQuickFramebufferObject::Renderer` must not touch its item outside `synchronize()`. The renderer is destroyed on the render thread after its item may already be deleted (window teardown, a delegate destroyed while live). The QML visibility trick (`PlayerSurface.qml:95`) covers normal flows but not destruction. The result is a use-after-free at exit or during delegate recycling, which VAL-FLU-TEARDOWN is probing for.

**Fix.** Share a small thread-safe state object (for example `QSharedPointer` to a struct with the atomic claim count, plus a `QPointer`/weak target for notifications) between item and renderer. The renderer keeps it alive and never dereferences the raw item.

### FLU-21 — Registry omits a linked crate (Minor, docs/verification)

**Evidence.** `docs/projects.toml:323` Fluorita `production_inputs` lists `celestina-core`, `fluorita-*` but not `celestina-rs/crates/siderita-ops`. Yet `fluorita/Cargo.toml` and `fluorita-engine/Cargo.toml` both depend on it (trash, `next_available`).

**Why it matters.** A change to `siderita-ops` leaves Fluorita's artifact reported current, so the landing will not rebuild or redeploy it.

**Fix.** Add the path, and consider a guard that derives inputs from `cargo metadata` path dependencies.

### FLU-22 — First-run seeding guesses XDG dirs (Minor, reuse)

**Evidence.** `fluorita/src/library/work.rs:609-611`: `existing(&[...])` over the Spanish and the English name of the Pictures folder, and the same for Videos and Music. `siderita/src/places.rs:24-80` parses `$XDG_CONFIG_HOME/user-dirs.dirs`.

**Why it matters.** README says Fluorita seeds "from the existing XDG Pictures, Videos and Music directories". A user whose `XDG_PICTURES_DIR` is elsewhere gets no seed. The same rule has two implementations.

**Fix.** Lift the `user-dirs.dirs` parser into `celestina-core::xdg` and use it from both apps.

### FLU-23 — Duplicate `file://` decoders (Minor, reuse)

**Evidence.** `fluorita/src/folders.rs:210-221` rejects `file://localhost/…`, while `fluorita/src/activation.rs:84-99` accepts it. Further copies exist in `grafita/src/url.rs:18`, `siderita/src/dbus.rs:137` and `celestina/src/provider_adapter/media.rs:170`.

**Why it matters.** The same URI gets different answers within one app. The inverse (`file_uri`) already has one owner.

**Fix.** Add `percent::local_path_from_file_uri` to `celestina-core` and delegate to it everywhere.

### FLU-24 — MPRIS misreports the rate (Minor)

**Evidence.** `fluorita/src/mpris.rs:383-396`: `rate`, `minimum_rate` and `maximum_rate` all return `1.0`, while F11 offers rates from `Speed` and the session confirms them.

**Fix.** Publish the confirmed speed and the `Speed` bounds, and handle `Rate` writes as `PlaybackRequest::SetSpeed`.

### FLU-25 — Documentation contradicts the checkout (Minor, docs)

**Evidence**
- `fluorita/STATUS.md:38` says "not yet committed", but the change landed in `2ff8738`.
- `:62` and `:64` still list trailer-on-hover, stream selection, speed and continuation as conditional; F11, F12 and F14 are delivered.
- `fluorita/README.md:70` describes `cpp/` as the image-probe seam only; `cpp/imagecanvas.cpp` is also there.
- `fluorita-qt/cpp/mpvvideoitem.cpp:142-146` still says "Advanced control hands the timing to the backend", although `ADVANCED_CONTROL` was removed (`docs/history/roadmap-through-2026-08-03.md:685-686`).

**Fix.** Update those lines. This is a docs-only unit.

### FLU-26 — Shared thumbnails violate the freedesktop spec (Minor)

**Evidence.** `fluorita-engine/src/artwork.rs:85-111` writes the PNG straight from `vo=image`. `Thumb::URI` and `Thumb::MTime` are not written anywhere in the area. `ArtworkValidity` compares the cache file's mtime with the source mtime (`artwork.rs:326-330`) instead of `Thumb::MTime` equality.

**Why it matters.** Other desktop thumbnailers treat these entries as invalid and regenerate or overwrite them. A source restored with an older mtime keeps a stale thumbnail.

**Fix.** Add the two `tEXt` chunks after rendering, or re-encode through the toolkit (still no new dependency), and validate by the spec rule. This must stay in step with Siderita's reader.

### FLU-27 — Watch and scan disagree on hidden ancestors (Minor)

**Evidence.** `fluorita-engine/src/watch.rs:173-176` rejects a path if *any* component starts with `.`, including the root's own ancestors. The scan (`library.rs:162`) only checks entry names below the root.

**Why it matters.** A root under a dotted directory (for example `~/.var/app/…/Pictures`) scans correctly, but no live update is ever applied. That contradicts the module's "so the watch and the walk agree".

**Fix.** Test only the components relative to the owning root, using `SourceSet::owner_of`.

### FLU-28 — Aborting spawns and an uncancellable frame job (Minor)

**Evidence.** `std::thread::spawn` at `player.rs:838`, `metadata.rs:245/381/447`, `batch.rs:160` and `editor.rs:675` panics (abort) if the OS refuses a thread. `library.rs` uses `Builder::spawn` and handles the error. Frame extraction passes `&celestina_core::CancellationToken::new()` (`player.rs:847`), and `Drop` joins it (`:1064`) for up to `FRAME_DEADLINE = 30 s`.

**Fix.** Use `Builder::spawn` and report failure like `library.rs` does. Give the frame job a host-owned token that `Drop` cancels before joining.

### FLU-29 — Tautological tests (Minor, tests)

**Evidence.** `fluorita/src/player.rs:1399-1409` tests a locally defined `fn publishes(current, published_by)`, not the queued closure, and its message describes behaviour the code does not have (see FLU-13). `fluorita-qt/src/lib.rs` tests `QML_MODULE`/`QML_TYPE` against literals, while the C++ registers `"org.celestina.fluorita.render", "MpvVideo"` by hand (`mpvvideoitem.cpp:297`) and never reads the constants.

**Fix.** Test the extracted handshake type (FLU-13). Either generate the C++ registration strings from the constants, or drop the constants.

### FLU-30 — `#[allow]` used to hide debt (Minor)

**Evidence.** `fluorita/src/rasteriser.rs:54,66,97` use `#[allow(clippy::too_many_arguments)]` on bridge functions (`draw_text` takes 11 parameters). `fluorita-engine/src/trailer.rs:105` uses `#[allow(clippy::cast_precision_loss, …)]`.

**Why it matters.** AGENTS: "Do not hide debt with `#[allow]`".

**Fix.** Pass a `cxx` shared struct (`TextSpec`, `ShapeSpec`) across the bridge. The trailer allow disappears with FLU-18.

## Limits

- `cargo test -p fluorita-engine` could not link (no `libmpv`), so the
  real-media tests, FLU-20's teardown use-after-free and Siderita's
  embedded player are reasoned from the code, not observed.
- The app crate (CXX-Qt, Qt 6) was not built and QML lint did not run.
- No Wayland session, portal or AT-SPI check ran.

## Follow-up

Each finding closes in the program unit that carries it; the program
ids, the rulings and the dependency order are in the
[monorepo audit](2026-09-26-monorepo-audit.md) record, and the units are ledger rows of the plans named
below. The suite rows are in the [monorepo hardening plan](../plans/active/2026-09-26-monorepo-hardening.md).

| Program | Ledger unit | Plan | Findings of this area it closes |
|---|---|---|---|
| P-2 | `AUD-1-D` | `docs/plans/active/2026-09-26-monorepo-hardening.md` | FLU-21 |
| P-6 | `RS-H1-A` | `celestina-rs/docs/plans/active/2026-09-26-hardening.md` | FLU-2 |
| P-7 | `FLU-H1-A` | `fluorita/docs/plans/active/2026-09-26-hardening.md` | FLU-1, FLU-2, FLU-3, FLU-4, FLU-6, FLU-15, FLU-16 |
| P-14 | `FLU-H1-B` | `fluorita/docs/plans/active/2026-09-26-hardening.md` | FLU-5, FLU-7, FLU-8, FLU-9, FLU-10, FLU-11, FLU-13, FLU-14, FLU-17, FLU-18, FLU-19, FLU-20, FLU-23, FLU-24, FLU-25, FLU-26, FLU-27, FLU-28, FLU-29, FLU-30 |
| P-15 | `SID-H1-C` | `siderita/docs/plans/active/2026-09-26-hardening.md` | FLU-12 |

Unscheduled backlog (Minor; taken when the file is next touched): FLU-22.

### Proposed units, as the auditor wrote them

The program replaces the component and `suite:` prefixes proposed here
with the owning product's primary prefix, as section 6 of the
[monorepo audit](2026-09-26-monorepo-audit.md) record explains; the grouping below is kept as the
auditor's reasoning.

Ordered by value over effort. Each unit uses one prefix.

1. **`fluorita-engine:` Land edits safely and bound what files claim.** Make Replace trash the original before the result takes its name, and cap and sanitise probed tags and backend durations. Also fix the worker's lost cancellation and strip XMP/MPF location data. Covers FLU-1, FLU-3, FLU-4, FLU-6, FLU-15.
2. **`suite:` One landing primitive and one owner for shared path rules.** Add a media-landing function to `celestina-core` that preserves mode and refuses to clobber, adopted by the engine. Lift `user-dirs.dirs` parsing and `file://` decoding into `celestina-core`, and register `siderita-ops` as a Fluorita production input. Covers FLU-2, FLU-21, FLU-22, FLU-23.
3. **`siderita:` Port the two handshake fixes to the embedded player.** Set `closing` before clearing the handle, and add a generation guard on handle publication. Covers FLU-12. This is small, urgent, and overlaps the SID audit.
4. **`fluorita:` Keep the catalogue true and off the GUI thread.** Make resync scans cancellable and project on a worker with `Arc` sharing. Drop same-path stale records and probe touched audio. Turn directory events into resyncs, track completeness per root, and fix the dotted-ancestor check. Covers FLU-5, FLU-7, FLU-8, FLU-9, FLU-10, FLU-27.
5. **`suite:` One owner for the playback session and surface handshake.** Extract the handshake state machine and session loop into `fluorita-core`/`fluorita-engine`, make both hosts marshal only, and make the render seam hold shared state instead of a raw item. Covers FLU-13, FLU-20, FLU-29.
6. **`fluorita:` Make the editor keep its promises.** Reopen copies from stored recipes with a bounded store, redact with a solid fill by default, add keyboard and accessible selection of annotations, and measure stills on a worker. Covers FLU-11, FLU-16, FLU-17, FLU-19.
7. **`fluorita:` Threads that end, and docs that match.** Close portal requests on cancel or drop, use `Builder::spawn`, and make the frame job cancellable. Publish the real MPRIS rate. Remove the dormant trailer encoder, write spec-valid thumbnail chunks, replace `#[allow]` with typed bridge structs, and correct STATUS, README and the C++ comment. Covers FLU-14, FLU-18, FLU-24, FLU-25, FLU-26, FLU-28, FLU-30.
