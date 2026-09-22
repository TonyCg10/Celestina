# The Performance list is operable, and the smoke proves a row's shape — H3-A

- **Date:** 2026-09-22
- **Scope:** `H3-A` of
  [`../plans/archive/2026-09-22-h3-processes.md`](../plans/archive/2026-09-22-h3-processes.md):
  the list's keyboard and accessibility path, the smoke's shape gate, and the
  six follow-ups H2 booked; H3 opened in the documents
- **Environment:** Rust 1.97.1 (pinned), Qt 6 with CXX-Qt 0.9.1, the author's
  checkout
- **Artifact:** `hematita/target/release/hematita`, rebuilt and re-sealed by
  `hematita/scripts/verify-production.sh`

## What changed

1. **The list is a list.** `PerformancePage.qml`'s `ListView` takes
   `activeFocusOnTab`, `keyNavigationEnabled`, `Accessible.role: Accessible.List`
   and a name, and its `currentIndex` is bound to `page.indexOf(page.selectedKey)`
   with the reverse write in `onCurrentIndexChanged`. Selection and current item
   are now the same thing, so an arrow key moves the selection, the view scrolls
   to it, and an off-screen row is reachable without a pointer. The screen
   reader follows the current item instead of only the realised delegates.
2. **The network subtitle waits for the row.** `subtitleFor` answers `""`
   unless the row is `ready`; a row with no numbers no longer claims "Cable".
3. **`no-rate` means what it says.** In `sample_cpu` only
   `CpuError::NoElapsedTime` maps to `ReasonKind::NoRate`; every other sampler
   error is `malformed(/proc/stat)`, which is what a line that does not parse
   into a rate actually is.
4. **The core count is this second's.** `rows_from` derives it from
   `reading.core_percents.len()` and rebuilds `core_rings` when it changes, so
   a hot-plugged core is drawn and a `/proc/stat` unreadable at startup no
   longer freezes the count at zero. `publish::cpu_numbers` is given that
   count; the identity's value stays the initial one.
5. **Static sysfs facts are read once per name.** `run` owns a
   `HashMap<String, DiskInfo>` and a `HashMap<String, (bool, bool)>`; the disk
   model, size and rotational flag, and an interface's shown verdict and
   wireless flag, are read when the name first appears and dropped when it goes.
   An interface's `operstate` and `speed` are still read every tick, because a
   cable pulled out changes them.
6. **`sample_gpu` has no dead arm.** The eight texts become `[String; 8]`
   through a `let ... else`, and a read that did not yield eight strings is
   `Unavailable(malformed(...))` rather than "no card".
7. **The capture asserts the disk set.** `tests/captures.rs` compares the
   sorted whole-device names against `["nvme0n1", "nvme1n1", "sda", "sdb"]`
   instead of counting them.
8. **The smoke proves shape.** `HEMATITA_SMOKE_SHAPE` reaches `Main.qml` as
   the window's `smokeShape` the way `reducedMotion` does; on the third
   revision the window prints the first row's kind and the lengths of its
   numbers and its history, once. `scripts/smoke.sh` sets the variable, waits
   ten seconds, and fails unless the line reads `hematita-shape cpu 3 60`.

## Two deviations from the brief's wording

- The printed line arrives as `qml: hematita-shape cpu 3 60`, so the gate is
  the `grep -E 'hematita-shape cpu 3 60$'` the brief provided for that case
  rather than the exact `case` match.
- `parse_diskstats` answers in file order, which on this capture is
  `nvme0n1, sda, nvme1n1, sdb`. The test sorts the names before comparing, the
  way `sample_disks` sorts its readings, so the assertion is the exact set
  rather than the order one kernel file happened to print.

## Procedure

One build cycle, after every edit above. The capture change invalidated the
production inputs after the first build, so the build step ran again before
the verification; nothing else was rebuilt to "check".

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets \
    && cargo clippy --release --all-targets --locked -- -D warnings \
    && cargo fmt --all --check)
(cd celestina-rs && cargo test -p hematita-core)
hematita/scripts/verify-production.sh
```

## Result

- **Build:** compiled;
  `>> Hematita release build steps completed (not installed)`.
- **Tests:** `test result: ok. 4 passed; 0 failed` in `hematita`;
  `37 passed; 0 failed` plus `5 passed; 0 failed` for `hematita-core` and its
  captures, run again by the verification.
- **Clippy** `-D warnings` and **`cargo fmt --all --check`:** clean.
- **qmllint:**
  `qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline warning(s))`
  — the row stayed at `0`, nothing suppressed, including the list's new
  accessible role and the window's `smokeShape`.
- **The shape line:** `qml: hematita-shape cpu 3 60` — the first row is the
  CPU, its numbers are the three the contract names, and its history is a full
  minute of samples.
- **Smoke:**
  `smoke: OK — binary alive for 10 s, the first row published the CPU contract, no QML errors, no auto-bindings`.
- **Verification exit:** `0`; `manifest: … (verified)`.

## Limits

- The keyboard walk was not performed: nothing was opened on the live or the
  nested session, and no screen reader was attached. That Tab reaches the list
  and that an arrow scrolls an off-screen row into view is argued from the
  `ListView` contract and shown only by a clean offscreen start; seeing it
  belongs to `VAL-H2`.
- The core-count resize was not observed: this machine hot-plugs no cores. It
  is the failure path and a different machine that the resize protects.
- The cached sysfs facts were not measured against the old per-tick reads; the
  claim is that fewer reads happen, not that anybody could see the difference.
