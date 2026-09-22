# The history property type, without lint suppressions — H1-D

- **Date:** 2026-09-21
- **Scope:** `H1-D` of
  [`../plans/archive/2026-09-21-h1-foundation.md`](../plans/archive/2026-09-21-h1-foundation.md):
  the review fixes on top of `H1-C` — the history property type, the removal
  of every `qmllint` suppression, the decorative checked state on the resource
  row, and the memory reading before the first snapshot
- **Environment:** Rust 1.97.1 (pinned), Qt 6, the author's checkout, no
  Wayland session used (offscreen only)
- **Artifact:** `hematita/target/release/hematita`, release profile, sealed by
  `scripts/verify-production.sh`

## Procedure

`H1-C` shipped the histories as `QList<QVariant>` and silenced the linter with
three `// qmllint disable unresolved-type` pairs. Both were wrong: the bridge
never refused `QList<f64>`, and this project suppresses no warnings. The
revert was attempted first and measured before anything was boxed.

```sh
# 1. Revert to QList<f64>, delete every suppression comment, then measure.
cd hematita && cargo fmt --all && ./scripts/build-production.sh
bash scripts/qmllint-cxxqt.sh hematita
# 2. Only because warnings remained: QVariant carrying a variant list.
cd hematita && cargo fmt --all && ./scripts/build-production.sh
bash scripts/qmllint-cxxqt.sh hematita
cd hematita && cargo test --release --locked
cd hematita && cargo clippy --release --all-targets --locked -- -D warnings
cd hematita && cargo fmt --all --check
hematita/scripts/verify-production.sh
```

## Result

- **Exit:** 0 for every command above.
- **Step 1 — `QList<f64>`, no suppressions.** The bridge compiled and linked
  exactly as the plan wrote it. `qmllint` then refused the tree:

  ```
  qmllint-production: hematita: warnings grew from 0 to 4; inventoried qmllint debt may not grow
  Warning: hematita/qml/components/PerformancePage.qml:52:42: Type "QList_f64" of property "cpuHistory" not found. This is likely due to a missing dependency entry or a type not being exposed declaratively. [unresolved-type]
  Warning: hematita/qml/components/PerformancePage.qml:61:42: Type "QList_f64" of property "memoryHistory" not found. This is likely due to a missing dependency entry or a type not being exposed declaratively. [unresolved-type]
  Warning: hematita/qml/components/PerformancePage.qml:74:56: Type "QList_f64" of property "cpuHistory" not found. This is likely due to a missing dependency entry or a type not being exposed declaratively. [unresolved-type]
  Warning: hematita/qml/components/PerformancePage.qml:74:82: Type "QList_f64" of property "memoryHistory" not found. This is likely due to a missing dependency entry or a type not being exposed declaratively. [unresolved-type]
  ```

  `QList<double>` is a sequence the engine hands JavaScript as an array; the
  name its cxx-qt alias carries into the generated metatype, `QList_f64`, is
  not a type the linter can resolve. The row may not rise above `0` and no
  warning may be suppressed, so this shape cannot ship.
- **Step 2 — the shape that ships.** The two properties are
  `#[qproperty(QVariant, cpu_history)]` and `memory_history`, each holding a
  variant list (`QVariant::from(&QList<QVariant>)`, the one `QVariant`
  conversion cxx-qt-lib 0.9.1 offers for a list; there is none from
  `QList<f64>`). Sixty boxed doubles per property per second is the accepted
  cost, taken only because step 1 measured that nothing cheaper lints clean.
  `qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline
  warning(s))` — the baseline row stays at `0` and no suppression comment
  remains anywhere in the project.
- **Tests:** `cargo test --release --locked` — `test result: ok. 1 passed; 0
  failed`.
- `cargo clippy --release --all-targets --locked -- -D warnings`: clean.
  `cargo fmt --all --check`: clean.
- **Verification:** `scripts/verify-production.sh` end to end, including
  `smoke: OK — binary alive for 8 s, no QML errors, no auto-bindings` and
  `manifest: hematita/target/production-artifact.toml (verified)`.

## The other three fixes

- `ResourceRow.qml` no longer sets `checkable`, `checked` or `autoExclusive`.
  They decorated nothing — the page owns the selection and paints it from
  `selected` — and the first click would have overwritten the `checked`
  binding. `Accessible.selected: row.selected` is what a screen reader reads
  and it stays.
- `Sampler`'s `Drop` comment said the thread checks the stop flag once per
  interval. It checks it every hundred milliseconds; the comment now says so,
  and says that is the worst case a close waits.
- `PerformancePage`'s `memoryValue` showed `0.0 GiB / 0.0 GiB` before the
  first snapshot. It now answers `—` while `memoryTotalKib` is zero, the same
  silence `percent()` keeps before the first CPU rate.

## Limits

No visual check was performed and none is claimed: nothing was opened on the
live or the nested Wayland session and no capture was taken. `VAL-H1` still
owns appearance, keyboard and focus. The boxing in `ring_list` was accepted on
a lint argument, not measured for cost; sixty doubles a second is far below
anything this application would notice, but that is a judgement, not a
benchmark.
