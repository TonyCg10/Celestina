<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Hematita H4 — Sensors, and the table's loose ends

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Hematita its Sensors page — every chip hwmon exposes, every temperature, fan, voltage, power and current channel, with the session's minimum and maximum and the hardware's own limits — and close the process-table items the H3 final review parked.

**Architecture:** `hematita-core` gains a `sensors` module that turns one chip's listing of file names and contents into typed channels with units converted (m°C → °C, mV → V, µW → W, mA → A, rpm as is). The sampler enumerates `/sys/class/hwmon` once for the static facts (chip name, labels, limits), reads the `_input`/`_average` files every tick, and ships a `SensorSnapshot` in the shared `Snapshot`. A new `HematitaSensors` hub publishes index-aligned chip and channel lists with a `revision`, owns the session minimum and maximum per channel, and derives a thermal `load` from the channel's own critical limit through the thresholds in `publish.rs`. `SensorsPage.qml` is a scrolling column of `ListSection` cards, one per chip, whose Spanish names and channel words are composed in QML from tokens. The parked table items are fixed in the same unit as the page, because they share its build.

**Tech Stack:** Rust 1.97.1, cxx-qt 0.9.1, Qt 6.9+ QML, `celestina-style` symlinks (`ListSection.qml` joins the set). No new dependencies.

**Spec:** [docs/superpowers/specs/2026-09-21-hematita-design.md](../specs/2026-09-21-hematita-design.md) §3 (sensors source), §4 (`sensors` module), §5 (`sensors.rs`), §6 (`SensorsPage`), §7 (H4 row). Carry-overs: the H2/H3 plans' list shape and the H1 plan's inventory generator and commit procedure.

## Global Constraints

- **The Celestina shell is in standby.** Never read, reuse, modify or reference `celestina/` or `celestina-rs/crates/celestina-shell-core`.
- **Language contract:** identifiers, comments, docs, tests, script messages, commit subjects in English; product copy only as `qsTr()` literals in QML; no Rust file carries a user-visible string. Channel labels the kernel provides (`Tctl`, `edge`, `PPT`, `vddgfx`, `+3.3V`) are data and are shown raw.
- **Build budget (author's rule):** two application build cycles in H4 — one at the end of H4-B (release build + release-profile tests/clippy + `verify-production.sh`), one inside `complete-production.sh` at H4-Z. H4-A is crate-only. Never `cargo` under `hematita/` outside those moments; never `cargo clean`; never a window on the live or nested session (`VAL-H4`).
- **Commits:** per unit after review; `hematita:` prefix; inventories through the H1 plan's generator (scratchpad `mkinv.py`); evidence under `hematita/docs/evidence/`, inventories under `hematita/docs/inventories/2026-09-22-h4-sensors/`; hooks never bypassed; subjects start with an accepted imperative; explicit pathspecs at commit time (another session may have staged files).
- **Rust invariants:** no `unsafe`; no production `unwrap`/`expect`/`panic!`; typed errors; blocking IO never on the Qt thread; the sampler hub as delivered by H3 (`subscribe`/`stop`); snapshots applied whole, stale generations dropped.
- **QML invariants:** every new file and symlink in `build.rs` `QML_FILES`; tokens only; `required property`; no tooltips; keyboard/AT reachability; `reducedMotion` honoured; no `Canvas`; no lint suppressions; the `hematita` qmllint row is `0` and may not rise.
- **Constants once:** `INTERVAL`, `PROCESS_TICKS` in `sampler.rs`; thresholds in `publish.rs` (a thermal load reuses `ELEVATED_PERCENT`/`CRITICAL_PERCENT` as a fraction of the critical limit); `HISTORY_SAMPLES` in `history.rs`.
- **Sensor contracts (index-aligned lists on `HematitaSensors`):** chips — `chipKeys` (the `hwmonN` directory name), `chipNames` (the kernel `name` file, data), `chipCounts`; channels — `channelChips` (chip index), `channelKinds` (`temperature|fan|voltage|power|current`), `channelIndices` (the kernel channel number), `channelLabels` (kernel label or `""`), `channelValues`, `channelMins`, `channelMaxs` (session), `channelLimitMax`, `channelLimitCrit` (hardware, `0` when absent), `channelLoads` (`normal|elevated|critical`), `channelStates` (`ready|unavailable`). Units per kind are fixed: °C, rpm, V, W, A. State: `revision`, `available`, `reasonKind`, `reasonPath`, `startFailed`.

---

## File structure

| Path | Responsibility |
|---|---|
| `celestina-rs/crates/hematita-core/src/sensors.rs` | `ChipListing` → `Chip { name, channels }`; unit conversion; limit parsing |
| `celestina-rs/crates/hematita-core/tests/fixtures/hwmon-*.txt` | captures of three chips' listings |
| `hematita/src/sampler.rs` | hwmon enumeration, per-chip static facts cache, per-tick values, `SensorSnapshot` |
| `hematita/src/publish.rs` | `thermal_load(value, crit) -> &'static str` |
| `hematita/src/sensors.rs` | `HematitaSensors` hub: lists + revision, session min/max |
| `hematita/qml/components/SensorsPage.qml`, `SensorChipCard.qml`, `SensorRow.qml` | the page, one card per chip, one row per channel |
| `hematita/qml/ListSection.qml` | new symlink into `celestina-style` |
| `hematita/qml/components/ProcessTable.qml`, `ProcessHeader.qml`, `hematita/src/sampler.rs`, `celestina-rs/crates/hematita-core/tests/…` | the parked H3 items |
| `hematita/{ROADMAP,STATUS,VALIDATION}.md`, `hematita/docs/plans/active/2026-09-22-h4-sensors.md` | H4 documents |

Ledger units:

| Unit | Kind | Content | Build |
|---|---|---|---|
| H4-A | `hematita-maintenance` | crate `sensors` with captures; the reverse-DNS cgroup regression test; H4 opened | none |
| H4-B | `hematita-maintenance` | sampler sensor section, `thermal_load`, `HematitaSensors`, the page and its two components, `Main.qml`; the parked table fixes | one |
| H4-Z | `hematita-milestone` | 0.5.0, `complete-production.sh`, documents closed, plan archived | one |

---

### Task 1: Open H4 in the documents

Same shape as H3's Task 1 with these values: plan `hematita/docs/plans/active/2026-09-22-h4-sensors.md`, Plan ID `h4-sensors`, checkpoint `H4`, `VAL-H4`; hypothesis "Every channel hwmon exposes can be read as a typed value with its unit, its kernel limits and the session's extremes, from files alone, and shown per chip without the monitor's idle cost growing"; tangible outcome "the installed 0.5.0 has a Sensors page listing every chip and channel of the author's machine, and the process table keeps its success message, its column alignment and a debounced search"; scope H4-A/H4-B/H4-Z as the table above; exclusions "alerts, fan control, history graphs per channel (a later phase if wanted), non-hwmon sources". Roadmap: `Status: active`, checkpoint `H4`, rows H4-A (dep H3-E), H4-B (H4-A), H4-Z (H4-B), a `## H4 — opened 2026-09-22` section replacing the "next checkpoint is H4" sentence. STATUS: active phase H4. VALIDATION:

```markdown
## VAL-H4 — Sensors on the real session

- **Status:** pending
- **Related implementation:** H4
- **Requires:** the deployed Hematita 0.5.0 on the real session; a GPU load
  and a compile to heat things up
- **Procedure:** open Sensores; count the chip cards against
  `ls /sys/class/hwmon/*/name`; read the processor's Tctl, the GPU's edge and
  junction, the NVMe composites, the six board fans and the three labelled
  board voltages; run a GPU load for a minute and watch the GPU temperature,
  fan and power rows move and their session maximum rise; hover nothing (no
  tooltips exist); walk the cards by keyboard and screen reader; on Procesos,
  terminate a `sleep` and confirm the confirmation sentence stays until you
  select another row; type a name quickly and confirm the table does not
  stutter; check Hematita's idle CPU on Sensores
- **Pass condition:** every chip in `/sys/class/hwmon` has a card and every
  `_input` channel a row with a plausible value and unit; labelled channels
  show the kernel's label, unlabelled ones a numbered word; limits appear
  where the kernel has them; session minimum and maximum move; the GPU rows
  change under load; every row is reachable and named; the terminate
  sentence persists; typing is smooth; idle CPU under 2 %
- **Result:** not run
- **Evidence:** none
```

Guard: `bash scripts/check-documentation-contract.sh` → OK. Lands inside the H4-A commit.

---

### Task 2: `hematita-core::sensors`

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/sensors.rs`
- Modify: `lib.rs` (`pub mod sensors;`)

**Interfaces:**
- `pub struct ChipListing { pub key: String, pub name: String, pub files: Vec<(String, String)> }` — the file names of one `hwmonN` directory with their contents (the caller reads only the files it recognises).
- `pub enum ChannelKind { Temperature, Fan, Voltage, Power, Current }` with `as_str()` (`temperature|fan|voltage|power|current`), `unit()` (`°C|rpm|V|W|A`), `from_prefix(&str) -> Option<Self>` (`temp|fan|in|power|curr`).
- `pub struct Channel { pub kind: ChannelKind, pub index: u32, pub label: String, pub value: f64, pub limit_max: Option<f64>, pub limit_crit: Option<f64> }`.
- `pub struct Chip { pub key: String, pub name: String, pub channels: Vec<Channel> }`.
- `pub fn parse_channel_file(name: &str) -> Option<(ChannelKind, u32, &str)>` — `temp3_crit` → `(Temperature, 3, "crit")`.
- `pub fn convert(kind: ChannelKind, raw: i64) -> f64` — m°C/1000, rpm, mV/1000, µW/1_000_000, mA/1000.
- `pub fn discover(listing: &ChipListing) -> Chip` — one channel per `<kind><n>_input` (power: `_average` when present, else `_input`), sorted by kind then index; a channel whose value file is not an integer is skipped; labels and limits attached when present.

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-rs/crates/hematita-core/src/sensors.rs
//! Sensors, as hwmon lays them out: one directory per chip, one file per
//! channel attribute, integers in the kernel's fixed units. The caller reads
//! the files; this module says which ones matter, what they mean and what a
//! person's unit of each is.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChipListing {
    /// The directory name, `hwmonN`: stable for the session, not across boots.
    pub key: String,
    /// The kernel driver's name for the chip, from its `name` file.
    pub name: String,
    /// `(file name, contents)` for every file the caller could read.
    pub files: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelKind {
    Temperature,
    Fan,
    Voltage,
    Power,
    Current,
}

impl ChannelKind {
    #[must_use]
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "temp" => Some(Self::Temperature),
            "fan" => Some(Self::Fan),
            "in" => Some(Self::Voltage),
            "power" => Some(Self::Power),
            "curr" => Some(Self::Current),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Temperature => "temperature",
            Self::Fan => "fan",
            Self::Voltage => "voltage",
            Self::Power => "power",
            Self::Current => "current",
        }
    }

    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            Self::Temperature => "°C",
            Self::Fan => "rpm",
            Self::Voltage => "V",
            Self::Power => "W",
            Self::Current => "A",
        }
    }
}

impl fmt::Display for ChannelKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub kind: ChannelKind,
    pub index: u32,
    pub label: String,
    pub value: f64,
    pub limit_max: Option<f64>,
    pub limit_crit: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub key: String,
    pub name: String,
    pub channels: Vec<Channel>,
}

/// `temp3_crit` → `(Temperature, 3, "crit")`; anything else `None`.
#[must_use]
pub fn parse_channel_file(name: &str) -> Option<(ChannelKind, u32, &str)> {
    todo!()
}

/// The kernel's integer in a person's unit.
#[must_use]
pub fn convert(kind: ChannelKind, raw: i64) -> f64 {
    todo!()
}

/// One channel per readable `<kind><n>_input` (a power channel prefers
/// `_average`), with its label and limits when the chip has them, sorted by
/// kind then index. A value that is not an integer skips its channel: a chip
/// with one broken file still shows its other readings.
#[must_use]
pub fn discover(listing: &ChipListing) -> Chip {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(files: &[(&str, &str)]) -> ChipListing {
        ChipListing {
            key: "hwmon3".to_owned(),
            name: "amdgpu".to_owned(),
            files: files.iter().map(|(n, c)| ((*n).to_owned(), (*c).to_owned())).collect(),
        }
    }

    #[test]
    fn channel_files_name_their_kind_index_and_attribute() {
        assert_eq!(parse_channel_file("temp3_crit"), Some((ChannelKind::Temperature, 3, "crit")));
        assert_eq!(parse_channel_file("in0_label"), Some((ChannelKind::Voltage, 0, "label")));
        assert_eq!(parse_channel_file("power1_average"), Some((ChannelKind::Power, 1, "average")));
        assert_eq!(parse_channel_file("curr2_input"), Some((ChannelKind::Current, 2, "input")));
        assert_eq!(parse_channel_file("fan1_input"), Some((ChannelKind::Fan, 1, "input")));
        assert_eq!(parse_channel_file("name"), None);
        assert_eq!(parse_channel_file("freq1_input"), None);
        assert_eq!(parse_channel_file("temp_input"), None);
        assert_eq!(parse_channel_file("tempx_input"), None);
    }

    #[test]
    fn units_are_converted_from_the_kernel_integers() {
        assert_eq!(convert(ChannelKind::Temperature, 45_750), 45.75);
        assert_eq!(convert(ChannelKind::Fan, 2777), 2777.0);
        assert_eq!(convert(ChannelKind::Voltage, 1308), 1.308);
        assert_eq!(convert(ChannelKind::Power, 48_000_000), 48.0);
        assert_eq!(convert(ChannelKind::Current, 1500), 1.5);
        assert_eq!(convert(ChannelKind::Temperature, -5_000), -5.0);
    }

    #[test]
    fn discovery_reads_inputs_with_labels_and_limits_in_kind_then_index_order() {
        let chip = discover(&listing(&[
            ("temp2_input", "60000\n"),
            ("temp2_label", "junction\n"),
            ("temp2_crit", "110000\n"),
            ("temp1_input", "45000\n"),
            ("temp1_label", "edge\n"),
            ("temp1_max", "100000\n"),
            ("fan1_input", "1200\n"),
            ("in0_input", "800\n"),
            ("in0_label", "vddgfx\n"),
            ("power1_average", "48000000\n"),
            ("power1_input", "999\n"),
            ("power1_label", "PPT\n"),
            ("power1_cap", "300000000\n"),
        ]));
        assert_eq!(chip.key, "hwmon3");
        assert_eq!(chip.name, "amdgpu");
        let kinds: Vec<(ChannelKind, u32)> = chip.channels.iter().map(|c| (c.kind, c.index)).collect();
        assert_eq!(
            kinds,
            vec![
                (ChannelKind::Temperature, 1),
                (ChannelKind::Temperature, 2),
                (ChannelKind::Fan, 1),
                (ChannelKind::Voltage, 0),
                (ChannelKind::Power, 1),
            ]
        );
        let edge = &chip.channels[0];
        assert_eq!(edge.label, "edge");
        assert_eq!(edge.value, 45.0);
        assert_eq!(edge.limit_max, Some(100.0));
        assert_eq!(edge.limit_crit, None);
        let junction = &chip.channels[1];
        assert_eq!(junction.limit_crit, Some(110.0));
        let power = &chip.channels[4];
        assert_eq!(power.value, 48.0, "average wins over input for power");
        assert_eq!(power.label, "PPT");
        assert_eq!(power.limit_max, Some(300.0), "the power cap is the power channel's max");
        assert_eq!(chip.channels[2].label, "");
    }

    #[test]
    fn a_broken_value_skips_its_channel_and_nothing_else() {
        let chip = discover(&listing(&[
            ("temp1_input", "hot\n"),
            ("temp2_input", "50000\n"),
            ("temp2_max", "not a number\n"),
        ]));
        assert_eq!(chip.channels.len(), 1);
        assert_eq!(chip.channels[0].index, 2);
        assert_eq!(chip.channels[0].limit_max, None);
    }

    #[test]
    fn a_chip_with_no_channels_is_a_chip_with_no_channels() {
        let chip = discover(&listing(&[("name", "gigabyte_wmi\n")]));
        assert!(chip.channels.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cd celestina-rs && cargo test -p hematita-core sensors` → 4 failures (the enum tests pass already).

- [ ] **Step 3: Implement**

```rust
pub fn parse_channel_file(name: &str) -> Option<(ChannelKind, u32, &str)> {
    let (head, attribute) = name.split_once('_')?;
    let digits_at = head.find(|c: char| c.is_ascii_digit())?;
    let (prefix, digits) = head.split_at(digits_at);
    let kind = ChannelKind::from_prefix(prefix)?;
    let index = digits.parse::<u32>().ok()?;
    (!attribute.is_empty()).then_some((kind, index, attribute))
}

pub fn convert(kind: ChannelKind, raw: i64) -> f64 {
    let raw = raw as f64;
    match kind {
        ChannelKind::Temperature | ChannelKind::Voltage | ChannelKind::Current => raw / 1000.0,
        ChannelKind::Fan => raw,
        ChannelKind::Power => raw / 1_000_000.0,
    }
}

pub fn discover(listing: &ChipListing) -> Chip {
    use std::collections::BTreeMap;
    // (kind, index) → attribute → contents
    let mut table: BTreeMap<(ChannelKind, u32), BTreeMap<&str, &str>> = BTreeMap::new();
    for (name, contents) in &listing.files {
        if let Some((kind, index, attribute)) = parse_channel_file(name) {
            table.entry((kind, index)).or_default().insert(attribute, contents.as_str());
        }
    }
    let number = |text: Option<&&str>| -> Option<i64> { text.and_then(|t| t.trim().parse::<i64>().ok()) };
    let mut channels = Vec::new();
    for ((kind, index), attributes) in table {
        let value_file = if kind == ChannelKind::Power && attributes.contains_key("average") {
            "average"
        } else {
            "input"
        };
        let Some(raw) = number(attributes.get(value_file)) else {
            continue;
        };
        let limit_max = match kind {
            ChannelKind::Power => number(attributes.get("cap")).or_else(|| number(attributes.get("max"))),
            _ => number(attributes.get("max")),
        }
        .map(|raw| convert(kind, raw));
        let limit_crit = number(attributes.get("crit")).map(|raw| convert(kind, raw));
        channels.push(Channel {
            kind,
            index,
            label: attributes.get("label").map_or(String::new(), |l| l.trim().to_owned()),
            value: convert(kind, raw),
            limit_max,
            limit_crit,
        });
    }
    Chip { key: listing.key.clone(), name: listing.name.clone(), channels }
}
```

`raw as f64` is exact for every hwmon integer (they are far below 2^53); the workspace lints do not enable `cast_precision_loss`. `BTreeMap` keyed by `(ChannelKind, u32)` gives the kind-then-index order for free because `ChannelKind` derives `Ord` in declaration order.

- [ ] **Step 4: Run tests, fmt, clippy** — pass, clean.

---

### Task 3: Captures, the cgroup regression test, and the H4-A unit

**Files:**
- Create: `tests/fixtures/hwmon-k10temp.txt`, `hwmon-amdgpu.txt`, `hwmon-it8696.txt`
- Modify: `tests/captures.rs`, `src/process.rs` (one test)

- [ ] **Step 1: Capture three chips as `file<TAB>contents` lines**

```bash
F=celestina-rs/crates/hematita-core/tests/fixtures
for chip in k10temp amdgpu it8696; do
  dir=$(grep -lx "$chip" /sys/class/hwmon/hwmon*/name | head -1 | xargs dirname)
  : > $F/hwmon-$chip.txt
  for f in $dir/name $dir/temp* $dir/fan* $dir/in* $dir/power* $dir/curr*; do
    [ -f "$f" ] || continue
    c=$(tr -d '\n' < "$f" 2>/dev/null) || continue
    printf '%s\t%s\n' "$(basename "$f")" "$c" >> $F/hwmon-$chip.txt
  done
done
wc -l $F/hwmon-*.txt
```

Some `_alarm`/`_beep` files may be unreadable or empty; the loop skips read failures and the parser ignores unknown attributes. Note the counts of `_input` lines per chip (`grep -c '_input' …`) for the tests: expected k10temp 2, amdgpu 5 (temp1–3, fan1, in0) + power1 via `_average` = 6 channels, it8696 6 fans + 10 voltages + 6 temperatures = 22.

- [ ] **Step 2: Capture tests**

```rust
use hematita_core::sensors::{discover, ChannelKind, ChipListing};

fn chip_listing(key: &str, text: &str) -> ChipListing {
    let mut name = String::new();
    let mut files = Vec::new();
    for line in text.lines() {
        let Some((file, contents)) = line.split_once('\t') else { continue };
        if file == "name" {
            name = contents.to_owned();
        }
        files.push((file.to_owned(), contents.to_owned()));
    }
    ChipListing { key: key.to_owned(), name, files }
}

#[test]
fn the_captured_processor_chip_has_tctl_and_tccd() {
    let chip = discover(&chip_listing("hwmon6", include_str!("fixtures/hwmon-k10temp.txt")));
    assert_eq!(chip.name, "k10temp");
    let labels: Vec<&str> = chip.channels.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Tctl", "Tccd1"]);
    assert!(chip.channels.iter().all(|c| c.kind == ChannelKind::Temperature && c.value > 0.0 && c.value < 120.0));
}

#[test]
fn the_captured_gpu_chip_reads_temperatures_fan_voltage_and_power() {
    let chip = discover(&chip_listing("hwmon3", include_str!("fixtures/hwmon-amdgpu.txt")));
    assert_eq!(chip.channels.len(), 6);
    let power = chip.channels.iter().find(|c| c.kind == ChannelKind::Power).expect("a power channel");
    assert_eq!(power.label, "PPT");
    assert!(power.limit_max.is_some(), "the power cap is the max");
    let junction = chip.channels.iter().find(|c| c.label == "junction").expect("junction");
    assert!(junction.limit_crit.is_some());
}

#[test]
fn the_captured_board_chip_has_six_fans_and_ten_voltages() {
    let chip = discover(&chip_listing("hwmon4", include_str!("fixtures/hwmon-it8696.txt")));
    assert_eq!(chip.channels.iter().filter(|c| c.kind == ChannelKind::Fan).count(), 6);
    assert_eq!(chip.channels.iter().filter(|c| c.kind == ChannelKind::Voltage).count(), 10);
    assert_eq!(chip.channels.iter().filter(|c| c.kind == ChannelKind::Temperature).count(), 6);
    let labelled: Vec<&str> = chip.channels.iter().filter(|c| !c.label.is_empty()).map(|c| c.label.as_str()).collect();
    assert_eq!(labelled, vec!["3VSB", "Vbat", "+3.3V"]);
}
```

Adjust the numeric expectations to the captured counts if this machine's listing differs from the enumeration above; say so in the evidence.

- [ ] **Step 3: The cgroup regression test** (deferred from H3) — in `process.rs` tests add:

```rust
    #[test]
    fn a_reverse_dns_scope_without_an_instance_number_keeps_its_whole_id() {
        let prefix = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/";
        assert_eq!(
            parse_cgroup(&format!("{prefix}app-org.example.Tool.scope\n")).map(|s| s.desktop_id),
            Some("org.example.Tool".to_owned())
        );
        assert_eq!(
            parse_cgroup(&format!("{prefix}app-gnome-org.example.Tool-77.scope\n")).map(|s| s.desktop_id),
            Some("org.example.Tool".to_owned())
        );
    }
```

- [ ] **Step 4: Whole crate green; close H4-A**

`cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings`. Evidence `2026-09-22-h4-core.md`; ledger/roadmap/STATUS; inventory `H4-A.numstat.tsv` (lib.rs, sensors.rs, process.rs, captures.rs, three fixtures, plan, README, ROADMAP, STATUS, VALIDATION, evidence); guards; commit `hematita-maintenance: Add the hwmon sensor discovery with captures and the cgroup regression test`.

---

### Task 4: The sampler reads sensors; `thermal_load`

**Files:**
- Modify: `hematita/src/sampler.rs`, `hematita/src/publish.rs`

**Interfaces:**
- `pub struct SensorSnapshot { pub chips: Vec<hematita_core::sensors::Chip> }`; `Snapshot.sensors: Section<SensorSnapshot>`.
- `publish::thermal_load(value: f64, crit: Option<f64>) -> &'static str` — `critical` when `value >= crit`, `elevated` when `value >= crit × ELEVATED_PERCENT / 100`, else `normal`; `normal` without a crit.

- [ ] **Step 1: Enumeration and per-tick reads**

```rust
use hematita_core::sensors::{self, Chip, ChipListing};

const HWMON_ROOT: &str = "/sys/class/hwmon";

#[derive(Clone, Debug)]
pub struct SensorSnapshot {
    pub chips: Vec<Chip>,
}

/// What a chip directory holds that does not change: its name, which files
/// exist, its labels and limits. Read once per directory; only the `_input`
/// and `_average` files are read again each tick.
struct ChipFacts {
    name: String,
    /// Every recognised channel file except the value files.
    statics: Vec<(String, String)>,
    /// The value files, read each tick.
    value_files: Vec<String>,
}

fn chip_facts(dir: &Path) -> Option<ChipFacts> {
    let name = read(&dir.join("name")).ok()?.trim().to_owned();
    let mut statics = Vec::new();
    let mut value_files = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.filter_map(Result::ok) {
        let Ok(file) = entry.file_name().into_string() else { continue };
        let Some((_, _, attribute)) = sensors::parse_channel_file(&file) else { continue };
        match attribute {
            "input" | "average" => value_files.push(file),
            "label" | "max" | "crit" | "cap" => {
                if let Ok(contents) = read(&entry.path()) {
                    statics.push((file, contents));
                }
            }
            _ => {}
        }
    }
    value_files.sort();
    Some(ChipFacts { name, statics, value_files })
}

fn sample_sensors(facts: &mut HashMap<String, ChipFacts>) -> Section<SensorSnapshot> {
    let root = Path::new(HWMON_ROOT);
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => {
            return Section::Unavailable(Reason { kind: ReasonKind::Unreadable, path: HWMON_ROOT.to_owned() })
        }
    };
    let mut keys: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok().map(|key| (key, entry.path())))
        .filter(|(key, _)| key.starts_with("hwmon"))
        .collect();
    keys.sort();
    facts.retain(|key, _| keys.iter().any(|(k, _)| k == key));
    let mut chips = Vec::new();
    for (key, dir) in keys {
        let Some(chip_facts) = (match facts.get(&key) {
            Some(existing) => Some(existing),
            None => chip_facts(&dir).and_then(|f| {
                facts.insert(key.clone(), f);
                facts.get(&key)
            }),
        }) else {
            continue;
        };
        let mut files = chip_facts.statics.clone();
        for value_file in &chip_facts.value_files {
            if let Ok(contents) = read(&dir.join(value_file)) {
                files.push((value_file.clone(), contents));
            }
        }
        chips.push(sensors::discover(&ChipListing { key: key.clone(), name: chip_facts.name.clone(), files }));
    }
    Section::Available(SensorSnapshot { chips })
}
```

Wire `let mut sensor_facts: HashMap<String, ChipFacts> = HashMap::new();` into `run` and `sensors: sample_sensors(&mut sensor_facts)` into every `Snapshot` (sensors are ~60 small reads; every tick is fine — measured in `VAL-H4`). A chip whose value file vanishes keeps its other channels; a chip directory that vanishes drops out of `facts` and the list.

- [ ] **Step 2: `thermal_load` with a test in `publish.rs`**

```rust
/// The load a temperature paints, from the chip's own critical limit: the
/// same two thresholds as a percentage, applied to the fraction of `crit`.
#[must_use]
pub fn thermal_load(value: f64, crit: Option<f64>) -> &'static str {
    let Some(crit) = crit.filter(|c| *c > 0.0) else { return "normal" };
    let percent = (value / crit * 100.0).clamp(0.0, 255.0) as u8;
    load_name(percent)
}
```

Test: `thermal_load(50.0, Some(100.0)) == "normal"`, `thermal_load(85.0, Some(100.0)) == "elevated"`, `thermal_load(100.0, Some(100.0)) == "critical"`, `thermal_load(200.0, None) == "normal"`, `thermal_load(1.0, Some(0.0)) == "normal"`.

---

### Task 5: `HematitaSensors`

**Files:**
- Create: `hematita/src/sensors.rs`; modify `main.rs` (`mod sensors;`), `build.rs` (`.files([...,"src/sensors.rs"])`)

```rust
//! The Sensors page's state, as Qt properties: every chip and channel of the
//! latest snapshot as index-aligned lists, plus the session's extremes per
//! channel, which only this object remembers.

use std::collections::HashMap;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList, QVariant};

use hematita_core::sensors::ChannelKind;

use crate::lists::{doubles, strings};
use crate::publish;
use crate::sampler::{self, Reason, Section, SensorSnapshot, Snapshot};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QStringList, chip_keys)]
        #[qproperty(QStringList, chip_names)]
        #[qproperty(QVariant, chip_counts)]
        #[qproperty(QVariant, channel_chips)]
        #[qproperty(QStringList, channel_kinds)]
        #[qproperty(QVariant, channel_indices)]
        #[qproperty(QStringList, channel_labels)]
        #[qproperty(QVariant, channel_values)]
        #[qproperty(QVariant, channel_mins)]
        #[qproperty(QVariant, channel_maxs)]
        #[qproperty(QVariant, channel_limit_max)]
        #[qproperty(QVariant, channel_limit_crit)]
        #[qproperty(QStringList, channel_loads)]
        #[qproperty(bool, available)]
        #[qproperty(QString, reason_kind)]
        #[qproperty(QString, reason_path)]
        #[qproperty(bool, start_failed)]
        type HematitaSensors = super::HematitaSensorsRust;

        #[qinvokable]
        fn start(self: Pin<&mut HematitaSensors>);
    }

    impl cxx_qt::Threading for HematitaSensors {}
}

pub struct HematitaSensorsRust {
    revision: i32,
    chip_keys: QStringList,
    chip_names: QStringList,
    chip_counts: QVariant,
    channel_chips: QVariant,
    channel_kinds: QStringList,
    channel_indices: QVariant,
    channel_labels: QStringList,
    channel_values: QVariant,
    channel_mins: QVariant,
    channel_maxs: QVariant,
    channel_limit_max: QVariant,
    channel_limit_crit: QVariant,
    channel_loads: QStringList,
    available: bool,
    reason_kind: QString,
    reason_path: QString,
    start_failed: bool,
    started: bool,
    last_generation: u64,
    /// `chipKey/kind/index` → (session min, session max).
    extremes: HashMap<String, (f64, f64)>,
}
```

`Default` as the other hubs (empty lists via `doubles(&[])`, `available: true`). `start()` subscribes with `sampler::subscribe` and queues `apply(generation, snapshot.sensors.clone())`. `apply`:

```rust
    fn apply(mut self: Pin<&mut Self>, generation: u64, section: Section<SensorSnapshot>) {
        if !publish::accepts(generation, self.rust().last_generation) {
            return;
        }
        self.as_mut().rust_mut().last_generation = generation;
        let snapshot = match section {
            Section::Available(snapshot) => {
                self.as_mut().set_available(true);
                self.as_mut().set_reason_kind(QString::default());
                self.as_mut().set_reason_path(QString::default());
                snapshot
            }
            Section::Unavailable(Reason { kind, path }) => {
                self.as_mut().set_available(false);
                self.as_mut().set_reason_kind(QString::from(kind.as_str()));
                self.as_mut().set_reason_path(QString::from(path.as_str()));
                return;
            }
        };
        let mut chip_keys = Vec::new();
        let mut chip_names = Vec::new();
        let mut chip_counts = Vec::new();
        let mut chips = Vec::new();
        let mut kinds = Vec::new();
        let mut indices = Vec::new();
        let mut labels = Vec::new();
        let mut values = Vec::new();
        let mut mins = Vec::new();
        let mut maxs = Vec::new();
        let mut limit_max = Vec::new();
        let mut limit_crit = Vec::new();
        let mut loads = Vec::new();
        let mut seen = Vec::new();
        for (chip_index, chip) in snapshot.chips.iter().enumerate() {
            chip_keys.push(chip.key.clone());
            chip_names.push(chip.name.clone());
            chip_counts.push(chip.channels.len() as f64);
            for channel in &chip.channels {
                let key = format!("{}/{}/{}", chip.key, channel.kind.as_str(), channel.index);
                let entry = self.as_mut().rust_mut().extremes.entry(key.clone()).or_insert((channel.value, channel.value));
                entry.0 = entry.0.min(channel.value);
                entry.1 = entry.1.max(channel.value);
                let (min, max) = *entry;
                seen.push(key);
                chips.push(chip_index as f64);
                kinds.push(channel.kind.as_str().to_owned());
                indices.push(f64::from(channel.index));
                labels.push(channel.label.clone());
                values.push(channel.value);
                mins.push(min);
                maxs.push(max);
                limit_max.push(channel.limit_max.unwrap_or(0.0));
                limit_crit.push(channel.limit_crit.unwrap_or(0.0));
                loads.push(match channel.kind {
                    ChannelKind::Temperature => publish::thermal_load(channel.value, channel.limit_crit),
                    _ => "normal",
                }.to_owned());
            }
        }
        self.as_mut().rust_mut().extremes.retain(|key, _| seen.contains(key));
        self.as_mut().set_chip_keys(strings(chip_keys));
        self.as_mut().set_chip_names(strings(chip_names));
        self.as_mut().set_chip_counts(doubles(&chip_counts));
        self.as_mut().set_channel_chips(doubles(&chips));
        self.as_mut().set_channel_kinds(strings(kinds));
        self.as_mut().set_channel_indices(doubles(&indices));
        self.as_mut().set_channel_labels(strings(labels));
        self.as_mut().set_channel_values(doubles(&values));
        self.as_mut().set_channel_mins(doubles(&mins));
        self.as_mut().set_channel_maxs(doubles(&maxs));
        self.as_mut().set_channel_limit_max(doubles(&limit_max));
        self.as_mut().set_channel_limit_crit(doubles(&limit_crit));
        self.as_mut().set_channel_loads(strings(loads));
        let ticket = publish::ticket(generation);
        self.as_mut().set_revision(ticket);
    }
```

If the borrow of `self.as_mut().rust_mut().extremes` inside the loop fights the later `self.as_mut()` calls, collect the per-channel rows into locals first and update `extremes` in a separate pass over `snapshot.chips` before building the lists. A unit test on a pure helper `fn fold_extremes(previous: Option<(f64, f64)>, value: f64) -> (f64, f64)` is enough for the min/max rule.

---

### Task 6: The page, the parked table fixes, the one build, H4-B

**Files:**
- Create: `hematita/qml/ListSection.qml` (symlink), `hematita/qml/components/SensorsPage.qml`, `SensorChipCard.qml`, `SensorRow.qml`
- Modify: `hematita/qml/Main.qml`, `build.rs`, `ProcessTable.qml`, `ProcessHeader.qml`, `hematita/src/sampler.rs`

- [ ] **Step 1: `SensorRow.qml`**

```qml
import QtQuick
import org.celestina.hematita 1.0

// One channel: the word for it, its value with the unit, the session's
// extremes, and the kernel's limit when it has one. Content family: a row in
// a grouped card, hover only, no selection — there is nothing to act on.
Item {
    id: row

    required property string label
    required property string valueText
    required property string extremesText
    required property string limitText
    required property string load

    implicitHeight: CelestinaTheme.rowHeight
    activeFocusOnTab: true

    Accessible.role: Accessible.ListItem
    Accessible.name: row.label + ", " + row.valueText
                     + (row.limitText.length > 0 ? ", " + row.limitText : "")

    CelestinaFocusRing {
        target: row
        cornerRadius: CelestinaTheme.radiusSm
        shown: row.activeFocus
    }

    Row {
        anchors.fill: parent
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.rightMargin: CelestinaTheme.spaceLg
        spacing: CelestinaTheme.spaceMd

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - value.width - parent.spacing
            Text {
                text: row.label
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                elide: Text.ElideRight
                width: parent.width
            }
            Text {
                text: row.extremesText + (row.limitText.length > 0 ? "   " + row.limitText : "")
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.features: CelestinaTheme.fontFeaturesTabular
                elide: Text.ElideRight
                width: parent.width
            }
        }

        Text {
            id: value
            anchors.verticalCenter: parent.verticalCenter
            text: row.valueText
            color: row.load === "critical" ? CelestinaTheme.danger
                 : row.load === "elevated" ? CelestinaTheme.warning
                 : CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            font.features: CelestinaTheme.fontFeaturesTabular
        }
    }
}
```

- [ ] **Step 2: `SensorChipCard.qml`**

```qml
pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.hematita 1.0

// One chip as a grouped card: the suite's list section with its uppercase
// eyebrow naming the chip, and one row per channel.
ListSection {
    id: card

    required property string title
    // { label, valueText, extremesText, limitText, load } per channel.
    required property var rows

    title: card.title

    Repeater {
        model: card.rows

        SensorRow {
            required property var modelData
            width: card.width
            label: modelData.label
            valueText: modelData.valueText
            extremesText: modelData.extremesText
            limitText: modelData.limitText
            load: modelData.load
        }
    }
}
```

`ListSection`'s default property is `rows` (its inner column's children) — the `Repeater` lands there. The `title` property collides with `ListSection.title`: keep `ListSection`'s and drop the redeclaration (`required property string title` is illegal on an inherited property; use `required title` syntax is not available for inherited properties either — so the card exposes `required property string chipTitle` and binds `title: card.chipTitle`).

- [ ] **Step 3: `SensorsPage.qml`**

```qml
pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.hematita 1.0

// Sensores: every chip as a card, every channel as a row, in a scrolling
// column. Every word is composed here from tokens; the kernel's labels are
// shown as they are.
Item {
    id: page

    required property HematitaSensors sensors

    property var cards: []

    function chipTitle(name, ordinal) {
        let base
        switch (name) {
        case "k10temp": base = qsTr("Procesador"); break
        case "amdgpu": base = qsTr("Gráfica"); break
        case "nvme": base = qsTr("Disco NVMe"); break
        case "it8696": base = qsTr("Placa base"); break
        case "gigabyte_wmi": base = qsTr("Placa base (WMI)"); break
        case "acpitz": base = qsTr("ACPI"); break
        case "ath12k_hwmon": base = qsTr("Wi-Fi"); break
        default: base = name
        }
        const suffix = ordinal > 1 ? " " + ordinal : ""
        return name === base ? base : base + suffix + " · " + name
    }

    function kindWord(kind, index) {
        switch (kind) {
        case "temperature": return qsTr("Temperatura %1").arg(index)
        case "fan": return qsTr("Ventilador %1").arg(index)
        case "voltage": return qsTr("Tensión %1").arg(index)
        case "power": return qsTr("Potencia %1").arg(index)
        case "current": return qsTr("Corriente %1").arg(index)
        }
        return kind + " " + index
    }

    function unit(kind) {
        switch (kind) {
        case "temperature": return "°C"
        case "fan": return "rpm"
        case "voltage": return "V"
        case "power": return "W"
        case "current": return "A"
        }
        return ""
    }

    function number(kind, value) {
        const digits = kind === "fan" ? 0 : kind === "voltage" ? 3 : 1
        return value.toLocaleString(Qt.locale(), "f", digits)
    }

    function weave() {
        const s = page.sensors
        const chipCount = Math.min(s.chipKeys.length, s.chipNames.length, s.chipCounts.length)
        const channelCount = Math.min(s.channelChips.length, s.channelKinds.length, s.channelIndices.length,
                                      s.channelLabels.length, s.channelValues.length, s.channelMins.length,
                                      s.channelMaxs.length, s.channelLimitMax.length, s.channelLimitCrit.length,
                                      s.channelLoads.length)
        const ordinals = {}
        const woven = []
        for (let c = 0; c < chipCount; ++c) {
            const name = s.chipNames[c]
            ordinals[name] = (ordinals[name] || 0) + 1
            woven.push({ key: s.chipKeys[c], title: page.chipTitle(name, ordinals[name]), rows: [] })
        }
        for (let i = 0; i < channelCount; ++i) {
            const chip = s.channelChips[i]
            if (chip < 0 || chip >= woven.length)
                continue
            const kind = s.channelKinds[i]
            const u = page.unit(kind)
            const limit = s.channelLimitCrit[i] > 0
                          ? qsTr("crítico %1 %2").arg(page.number(kind, s.channelLimitCrit[i])).arg(u)
                          : s.channelLimitMax[i] > 0
                            ? qsTr("máx. %1 %2").arg(page.number(kind, s.channelLimitMax[i])).arg(u)
                            : ""
            woven[chip].rows.push({
                label: s.channelLabels[i].length > 0 ? s.channelLabels[i] : page.kindWord(kind, s.channelIndices[i]),
                valueText: page.number(kind, s.channelValues[i]) + " " + u,
                extremesText: qsTr("mín. %1 · máx. %2").arg(page.number(kind, s.channelMins[i])).arg(page.number(kind, s.channelMaxs[i])),
                limitText: limit,
                load: s.channelLoads[i]
            })
        }
        page.cards = woven
    }

    Connections {
        target: page.sensors
        function onRevisionChanged() { if (page.visible) page.weave() }
    }
    onVisibleChanged: if (visible) page.weave()
    Component.onCompleted: page.weave()

    Flickable {
        id: flick
        anchors.fill: parent
        anchors.rightMargin: CelestinaTheme.spaceLg
        contentWidth: width
        contentHeight: column.implicitHeight
        clip: true
        Accessible.role: Accessible.List
        Accessible.name: qsTr("Sensores")

        Column {
            id: column
            width: flick.width
            spacing: CelestinaTheme.spaceLg

            Repeater {
                model: page.cards.length

                SensorChipCard {
                    required property int index
                    readonly property var card: index < page.cards.length ? page.cards[index] : { title: "", rows: [] }
                    width: column.width
                    chipTitle: card.title
                    rows: card.rows
                }
            }

            Text {
                visible: !page.sensors.available
                text: qsTr("No se pudo leer %1").arg(page.sensors.reasonPath)
                color: CelestinaTheme.danger
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
            }
        }
    }

    CelestinaScrollBar {
        surface: flick
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        anchors.right: parent.right
    }
}
```

A `Repeater` with an integer model and index reads keeps the cards stable across ticks (the same reasoning as the tables); rows inside a card are rebuilt per tick — ~60 rows, acceptable, and the page only weaves while visible.

- [ ] **Step 4: `Main.qml`** — `HematitaSensors { id: sensorHub }`, replace the last placeholder with `SensorsPage { sensors: sensorHub }`, `sensorHub.start()` in `Component.onCompleted`; update the sections comment.

- [ ] **Step 5: The parked table items**

- `ProcessTable.qml`: in `onSelectedPidChanged`, call `clearAction()` only when `!table.anchoring` (a selection released by the re-anchor is not the person moving on), so a successful terminate keeps its sentence until the person selects another row.
- `ProcessTable.qml`: the header and the rows share one inset: give `ProcessHeader` `anchors.leftMargin`/`rightMargin: CelestinaTheme.spaceXs` (the surface padding) so cells align, and make `fixedTotal` subtract `2 * CelestinaTheme.spaceXs` from the name column's share.
- `ProcessTable.qml`: `weave()` runs only while `visible` (`Connections` handler gated; `onVisibleChanged: if (visible) weave()`), so the hidden page costs nothing per tick.
- `ProcessTable.qml`: the search field feeds `filterText` through a 150 ms `Timer` (restart on each keystroke; on trigger set `filterText` and `refresh()`).
- `ProcessTable.qml`: `layout()` builds the grouped sequence in one pass: bucket rows by `row.group` into arrays first, then emit group + bucket.
- `sampler.rs`: `subscribe` registers the subscriber inside the `handle` lock (take the `handle` lock first, then push under `subscribers`), and the comment on `stop()` states the guarantee that now holds.

- [ ] **Step 6: `build.rs`** — add `"qml/ListSection.qml"`, `"qml/components/SensorRow.qml"`, `"qml/components/SensorChipCard.qml"`, `"qml/components/SensorsPage.qml"` to `QML_FILES`; `src/sensors.rs` to `.files`. Symlink: `ln -s ../../celestina-style/ListSection.qml hematita/qml/ListSection.qml` (it depends on `CelestinaSectionLabel` and `CelestinaSurface`, both already linked).

- [ ] **Step 7: The one build cycle; close H4-B**

```bash
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets && cargo clippy --release --all-targets --locked -- -D warnings && cargo fmt --all --check)
hematita/scripts/verify-production.sh
HEMATITA_SMOKE_SHAPE=1 QT_QPA_PLATFORM=offscreen timeout 8 hematita/target/release/hematita 2>&1 | grep -E 'hematita-shape|TypeError|ReferenceError|Unable to assign|Cannot read property'
```

Evidence `2026-09-22-h4-sensors-page.md` (commands, counts, the sensor contracts, the parked fixes each named, Limits: no visual check, chip names of this machine only mapped, `VAL-H4`). Ledger/roadmap/STATUS; inventory `H4-B.numstat.tsv`; guards; commit `hematita-maintenance: Add the Sensors page with every hwmon chip and close the process table follow-ups`.

---

### Task 7: Implementation exit — H4-Z

As H3-Z: `python3 scripts/version_tool.py bump hematita milestone --unit H4-Z --summary "Add the Sensors page"`; `complete-production.sh`; hashes; roadmap `idle`/`none` with `## H4 — closed 2026-09-22` and H5 named next (with its written decision as its first step); STATUS `Delivered as 0.5.0: H4`; AGENTS.md unchanged unless a rule changed; plan archived (`Successor: H5`); READMEs; evidence `2026-09-22-h4-production-completion.md` (keyed-digest note); inventory `H4-Z.numstat.tsv` with the deleted active path; guards; commit `hematita-milestone: Add the Sensors page`.

---

## Self-review

**Spec coverage (H4):** §3 hwmon source with units → Task 2; §4 `sensors::discover(ChipListing) → Chip` with `Channel { kind, label, value }` and unit conversion → Task 2 (plus limits, which the spec's page needs for "minimum and maximum" context); §5 `sensors.rs` with chip, kind, label, value, unit, session min and max → Tasks 4–5 (lists + revision instead of `QAbstractListModel`, as H2/H3); §6 `SensorsPage` as `ListSection` cards per chip with value, min and max per row → Task 6; §7 H4 row → all. The H3 parked items → Task 6 Step 5 and Task 3 Step 3.

**Placeholder scan:** capture counts are confirmed at execution; no TBD.

**Type consistency:** `ChipListing`/`Chip`/`Channel`/`ChannelKind::{as_str,unit}`/`parse_channel_file`/`discover` match between Tasks 2, 4 and 5; `SensorSnapshot`/`Snapshot.sensors` between Tasks 4 and 5; `publish::thermal_load` between Tasks 4 and 5; the QML property names are the camelCase of Task 5's snake_case and are what `SensorsPage.weave()` reads; `SensorChipCard { chipTitle, rows }` and `SensorRow { label, valueText, extremesText, limitText, load }` match their use.
