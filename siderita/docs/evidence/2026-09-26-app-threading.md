# Evidence: deterministic jobs, current answers and a free Qt thread

- **Date:** 2026-09-28
- **Scope:** `SID-H1-C` (program unit P-15) of the
  [SID-H1 plan](../plans/active/2026-09-26-hardening.md): SID-5, SID-6,
  SID-7, SID-8, SID-9, SID-10, SID-11, SID-12, SID-13, SID-19, SID-24,
  SID-25, SID-26, SID-27, SID-28 and SID-30 from the
  [Siderita audit](../../../docs/evidence/2026-09-26-monorepo-audit-siderita.md),
  with the matching share of SID-29; FLU-12 from the
  [Fluorita audit](../../../docs/evidence/2026-09-26-monorepo-audit-fluorita.md)
  (ported into Siderita's own handshake copy, ruling R-A23); the Siderita
  adoption halves of SID-18/RS-1 and RS-4 from the
  [shared-crates audit](../../../docs/evidence/2026-09-26-monorepo-audit-shared-crates.md);
  STY-4 from the
  [shell and style audit](../../../docs/evidence/2026-09-26-monorepo-audit-shell-style.md);
  program row P-15 of the
  [monorepo audit](../../../docs/evidence/2026-09-26-monorepo-audit.md)
- **Environment:** session worktree `siderita-SID-H1-C` on branch
  `unit/siderita/SID-H1-C`, stacked on `SID-H1-B` (itself on `RS-H1-A`)
  merged with `SID-H1-A`, base `24067e8`; Linux container, uid 0; rustc
  1.94.1 in the app crate, the pinned toolchain in `celestina-rs`; Cargo
  `--offline` with a target directory of this session's own; no Qt 6 SDK,
  no CXX-Qt build, no `qmltestrunner`, no `qmllint`, no libmpv (a stub
  `libmpv.so` that aborts when called was used only to link the mirror
  below)
- **Artifact:** the landing builds it

## Procedure

```sh
cd celestina-rs
cargo test -p siderita-core -p siderita-embedded --offline
cargo clippy -p siderita-core -p siderita-embedded --all-targets --offline -- -D warnings
cargo fmt --check -p siderita-core -p siderita-embedded
cd ../siderita && cargo fmt --check

# The app crate, type-checked without Qt: every CXX-Qt bridge in
# siderita/src is replaced by a stub module of the same shape (properties'
# getters and setters, signals, extern "C++" functions), cxx-qt and
# cxx-qt-lib by signature stubs, and the rest of src/ is compiled as is.
python3 <scratchpad>/sid-h1c-stubgen.py
cd <scratchpad>/sid-h1c-appcheck
cargo clippy --offline --tests -- -D warnings -A dead_code
RUSTFLAGS="-L <stub libmpv>" cargo test --offline

bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
sh scripts/check-documentation-contract.sh
```

## Result

- **Exit:** every command above exits 0 except the mirror's `cargo test`,
  which runs 137 tests green and fails the 5 `thumbnails` tests that call
  the C++ provider (they need Qt; the stub panics).
- **Pure crates:** `siderita-core` 47 (48 after fix round 1) unit tests and 1 integration test,
  `siderita-embedded` 12; clippy `-D warnings` and `fmt --check` clean.
- **Guards:** architecture, language (145 legacy files; rows for
  `controller/fileops.rs`, `devices.rs` and `components/SizeRow.qml`
  retired, their text translated or moved into product-copy owners) and
  documentation: OK. `controller.rs` fell from 754 to 753 lines and its
  ratchet with it; `FolderView.qml` was not touched (744); the
  `SizeRow.qml:Slider` control row is gone.

### RED and GREEN

- **SID-12:** `a_huge_executable_is_read_in_bounded_bytes` measures the
  thread's own `rchar` from `/proc/thread-self/io`. RED on the old reader
  with a 64 MiB sparse `.exe` (`read 67108960 bytes of a 67108864-byte
  file`; 4 GiB was not tried on the old code, which would have allocated
  it). GREEN with a 4 GiB sparse `.exe`: the icon comes back whole under
  64 KiB of reads. `a_resource_claiming_gigabytes_is_refused_unread` covers
  a resource entry claiming 2 GiB or pointing past the end. The rewrite also
  stops leaking every icon (`Box::leak` per read).
- **SID-25:** `a_request_behind_a_stuck_scan_is_served_by_a_fresh_worker`
  and `dropping_an_executor_with_a_stuck_scan_does_not_hang` hung on the old
  executor (killed by `timeout`), `abandoned_workers_are_capped` failed its
  assertion; GREEN after the rewrite. `ScanExecutor::new` returns
  `WorkerUnavailable` instead of `expect`.
- **SID-9 / SID-24 / SID-8:** `a_superseded_or_closed_search_answer_is_refused`
  (the search-generation test the plan names), three `SingleFlight` tests
  and `a_scan_is_in_flight_until_it_is_answered_or_cancelled`, RED by
  compilation (new owners) and GREEN.
- **App tests written, run only in the mirror:** SID-18 foreign host and
  query/fragment refused by the clipboard decoder and by `FileManager1`'s
  folder lookup; RS-4 a FIFO named `x.desktop` neither blocks nor hides the
  chooser's real entry, and is not read for a launcher icon; SID-19 a tab or
  bar in a portal filter cannot shift its fields; SID-11 a burst of
  Magnetita changes is one reload and a button press does not wait. They
  pass in the mirror; RED there is by compilation.
- **QML tests written, not run:** `tst_compress_dialog.qml` follows the
  asynchronous suggestion and gains a repeated-suggestion case;
  `tst_size_row.qml` is new (STY-4: role and name, one tenth per arrow,
  bounds).

## Observed facts, by finding

- **SID-5:** write workers start through `jobs::spawn_worker`, which keeps
  their handles; `main` calls `jobs::shutdown(10 s)` after `exec()`
  returns: every job is cancelled (a paused one resumes to reach its
  check) and each worker is joined, a worker still blocked past the deadline
  being left and counted in the log. Quitting cancels rather than asks.
- **SID-6:** a worker ends its job itself (`JobEnd::Done`) as it returns; a
  batch parked on a password ends when its tab's state is dropped. The
  job's outcome (failures, undo record, notice) is still delivered only to
  the tab that started it; with that tab closed, other tabs see the ring end
  and their folders rescan, but not the report (a follow-up below).
- **SID-7:** a change during a job is recorded on the watch and replayed
  (`replay_deferred_change`) in every tab when the register empties.
- **SID-8:** `refresh_quiet` returns while a navigation or a non-quiet scan
  is in flight; a rolled-back navigation replays a change it held. A change
  that reaches the folder after a scan of it began keeps the watch stale
  when that scan lands (`WatchState::begin_rescan`), and one more quiet
  rescan follows, so the change is not dropped (fix round 1, F2).
- **SID-10:** undo, restore, restore all, purge and empty Trash run as jobs
  (new kinds `Undo`, `Restore`, `Purge`) with per-entry progress and
  Cancel between entries; undoing a move also reports the bytes it copies
  back, through the same `entry_progress` paste and trash use (fix round 1,
  F3). Restore and purge report entries, not bytes.
- **SID-11:** `devicemodel.rs` keeps one UDisks2 and one Magnetita worker,
  each with one connection for the process; tabs subscribe once and read
  the kept listing; ring, media keys and send-to-phone ride the Magnetita
  worker; mount and unmount ask the model to list again.
- **SID-13:** creations and renames run on workers
  (`write_off_thread`); paste planning on a reader; the quick-look text
  (`request_preview_text` → `preview_text`), the compress name
  (`suggest_archive_name` → `archive_suggestion`) and the favourites' kinds
  on readers with generations; `trash_record` no longer stats.
- **SID-18/RS-1:** `dbus::uri_to_path` and `dbus::path_to_uri` are deleted;
  every caller uses `celestina_core::file_uri`.
- **RS-4:** «Abrir con…» gathers through `desktop_entry::scan` on a reader
  (`is_listable` is the visibility rule, 4096 ids at most); `open_with_app`
  runs `xdg-mime default` and the launch on a reader; a launcher's icon is
  read with `desktop_entry::read` and resolved on the thumbnail pool through
  the new `siderita_own_icon_path` export, so `own_icon_url` reads nothing.
- **SID-19:** filter names lose tabs and line breaks, a pattern holding a
  delimiter widens to `*`, and the fallback `expect` is gone.
- **SID-24:** Trash and Recientes listings go through `SingleFlight`.
- **SID-26:** thumbnails run on a pool of 2 to 4 threads of their own, and
  `cancel()` takes a queued request back or skips its work. Quitting drains
  the pool: queued requests are dropped and running ones get at most one
  second (`siderita_thumbnail_shutdown`, called from `main` after the jobs).
- **SID-27:** both dialogs have `Accessible.Dialog` and a name bound to
  their heading; their injected properties are `required`, and the name
  prompt's controller is typed (the compress dialog keeps `var` because its
  QML test hands in stubs).
- **SID-28:** the operation ring's arc and hover scale, the sidebar chevron
  and the wheel glide are gated on `CelestinaTheme.reducedMotion`.
- **SID-30:** STATUS's stale "Uncommitted in the checkout" bullets now say
  they were committed, the SID-A1 staging sentence is corrected, README
  names the external RAR/7z tools, and the two misleading controller
  comments (`trash.rs`, `fileops.rs`) are rewritten; the `siderita-ops`
  comments were already corrected by `SID-H1-B`.
- **FLU-12:** `close` marks `closing` before clearing the handle; a session
  generation, bumped by every open and teardown, guards the queued handle.
- **STY-4:** `SizeRow` is built on `CelestinaSlider`; a move is rounded to
  tenths inside the row's bounds, and a bound that is not a whole tenth
  (1.25, 0.75) is reported exactly (fix round 1, F1).

## Limits

- Nothing ran with Qt: no build of the app, the C++ (`thumbnailprovider.cpp`:
  the bounded pool, `cancel()`, `loadIconFile`), `qmllint`, the QML tests or
  the offscreen smoke. The mirror proves the Rust type-checks and its Qt-free
  tests pass against stubs of the CXX-Qt shapes; its fidelity to the real
  code generation is by reading.
- The author should check in a real session: quitting during a large copy
  (no partial file left), closing the tab that started a job, a USB hotplug
  with several tabs open, Magnetita slow to start, «Abrir con…», launcher
  icons in a folder of `.desktop` files, the quick-look text on a slow
  mount, the compress dialog's name, and reduced motion.
- A launcher icon is not written to the shared thumbnail cache (fix round 1,
  F6), so it follows the icon theme; Qt's own image cache still holds it
  while the delegate lives.
- A stopped search that found nothing now shows its empty result with the
  "detenida" summary instead of leaving "Buscando…" up.
- Quitting cancels jobs instead of asking first.

## Follow-up

- Adopt the handshake `FLU-H1-B` extracts into `fluorita-core` /
  `fluorita-engine` and delete Siderita's copy in `media.rs` (ruling R-A23).
- `is_archive` / `are_archives` still sniff bytes on the Qt thread when a
  menu opens, and `path_exists` stats there for the save picker; neither
  was cited by the audit.
- A close guard in `Main.qml` that asks before cancelling running jobs.
- Deliver a job's outcome to a live tab when the tab that started it has
  closed (SID-6's second half).
- Readers stuck on a hung mount are not bounded across generations: a
  search, a preview or «Abrir con…» retried against one leaves a thread
  blocked per try (the Trash, Recientes and favourite listings are single
  flight and do not).
- Each tab's scan executor waits up to 500 ms on drop, on the Qt thread,
  when its worker is busy; closing several tabs on a slow mount adds up.

## Fix round 1

- **F1 (Important):** `SizeRow.snap` passes a target at a bound through
  unrounded and clamps the rounded value to `[minValue, maxValue]`;
  `tst_size_row.qml` gains the 1.25 and 0.75 cases (not run: no Qt).
- **F2:** `WatchState` records a change observed during a rescan
  (`begin_rescan`); `mark_rescanned` then leaves the snapshot stale and the
  controller replays one quiet rescan after the landing (and not while a
  job writes). `a_change_during_a_rescan_keeps_the_snapshot_stale` is GREEN,
  and RED when `mark_rescanned` is mutated back to always-fresh.
- **F3:** undo reports each entry and, for a move, its bytes; the paste,
  trash and undo read-outs share one `entry_progress` helper.
- **F4:** recorded above as a follow-up.
- **F5:** `siderita_thumbnail_shutdown` (`clear()` plus a bounded
  `waitForDone`) is exported through the embedded bridge and called from
  `main`; the pool comment is corrected.
- **F6:** launcher icons skip `writeCache`.
- **F7:** `suggest_archive_name` retires the earlier suggestion on its early
  return; the portal test sits after `use super::*;`; the reader pile-up and
  the executor's drop wait are recorded as follow-ups.
- **Checks:** `cargo test -p siderita-core -p siderita-embedded` (48 + 1 +
  12), clippy and fmt clean, the stub mirror clippy-clean with 137 tests
  green (the 5 `thumbnails` tests need Qt), the three guards.

## Landing

- **Base revision:** `339461d963fde18d69ea0e64430af1fb23be1a61`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: artifact is not verified yet; run verify-production.sh; tests or rules changed; run verify-production.sh again; `production_artifact.py check celestina-rs --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; artifact digest or set does not match the recorded build; tests or rules changed; run verify-production.sh again
- **Build:** siderita verify: verify-production.sh exit 0, manifest source_fingerprint sha256:105ff14c051b22f685b6ede81b76bd1eb774cd4816c21ee8335b5cdd5958eb7a, verification_fingerprint sha256:fa82ca7246fe59062a048c43f9f92a2260f78c5b8953058902516ad8ae7e1246; celestina-rs build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:a10afe0f60400db3bf83df66c2878464475cc8a6b5d871ff46b37b83f7c40848, verification_fingerprint sha256:29ef0a117d4dfefe8b6dc66b2f7eeeb36aa6c0463fd6b406340ee01439fe983a
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
