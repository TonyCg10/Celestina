# The sampler, the resources object and the Performance page — H1-C

> Superseded on the history-property point by [H1-D](2026-09-21-h1-history-type.md): the histories ship as QVariant lists with no lint suppressions.

- **Date:** 2026-09-21
- **Scope:** `H1-C` of
  [`../plans/archive/2026-09-21-h1-foundation.md`](../plans/archive/2026-09-21-h1-foundation.md):
  the one-second sampling thread, `HematitaResources`, single-instance
  activation over the session bus, and the Performance page
- **Environment:** Rust 1.97.1 (pinned), Qt 6, the author's checkout, no
  Wayland session used (offscreen only)
- **Artifact:** `hematita/target/release/hematita`, release profile, sealed by
  `scripts/verify-production.sh`

## Procedure

The load-state policy was written test-first. The test was run against a
`todo!()` body in the release profile (the profile the rest of the unit is
verified in, so the bridge compiles once), then the body was implemented and
the whole suite rerun.

```sh
cd hematita && cargo test --release --locked load_names          # RED
cd hematita && cargo test --release --locked --all-targets       # GREEN
cd hematita && cargo clippy --release --all-targets --locked -- -D warnings
cd hematita && cargo fmt --all --check
hematita/scripts/build-production.sh
bash scripts/qmllint-cxxqt.sh hematita
hematita/scripts/smoke.sh
hematita/scripts/verify-production.sh
```

The single-instance behaviour was checked on a private session bus, offscreen,
without opening a window on the author's session:

```sh
dbus-run-session -- sh -c '
  QT_QPA_PLATFORM=offscreen ./target/release/hematita & first=$!
  sleep 3
  QT_QPA_PLATFORM=offscreen timeout 5 ./target/release/hematita; echo "exit=$?"
  kill -TERM "$first"'
```

## Result

- **Exit:** 0 for every command above.
- **RED:** `cargo test --release --locked load_names` — `test result: FAILED.
  0 passed; 1 failed`, panicking `not yet implemented` at
  `src/resources.rs:27`.
- **GREEN:** `cargo test --release --locked --all-targets` —
  `test result: ok. 1 passed; 0 failed` for
  `resources::tests::load_names_the_state_the_page_paints`.
- `cargo clippy --release --all-targets --locked -- -D warnings`: clean.
- `cargo fmt --all --check`: clean after one `cargo fmt --all` pass.
- **qmllint:** `qmllint-production: OK — org.celestina.hematita (0 non-fatal
  baseline warning(s))`. The two `Unqualified access` warnings H1-A left in
  `NavStrip.qml`'s delegate were fixed here (an `id` on the delegate plus
  `pragma ComponentBehavior: Bound`), so `scripts/qmllint-baseline.tsv`'s
  `hematita` row falls from `2` to `0` in this commit.
- **Smoke:** `smoke: OK — binary alive for 8 s, no QML errors, no
  auto-bindings`. Eight seconds is more than two sampling intervals, so the
  graph bindings ran against real snapshots.
- **Verification:** `scripts/verify-production.sh` end to end — the crate's
  `17 passed; 0 failed` unit tests plus `3 passed; 0 failed` captures,
  clippy, fmt, the qmllint line above, the smoke line above, and
  `manifest: hematita/target/production-artifact.toml (verified)`.
- **Twice-launch:** the second launch printed `second: exit=0
  elapsed_ms=9` — it reached the running instance over `Activate` and left
  without building a window. The first was then ended by its own pid
  (`first pid 1349980 exited on SIGTERM`), never by process name.

## Deviations from the plan

- The CPU identity travels in the first snapshot rather than being read in
  `HematitaResourcesRust::default()`. `default()` runs on the Qt thread and
  reading `/proc` there would break the rule that blocking IO stays off it, so
  `Snapshot` carries `identity: Option<Identity>`, set only on generation 1.
- `CpuReading` does not carry `core_percents`. Nothing reads it before H2 and
  a field nobody reads is a dead-code warning under `-D warnings`; the core
  crate still computes and tests the per-core rates.
- The history properties are `QList<QVariant>` (`QList_QVariant` in the
  bridge), not `QList<f64>`. `QList<f64>` compiled and linked, but it is not a
  sequence QML is guaranteed to hand JavaScript as an array, and the variant
  list is. The four `unresolved-type` warnings a cxx-qt sequence alias raises
  in `qmllint` are disabled per binding in `PerformancePage.qml` with a
  comment saying why.
- `PerformancePage`'s resource handle is `metrics`, not `resources`:
  `QQuickItem` already owns `resources` for its non-visual children, and
  shadowing it is both a qmllint `property-override` warning and a real
  hazard.

## Limits

No visual check was performed and none is claimed: nothing was opened on the
live or the nested Wayland session, no capture was taken, and `grim` was not
run. Layout, colour, the focus ring, keyboard reachability and the graph's
appearance all belong to `VAL-H1`. The offscreen smoke proves the objects
construct and the bindings evaluate without QML errors for eight seconds; it
proves nothing about what the page looks like. Per-core percentages, disks,
interfaces and sensors are out of H1.
