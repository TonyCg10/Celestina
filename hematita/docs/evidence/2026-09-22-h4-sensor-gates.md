# The hwmon cache key, the sensor rows and the smoke's section walk — H4-C

- **Date:** 2026-09-22
- **Scope:** `H4-C` of
  [`../plans/active/2026-09-22-h4-sensors.md`](../plans/active/2026-09-22-h4-sensors.md):
  the review findings on `H4-B` — the `hwmon` cache validated by chip name,
  sensor rows that survive a tick, a smoke that shows every section, the
  contract note on `channelStates`, and two smaller fixes
- **Environment:** the author's checkout, an AMD Ryzen 7 9800X3D session
  under niri; every run headless (`QT_QPA_PLATFORM=offscreen`)
- **Artifact:** `hematita/target/production-artifact.toml`, verified

## What changed

1. **The `hwmon` cache is validated by the chip's name, not by its index**
   (`src/sampler.rs`). `hwmonN` is not an identity: a device that goes away
   frees its index and the next device to bind can be given it, and the
   cached facts — the labels, the `max` and the `crit` — would then be read
   as if they described the new device. A stale `crit` is not a cosmetic
   error: it is the divisor `publish::thermal_load` colours a temperature by.
   `sample_sensors` now reads each chip's `name` every tick (one small file
   per chip) and drops the cached `ChipFacts` when it differs, so the next
   line re-enumerates that directory from scratch.
2. **The session's extremes are keyed by chip name too** (`src/sensors.rs`).
   `extreme_key` became `chipKey/chipName/kind/index`, so a re-bound index
   starts fresh extremes instead of inheriting the minimum and maximum of
   whatever held that number before it — the same reasoning the process
   table already applies to a recycled PID. The unit test gained the case.
3. **A sensor row survives its tick** (`qml/components/SensorChipCard.qml`).
   The card's `Repeater` took `card.channels` — a fresh array every second —
   as its model, so every `SensorRow` was destroyed and rebuilt each tick,
   taking the keyboard focus with it: a row could not be held long enough to
   read it with Tab. The model is now `card.channels.length` and each row
   reads its own values by index through a `readonly property var channel`,
   which is the pattern the outer card `Repeater` and both process tables
   already use. A `card.emptyChannel` covers an index momentarily past the
   array.
4. **The smoke shows every section** (`src/main.rs`, `qml/Main.qml`,
   `scripts/smoke.sh`). A `StackLayout` builds only the page it is showing,
   so the three pages nobody selected were never constructed and a page that
   could not be built at all would have passed the gate — exactly the shape
   of failure the smoke exists to catch. `HEMATITA_SMOKE_SECTIONS` now
   reaches the window as `smokeSections`, where a repeating one-second
   `Timer` advances `currentSection` 0→1→2→3 and then stops. It prints
   nothing: the gate is that the error scan still finds nothing once all four
   pages have been up, which the existing ten-second timeout leaves time for.
   `HEMATITA_SMOKE_SHAPE` is untouched, and the smoke's success line now says
   the sections were shown.
5. **An unmapped driver gets its ordinal too**
   (`qml/components/SensorsPage.qml`). `chipTitle` appended the ordinal only
   on the mapped branch, so a machine with two chips of an untranslated
   driver would show two cards with the same eyebrow. (This machine's two
   `nvme` chips are mapped, so the bug was latent here and would have shown
   on another machine.)
6. **Return applies the search at once**
   (`qml/components/ProcessTable.qml`). The 150 ms debounce is right for a
   burst of keystrokes and wrong for a finished word: `Keys.onReturnPressed`
   and `onEnterPressed` stop the timer and apply the filter immediately. The
   timer moved out of the field and beside it, and the one path to the hub is
   now the new `table.applyFilter()`, called from the timer and from Return.

## The contract note on `channelStates`

The plan's sensor contract lists a `channelStates` column beside the others.
It is **not** published, and it is struck from the contract rather than
added: `hematita_core::sensors::discover` skips any channel whose value file
does not parse, so a published channel is by construction a channel that was
read, and the `unavailable` state the column would carry is unreachable. The
one failure a person can meet is the whole section's — `/sys/class/hwmon`
itself unreadable — which `available`, `reasonKind` and `reasonPath` already
carry, and which the page already says. The `H4-C` ledger row records the
same deviation. (The plan document under `docs/superpowers/` is outside this
work's pathspec and was not edited.)

## Procedure

```sh
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets \
  && cargo clippy --release --all-targets --locked -- -D warnings \
  && cargo fmt --all --check)
hematita/scripts/verify-production.sh
QT_QPA_PLATFORM=offscreen QT_ASSUME_STDERR_HAS_CONSOLE=1 \
  HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 \
  timeout 10 hematita/target/release/hematita 2>&1 \
  | grep -E 'TypeError|ReferenceError|Unable to assign|Cannot read property'
```

## Result

- **Exit:** 0 for every command; the offscreen run ends at the timeout
  (`rc=124`, the binary alive throughout) and its grep matched nothing.
- `build-production.sh` — `Finished release profile [optimized] target(s) in
  14.37s`, `>> Hematita release build steps completed (not installed)`.
  Compiled on the first attempt.
- `cargo test --release --locked --all-targets` — `12 passed; 0 failed`,
  including the renamed
  `sensors::tests::an_extreme_is_remembered_per_chip_name_kind_and_index`,
  which now also asserts that the same index under two driver names is two
  different keys.
- `cargo clippy --release --all-targets --locked -- -D warnings` — no
  warnings. `cargo fmt --all --check` — no diff.
- `verify-production.sh` — `qmllint-production: OK —
  org.celestina.hematita (0 non-fatal baseline warning(s))` with no
  suppressions added; `smoke: OK — binary alive for 10 s, every section was
  shown, the first row published the CPU contract, no QML errors, no
  auto-bindings`; manifest `verified`.
- The manual ten-second walk printed exactly one line, `qml: hematita-shape
  cpu 3 60`, and none of the four error patterns. This is the first run in
  which `SensorsPage`, `SensorChipCard` and `SensorRow` were actually
  constructed — the `H4-B` limit that said they had not been is now closed,
  for construction.

## Limits

- **Still no visual check.** Every run was headless. That the three sensor
  components construct without error is not that they look right, space
  right or colour right; that is `VAL-H4`.
- **The keyboard fix is reasoned, not observed.** Nothing here presses Tab.
  That a `SensorRow` now survives its tick follows from the model being a
  count rather than a fresh array — the same change that fixed the process
  list — but only a real session proves the focus stays.
- **The re-bind path is not exercised.** No device was unbound and re-bound
  during a run; the name check is proved by reading, not by a live rebind.
- One unreadable tick still erases a channel's extremes (the `retain` drops
  a key the tick did not publish). The controller deferred this to `H5`.
- The chip names are still mapped for this machine's drivers only.

## Follow-up

`H4-Z` (implementation exit and 0.5.0). `VAL-H4` is recorded pending in
`VALIDATION.md` and does not block this closure.
