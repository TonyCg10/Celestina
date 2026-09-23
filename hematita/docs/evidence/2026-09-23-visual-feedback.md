# The first real-session visual and naming defects — VIS-1

- **Date:** 2026-09-23
- **Scope:** `VIS-1` of
  [`../plans/archive/2026-09-23-visual-feedback.md`](../plans/archive/2026-09-23-visual-feedback.md):
  the nine defects the author reported on first looking at 0.6.1 on the
  real session, and 0.6.2
- **Environment:** the author's checkout, an AMD Ryzen 7 9800X3D session
  under niri
- **Artifact:** `hematita/target/production-artifact.toml` (verified and
  deployed to `~/.local/bin/hematita` at 0.6.2)

## What changed, per defect

1. **The strip's items sat at the top of their pills.** Cause: `NavItem`'s
   `contentItem` was a `Column` with `anchors.centerIn: parent`, and a
   `Control` positions and sizes its `contentItem` itself, so the anchor was
   overridden and the column sat at the content rectangle's top. Fix
   (`qml/components/NavItem.qml`): the `contentItem` is an `Item` whose
   implicit size is the column's, and the column centres inside it.
2. **The sparklines overflowed their rounded background.** Cause: the
   `Shape` filled the whole graph, so the area fill reached the square
   corners under the background's radius. Fix
   (`qml/components/HistoryGraph.qml`): the shape is drawn in an inner `Item`
   inset by `spaceXs` on every side with `clip: true`, and the points are
   computed against that plot's own width and height.
3. **Every graph was the same colour.** Fix: `HistoryGraph` gained
   `required property string kind` and takes its trace from the kind's glyph
   accent — `cpu` blue, `memory` violet, `gpu` coral, `disk` amber,
   `network` green — while `elevated` still overrides to `warning` and
   `critical` to `danger`. The kind is threaded from `PerformancePage`
   through `ResourceRow`, `ResourceDetail` and `CoreGrid` (always `cpu`);
   the row's value text takes the same colour at `mutedContentOpacity`.
4. **The Processes page read as one flat surface.** Fix
   (`ProcessTable.qml`, `ProcessHeader.qml`, `ServicesPage.qml`): the bar
   stays on the canvas — the search keeps its `Search` shape, the count is a
   `textFaint` caption, and each capsule's Ghost glyphs get
   `spacing: spaceXs` and `inset: spaceXs`. The table is one
   `CelestinaSurface { role: Panel }` with no padding holding the header
   (caption size, `sectionLetterSpacing`, `textFaint`, the active column in
   `text` with its sort glyph), a `divider` hairline, and the `ListView`
   inset by `spaceSm` on every side and clipped, with the scrollbar inside
   the inset; the name column's share gives the new inset back. The Services
   page has the same bar and card, with a label header naming
   the unit and its scope, because its list is not sortable.
5. **Names nobody could read.**
   - Processes: `hematita_core::process_view::display_name(&str) -> &str`
     returns a path's last segment, split on `/` and `\`, ignoring a
     trailing separator. Only a name that starts with `/` or carries a `\`
     is a path: kernel threads are named `kworker/0:1` and must stay whole.
     `HematitaProcesses` publishes it as the additive `processDisplayNames`
     list; the row shows it and its `Accessible.name` carries the full name.
   - Services: `ServiceRow` leads with the description when it is non-empty
     and differs from the name, the unit name the second line; the search in
     `services::project` already matched both.
   - Sensors: `SensorsPage.labelWord` maps the known hwmon labels to
     product copy through `qsTr()` and falls back to the raw label.
6. **Sensors needed a colour per kind.** `SensorRow` gained `kind`: the
   value and a `compStatusIndicatorSize` dot before the label take
   temperature coral, fan cyan, voltage violet, power green, current amber,
   the thermal load still overriding to `warning`/`danger`.
7. **Absurd limits.** Cause: an NVMe `temp1_max` is the kernel's
   all-ones sentinel, 65 261.85 °C. Fix (`celestina-rs/crates/hematita-core/src/sensors.rs`):
   `plausible_limit(kind, value)` with the named constants
   `TEMPERATURE_LIMIT_CEILING` (200 °C), `TEMPERATURE_LIMIT_FLOOR`
   (−100 °C), `FAN_LIMIT_CEILING` (100 000 rpm), `VOLTAGE_LIMIT_CEILING`
   (1 000 V) and `POWER_LIMIT_CEILING` (100 000 W); `discover` drops a `max`
   or `crit` outside them. The page words the kernel's maximum as a limit
   rather than a second maximum, and joins it to the extremes with " · ". Readings are formatted with
   `toFixed` and the locale's `decimalPoint`, because
   `Number.toLocaleString` always inserts the group separator and "3,650 rpm"
   read as three and a bit in a comma-decimal locale.
8. **Every own process's disk rates read "—".** Investigation of
   `sample_processes` in `src/sampler.rs`: `/proc/<pid>/io` is read on
   every process tick for every process whose uid is the author's; the
   reading is pushed under `io_key(pid, start_ticks)` and the rate is looked
   up under the same key; `NamedCounters::sample` stores every reading,
   including the first, and rates the second (covered by its existing
   tests). **The pipeline was not defective.** The defect was downstream:
   `read_bytes`/`write_bytes` count storage IO only, so most processes
   measure an honest `0` between two readings, and `row_of` published a
   missing rate as `0` too, while `ProcessTable.rateText` drew anything not
   above zero as "—". A measured idle process and an unreadable one were
   indistinguishable. Fix (`src/processes.rs`, `ProcessTable.qml`): a
   missing rate is published as `NO_RATE` (`-1`), the page draws "—" only
   below zero and "0 B/s" for a measured zero. Tests:
   `a_reading_without_rates_yet_reads_as_no_rate_not_zero` and
   `a_measured_idle_rate_stays_zero`.
9. **Two `hematita` processes were running.** The activation hand-off
   (`activation::hand_off`) needs the session bus: a launch that cannot
   reach it, or cannot call `Activate`, opens its own window. That failure
   was silent. It is now said once on stderr; a `ServiceUnknown` or
   `NameHasNoOwner` answer — the ordinary first launch — says nothing. No
   other change.

## Procedure

The crate tests, the binary's tests, one `complete-production.sh`, the
status script, a plain SHA-256 comparison of the built and installed bytes,
and a 12-second offscreen walk through every section.

## Result

```text
$ cd celestina-rs && cargo test -p hematita-core
test result: ok. 73 passed; 0 failed   (67 before: +4 display_name, +2 limits)
test result: ok. 11 passed; 0 failed   (captures)

$ cd hematita && cargo fmt --all && cargo test --release
test result: ok. 23 passed; 0 failed

$ bash hematita/scripts/complete-production.sh        # exit 0
Architecture contract: OK
qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline warning(s))
smoke: OK — binary alive for 10 s, every section was shown, the first row
published the CPU contract, the Sensors page published chips, the Services
page listed system units, no QML errors, no auto-bindings
>> verified Hematita deployed to /home/toni/.local without rebuilding
artifact: hematita current and verified
installed: OK /home/toni/.local/bin/hematita

$ sha256sum hematita/target/release/hematita ~/.local/bin/hematita
baf5dd651d65c60f85bb7b5b9944c1e01d1e1dec7386b7fb2d3d4f5822373b40  hematita/target/release/hematita
baf5dd651d65c60f85bb7b5b9944c1e01d1e1dec7386b7fb2d3d4f5822373b40  /home/toni/.local/bin/hematita

$ HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 QT_ASSUME_STDERR_HAS_CONSOLE=1 \
  QT_QPA_PLATFORM=offscreen timeout 12 hematita/target/release/hematita
qml: hematita-shape cpu 3 60
qml: hematita-sensors 9 44
qml: hematita-services 187 true true
(no TypeError, ReferenceError, Unable to assign or Cannot read property)
```

The first `complete-production.sh` run stopped in verification on a test
module that did not import `NO_RATE`; the import was added, the binary's
tests were run once with `cargo test --release`, and the second run is the
one recorded above.

## Limits

- No agent looked at any screen: the offscreen walk proves the pages
  construct and publish, not that the strip is centred, the colours read or
  the card's corners are clear. The author re-validates in `VAL-VIS-1`.
- The label dictionary covers the labels this machine's drivers publish;
  another machine's unknown labels show raw, by design.
- The second-instance case was not reproduced here; only the silent failure
  was made audible.
