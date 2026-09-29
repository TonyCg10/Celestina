# Evidence: a true library off the GUI thread, reopenable copies and one playback handshake

- **Date:** 2026-09-27
- **Scope:** `FLU-H1-B` (program unit P-14) of the
  [FLU-H1 plan](../plans/active/2026-09-26-hardening.md): FLU-5, FLU-7,
  FLU-8, FLU-9, FLU-10, FLU-11, FLU-13, FLU-14, FLU-17, FLU-18, FLU-19,
  FLU-20, FLU-23, FLU-24, FLU-25, FLU-26, FLU-27, FLU-28, FLU-29 and FLU-30 of
  the [Fluorita audit](../../../docs/evidence/2026-09-26-monorepo-audit-fluorita.md),
  program row P-14 of the
  [monorepo audit](../../../docs/evidence/2026-09-26-monorepo-audit.md); two
  follow-ups carried from earlier reviews (file names shown in the interface,
  and a Trash by copy that leaves the file in place)
- **Environment:** session worktree `fluorita-FLU-H1-B` on branch
  `unit/fluorita/FLU-H1-B`, stacked on `unit/fluorita/FLU-H1-A` (`0fb208e`),
  which sits on `unit/celestina-rs/RS-H1-A`; Linux container (kernel 6.18)
  running as uid 0; rustc and cargo 1.94.1; Cargo `--offline` with a private
  target directory in the session scratchpad; no Qt 6 SDK or CXX-Qt build, no
  C++ toolchain run against Qt, no libmpv, no `qmllint`, no Wayland session,
  no portal or D-Bus session bus and no AT-SPI bus
- **Artifact:** the landing builds it (Fluorita `complete-production.sh`, and
  Siderita and Magnetita, which link `fluorita-core`, `fluorita-engine` and
  the `fluorita-qt` seam)

## Procedure

```sh
cd celestina-rs
cargo test -p fluorita-core -p fluorita-qt -p celestina-core --offline
cargo test -p fluorita-engine --lib --offline --no-run   # then one process per test, below
cargo clippy -p fluorita-core -p fluorita-engine -p fluorita-qt --all-targets --offline -- -D warnings
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo fmt --all -- --check
cargo doc -p fluorita-core -p fluorita-engine --no-deps --offline
cd ../fluorita && cargo fmt -- --check && cd ..
rg -n libx264 celestina-rs
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

`fluorita-engine` links the system `libmpv`, which this container lacks, so
its unit tests were linked against the stand-in the `FLU-H1-A` record
describes — a shared library whose `mpv_*` functions call `abort()` — and run
one test per process, so the four tests that reach the backend abort alone.

The Fluorita application crate cannot be built here (CXX-Qt needs Qt), so it
was type-checked and its Qt-free tests run through a mirror in the session
scratchpad: every file of `fluorita/src` copied unchanged, except that each
`#[cxx_qt::bridge]` and `#[cxx::bridge]` module is replaced by a stub module
of the same shape — the generated getters (`&T`), setters (`T` by value),
`rust`, `rust_mut` (`Pin<&mut T>`) and `qt_thread`, the declared C++
functions, and the canvas's shared structs — against stand-in `cxx-qt` and
`cxx-qt-lib` crates. Then `cargo build --tests`, `cargo clippy --tests` and
`cargo test` on that mirror.

## Result

- **Exit:** every command above exited 0; `rg -n libx264 celestina-rs` printed
  nothing.
- **Tests:** `fluorita-core` 173 passed (160 before the unit); `celestina-core`
  82; `fluorita-qt` 4; `fluorita-engine` lib 164 passed, 0 failed, and the same
  4 reach libmpv (143 before). The app mirror: 61 passed; the 7 tests that
  call the toolkit (the image probe and the canvas) panicked on the stub, as
  they must without Qt.
- **Clippy:** clean with warnings denied on the three crates and on the whole
  workspace. On the app mirror the one warning is `nonminimal_bool` in
  `player.rs::preview`, a line this unit does not touch.
- **Docs:** `cargo doc` prints 2 warnings, both on lines this unit does not
  touch (it made `edit_store::MAX_BYTES` public, which removed a third).
- **Guards:** architecture OK, language OK (148 legacy files; the
  `fluorita/src/player.rs` row lowered from 2 to 1 because the session loop
  and its sentence left that file), documentation OK.

### RED and GREEN

- **Written first, RED by compilation:** the core coverage tests
  (`ScanCoverage`, `absorb_changed`, `forget_under`), the engine's per-root
  scan tests, the watch's folder tests (`interpret` against the roots), the
  displayed-name tests, the save-over-a-reopened-copy tests, the edit-store
  bound and recency tests, and the thumbnail-key tests.
- **RED by assertion:** `copy::a_name_cannot_break_or_disguise_the_sentence`
  failed on the old `replacement_kept`.
- **RED by mutation** (the fix reverted in place, the test run, the file
  restored):
  - `edit_store::usable` comparing raw identities: the round-trip test failed —
    a recipe written to disk was never usable again, because the store keeps
    whole seconds and the base was measured in nanoseconds. That latent
    defect is why wiring the reopen alone would not have closed FLU-11.
  - `SurfaceHandshake::close` not moving the generation on a close before
    publication: 2 handshake tests failed.
  - The depth bound stopping the root instead of leaving the subtree
    unexplored: `depth_is_bounded` and
    `one_deep_root_does_not_stop_another_from_being_judged` failed.
  - `interpret` ignoring folders: the three folder tests failed.
- **GREEN:** everything above passes.

### Per finding

- **FLU-5.** The watch's resync walks under the host's token; a cancelled
  walk returns instead of continuing (`library/work.rs::watch_library`).
- **FLU-7.** Every re-projection — selection, query, trash, new artwork, a
  watch batch, a scan snapshot for a scope the person has since left — goes
  through one projection worker with a ticket (`library.rs::request_projection`,
  `finish_projection`). `LibrarySnapshot` carries `Arc<Catalogue>`; the watch
  makes its one copy per batch on its own thread and hands the host an `Arc`.
  No projection, `stat` or pending-artwork count runs on the GUI thread.
- **FLU-8.** `Catalogue::absorb_changed` drops a record at the same path under
  another identity; the watch probes the audio a batch touched (at most 32 per
  batch, and after a resync the untagged audio it brought in), through the
  scan's worker and under the host's token.
- **FLU-9.** The watch judges each path against the root that holds it: a
  folder created or moved in asks for a walk; a path that left and is not a
  known file takes every record under it (`LibraryChange::RemovedTree`,
  `Catalogue::forget_under`); a root that itself changed asks for a walk,
  which keeps an unplugged drive's items; attribute changes on a folder are
  ignored.
- **FLU-10.** `ScanCoverage` (core) records per pass which roots were walked
  to the end, which answered, and which directories were not read (below the
  depth bound, or unreadable below the root). Only records it judges are
  marked missing, and only those under a walked root that answered are
  forgotten. A ceiling or deadline leaves the current and later roots
  unjudged; past 1 024 unexplored directories a root is not judged.
- **FLU-11.** The engine store keeps recency (oldest first on disk), holds at
  most 512 recipes, never writes past its byte bound (oldest dropped whole),
  compares a base at the precision it stores, and replays a recipe through
  the document (`StoredEdit::reopen`), so every step can be undone. The app's
  `recipes.rs` remembers after a copy lands, reopens a copy on the editor's
  opener thread, and forgets when a file is trashed or replaced; a store that
  cannot be read is reported and never saved over, and one process-wide lock
  serialises its read-modify-write. A reopened copy renders from its original
  and saves beside or over itself (`SaveRequest::target`): a replacement sends
  the copy to the Trash, never the original.
- **FLU-13.** `fluorita_core::SurfaceHandshake` owns the protocol (open,
  close, released, publishes; every close moves the generation; a release is
  taken once; a newer open replaces a parked one; an explicit close drops
  it). `fluorita_engine::run_session` owns the session loop behind a
  `SessionHost`, tested against a fake engine. Fluorita's player only carries
  out their steps; its snapshots and failures are also dropped when they
  belong to a session the player has left. Siderita adopts both in P-15.
- **FLU-14.** The portal request runs on a private connection; the wait
  watches the host's token; an unanswered request is closed with
  `org.freedesktop.portal.Request.Close`; the connection is closed, which
  ends the listener's stream, and the listener is joined within a one-second
  grace. The library's folder chooser and the metadata panel's cover chooser
  both pass their host's token, and `Drop` cancels before joining.
- **FLU-17.** `select_next_object(forward)` steps the selection through the
  marks in drawing order and wraps; the surface binds `]` and `[` (Tab stays
  focus navigation). Every mark exposes `Accessible.role`, a name in the
  toolbar's own words (a text's words included), `selectable`, `selected`
  and a press action that selects it.
- **FLU-18.** `trailer.rs`, `Job::Trailer`, `TrailerJob`, `TrailerOutcome`,
  `MediaEngine::produce_trailer`, the unused `fluorita-core::preview` module
  and the four trailer tests in `tests/real_media.rs` are gone;
  `rg -n libx264 celestina-rs` is empty.
- **FLU-19.** Stills are measured on a worker by one helper,
  `image::ImageDecision::measure` over `player::probe_still`, in the viewer
  (under the session's generation, the newest request waiting) and in the
  editor's opener; the batch and the cover chooser use the same probe.
- **FLU-20.** `MpvRenderState` (in `fluorita-qt`) is shared by the item, its
  renderer and libmpv's update callback; the renderer never holds the item,
  the item detaches itself in its destructor, and every notification is
  queued through the state under its lock. `setHandle(0)` now queues
  `contextReleased` instead of emitting it inside the write, removing the
  re-entry the audit noted.
- **FLU-23.** `folders.rs` and `activation.rs` read URIs with
  `celestina_core::file_uri::to_path`; `file://localhost/` is accepted by
  both, and malformed escapes, queries and NULs are refused by both.
- **FLU-24.** MPRIS `Rate` is the confirmed speed (the normal rate before a
  report), `MinimumRate`/`MaximumRate` are `Speed`'s bounds, `Rate` writes
  become the speed menu's request (zero pauses, as the spec says), and a rate
  change is announced.
- **FLU-25.** `STATUS.md` no longer calls `2ff8738` uncommitted nor F11, F12
  and F14 conditional; `README.md` names both `cpp/` seams, the engine's real
  responsibilities and the thumbnail keys; the stale "advanced control"
  comment in `mpvvideoitem.cpp` is corrected.
- **FLU-26.** Fluorita's thumbnails carry `Thumb::URI` and `Thumb::MTime`
  (`tEXt` after `IHDR`, CRC-checked), written owner-only from the first byte,
  and an entry is judged by the spec rule — the recorded mtime equals the
  source's — reading at most 4 KiB of it; an entry with no keys is produced
  again.
- **FLU-27.** Hidden-name filtering applies only below the owning root, so a
  root under `~/.var/app/…` is live.
- **FLU-28.** Every worker in the app starts through
  `std::thread::Builder::spawn` and reports a refused thread instead of
  aborting (`copy::WORKER_NOT_STARTED`); the frame extraction runs under a
  host-owned token that `Drop` cancels before joining.
- **FLU-29.** The tautological handshake test and the unread `QML_MODULE` and
  `QML_TYPE` constants are gone; the handshake is tested on its owner, and
  `fluorita-qt` tests the registration and the raw-pointer rule against the
  C++ it ships.
- **FLU-30.** The canvas bridge passes `LineOrder`, `ShapeOrder` and
  `TextOrder` shared structs; no `#[allow]` remains in `fluorita/src`, and the
  trailer's went with FLU-18.
- **Displayed names.** `fluorita_core::displayed_name` bounds and strips every
  file name a sentence or title shows (engine and editor save notices, the
  frame notice, the metadata panel's name, the window title), and record
  names use the same sanitiser.
- **A Trash that leaves the file in place.** After `siderita_ops::trash`
  reports success, the library checks whether the path still exists; if it
  does, the record stays and the person is told the file changed while it was
  being copied and is still in its folder. This needs no field from
  `SID-H1-B`, so it compiles on this branch and after that unit lands.

### Owners and boundaries

- The handshake had two implementations (Fluorita's player and Siderita's
  modal); its state machine now lives in `fluorita-core` and the loop in
  `fluorita-engine`. Fluorita's copies (`decide_open`, `run_session`,
  `Command`, `Snapshot`, `closing`, `pending_open`, `generation`) are deleted,
  not wrapped. Siderita's copy is P-15's to delete.
- The still-measuring recipe had four copies (viewer, editor, batch, cover);
  they now share `probe_still` and `ImageDecision::measure`.
- The `file://` readers in Fluorita delegate to `celestina-core`.
- Dependencies point inward: app → engine → core, and nothing in the core or
  engine names Qt.

### Also fixed

`FLU-H1-A` changed the frame extraction to
`duration_from_seconds(self.position_seconds())`, but a CXX-Qt getter returns
`&f64`, so the application would not have compiled. The mirror caught it and
the call now dereferences the value.

## Limits

- **Not built here:** the Fluorita application, its C++ (`imagecanvas.cpp`
  with the shared structs from the generated `fluorita/src/rasteriser.cxx.h`,
  and `mpvvideoitem.cpp`), the QML (`qmllint` did not run), Siderita and
  Magnetita. The mirror proves the Rust type-checks against a model of the
  CXX-Qt API, not that the real bridge generates the same signatures. The
  canvas and probe tests have not run.
- **Not run:** the 4 engine tests that need libmpv, `tests/real_media.rs`,
  and every real-session behaviour: portal dialogs being withdrawn, the render
  seam at window teardown, AT-SPI announcements of the marks, MPRIS rate on a
  real bus, the watch against real `inotify` folder moves.
- **Uid 0:** `an_unreadable_folder_inside_a_root_is_a_gap_not_a_deletion`
  returns early when permissions do not close a folder, which they do not for
  root; the gap rule it checks was exercised only by reading.
- **Cost moved, not removed:** judging a thumbnail now reads up to 4 KiB of
  the cache entry instead of one `stat`; on a very large library the pending
  count is slower, but it runs on the projection worker.
- **Behaviour to note:** a still now shows "abriendo" until it is measured; a
  folder created empty in a root costs a walk; a close requested while an open
  is waiting drops that open.
- **Author checks:** `VAL-FLU-EDIT`, `VAL-FLU-METADATA`, `VAL-FLU-TEARDOWN`
  and `VAL-FLU-IMMERSIVE` in [VALIDATION.md](../../VALIDATION.md) name what to
  look for.

## Follow-up

- ADR 0009 still says libmpv "does not encode"; the removed trailer path
  proved it can. The ADR lives under the root `docs/decisions/`, outside this
  unit's prefix, so correcting it needs a suite documentation unit.
- Siderita adopts `SurfaceHandshake`, `run_session` and the queued release in
  P-15 (`SID-H1-C`); the queued release already removes the ordering that left
  its preview stuck (FLU-12).
- `player.rs::preview` trips clippy's `nonminimal_bool` under 1.94.
- `fluorita/qml/Main.qml` `libraryReached` calls `mediaPlayer.close()` and
  then checks `renderHandle === 0`, which is always true after `close()`, so
  `releaseSettled` runs at once and hides `playerFrame`; a hidden item is
  never synchronised again and the surface's release may never come. Found by
  the review, outside this diff; recorded under `VAL-FLU-TEARDOWN` for a
  corrective unit.
- `fluorita/spikes/` still names `libx264` in the measurement scripts that
  chose the backend; they are the historical spike, not the engine, and are
  left as they are.

## Fix round 1

- **Date:** 2026-09-28. The branch now carries two merges on top of the
  unit's work (`RS-H1-A` and `FLU-H1-A` `af7dde4`, whose `player.rs` fix is
  the one this unit had made); HEAD was `9b74a7b` before this round. The
  private target directory was recreated at the same path.
- **I-1, a picture that fails to start.** `run_session` no longer ends the
  loop when `start()` fails after `Start`: by then the surface holds a render
  context built from the session's handle and the handshake is
  `Live { published: true }`, so closing would destroy the core under the
  render thread. The failure is reported and the loop keeps polling until
  `Stop` or a dropped sender, so the host's close goes through the
  handshake. Only a session that never handed out a handle closes on its own
  (a failed open, or sound that failed to start). The module doc says so.
  Test `a_film_that_fails_to_start_stays_open_until_its_host_closes_it`
  (fake engine whose `start` fails): RED — the log held `close` right after
  the failed `start`, before any `Stop`; GREEN.
- **I-2, a store read as empty.** `edit_store::load` returns `absent` only for
  a file that is not there or is empty; a read error (permission, I/O, bytes
  that are not UTF-8, a directory) is `EngineError::Io`, and an unknown header
  is `EngineError::UnusableSource`. `recipes.rs` already leaves a store it
  cannot read untouched, so no write can replace its recipes. Tests: an empty
  file is absent; an unknown header and an unreadable file are errors (both
  RED before: they loaded as absent); in the app mirror,
  `a_store_this_build_cannot_read_keeps_every_recipe_in_it`.
- **M-1, a projection that outlives a scan's start.** `start_scan` moves the
  projection ticket, so a projection that was running publishes nothing; it
  also records the new configuration once the worker has it (the worker
  stores it before walking), so a projection made during the walk keeps a
  selection naming the added folder; and a stale projection is not started
  again while the state is `scanning` — the scan's own publication asks for
  one when a query or a selection needs it. Not unit-tested: the logic lives
  in the QObject, which the Qt-free mirror cannot run; it type-checks there.
- **M-2, queued resyncs.** `LibraryWatcher::drain` discards the batches
  already queued, and the watch calls it right before a resync walk, so a
  run of bursts during a long copy is one walk. Test
  `a_queued_run_of_resyncs_collapses_into_the_walk_about_to_happen`: RED by
  mutation (a drain that discards nothing), GREEN.
- **M-3, a silent keyboard selection.** Focus stays on the canvas, where the
  arrows, Delete and `]`/`[` work; after `]` or `[` the surface announces the
  selected mark with `Accessible.announce` (Qt 6.8, below the suite's 6.9
  floor), its name from `EditObjectLayer.describeObject`. `VAL-FLU-EDIT` now
  asks for Orca to speak it. Not run: no `qmllint`, no AT-SPI here.
- **N-1.** A refused session thread now shows `copy::WORKER_NOT_STARTED` and
  logs the reason.
- **N-2.** The MPRIS title goes through the displayed-name rule:
  `fluorita_core::displayed_stem` (new, beside `displayed_name`, tested), which
  `MediaRecord::display_name` now uses too, so the library and the panel name
  an untagged file the same way.
- **N-3.** The spike scripts are left; noted under Follow-up.
- **Checks (all exit 0):** `fluorita-core` 174 passed, `celestina-core` 82,
  `fluorita-qt` 4; `fluorita-engine` lib 169 passed, 0 failed, the same 4
  reach libmpv; clippy `-D warnings` on the workspace; fmt; the app mirror
  type-checks, its clippy shows only the pre-existing `nonminimal_bool`, and
  62 of its tests pass with the same 7 needing Qt; the three guards.

## Fix round 2

- **Date:** 2026-09-28.
- **F-1, changes queued behind a walk.** `LibraryWatcher::drain` now hands the
  queued batches back instead of discarding them. The watch's batch folding
  moved into `library/work.rs::fold_batch`, which walks once for a queued run
  of resyncs and then re-applies every other queued change, checked against
  the disk as it is now: `Touched` through `absorb_one` (which stats), and
  `Removed` or `RemovedTree` only if the path is still gone. A walk does not
  judge a file past its ceiling or deadline, below the depth bound, in an
  unreadable folder or in a root that did not answer, and those changes
  would otherwise have been lost. Tests, run in the app mirror:
  `changes_queued_behind_a_walk_still_apply_where_the_walk_cannot_judge` (a
  first root trips `max_files`; the queued removal in the second root takes
  its record and the queued arrival adds one) — RED by mutation (the queued
  changes dropped, as the old drain did): "the deleted file stayed"; GREEN —
  and `a_queued_removal_of_a_file_that_is_back_is_not_applied`. The engine's
  drain test now also checks that a non-resync change is handed back.
- **Nit 1.** The I-1 test records the log and the failure count inside the
  scope, waits for the reported failure on a bounded condition instead of a
  fixed sleep, sends `Stop` without asserting on the send, and asserts after
  the scope. RED by mutation (the old `break` restored): it fails with the
  log `["open embedded=true", "start", "close"]` instead of hanging.
- **Nit 2.** The announcement reads "Seleccionado: %1", which agrees with
  every mark's name.
- **Checks (all exit 0):** `fluorita-core` 174, `celestina-core` 82,
  `fluorita-qt` 4; `fluorita-engine` lib 169 passed, 0 failed, the same 4
  reach libmpv; workspace clippy `-D warnings`; fmt; the app mirror
  type-checks, its clippy shows only the pre-existing `nonminimal_bool`, and
  64 of its tests pass with the same 7 needing Qt; the three guards.

## Landing

- **Base revision:** `fad4f15da99260b58a15c078a1011e103d64c2cf`
- **Check:** `production_artifact.py check fluorita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; `production_artifact.py check magnetita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build
- **Build:** fluorita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:bcd86d0ec121aa61f985a3c9e55d054ef387777bcf8073fc13f69e946c2a05be, verification_fingerprint sha256:52856bae821097c06c36c76d1661d0c1963bf33a27e685af9e1aadae638278d2; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:31cb08fb58c29c23f4c5f394305d605e5bbfb5efdd44a75b0ef38b9207a79667, verification_fingerprint sha256:28e412e3c3373b90aa086c3a31b3303dfa2cb362a840d48551fdfe26076d743e; siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a424b33da6ea8d4655d980fd6bf9b7cfb3e3fe609a278df7a0a66b0407ff98e6, verification_fingerprint sha256:7d31ed8dfa9c5c7a2b29aea59826d8462f7f9ad91e5309901b6f6673d52257d9; magnetita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:3d584bae73dde741fcea5141550f4f7a6a07675e7252932929e04d841e32a79f, verification_fingerprint sha256:b9eae1f377c797835520fc71e026e35f8cc078e51727dd418bd7b9b94633bc38
- **Deploy:** after the push: fluorita: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh
