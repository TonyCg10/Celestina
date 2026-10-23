# Evidence: CEL-2-L — See the desktop: the overview and the shortcut sheet

- **Date:** 2026-10-17
- **Scope:** unit CEL-2-L of plan `cel-2-parity` on `unit/celestina/CEL-2-L`,
  worktree `celestina-CEL-2-L`, not yet landed. Design:
  [the CEL-2-L design](../../../docs/superpowers/specs/2026-10-17-cel-2-l-see-the-desktop-design.md);
  working plan:
  [the implementation plan](../../../docs/superpowers/plans/2026-10-17-cel-2-l-see-the-desktop.md).
  Code:
  `celestina-rs/crates/celestina-shell/src/{keybinds,thumbnail,lib}.rs` and
  `celestina-rs/crates/celestina-shell/tests/{kdl_parses_niri_config.rs,fixtures/}`;
  `celestina/src/capture/` (`mod`, `worker`), `celestina/src/overview/`
  (`mod`, `view`, `state`), `celestina/src/providers/{keybinds,niri,mod}.rs`,
  additive hunks in `celestina/src/{app,command,copy,message,main,lib}.rs`,
  `celestina/tests/keybinds_probe.rs`, `celestina/scripts/smoke.sh`, and the
  documents of this unit.
  The lock files `celestina/Cargo.lock` and `celestina-rs/Cargo.lock` change
  with the new `kdl` dependency; `celestina/Cargo.toml` also enables the
  `process` feature of `rustix` (safe `setrlimit`, see Review).
  `celestina/src/providers/worker.rs` is not changed by this unit.
- **Environment:** Linux 7.2.10-1-cachyos, niri 26.04 (nested for the smoke),
  the toolchain `celestina/rust-toolchain.toml` pins. Cargo commands ran from
  `celestina/` and `celestina-rs/` with `--locked`.
- **Artifact:** none sealed by this record. The landing builds, verifies and
  deploys its own; `build-production.sh`, `verify-production.sh` and
  `deploy-production.sh` were not run in the worktree.
- **Source of the results:** the first block under "Automated evidence" is what
  the implementers reported earlier; the "Latest fix pass" block is from runs
  of the last fix pass in this worktree, and its numbers supersede the older
  ones.

## What was built

- **The overview panel.** One centred overlay (`celestina-overview`, an overlay
  layer surface shaped like the launcher; keyboard exclusive). It draws the
  workspaces of the focused output as the person arranged them: columns left to
  right, rows down a column, true relative sizes, each window a tile with its
  icon and title. Geometry comes from the `workspace_map` owner in
  `celestina-shell`, which this unit uses and does not copy again.
- **One preview per opening.** The workspace that is active on the output the
  panel opens on carries one `wlr-screencopy` capture of that whole output,
  taken once when the panel opens, scaled so its longer side is at most
  `PREVIEW_MAX_SIDE = 640` px, drawn as a rounded, dimmed backdrop behind that
  workspace's tiles. It is discarded when the panel closes, never refreshed
  while it is open, never written to disk, logged or sent over the bus. No
  other workspace and no window tile has a preview.
- **The shortcut sheet.** The `shortcuts` mode lists the keybinds of the
  author's niri config grouped by binding mode. The config is read once each
  time the mode opens, from `$NIRI_CONFIG`, then
  `$XDG_CONFIG_HOME/niri/config.kdl`, with a size bound, off the UI thread.
  The whole read and parse runs in a child process: the shell's own binary
  re-executed as `celestina --keybinds-probe PATH` (internal, not in the usage
  text), with an empty environment, no stdin or stderr, a JSON answer on
  stdout read through a 2 MiB cap, and a 3 s deadline after which the child is
  killed. The parent never touches the path itself (no `stat`): missing,
  unreadable and too-large files are the child's answers. The child lowers
  `RLIMIT_CORE` to 0, `RLIMIT_AS` to 1 GiB and `RLIMIT_CPU` to 5 s before it
  opens anything. The parent looks at its worker's stop flag every 50 ms and
  kills and reaps the child when it is raised. `celestina --check` reads through the same child. There is no
  watcher and no standing thread. The parser (`keybinds.rs`, the `kdl` crate)
  is pure and bounded: file size, `{` and comment nesting before the
  recursive parser runs (a first defence only, see Review), node count, label
  length, and include depth; the parent re-applies the group, bind and
  length bounds to whatever the child answers. Each bind keeps its title, its raw action token and its
  spawn program apart.
- **Verbs.** `celestina overview toggle|open|close` and
  `celestina shortcuts toggle|open|close`, additive in `command.rs` with the
  usage text. The panel's Spanish strings are in `copy.rs`.
- **Niri model.** `providers/niri.rs` keeps the numbers the geometry tiles
  draw (the column and row of `pos_in_scrolling_layout` and `tile_size`). A
  change that only scrolls the view publishes nothing.
- **No duty switch.** Nothing runs until a verb asks for the panel, and nothing
  is watched or captured in between. `shell.json` gains no field.

## Design change

The design first set a thumbnail for each window, cropped from an output
capture. niri 26.04 reports `tile_pos_in_workspace_view = null` for every tiled
window (also recorded in
`selenita/docs/evidence/2026-10-09-screenshots.md`), so a window's rectangle on
the output cannot be known and cannot be cropped, and niri offers no
per-window capture source (`ext-foreign-toplevel-image-capture-source` is
absent). On 2026-10-17 the author chose instead to capture the whole output
once and show it as the preview of the workspace active on it. Window tiles stay
geometry only. The design record is updated to match.

## Procedure

The checks below are the project's standard commands. The implementers ran
them in this worktree; this documentation pass did not rerun them. The
language and documentation guards were run by this pass (see Result).

From `celestina/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
sh scripts/smoke.sh --binary <target>/debug/celestina   # run twice, in a nested niri
```

From `celestina-rs/`:

```sh
cargo fmt --all --check
cargo clippy -p celestina-shell --all-targets --locked -- -D warnings
cargo test -p celestina-shell --locked
```

From the worktree root:

```sh
python3 scripts/check-language-contract.py
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
```

## Automated evidence

Results as reported by the implementers from this worktree:

- **`cargo test` in `celestina/`:** 585 library tests passed, plus the
  integration tests of the crate.
- **`cargo test` in `celestina-rs/` (`celestina-shell`):** 215 tests passed,
  plus the integration tests (`kdl_parses_niri_config`, which parses a fixture
  config).
- **`cargo clippy -- -D warnings` and `cargo fmt --check`:** clean in both
  workspaces.
- **`python3 scripts/check-language-contract.py`:** OK.
- **`bash scripts/check-architecture-contract.sh`:** OK.
- **Nested-niri smoke (`scripts/smoke.sh`, in a nest, two consecutive runs):**
  passed both times. The lines it reported include:
  - `capture: preview mean luma 72`
  - `left luma 11 right luma 133; open window ... blocked window ... (left half black)`
  - `shortcuts loaded 1 group(s)`
  - the capture was taken before the overview's surface existed
  - no frame at rest

Re-run by the fix pass after the re-review (2026-10-17 to 2026-10-21, this
worktree):

- **`cargo test --workspace --locked` in `celestina/`:** 599 library tests,
  plus `cli` (3), `keybinds_probe` (4), `notifications_bus` (1) and
  `wallpaper_decode` (26, 1 ignored), and the toolkit crates; all passed.
- **`cargo test -p celestina-shell --locked` in `celestina-rs/`:** 216 unit
  tests and the integration tests passed.
- **`cargo clippy ... -- -D warnings` and `cargo fmt --all --check`:** clean in
  both workspaces.
- **`celestina --check`** (the smoke's first step) read the author's real
  config through the probe child: `state=7 group(s) read`.
- **Nested-niri smoke** on a copy of the debug binary, ten runs, only inside
  the smoke's own nest (`--own-bus`): runs 1, 3 and 9 passed in full, with
  `capture taken before the overview's surface existed`,
  `shortcuts loaded 1 group(s), as the profile declares`,
  `capture: preview mean luma 72`, `left luma 15 right luma 129; ... blocked
  window 2 column 1 (left half black)` (run 9) and
  `no frame at rest after the overview closed`. The other seven failed:
  runs 2, 4, 5 and 6 because the capture's own connection did not answer its
  first sync within the 400 ms request deadline (`capture: unavailable: the
  compositor did not answer a sync within 1000 ms`; the panel then opened with
  geometry only, as designed), each while another session's nested smoke was
  running beside it (seen in a process monitor for runs 5 and 6); runs 7, 8
  and 10 on frames at rest in parts of the smoke this pass did not change
  (the base run, the bar run, and the Island's surface after the overview
  closed in run 10). The cause of those failures was not proven; this pass
  did not show that they are unrelated to its changes, only that each
  coincided with a slow or busy nest.
- **Quiet-machine A/B (2026-10-21, orchestrator):** with no other session's
  nest or build running, the same debug binary passed the full smoke in 6 of 6
  consecutive runs (plus 2 earlier passes), and in the same 6 runs its
  base-run `winit frames` / `rest_frames` matched, pair by pair, a debug build
  of the base commit `a0cdaf18` (17/2 in five pairs, 37/2 in one; the base
  build once showed 14/1). Two runs made right after a build, while the
  machine was still loaded, failed on the base run's frames at rest (8 and
  more). So the earlier failures are consistent with load, and no regression
  of frames at rest was measured; this is still not a proof, and the
  author's real hardware is untested.

Latest fix pass (2026-10-21, this worktree, after the reviewer's findings
under Review item 4):

- **`cargo test --workspace --locked` in `celestina/`:** 603 library tests,
  `cli` 3, `keybinds_probe` 5, `notifications_bus` 1, `wallpaper_decode` 26
  (1 ignored); `celestina-paint` 52 + `painter_contract` 2; `celestina-ui` 81 +
  `field` 7 + `reconcile` 9; `celestina-wayland` 64; no failure.
- **The red test**, run 50 times alone and the whole `overview::slot` module
  50 times, and the whole library binary 8 times: 0 failures.
- **`cargo test -p celestina-shell --locked` in `celestina-rs/`:** 216 unit
  tests plus the integration tests (1 and 2); no failure.
- **`cargo clippy ... -D warnings` and `cargo fmt --all --check`:** clean in
  both workspaces.
- **`python3 scripts/check-language-contract.py`:** `OK (131 legacy file(s)
  ratcheted)`; **`bash scripts/check-documentation-contract.sh`:** `OK` (with
  the recorded `SID-G7-D` errata lines); **`bash
  scripts/check-architecture-contract.sh`:** `OK`.
- **Nested-niri smoke:** not run in this pass, because other sessions' builds
  and tests were running on the machine; the smoke results above are those of
  the earlier passes, with the code before this pass's changes.

- **Smoke on the final tree (2026-10-22, orchestrator):** after the last fix
  pass, two consecutive nested runs of `scripts/smoke.sh` on a debug copy
  passed in full (`smoke: OK`; preview mean luma 72, left/right half luma
  11/133 with the blocked window's half black, shortcuts loaded 1 group, no
  frame at rest after close), with other sessions' builds and tests running
  (load average about 4.5).

## Review

Four review rounds:

1. **Sonnet** reviewed the capture path; **Opus** reviewed the rest.
2. A fix pass applied the main findings; **Sonnet** re-reviewed the capture
   path.

Main findings fixed:

- a parser stack-overflow guard (nesting bound before the recursive parser);
- capture lifecycle generations: each opening carries a number, so a reply,
  timer or bindings result from a closed or timed-out opening is dropped;
- fractional-scale frame matching (a real scale in `[announced - 1,
  announced + 0.5]`, aspect ratio within 1 %);
- a 128 MiB budget for frames in flight, checked before allocation;
- Spanish labels in `copy.rs`.

3. A re-review found that the nesting guard does not close the parser's
   stack overflow: seven families of crafted files, each under 256 KiB
   (`/-` repeated, an open `"""` string followed by `{`, an open `r"` or
   `"\` followed by `/*`, `a=` repeated, `a(b)` repeated, failed `\u{zz}`
   escapes repeated), abort a process that parses them. On 2026-10-17 the
   author decided the config is parsed in a child process. A fix pass then
   applied that and the other findings:

- **Child-process parse.** Described under "What was built". No parse runs
  in the shell's process, the check included; the in-process reader stays
  only as the child's body and for pure unit tests. The child is `/proc/self/exe`
  (falling back to `current_exe`), so a deployment that replaces the binary
  on disk does not break the probe of a running shell. Each of the seven
  families was run against the debug binary's probe mode directly: every one
  exits 134 (SIGABRT, the stack overflow) in the child, and the parent
  answers `Invalid`. `check_nesting` stays as a cheap first defence; its scan
  of a run of `#` that opens no raw string was quadratic and now steps over
  the run once.
- **One capture in the whole process.** The in-flight mark is process-wide
  (`capture::worker::IN_FLIGHT`) and travels with the request as a ticket
  that clears it when dropped: after the capture, during a capturer's
  panic, or with a request the closed queue discards. A given-up capture
  still draining keeps the next opening refused as busy (that opening shows
  geometry only), and a fresh worker that met such a refusal is let go at
  once. The worker never clears a mark it does not hold, so the old order
  question (queue closed before or after the mark) is gone; a request
  accepted in the instant its worker stops gets no reply, and the app's
  timer bounds that wait.
- **Budget counts the read-back.** A frame is read back into a buffer of its
  own size while its memory file still exists, so the peak is every frame's
  storage plus the largest frame once more. The 128 MiB budget is checked
  against that peak before allocating: one 4K or 5K output fits, an 8K frame
  no longer does.
- **Shutdown honoured at every step.** Before connecting, in the first syncs
  and every pump, before each `copy` request (a buffer allocated meanwhile is
  released), and before and after each read-back, so the scale of a given-up
  frame never runs.
- **App flow.** When the surface cannot be created, the steps after it
  (reading the bindings, handing over previews) are dropped, and the
  bindings reader starts only for an open panel. An output removed while its
  opening waits for the capture gives that opening up (capture cancelled,
  previews dropped) instead of opening on another output.
- **Tests.** Wire-format round trip, refusal of garbled answers and bounds
  re-applied to a lying answer; the probe's deadline, output cap, failure,
  signal and spawn failure with `sh` and `sleep` stand-ins; an integration
  test (`celestina/tests/keybinds_probe.rs`) that writes the seven families
  and checks each is `Invalid` without harming the test process, and that
  the fixture loads through the probe exactly as in process; `clear_pending`
  cancels a capture in flight (the capturer sees the stop); a draining
  capture keeps the next opening busy until it ends; a closed queue frees
  the mark; the reply's preview cap and every preview's size, tested on
  frames of several shapes instead of on constants.

4. A reviewer of that fix pass ran the unit tests repeatedly and found a red
   test: `overview::slot::tests::a_cancelled_capture_publishes_nothing_and_a_reopen_asks_anew`
   failed in 6 of 7 runs, because its two-spawn spawner was consumed by a
   fresh worker that is let go on a busy refusal while the cancelled capture
   still drains. The product logic was right; the test raced. It now waits for
   the cancelled capture's `ended` signal and has spawns to spare. The same
   review found, and a second fix pass fixed:

- the probe took no stop flag: `run_probe` now takes a `&AtomicBool`, checks
  it every 50 ms in the read loop and the wait loop, and kills and reaps the
  child when it is raised; the reader worker passes its own flag, so dropping
  the worker (the shell's exit included) does not leave a child behind;
- the parent's `stat` preflight (`refused_by_stat`, and its use after a
  failure) is gone, so nothing touches the path outside the child's 3 s
  deadline;
- the child lowers its core-dump, address-space and CPU limits through
  rustix's safe `setrlimit` (best effort, as the parent's deadline and cap
  still bound it);
- a child that cannot be started or read, or a stopped probe, is
  `Unreadable`, no longer `Invalid`;
- a relative `$NIRI_CONFIG` is made absolute against the current directory
  (and `load_with` does the same for any path), so the includes confined to
  its directory are not silently lost;
- the vestigial 8 MiB reader stack and `Worker::spawn_with_stack` (and its two
  tests) are removed, since the parse no longer runs in process; the shared
  `providers/worker.rs` is back at its base bytes;
- when the bindings' reader thread cannot start, the flow's `asked` mark is
  reset (`Flow::reader_failed`) and the panel is handed an `Unreadable`
  answer, so the Shortcuts page shows its unavailable line and the next
  switch asks again.

## Result

The unit is implemented and its automated checks passed in this worktree, as
reported above and in the last fix pass's block. The overview and the shortcut sheet were exercised only in a
nested niri. The author's rows VAL-CEL2-OVERVIEW and VAL-CEL2-SHORTCUTS
(in [VALIDATION.md](../../VALIDATION.md)) are `pending` and have not been run.

Run by this documentation pass, from the worktree root:

- `python3 scripts/check-language-contract.py`: `Language contract: OK
  (131 legacy file(s) ratcheted)`, exit 0.
- `bash scripts/check-documentation-contract.sh`: `Documentation contract: OK`,
  exit 0. The run prints `erratum` lines for `SID-G7-D`, an unrelated Siderita
  inventory; they are recorded errata, not failures of this unit.

## Limits

Nothing was checked in a real session. Not proven:

- a residual window, left as it is: a verb queued while a panel is closing can
  start a capture while the dying panel's surface is still mapped, so that
  one preview may show the panel on its way out;
- the seven crash families were run against the debug build; the release
  build was not run against them (the child isolates either way);
- the probe costs one process start per opening of the Shortcuts mode:
  about 6 ms for the fixture with the debug binary (median of ten runs, this
  machine); the author's real config through the release binary was not
  timed;

- real session behaviour: the panel beside Noctalia, the blur, focus and
  keyboard grab against the author's compositor state;
- rotated outputs: a rotated or odd-aspect frame is refused, so that output
  gets no preview and shows geometry only;
- fractional scale on the real 4K monitor (the author's output runs at 1.5);
- `block-out-from "screen-capture"`: verified only in the nest;
- the `wl_output` scale rounding assumption: `wl_output` announces the ceiling
  of a fractional scale, and the frame match relies on that;
- accessibility: the toolkit exposes no accessibility tree, so the panel has
  no AT-SPI evidence;
- `tile_pos_in_workspace_view` is `null` on niri 26.04, which is why no
  per-window thumbnail exists;
- `build-production.sh`, `verify-production.sh` and the deploy were not run in
  the worktree; the landing runs them.

## Follow-up

- The author's rows VAL-CEL2-OVERVIEW and VAL-CEL2-SHORTCUTS in
  [VALIDATION.md](../../VALIDATION.md). Test with a nested niri first.
- The landing writes the base revision, the inventory link, the diffstat and
  the production check, as in the sibling records.

## Rebase

Rebased onto `ae9ffaa4` (units A, B, C, D, E, F, G, J and K). Conflicts in
`src/app.rs`, `src/message.rs`, `src/copy.rs`, `src/lib.rs`,
`src/providers/mod.rs`, `Cargo.toml`, `scripts/smoke.sh`, README, STATUS,
VALIDATION and the ledger were resolved by keeping every other unit's duties
beside this one's. The bubbles tests got `..Default::default()` for the new
settings fields; the `copy.rs` block of this unit was re-appended after `main`'s
tests module and then moved before it (clippy `items_after_test_module`). `overview::slot::reap` now hands the worker to
`providers::Reaper` (a named `reap-overview` thread, the reaper dropped
because nothing restarts an overview worker that it must be ordered before),
so the loop's thread still never joins a worker.

One overlay at a time: the overview and the launcher already replaced each
other (`open_overview_surface` and the capture step close `self.launcher`,
which holds the bubbles selector too; `Verb::LauncherToggle` calls
`close_overview`). The rebase added the missing direction: `select_bubbles`
now calls `close_overview` before it opens the selector. The Island pages
(clipboard, connectivity, phone, notifications) and the OSD take no exclusive
keyboard and draw below the overview's overlay layer, so they need no change.
The flow has no `RuntimeCtl`-free seam for a test of this, so the proof is the
reasoning above and the smoke.

Smoke on the rebased tree (2026-10-22): two consecutive full runs of
`scripts/smoke.sh` on the debug build of the rebased branch, every unit's runs
included, ended `smoke: OK`, with the overview run's lines `capture: preview
mean luma 72` and `left luma 11 right luma 133` (and the mirror image in the
second run, with the blocked window on the other side), `no frame at rest after
the overview closed`.

## Landing

Not landed. The landing computes the inventory and closes the ledger row.

## Landing

- **Base revision:** `df534373e2764bd11e1fb7b91a573a5ff88fe812`
- **Check:** `production_artifact.py check celestina --require-verified` exit 0: artifact: celestina current; `production_artifact.py check celestina-rs --require-verified` exit 0: artifact: celestina-rs current; `production_artifact.py check siderita --require-verified` exit 0: artifact: siderita current; `production_artifact.py check magnetita --require-verified` exit 0: artifact: magnetita current; `production_artifact.py check magnetita-android --require-verified` exit 0: artifact: magnetita-android current; `production_artifact.py check grafita --require-verified` exit 0: artifact: grafita current; `production_artifact.py check fluorita --require-verified` exit 0: artifact: fluorita current; `production_artifact.py check hematita --require-verified` exit 0: artifact: hematita current; `production_artifact.py check cuprita --require-verified` exit 0: artifact: cuprita current; `production_artifact.py check calcita --require-verified` exit 0: artifact: calcita current; `production_artifact.py check selenita --require-verified` exit 0: artifact: selenita current
- **Build:** artifact current; no build
- **Deploy:** after the push: celestina: deploy-production.sh, status-production.sh; siderita: deploy-production.sh, status-production.sh; magnetita: deploy-production.sh, status-production.sh; grafita: deploy-production.sh, status-production.sh; fluorita: deploy-production.sh, status-production.sh; hematita: deploy-production.sh, status-production.sh; cuprita: deploy-production.sh, status-production.sh; calcita: deploy-production.sh, status-production.sh; selenita: deploy-production.sh, status-production.sh
