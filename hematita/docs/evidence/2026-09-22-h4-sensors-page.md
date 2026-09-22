# The Sensors page with every hwmon chip and the process table follow-ups — H4-B

- **Date:** 2026-09-22
- **Scope:** `H4-B` of
  [`../plans/archive/2026-09-22-h4-sensors.md`](../plans/archive/2026-09-22-h4-sensors.md):
  the sampler's sensors section, `publish::thermal_load`, the
  `HematitaSensors` hub, the Sensors page and its two components, and the
  six process-table items parked by `H3-E`
- **Environment:** the author's checkout, an AMD Ryzen 7 9800X3D session
  under niri; every run headless (`QT_QPA_PLATFORM=offscreen`)
- **Artifact:** `hematita/target/production-artifact.toml`, verified

## What changed

1. **`sampler.rs` reads hwmon every tick.** `sample_sensors` lists
   `/sys/class/hwmon`, keeps every `hwmonN` directory in sorted order, and
   turns each into a `hematita_core::sensors::Chip` through `discover`. What
   a directory cannot change is read once into a private `ChipFacts` (the
   chip's `name`, and every `_label`, `_max`, `_crit`, `_cap` file); only the
   `_input` and `_average` files are read again each tick. A chip whose value
   file vanishes keeps its other channels — the read failure skips that one
   file; a chip directory that vanishes is dropped from the cache and from
   the list by the `retain` over this tick's keys. `/sys/class/hwmon` itself
   being unreadable makes the whole section `Section::Unavailable` with
   `unreadable` and that path, while every other section keeps publishing.
   `Snapshot` gained `sensors: Section<SensorSnapshot>`, filled on every
   tick (not every second tick as processes are).
2. **`publish::thermal_load(value, crit)`** gives a temperature the same two
   thresholds the rest of the monitor uses (`ELEVATED_PERCENT` 80,
   `CRITICAL_PERCENT` 90), applied to the fraction of the chip's own
   `crit` limit rather than to a percentage of anything global: a channel
   without a usable `crit` (absent, or zero) is `normal`, because a limit
   nobody published is not a limit at zero.
3. **`src/sensors.rs` — `HematitaSensors`.** A third hub beside
   `HematitaResources` and `HematitaProcesses`, published the same way: a
   `start()` that subscribes to the shared sampler once, an `apply` that
   drops a generation not newer than the last applied, and index-aligned
   lists with `revision` set last so the page never reads one list from this
   snapshot beside another from the last. The one thing only this object
   knows is the session's extremes per channel, kept in a
   `chipKey/kind/index` → `(min, max)` map; a channel the machine stops
   publishing takes its extremes with it. `apply` runs in two passes — the
   extremes first over the chips, then the lists reading them back
   immutably — per the controller's decision 3, which also asked for the
   pure `fold_extremes` helper (first reading is both ends, later readings
   only widen) with its own unit test.
4. **The page.** `SensorsPage` composes every word from tokens: the chip
   title from the driver name — seven of this machine's drivers (`k10temp`,
   `amdgpu`, `nvme`, `it8696`, `gigabyte_wmi`, `acpitz`, `ath12k_hwmon`) get
   a Spanish word through `qsTr()` naming what they measure (the processor,
   the graphics card, an NVMe disk, the board, the board over WMI, ACPI and
   Wi-Fi respectively), and an unmapped driver shows its own name — the
   channel word from the kind token when the kernel gave no label, the unit
   and the digit count from the kind, and the limit sentence from whichever
   of `crit` and `max` the chip published. A repeated driver name gets an ordinal. The kernel's own
   labels are shown raw. `SensorChipCard` is one chip as the suite's
   `ListSection`; `SensorRow` is one channel — label, value coloured by its
   load token, the session's extremes and the kernel's limit — hover only,
   no selection, reachable by Tab with a focus ring and an
   `Accessible.name` that names the channel, its value and its limit.
5. **`Main.qml`** gained `HematitaSensors { id: sensorHub }`, started in
   `Component.onCompleted`, and the fourth section's placeholder became
   `SensorsPage { sensors: sensorHub }`.
6. **The six parked items**, all in `qml/components/ProcessTable.qml`
   except the last:
   - **The success message survives a rebuild.** `onSelectedPidChanged`
     returns early while `table.anchoring`, so `clearAction()` runs only for
     a selection the person made. A selection released by the re-anchor —
     the row left the rebuilt list — is not the person moving on, and a
     successful terminate keeps its sentence until another row is picked.
   - **The header and the rows share one inset.** `ProcessHeader` is given
     `Layout.leftMargin`/`rightMargin: CelestinaTheme.spaceXs` (the row
     surface's padding, which is why the cells were one inset off their
     values), and `fixedTotal` now adds `2 * CelestinaTheme.spaceXs` so the
     name column's share gives that padding back on both sides.
   - **A hidden table costs nothing per tick.** The `Connections` handler on
     `revision` and a new `onVisibleChanged` both gate `weave()` on
     `table.visible`; the page that is not showing is rebuilt whole when it
     comes back.
   - **The search is debounced.** `onTextChanged` restarts a 150 ms `Timer`
     whose `onTriggered` sets `filterText` and calls `refresh()`, so a typed
     word re-sorts every process once instead of once per letter.
   - **`layout()` is one pass.** The grouped sequence buckets every row by
     its application index first and then emits application + bucket,
     instead of scanning every row once per application.
   - **`sampler::subscribe` registers inside the `handle` lock**
     (`src/sampler.rs`): the `handle` lock is taken first and held across the
     push onto `subscribers`, so a `subscribe` racing a `stop()` either
     registers before that `stop` drains the list and is dropped with it, or
     waits and registers on a hub already stopped and armed again. `stop()`'s
     comment now states the guarantee this makes total: no subscriber can be
     registered between the drain and the lowering of the flag, so a later
     `subscribe` never finds a drained hub holding a subscriber that will
     never be called.

## The sensor contracts

- **Chip lists**, one entry per `hwmonN` directory in sorted key order:
  `chipKeys` (the `hwmonN` key, stable for the session and not across
  boots), `chipNames` (the kernel driver's name), `chipCounts` (how many
  channels that chip published).
- **Channel lists**, one entry per channel of every chip, in chip order and
  within a chip sorted by kind then kernel index: `channelChips` (the index
  into the chip lists), `channelKinds`, `channelIndices` (the kernel's index
  within the kind), `channelLabels` (the kernel's own label, empty when the
  chip has none), `channelValues`, `channelMins`, `channelMaxs` (the
  session's extremes), `channelLimitMax`, `channelLimitCrit` (`0` for a
  limit the chip did not publish), `channelLoads`.
- **Tokens.** Kind: `temperature`, `fan`, `voltage`, `power`, `current`.
  Load: `normal`, `elevated`, `critical` — only a temperature can be other
  than `normal`; every other kind is `normal`, since the kernel's `max` on a
  voltage or a fan is not a danger scale. Reason: `unreadable`, `malformed`,
  `no-rate`, with `reasonPath`.
- **Units per kind**, applied by the page: temperature `°C` (1 decimal), fan
  `rpm` (0), voltage `V` (3), power `W` (1), current `A` (1). The kernel's
  fixed-point integers are converted in `hematita_core::sensors::convert`.
- `revision` is set last, after every list above is in place.

## Per-tick cost

This machine has 9 `hwmon` directories (`acpitz`, `amdgpu`, `ath12k_hwmon`,
`gigabyte_wmi`, `it8696`, `k10temp`, `nvme` ×2, `prom21_xhci`). Each tick
reads **46 files** — every `_input` and `_average` — plus one `readdir` of
`/sys/class/hwmon`. The first tick additionally reads the 9 `name` files, the
47 `_label`/`_max`/`_crit`/`_cap` files and one `readdir` per directory, and
never reads any of them again while the directory exists. Every one is a
small sysfs file and every read happens on the sampler thread, never on the
Qt thread.

## Procedure

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets \
  && cargo clippy --release --all-targets --locked -- -D warnings \
  && cargo fmt --all --check)
hematita/scripts/verify-production.sh
QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 HEMATITA_SMOKE_SHAPE=1 \
  timeout 8 hematita/target/release/hematita 2>&1 \
  | grep -E 'hematita-shape|TypeError|ReferenceError|Unable to assign|Cannot read property'
```

## Result

- **Exit:** 0 for every command.
- `build-production.sh` — `Finished release profile ... in 16.06s`,
  `>> Hematita release build steps completed (not installed)`. The only
  compiler output was the pre-existing `-Wsfinae-incomplete` warnings from
  Qt's own headers, now also on `sensors.cxx.cpp`.
- `cargo test --release --locked --all-targets` — `12 passed; 0 failed`,
  including the two new `sensors::tests`
  (`extremes_start_at_the_first_reading_and_only_widen`,
  `an_extreme_is_remembered_per_chip_kind_and_index`) and the new
  `publish::tests::a_temperature_loads_against_its_own_critical_limit`
  (50/100 `normal`, 85/100 `elevated`, 100/100 `critical`, no limit
  `normal`, a zero limit `normal`).
- `cargo clippy --release --all-targets --locked -- -D warnings` — no
  warnings. `cargo fmt --all --check` — no diff (one `cargo fmt --all` pass
  had already re-wrapped the brief's snippets; no logic changed).
- `verify-production.sh` — `hematita-core` `59 passed`, `captures.rs`
  `11 passed`, doc-tests `0`; `qmllint-production: OK —
  org.celestina.hematita (0 non-fatal baseline warning(s))`; `smoke: OK —
  binary alive for 10 s, the first row published the CPU contract, no QML
  errors, no auto-bindings`; manifest `verified`.
- The 8-second offscreen run printed `qml: hematita-shape cpu 3 60` and
  matched none of `TypeError`, `ReferenceError`, `Unable to assign`,
  `Cannot read property`. (`QT_ASSUME_STDERR_HAS_CONSOLE=1` is needed for
  Qt to write `console.info` to a captured stderr, as `scripts/smoke.sh`
  already sets.)

## Departures from the brief

- `SensorChipCard` exposes `chipTitle`, not `title`, and binds
  `title: card.chipTitle` — the controller's decision 2, because
  `ListSection.title` is inherited and an inherited property can be neither
  redeclared nor made `required`. The same collision applies to the brief's
  `required property var rows`: `rows` is `ListSection`'s **default**
  property (its inner column's children, where the `Repeater` lands), so the
  channel data is exposed as `channels` under exactly the same rule, and
  `SensorsPage` passes `chipTitle` and `channels`.
- `apply` does the two passes of decision 3 with the `fold_extremes` helper,
  rather than mutating `extremes` inside the list-building loop.
- `ProcessHeader`'s inset is `Layout.leftMargin`/`rightMargin` rather than
  `anchors.leftMargin`/`rightMargin`: the header is a `ColumnLayout` child,
  where anchors do not apply.

## Limits

- **No visual check.** Every run here was headless; nothing in this document
  proves how the page looks, how the cards space, or that a value's colour
  reads as intended. That is `VAL-H4`.
- **The page's delegates were not constructed in the headless run.** The
  Sensors page is the fourth `StackLayout` child, so it is not visible, and
  its `weave()` is gated on visibility — the `Repeater`'s model stayed at 0
  and no `SensorChipCard`/`SensorRow` was instantiated. `qmllint` resolves
  both files with zero warnings, but only `VAL-H4` (or a run with the
  section selected) proves they construct.
- **The chip names are mapped for this machine's drivers only.** Seven
  driver names are given Spanish words; every other driver shows the
  kernel's own name, which is correct but not translated.
- The per-tick count above is this machine's; another machine with more
  chips reads proportionally more small files.
- Nothing here proves the sampler's cost under a profiler; the count is an
  inventory of reads, not a measurement. `VAL-H4` covers the felt cost.

## Follow-up

`H4-Z` (implementation exit and 0.5.0). `VAL-H4` is recorded pending in
`VALIDATION.md` and does not block this closure.
