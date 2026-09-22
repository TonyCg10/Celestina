# The resource order and a list model that survives a revision — H2-C

- **Date:** 2026-09-22
- **Scope:** `H2-C` of
  [`../plans/active/2026-09-22-h2-resources.md`](../plans/active/2026-09-22-h2-resources.md):
  the six review findings against `H2-B` — disk and interface order, the
  `ListView` model, the GPU subtitle and its reason file, the per-core toggle,
  and total indexing in `sample_gpu`
- **Environment:** Rust 1.97.1 (pinned), Qt 6 with CXX-Qt 0.9.1, the author's
  checkout
- **Artifact:** `hematita/target/release/hematita`, rebuilt and re-sealed by
  `hematita/scripts/verify-production.sh`

## What each finding changed

1. **Order.** `sample_disks` and `sample_interfaces` sort their `readings` by
   name before rating. The order is now the machine's inventory rather than
   whatever order `/proc/diskstats` or `/proc/net/dev` happened to list, it no
   longer changes when a read fails (the sysfs fallback already sorted), and
   the `rates.iter().find(...)` lookup became deterministic.
2. **The list model.** `PerformancePage.qml` gives the `ListView`
   `model: page.rows.length` and the delegate `required property int index`
   with `readonly property var row: page.rowAt(index)`. The woven array is
   still reassigned each revision, so every binding on `row.*` re-evaluates,
   but a same-count second changes no model — delegates, scroll position and
   current index survive. Only a resource appearing or disappearing rebuilds
   the list. `page.rowAt` answers a row-shaped `emptyRow` for the negative
   index a delegate briefly carries while it is torn down, so no binding ever
   reads a field of `undefined`.
3. **The GPU subtitle** is now empty: `1002:7550` is a key the code matches
   on, not a line a person reads.
4. **The GPU reason names the file that failed.**
   `GpuError::UnreadableNumber { file, .. }` maps to
   `malformed(&device.join(file))` instead of always blaming
   `gpu_busy_percent`.
5. **The per-core toggle** no longer assigns into `checked`. See the
   deviation below.
6. **`sample_gpu` indexes nothing.** The eight texts become a `[String; 8]`
   via `<[String; 8]>::try_from` and are destructured into eight bindings, so
   an out-of-range index is impossible by construction rather than by reading
   the code.

## One deviation from the review's wording

Finding 5 asked for `onClicked: detail.showCores = !detail.showCores` with
`checked: detail.showCores` kept as a binding. That would not have held: a
`checkable` `AbstractButton` toggles its own `checked` on click, so Qt itself
would have destroyed the binding on the first press — the same defect the
finding names, moved one step.

The fix keeps the finding's intent and the shared button's stated contract
("a toggle is `checkable: true` and nothing else"): the button owns its
checked state, and `ResourceDetail` reads it —
`readonly property bool showCores: coreToggle.checked`. Nothing assigns into
`checked` from anywhere, so no binding can be destroyed, and the two values
cannot drift because there is only one.

## Procedure

One build cycle, after all six edits:

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets \
    && cargo clippy --release --all-targets --locked -- -D warnings \
    && cargo fmt --all --check)
hematita/scripts/verify-production.sh
QT_QPA_PLATFORM=offscreen timeout 6 hematita/target/release/hematita 2>&1 \
    | grep -E 'Error|Warning|unavailable|Binding loop|Cannot anchor|TypeError'
```

## Result

- **Build:** compiled first time;
  `>> Hematita release build steps completed (not installed)`.
- **Tests:** `test result: ok. 4 passed; 0 failed` (the four `publish::tests`,
  unchanged by this unit); `hematita-core` `37 passed; 0 failed` plus
  `5 passed; 0 failed` for the captures, run again by the verification.
- **Clippy** `-D warnings` and **`cargo fmt --all --check`:** clean in both
  workspaces.
- **qmllint:**
  `qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline warning(s))`
  — the row stayed at `0` and nothing is suppressed, including the new
  delegate's `required property int index` and its `row` lookup.
- **Smoke:** `smoke: OK — binary alive for 8 s, no QML errors, no auto-bindings`.
- **Verification exit:** `0`; `manifest: … (verified)`.
- **Offscreen run:** the grep, widened with `TypeError`, printed nothing;
  `rc=124`. Six seconds means six revisions through the new delegate path
  with no type error from `row.*` and no binding loop.

## Limits

- Still no visual check: nothing was opened on the live or nested session.
  That the scroll position and selection now survive a revision is argued from
  the model's type and shown only by a silent offscreen run; seeing a list
  hold its place while it updates belongs to `VAL-H2`.
- The order fix was not observed changing anything on this machine, whose
  `/proc/diskstats` already happens to list its four whole disks in name
  order; it is the failure path and a differently ordered machine that the
  sort protects.
- Deferred by the controller and not done here: re-reading static sysfs facts
  every tick, and resizing `core_rings` when a core is hot-plugged.
