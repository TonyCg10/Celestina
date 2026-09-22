<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Hematita H2 — Every resource on the Performance page

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Performance page's fixed CPU/memory pair into a live list of every resource the machine exposes — CPU with its per-core grid, memory and swap, the AMD GPU, each whole disk and each network interface — with per-resource availability and a typed reason when one cannot be read.

**Architecture:** `hematita-core` gains pure parsers for `/proc/diskstats`, `/proc/net/dev` and the `amdgpu` sysfs files plus one generic named-counter rate sampler; the ring learns to normalise a series by its own maximum. The sampler thread enumerates disks and interfaces every second and publishes one whole `Snapshot` whose sections carry a typed `Reason` instead of raw text. `HematitaResources` is rewritten to publish index-aligned lists (keys, kinds, labels, states, reasons, loads, numbers, histories) plus a `revision` ticket — the suite's precedent for list data, since CXX-Qt 0.9 cannot override `QAbstractListModel` — and the page weaves rows from them, composes every user-visible word through `qsTr()`, and keeps the selection by key so a hot-plugged disk never shifts it.

**Tech Stack:** Rust 1.97.1 (pinned), cxx-qt 0.9.1 / cxx-qt-lib 0.9.1, Qt 6.9+ QML with `QtQuick.Shapes`, `celestina-style` symlinks. No new dependencies.

**Spec:** [docs/superpowers/specs/2026-09-21-hematita-design.md](../specs/2026-09-21-hematita-design.md) §3 (sources), §4 (crate modules `disk`, `network`, `gpu`), §5 (adapter), §6 (surface: per-core toggle), §7 (H2 row). The H1 plan's Global Constraints, inventory generator and commit procedure carry over: [2026-09-21-hematita-h1-foundation.md](2026-09-21-hematita-h1-foundation.md).

## Global Constraints

- **The Celestina shell is in standby.** Never read, reuse, modify or reference `celestina/` or `celestina-rs/crates/celestina-shell-core`.
- **Language contract:** identifiers, comments, docs, tests, script messages and commit subjects are English. Product copy is Spanish and appears only as the literal argument of `qsTr()` in QML. After H2 no Rust file carries a user-visible string: `resources.rs` loses its `language-contract: product-copy` marker.
- **Build budget (author's rule):** builds heat and slow the machine. H2 runs exactly two application build cycles: one at the end of H2-B (release build + release-profile tests/clippy + `verify-production.sh`) and one inside `complete-production.sh` at H2-Z. H2-A is crate-only (`cargo test -p hematita-core` is cheap). Never run `cargo` under `hematita/` outside those two moments; never `cargo clean`; never open a window on the live or nested session (offscreen only; appearance is `VAL-H2`).
- **Commits:** authorised per unit once its review passes, with the messages given here; `hematita:` prefix; inventories through the generator at the H1 plan's "Inventory generator" section (write it to the scratchpad as `mkinv.py` if it is not there); hooks are never bypassed. Each unit's evidence goes to `hematita/docs/evidence/`, its inventory to `hematita/docs/inventories/2026-09-22-h2-resources/<UNIT>.numstat.tsv`.
- **Rust invariants:** no `unsafe`, no production `unwrap`/`expect`/`panic!`, typed errors, blocking IO never on the Qt thread, the sampler thread stops deterministically, snapshots applied whole and stale ones dropped.
- **QML invariants:** every file in `build.rs` `QML_FILES`; tokens only from `CelestinaTheme`; `required property`; no tooltips; keyboard and AT reachability; `reducedMotion` honoured; no `Canvas`; no lint-suppression comments; the `hematita` qmllint row is `0` and may not rise.
- **Constants live once:** `INTERVAL` (sampler.rs), `HISTORY_SAMPLES` (history.rs), `ELEVATED_PERCENT`/`CRITICAL_PERCENT` (resources.rs), `SECTOR_BYTES = 512` (disk.rs).
- **Kind contract (both sides read it):** the `resourceNumbers` entry of a row is a list of doubles whose meaning depends on the row's kind:
  - `cpu`: `[percent, frequencyMhz, cores]`
  - `memory`: `[usedKib, totalKib, swapUsedKib, swapTotalKib]`
  - `gpu`: `[busyPercent, memoryBusyPercent, vramUsedBytes, vramTotalBytes, gttUsedBytes, gttTotalBytes, coreMhz, memoryMhz]`
  - `disk`: `[readBytesPerSecond, writeBytesPerSecond, sizeBytes, rotational]`
  - `network`: `[rxBytesPerSecond, txBytesPerSecond, speedMbit, wireless, up]`
  A row whose state is `waiting` (no rate yet) or `unavailable` carries zeros.
- **Reason contract:** `resourceReasonKinds` entries are `""`, `unreadable`, `malformed` or `no-rate`; `resourceReasonPaths` carries the file that failed. QML composes the sentence; Rust never formats prose for the window.

---

## File structure

| Path | Responsibility |
|---|---|
| `celestina-rs/crates/hematita-core/src/rate.rs` | `NamedCounters<N>`: named counter vectors → per-second rates between two readings |
| `celestina-rs/crates/hematita-core/src/disk.rs` | `/proc/diskstats` whole-device parsing; sysfs size/model/flags parsing |
| `celestina-rs/crates/hematita-core/src/network.rs` | `/proc/net/dev` parsing; sysfs link type/operstate/speed parsing |
| `celestina-rs/crates/hematita-core/src/gpu.rs` | `amdgpu` sysfs values → `GpuReading`; `pp_dpm_*` active level |
| `celestina-rs/crates/hematita-core/src/history.rs` | adds `Ring::max()` and `Ring::fractions()` |
| `celestina-rs/crates/hematita-core/tests/fixtures/{proc-diskstats.txt,proc-net-dev.txt}` | captures |
| `celestina-rs/crates/hematita-core/tests/captures.rs` | two more capture tests |
| `hematita/src/sampler.rs` | `Reason`, `Section<T>`, GPU/disk/network sections, topology, elapsed time |
| `hematita/src/publish.rs` | pure decisions the adapter makes (fraction, load, kind numbers, generation acceptance) with tests |
| `hematita/src/resources.rs` | rewritten: index-aligned lists + `revision`; per-core histories; no product copy |
| `hematita/src/main.rs` | environment set before any bus work |
| `hematita/qml/components/PerformancePage.qml` | weaves rows on `revision`; selection by key; failure banner inside the column layout |
| `hematita/qml/components/ResourceRow.qml` | unchanged API; unavailable state |
| `hematita/qml/components/ResourceDetail.qml` | kind-driven facts; per-core grid toggle |
| `hematita/qml/components/CoreGrid.qml` | new: grid of small `HistoryGraph`s, one per core |
| `hematita/qml/components/HistoryGraph.qml` | stroke width token; enter motion under `reducedMotion` |
| `hematita/qml/components/NavItem.qml`, `NavStrip.qml` | drop decorative checked state; `Accessible.name` |
| `hematita/build.rs` | registers `CoreGrid.qml` and `src/publish.rs` |
| `hematita/{ROADMAP,STATUS,VALIDATION,AGENTS}.md`, `hematita/docs/plans/active/2026-09-22-h2-resources.md` | H2 documents |

Ledger units:

| Unit | Kind | Content | Build |
|---|---|---|---|
| H2-A | `hematita-maintenance` | crate: `rate`, `disk`, `network`, `gpu`, ring fractions, captures | none (crate tests) |
| H2-B | `hematita-maintenance` | sampler sections, `publish.rs`, `resources.rs` lists, QML list/detail/grid, failure path, the H1 follow-ups | one (build + release tests + verify) |
| H2-Z | `hematita-milestone` | 0.3.0, `complete-production.sh`, documents closed, plan archived | one (complete) |

The H1 roadmap booked the failure-path follow-ups as `H2-A`; this plan folds them into H2-B because every one of them needs the same build the list rewrite needs, and H2-A becomes the crate unit. Task 1 updates the roadmap accordingly when it opens the checkpoint.

---

### Task 1: Open H2 in the documents

**Files:**
- Create: `hematita/docs/plans/active/2026-09-22-h2-resources.md`
- Modify: `hematita/ROADMAP.md`, `hematita/STATUS.md`, `hematita/VALIDATION.md`, `hematita/docs/plans/active/README.md`

No build. This task lands inside the H2-A commit (its files are in H2-A's inventory), so it has no commit of its own.

- [ ] **Step 1: Write the plan ledger**

```markdown
# H2 — Every resource on the Performance page

- **Opened:** 2026-09-22
- **Plan ID:** h2-resources
- **Status:** active
- **Authorization:** the author asked to open the H2 plan on 2026-09-22
- **Scope:** hematita
- **Implementation checkpoint:** H2
- **Author-validation checkpoint:** `VAL-H2` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

The side list can become a live inventory of the machine — every whole disk,
every interface, the GPU — read from `/proc` and `/sys` once a second by the
same thread, each with its own minute of history and its own honest absence,
without the monitor's idle cost becoming visible.

## Tangible outcome

The installed 0.3.0 shows a Performance list with the processor (and a
per-core grid), memory with swap, the AMD GPU, each whole disk and each
network interface; a resource that cannot be read says so in its own row with
a Spanish sentence composed in QML from a typed reason.

## Scope

- `H2-A` — `hematita-core`: `rate`, `disk`, `network`, `gpu`, ring fractions,
  captures.
- `H2-B` — sampler sections and topology, `publish.rs`, `HematitaResources`
  as index-aligned lists, the page weaving rows, kind-driven detail, the
  per-core grid, per-resource failure state, and the follow-ups H1 booked
  (banner in the layout, `NavItem` checked state, graph token and motion,
  `NavStrip` name, environment before bus work, ring alignment, `generation`).
- `H2-Z` — implementation exit and 0.3.0.

## Exclusions

- Sensors (H4) — the GPU card shows busy, memory and clocks only; its
  temperature and power arrive with hwmon.
- Non-AMD GPUs; per-process anything (H3); history persistence.

## Build order

1. `H2-A`, then `H2-B`, then `H2-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary lists the
processor, memory, GPU, every whole disk and every interface with live
graphs.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H2-A | `hematita:` | planned | `celestina-rs/crates/hematita-core/`, `hematita/ROADMAP.md`, `hematita/STATUS.md`, `hematita/VALIDATION.md`, this plan | — | Named-counter rates, diskstats, net/dev and amdgpu parsers, sysfs helpers, ring fractions, captures; H2 opened in the documents | `cargo test -p hematita-core` | `VAL-H2` |
| H2-B | `hematita:` | planned | `hematita/src/`, `hematita/qml/`, `hematita/build.rs` | — | Snapshot sections with typed reasons and topology; `publish.rs` tested; `HematitaResources` as lists with a revision; the page weaving rows, kind-driven detail, per-core grid, per-row failure state; the H1 follow-ups | `scripts/verify-production.sh` | `VAL-H2` |
| H2-Z | `hematita:` | planned | `hematita/`, `docs/version-history.tsv` | — | Implementation exit, 0.3.0, documents closed, plan archived | `scripts/complete-production.sh` | `VAL-H2` |
```

- [ ] **Step 2: Open the checkpoint in `ROADMAP.md`**

Set `- **Status:** active` and `- **Active implementation checkpoint:** H2`. Add to the build-order table:

```markdown
| H2-A | planned | H1-Z | `hematita-core`: rate, disk, network, gpu, ring fractions, captures | `cargo test -p hematita-core` |
| H2-B | planned | H2-A | sampler sections, `publish.rs`, list-publishing `HematitaResources`, page, per-core grid, failure path and H1 follow-ups | `scripts/verify-production.sh` |
| H2-Z | planned | H2-B | implementation exit and 0.3.0 | `scripts/complete-production.sh` |
```

Replace the `## H2 — planned first unit` section with:

```markdown
## H2 — opened 2026-09-22

The follow-ups H1 booked as its first unit are delivered inside `H2-B`: every
one of them needs the build the list rewrite needs, so they share it. The
units and their exit are in the
[active plan](docs/plans/active/2026-09-22-h2-resources.md).
```

- [ ] **Step 3: Update `STATUS.md` and `VALIDATION.md`**

`STATUS.md`: `Updated: 2026-09-22`, `Next phase` → `Active phase: H2 (every resource on the Performance page), opened 2026-09-22`.

Append to `VALIDATION.md`:

```markdown
## VAL-H2 — Every resource, live, on the real session

- **Status:** pending
- **Related implementation:** H2
- **Requires:** the deployed Hematita 0.3.0 on the real session; a USB disk to
  plug in; the Wi-Fi interface up
- **Procedure:** launch `hematita`; read the side list; copy a large file
  between two disks and watch both rows; download something and watch the
  interface; run a GPU load and watch the GPU row; open the processor and
  toggle the per-core grid; plug a USB disk in and out; make `/proc/diskstats`
  unreadable is not possible, so instead unplug the Wi-Fi and confirm its row
  says it is down rather than vanishing; watch Hematita's own CPU while idle
- **Pass condition:** every whole disk and interface appears with its model or
  name; rates match another tool within a few percent; the selected row stays
  selected across a hot-plug; the grid shows one graph per core; an unreadable
  or absent source is a Spanish sentence in its own row, not a frozen number;
  Hematita idles under 1 % CPU
- **Result:** not run
- **Evidence:** none
```

`docs/plans/active/README.md`: the sentence "The active plan is H2 — every resource on the Performance page, which the project roadmap names as its active implementation checkpoint.", with the plan title as a Markdown link to `2026-09-22-h2-resources.md` (the guard resolves links relative to the README, which is why the link is spelled out here rather than written). Keep the inventories paragraph.

- [ ] **Step 4: Run the documentation guard**

Run: `bash scripts/check-documentation-contract.sh`
Expected: `Documentation contract: OK` (the Siderita errata lines are pre-existing).

---

### Task 2: `hematita-core::rate` — named counters to per-second rates

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/rate.rs`
- Modify: `celestina-rs/crates/hematita-core/src/lib.rs` (add `pub mod rate;`)

**Interfaces:**
- Produces: `pub struct NamedCounters<const N: usize>`, `new()`, `sample(&mut self, readings: &[(String, [u64; N])], elapsed: Duration) -> Vec<(String, [f64; N])>`, `reset(&mut self)`.

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-rs/crates/hematita-core/src/rate.rs
//! Rates from counters that only ever grow.
//!
//! Disks and interfaces both report cumulative byte counters per name, so the
//! arithmetic that turns two readings into bytes per second is written once:
//! a name seen for the first time has no rate yet, a name that disappeared is
//! forgotten, and a counter that went backwards (a reset, a re-plug) yields
//! zero rather than a negative number or a wrap.

use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug)]
pub struct NamedCounters<const N: usize> {
    previous: HashMap<String, [u64; N]>,
}

impl<const N: usize> Default for NamedCounters<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> NamedCounters<N> {
    #[must_use]
    pub fn new() -> Self {
        Self { previous: HashMap::new() }
    }

    /// Per-second rates for every name present in both this reading and the
    /// previous one, in the order of `readings`. Names new to this reading
    /// are remembered but not rated; names missing from it are dropped.
    /// A zero `elapsed` rates nothing.
    pub fn sample(
        &mut self,
        readings: &[(String, [u64; N])],
        elapsed: Duration,
    ) -> Vec<(String, [f64; N])> {
        todo!()
    }

    pub fn reset(&mut self) {
        self.previous.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(name: &str, values: [u64; 2]) -> (String, [u64; 2]) {
        (name.to_owned(), values)
    }

    #[test]
    fn the_first_reading_of_a_name_has_no_rate() {
        let mut counters = NamedCounters::<2>::new();
        let rates = counters.sample(&[reading("sda", [1000, 2000])], Duration::from_secs(1));
        assert!(rates.is_empty());
    }

    #[test]
    fn the_second_reading_rates_the_difference_per_second() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [1000, 2000])], Duration::from_secs(1));
        let rates = counters.sample(&[reading("sda", [3000, 2000])], Duration::from_secs(2));
        assert_eq!(rates, vec![("sda".to_owned(), [1000.0, 0.0])]);
    }

    #[test]
    fn a_name_that_appears_later_is_rated_from_its_own_second_reading() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [0, 0])], Duration::from_secs(1));
        let rates = counters.sample(
            &[reading("sda", [10, 0]), reading("sdb", [5, 5])],
            Duration::from_secs(1),
        );
        assert_eq!(rates, vec![("sda".to_owned(), [10.0, 0.0])]);
        let rates = counters.sample(
            &[reading("sda", [20, 0]), reading("sdb", [15, 5])],
            Duration::from_secs(1),
        );
        assert_eq!(rates.len(), 2);
        assert_eq!(rates[1], ("sdb".to_owned(), [10.0, 0.0]));
    }

    #[test]
    fn a_name_that_disappears_is_forgotten() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sdb", [0, 0])], Duration::from_secs(1));
        counters.sample(&[], Duration::from_secs(1));
        let rates = counters.sample(&[reading("sdb", [100, 0])], Duration::from_secs(1));
        assert!(rates.is_empty(), "a re-plugged disk starts over");
    }

    #[test]
    fn a_counter_that_goes_backwards_rates_zero() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [1000, 0])], Duration::from_secs(1));
        let rates = counters.sample(&[reading("sda", [10, 0])], Duration::from_secs(1));
        assert_eq!(rates, vec![("sda".to_owned(), [0.0, 0.0])]);
    }

    #[test]
    fn no_time_between_readings_rates_nothing() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [0, 0])], Duration::from_secs(1));
        let rates = counters.sample(&[reading("sda", [10, 0])], Duration::ZERO);
        assert!(rates.is_empty());
    }

    #[test]
    fn reset_forgets_everything() {
        let mut counters = NamedCounters::<2>::new();
        counters.sample(&[reading("sda", [0, 0])], Duration::from_secs(1));
        counters.reset();
        assert!(counters.sample(&[reading("sda", [10, 0])], Duration::from_secs(1)).is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cd celestina-rs && cargo test -p hematita-core rate`
Expected: 7 failures with `not yet implemented`.

- [ ] **Step 3: Implement**

```rust
    pub fn sample(
        &mut self,
        readings: &[(String, [u64; N])],
        elapsed: Duration,
    ) -> Vec<(String, [f64; N])> {
        let seconds = elapsed.as_secs_f64();
        let mut rates = Vec::new();
        let mut current = HashMap::with_capacity(readings.len());
        for (name, values) in readings {
            if seconds > 0.0 {
                if let Some(previous) = self.previous.get(name) {
                    let mut rate = [0.0; N];
                    for (slot, (now, before)) in rate.iter_mut().zip(values.iter().zip(previous)) {
                        // A counter that went backwards is a reset, not a
                        // negative flow: saturate to zero.
                        *slot = now.saturating_sub(*before) as f64 / seconds;
                    }
                    rates.push((name.clone(), rate));
                }
            }
            current.insert(name.clone(), *values);
        }
        self.previous = current;
        rates
    }
```

`u64 as f64` is deliberate: byte counters are far below 2^53 per second.

- [ ] **Step 4: Run tests, fmt, clippy**

Run: `cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings`
Expected: all pass, clean. If clippy flags the `as f64` cast under a workspace lint, wrap it in `f64::from(u32::try_from(x).unwrap_or(u32::MAX))`-free form: keep `as f64` with the comment; the workspace enables `clippy::all`, not `pedantic`.

---

### Task 3: `hematita-core::disk`

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/disk.rs`
- Modify: `celestina-rs/crates/hematita-core/src/lib.rs` (add `pub mod disk;`)

**Interfaces:**
- Produces: `pub const SECTOR_BYTES: u64 = 512;` `pub struct DiskStat { pub name: String, pub read_sectors: u64, pub write_sectors: u64 }`; `pub fn parse_diskstats(text: &str) -> Result<Vec<DiskStat>, DiskError>`; `pub fn is_whole_device(name: &str) -> bool`; `pub fn parse_size_sectors(text: &str) -> Result<u64, DiskError>`; `pub fn parse_flag(text: &str) -> Result<bool, DiskError>`; `pub fn clean_model(text: &str) -> String`; `pub enum DiskError { TooFewFields { line: String }, UnreadableNumber { line: String } }`.

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-rs/crates/hematita-core/src/disk.rs
//! Disks, as `/proc/diskstats` and `/sys/block` report them.
//!
//! Only whole devices are shown: a partition's traffic is already counted by
//! its disk, and a person thinks in drives. The kernel's `diskstats` sectors
//! are always 512 bytes regardless of the device's own sector size.

use std::fmt;

pub const SECTOR_BYTES: u64 = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskStat {
    pub name: String,
    pub read_sectors: u64,
    pub write_sectors: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiskError {
    TooFewFields { line: String },
    UnreadableNumber { line: String },
}

impl fmt::Display for DiskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewFields { line } => write!(formatter, "diskstats line is too short: {line}"),
            Self::UnreadableNumber { line } => {
                write!(formatter, "disk value is not a number: {line}")
            }
        }
    }
}

impl std::error::Error for DiskError {}

/// Whether a `/proc/diskstats` or `/sys/block` name is a drive rather than a
/// partition or a virtual device: `sda` yes, `sda1` no, `nvme0n1` yes,
/// `nvme0n1p2` no, `mmcblk0` yes, `mmcblk0p1` no; `loop*`, `ram*`, `zram*`,
/// `dm-*`, `sr*` and `md*` never.
#[must_use]
pub fn is_whole_device(name: &str) -> bool {
    todo!()
}

/// Parses `/proc/diskstats`, keeping whole devices only.
///
/// # Errors
///
/// A line with fewer than the fourteen classic fields, or a non-numeric
/// sector count.
pub fn parse_diskstats(text: &str) -> Result<Vec<DiskStat>, DiskError> {
    todo!()
}

/// `/sys/block/DEV/size`: sectors of 512 bytes.
///
/// # Errors
///
/// Not one integer.
pub fn parse_size_sectors(text: &str) -> Result<u64, DiskError> {
    todo!()
}

/// `/sys/block/DEV/queue/rotational` and `/sys/block/DEV/removable`: `0` or `1`.
///
/// # Errors
///
/// Anything else.
pub fn parse_flag(text: &str) -> Result<bool, DiskError> {
    todo!()
}

/// `/sys/block/DEV/device/model`, trimmed and with runs of blanks collapsed.
#[must_use]
pub fn clean_model(text: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISKSTATS: &str = "\
 259       0 nvme0n1 971829 144408 138614792 453932 609417 4538 93511328 416674 0 298818 897538 53732 0 1547679744 22193 3126 4738
 259       1 nvme0n1p1 971733 144408 138609576 453913 609417 4538 93511328 416674 0 191310 892781 53732 0 1547679744 22193 0 0
   8       0 sda 16756139 2604077 1081553769 67141404 1278548 328194 853593593 58985297 0 5007622 126145546 0 0 0 0 16999 18843
 254       0 zram0 100 0 800 0 200 0 1600 0 0 0 0 0 0 0 0 0 0
   7       0 loop0 1 0 8 0 0 0 0 0 0 0 0 0 0 0 0 0 0
";

    #[test]
    fn whole_devices_are_drives_not_partitions_or_virtual_devices() {
        for name in ["sda", "sdz", "nvme0n1", "nvme12n3", "mmcblk0", "vda", "hda"] {
            assert!(is_whole_device(name), "{name}");
        }
        for name in [
            "sda1", "nvme0n1p1", "mmcblk0p2", "loop0", "ram0", "zram0", "dm-0", "sr0", "md127", "",
        ] {
            assert!(!is_whole_device(name), "{name}");
        }
    }

    #[test]
    fn diskstats_keeps_whole_devices_with_their_sector_counts() {
        let disks = parse_diskstats(DISKSTATS).expect("readable diskstats");
        assert_eq!(disks.len(), 2);
        assert_eq!(
            disks[0],
            DiskStat { name: "nvme0n1".to_owned(), read_sectors: 138_614_792, write_sectors: 93_511_328 }
        );
        assert_eq!(disks[1].name, "sda");
        assert_eq!(disks[1].write_sectors, 853_593_593);
    }

    #[test]
    fn a_short_or_unreadable_diskstats_line_is_refused() {
        assert!(matches!(
            parse_diskstats("   8       0 sda 1 2 3\n"),
            Err(DiskError::TooFewFields { .. })
        ));
        assert!(matches!(
            parse_diskstats("   8       0 sda 1 2 x 4 5 6 7 8 9 10 11 12 13 14\n"),
            Err(DiskError::UnreadableNumber { .. })
        ));
        assert_eq!(parse_diskstats("").expect("empty is fine"), vec![]);
    }

    #[test]
    fn sysfs_helpers_read_one_value_each() {
        assert_eq!(parse_size_sectors("1953525168\n"), Ok(1_953_525_168));
        assert!(matches!(parse_size_sectors("big\n"), Err(DiskError::UnreadableNumber { .. })));
        assert_eq!(parse_flag("0\n"), Ok(false));
        assert_eq!(parse_flag("1\n"), Ok(true));
        assert!(matches!(parse_flag("yes\n"), Err(DiskError::UnreadableNumber { .. })));
        assert_eq!(clean_model("Samsung SSD 990 PRO 1TB                 \n"), "Samsung SSD 990 PRO 1TB");
        assert_eq!(clean_model("My   Passport  2627"), "My Passport 2627");
        assert_eq!(clean_model(""), "");
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cd celestina-rs && cargo test -p hematita-core disk` → 4 failures.

- [ ] **Step 3: Implement**

```rust
pub fn is_whole_device(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    for prefix in ["loop", "ram", "zram", "dm-", "sr", "md"] {
        if name.starts_with(prefix) {
            return false;
        }
    }
    if let Some(rest) = name.strip_prefix("nvme") {
        // nvme<ctrl>n<ns>[p<part>]
        return rest.contains('n') && !rest.contains('p');
    }
    if let Some(rest) = name.strip_prefix("mmcblk") {
        return !rest.contains('p');
    }
    // sdX, vdX, hdX: letters only after the prefix.
    name.len() >= 3 && !name.ends_with(|c: char| c.is_ascii_digit())
}

pub fn parse_diskstats(text: &str) -> Result<Vec<DiskStat>, DiskError> {
    let mut disks = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        // major minor name reads merged rsectors rms writes wmerged wsectors …
        if fields.len() < 14 {
            return Err(DiskError::TooFewFields { line: line.to_owned() });
        }
        let name = fields[2];
        if !is_whole_device(name) {
            continue;
        }
        let number = |index: usize| -> Result<u64, DiskError> {
            fields[index]
                .parse::<u64>()
                .map_err(|_| DiskError::UnreadableNumber { line: line.to_owned() })
        };
        disks.push(DiskStat {
            name: name.to_owned(),
            read_sectors: number(5)?,
            write_sectors: number(9)?,
        });
    }
    Ok(disks)
}

pub fn parse_size_sectors(text: &str) -> Result<u64, DiskError> {
    text.trim()
        .parse::<u64>()
        .map_err(|_| DiskError::UnreadableNumber { line: text.trim().to_owned() })
}

pub fn parse_flag(text: &str) -> Result<bool, DiskError> {
    match text.trim() {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(DiskError::UnreadableNumber { line: other.to_owned() }),
    }
}

pub fn clean_model(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

- [ ] **Step 4: Run tests, fmt, clippy** — as in Task 2. Expected: pass, clean.

---

### Task 4: `hematita-core::network`

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/network.rs`
- Modify: `celestina-rs/crates/hematita-core/src/lib.rs` (add `pub mod network;`)

**Interfaces:**
- Produces: `pub struct InterfaceStat { pub name: String, pub rx_bytes: u64, pub tx_bytes: u64 }`; `pub fn parse_net_dev(text: &str) -> Result<Vec<InterfaceStat>, NetworkError>` (drops `lo`); `pub fn parse_link_type(text: &str) -> Result<u32, NetworkError>`; `pub const ARPHRD_ETHER: u32 = 1;` `pub fn parse_operstate(text: &str) -> bool` (true for `up`); `pub fn parse_speed_mbit(text: &str) -> Option<u32>` (None for absent/invalid/negative); `pub enum NetworkError { TooFewFields { line: String }, UnreadableNumber { line: String } }`.

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-rs/crates/hematita-core/src/network.rs
//! Interfaces, as `/proc/net/dev` and `/sys/class/net` report them.
//!
//! Loopback is dropped: traffic to oneself is not what a person means by
//! "network". Wireless is a fact of the interface, read from the presence of
//! its `wireless` directory by the caller; the link speed file is absent or
//! invalid on Wi-Fi and on a cable that is down, which is why speed is an
//! `Option` and never an error.

use std::fmt;

/// `ARPHRD_ETHER`: Ethernet and Wi-Fi both report it.
pub const ARPHRD_ETHER: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceStat {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkError {
    TooFewFields { line: String },
    UnreadableNumber { line: String },
}

impl fmt::Display for NetworkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewFields { line } => write!(formatter, "net/dev line is too short: {line}"),
            Self::UnreadableNumber { line } => {
                write!(formatter, "network value is not a number: {line}")
            }
        }
    }
}

impl std::error::Error for NetworkError {}

/// Parses `/proc/net/dev`, dropping `lo`. The two header lines are skipped
/// by shape (no `:`), not by count.
///
/// # Errors
///
/// A data line with fewer than the sixteen counters, or a non-numeric byte
/// count.
pub fn parse_net_dev(text: &str) -> Result<Vec<InterfaceStat>, NetworkError> {
    todo!()
}

/// `/sys/class/net/IF/type`.
///
/// # Errors
///
/// Not one integer.
pub fn parse_link_type(text: &str) -> Result<u32, NetworkError> {
    todo!()
}

/// `/sys/class/net/IF/operstate` is `up`.
#[must_use]
pub fn parse_operstate(text: &str) -> bool {
    text.trim() == "up"
}

/// `/sys/class/net/IF/speed` in Mbit/s; `None` when the kernel does not know.
#[must_use]
pub fn parse_speed_mbit(text: &str) -> Option<u32> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NET_DEV: &str = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1157448090 20765012    0    0    0     0          0         0 1157448090 20765012    0    0    0     0       0          0
enp9s0: 21272700923 25711259    0    0    0     0          0      2003 96284682 1092621    0   73    0     0       0          0
 wlan0: 266919562  395078    0    0    0     0          0         0 186592009  316705    0    0    0     0       0          0
";

    #[test]
    fn net_dev_drops_loopback_and_reads_bytes_both_ways() {
        let interfaces = parse_net_dev(NET_DEV).expect("readable net/dev");
        assert_eq!(interfaces.len(), 2);
        assert_eq!(
            interfaces[0],
            InterfaceStat { name: "enp9s0".to_owned(), rx_bytes: 21_272_700_923, tx_bytes: 96_284_682 }
        );
        assert_eq!(interfaces[1].name, "wlan0");
        assert_eq!(interfaces[1].tx_bytes, 186_592_009);
    }

    #[test]
    fn a_short_or_unreadable_net_dev_line_is_refused() {
        assert!(matches!(
            parse_net_dev("eth0: 1 2 3\n"),
            Err(NetworkError::TooFewFields { .. })
        ));
        assert!(matches!(
            parse_net_dev("eth0: x 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16\n"),
            Err(NetworkError::UnreadableNumber { .. })
        ));
        assert_eq!(parse_net_dev("").expect("empty is fine"), vec![]);
    }

    #[test]
    fn sysfs_helpers_tolerate_what_wifi_and_a_down_cable_report() {
        assert_eq!(parse_link_type("1\n"), Ok(ARPHRD_ETHER));
        assert!(matches!(parse_link_type("ether\n"), Err(NetworkError::UnreadableNumber { .. })));
        assert!(parse_operstate("up\n"));
        assert!(!parse_operstate("down\n"));
        assert!(!parse_operstate("unknown\n"));
        assert_eq!(parse_speed_mbit("1000\n"), Some(1000));
        assert_eq!(parse_speed_mbit("-1\n"), None);
        assert_eq!(parse_speed_mbit(""), None);
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p hematita-core network` → 3 failures.

- [ ] **Step 3: Implement**

```rust
pub fn parse_net_dev(text: &str) -> Result<Vec<InterfaceStat>, NetworkError> {
    let mut interfaces = Vec::new();
    for line in text.lines() {
        let Some((name, counters)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name == "lo" {
            continue;
        }
        let fields: Vec<&str> = counters.split_whitespace().collect();
        // 8 receive + 8 transmit counters; bytes are the first of each half.
        if fields.len() < 16 {
            return Err(NetworkError::TooFewFields { line: line.to_owned() });
        }
        let number = |index: usize| -> Result<u64, NetworkError> {
            fields[index]
                .parse::<u64>()
                .map_err(|_| NetworkError::UnreadableNumber { line: line.to_owned() })
        };
        interfaces.push(InterfaceStat {
            name: name.to_owned(),
            rx_bytes: number(0)?,
            tx_bytes: number(8)?,
        });
    }
    Ok(interfaces)
}

pub fn parse_link_type(text: &str) -> Result<u32, NetworkError> {
    text.trim()
        .parse::<u32>()
        .map_err(|_| NetworkError::UnreadableNumber { line: text.trim().to_owned() })
}

pub fn parse_speed_mbit(text: &str) -> Option<u32> {
    text.trim().parse::<u32>().ok()
}
```

- [ ] **Step 4: Run tests, fmt, clippy** — Expected: pass, clean.

---

### Task 5: `hematita-core::gpu`

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/gpu.rs`
- Modify: `celestina-rs/crates/hematita-core/src/lib.rs` (add `pub mod gpu;`)

**Interfaces:**
- Produces: `pub struct AmdgpuFiles<'a> { pub busy_percent: &'a str, pub memory_busy_percent: &'a str, pub vram_used: &'a str, pub vram_total: &'a str, pub gtt_used: &'a str, pub gtt_total: &'a str, pub sclk: &'a str, pub mclk: &'a str }`; `pub struct GpuReading { pub busy_percent: u8, pub memory_busy_percent: u8, pub vram_used: u64, pub vram_total: u64, pub gtt_used: u64, pub gtt_total: u64, pub core_mhz: Option<u32>, pub memory_mhz: Option<u32> }`; `pub fn parse_amdgpu(files: &AmdgpuFiles<'_>) -> Result<GpuReading, GpuError>`; `pub fn parse_dpm_active_mhz(text: &str) -> Option<u32>`; `pub fn is_amdgpu_driver_link(target: &str) -> bool`; `pub enum GpuError { UnreadableNumber { file: &'static str, text: String } }`.

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-rs/crates/hematita-core/src/gpu.rs
//! The AMD GPU, as `amdgpu`'s sysfs files report it.
//!
//! Only AMD in this phase: it is the card the author has, and its driver
//! exposes busy percentages, memory and clock levels as plain files. The
//! caller reads the files; this module turns their text into one reading.
//! Clock levels are optional: a driver in a power-saving mode marks no level
//! active, and that is not an error.

use std::fmt;

pub struct AmdgpuFiles<'a> {
    pub busy_percent: &'a str,
    pub memory_busy_percent: &'a str,
    pub vram_used: &'a str,
    pub vram_total: &'a str,
    pub gtt_used: &'a str,
    pub gtt_total: &'a str,
    pub sclk: &'a str,
    pub mclk: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuReading {
    pub busy_percent: u8,
    pub memory_busy_percent: u8,
    pub vram_used: u64,
    pub vram_total: u64,
    pub gtt_used: u64,
    pub gtt_total: u64,
    pub core_mhz: Option<u32>,
    pub memory_mhz: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuError {
    UnreadableNumber { file: &'static str, text: String },
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnreadableNumber { file, text } => {
                write!(formatter, "amdgpu {file} is not a number: {text}")
            }
        }
    }
}

impl std::error::Error for GpuError {}

/// Whether a `/sys/class/drm/cardN/device/driver` link points at `amdgpu`.
#[must_use]
pub fn is_amdgpu_driver_link(target: &str) -> bool {
    target.rsplit('/').next() == Some("amdgpu")
}

/// Parses the eight files into one reading. Percentages saturate at 100.
///
/// # Errors
///
/// A busy or memory file that is not an integer.
pub fn parse_amdgpu(files: &AmdgpuFiles<'_>) -> Result<GpuReading, GpuError> {
    todo!()
}

/// The active level of a `pp_dpm_sclk`/`pp_dpm_mclk` file: the line marked
/// with `*`, as MHz.
#[must_use]
pub fn parse_dpm_active_mhz(text: &str) -> Option<u32> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCLK: &str = "0: 500Mhz \n1: 1568Mhz *\n2: 2400Mhz \n";
    const MCLK: &str = "0: 96Mhz \n1: 456Mhz \n5: 1258Mhz *\n";

    fn files<'a>(busy: &'a str, vram_used: &'a str) -> AmdgpuFiles<'a> {
        AmdgpuFiles {
            busy_percent: busy,
            memory_busy_percent: "15\n",
            vram_used,
            vram_total: "17095983104\n",
            gtt_used: "1299664896\n",
            gtt_total: "33486536704\n",
            sclk: SCLK,
            mclk: MCLK,
        }
    }

    #[test]
    fn the_eight_files_become_one_reading() {
        let reading = parse_amdgpu(&files("100\n", "11859542016\n")).expect("readable");
        assert_eq!(
            reading,
            GpuReading {
                busy_percent: 100,
                memory_busy_percent: 15,
                vram_used: 11_859_542_016,
                vram_total: 17_095_983_104,
                gtt_used: 1_299_664_896,
                gtt_total: 33_486_536_704,
                core_mhz: Some(1568),
                memory_mhz: Some(1258),
            }
        );
    }

    #[test]
    fn percentages_saturate_and_bad_numbers_are_refused() {
        assert_eq!(parse_amdgpu(&files("250\n", "0\n")).expect("readable").busy_percent, 100);
        assert_eq!(
            parse_amdgpu(&files("busy\n", "0\n")),
            Err(GpuError::UnreadableNumber { file: "gpu_busy_percent", text: "busy".to_owned() })
        );
        assert!(matches!(
            parse_amdgpu(&files("1\n", "lots\n")),
            Err(GpuError::UnreadableNumber { file: "mem_info_vram_used", .. })
        ));
    }

    #[test]
    fn the_active_clock_level_is_the_starred_line() {
        assert_eq!(parse_dpm_active_mhz(SCLK), Some(1568));
        assert_eq!(parse_dpm_active_mhz("0: 500Mhz \n1: 1568Mhz \n"), None);
        assert_eq!(parse_dpm_active_mhz(""), None);
        assert_eq!(parse_dpm_active_mhz("1: fastMhz *\n"), None);
    }

    #[test]
    fn the_driver_link_names_amdgpu_at_its_end() {
        assert!(is_amdgpu_driver_link("../../../../../../bus/pci/drivers/amdgpu"));
        assert!(!is_amdgpu_driver_link("../../bus/pci/drivers/nvidia"));
        assert!(!is_amdgpu_driver_link(""));
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p hematita-core gpu` → 3 failures (the driver-link test passes already).

- [ ] **Step 3: Implement**

```rust
pub fn parse_amdgpu(files: &AmdgpuFiles<'_>) -> Result<GpuReading, GpuError> {
    fn number(file: &'static str, text: &str) -> Result<u64, GpuError> {
        text.trim()
            .parse::<u64>()
            .map_err(|_| GpuError::UnreadableNumber { file, text: text.trim().to_owned() })
    }
    fn percent(file: &'static str, text: &str) -> Result<u8, GpuError> {
        Ok(u8::try_from(number(file, text)?.min(100)).unwrap_or(100))
    }
    Ok(GpuReading {
        busy_percent: percent("gpu_busy_percent", files.busy_percent)?,
        memory_busy_percent: percent("mem_busy_percent", files.memory_busy_percent)?,
        vram_used: number("mem_info_vram_used", files.vram_used)?,
        vram_total: number("mem_info_vram_total", files.vram_total)?,
        gtt_used: number("mem_info_gtt_used", files.gtt_used)?,
        gtt_total: number("mem_info_gtt_total", files.gtt_total)?,
        core_mhz: parse_dpm_active_mhz(files.sclk),
        memory_mhz: parse_dpm_active_mhz(files.mclk),
    })
}

pub fn parse_dpm_active_mhz(text: &str) -> Option<u32> {
    text.lines()
        .find(|line| line.trim_end().ends_with('*'))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|token| token.strip_suffix("Mhz").or_else(|| token.strip_suffix("MHz")))
        .and_then(|digits| digits.parse::<u32>().ok())
}
```

- [ ] **Step 4: Run tests, fmt, clippy** — Expected: pass, clean.

---

### Task 6: Ring fractions, captures, and the H2-A unit

**Files:**
- Modify: `celestina-rs/crates/hematita-core/src/history.rs`
- Create: `celestina-rs/crates/hematita-core/tests/fixtures/proc-diskstats.txt`, `proc-net-dev.txt`
- Modify: `celestina-rs/crates/hematita-core/tests/captures.rs`
- Create: `hematita/docs/evidence/2026-09-22-h2-core.md`
- Modify: the H2 plan ledger, `hematita/ROADMAP.md`, `hematita/STATUS.md`

**Interfaces:**
- Produces: `Ring::max(&self) -> f32` (0 when empty), `Ring::fractions(&self) -> Vec<f32>` (each value divided by `max`, zeros when `max <= 0`, always `HISTORY_SAMPLES` long).

- [ ] **Step 1: Write the failing tests** (append to `history.rs` tests)

```rust
    #[test]
    fn fractions_scale_a_series_by_its_own_peak() {
        let mut ring = Ring::new();
        ring.push(50.0);
        ring.push(200.0);
        let fractions = ring.fractions();
        assert_eq!(fractions.len(), HISTORY_SAMPLES);
        assert_eq!(ring.max(), 200.0);
        assert_eq!(fractions[HISTORY_SAMPLES - 2], 0.25);
        assert_eq!(fractions[HISTORY_SAMPLES - 1], 1.0);
        assert_eq!(fractions[0], 0.0);
    }

    #[test]
    fn a_flat_or_empty_series_has_no_peak_to_scale_by() {
        let mut ring = Ring::new();
        assert_eq!(ring.max(), 0.0);
        assert_eq!(ring.fractions(), vec![0.0; HISTORY_SAMPLES]);
        ring.push(0.0);
        ring.push(0.0);
        assert_eq!(ring.fractions(), vec![0.0; HISTORY_SAMPLES]);
    }
```

Add the methods with `todo!()` bodies:

```rust
    /// The largest sample in the window, or 0 when empty.
    #[must_use]
    pub fn max(&self) -> f32 {
        todo!()
    }

    /// The window scaled to its own peak: 1.0 is the busiest second shown. A
    /// window with no positive sample answers zeros. This is how a throughput
    /// graph gets a shape without a ceiling nobody knows in advance.
    #[must_use]
    pub fn fractions(&self) -> Vec<f32> {
        todo!()
    }
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p hematita-core history` → 2 failures.

- [ ] **Step 3: Implement**

```rust
    pub fn max(&self) -> f32 {
        self.values().into_iter().fold(0.0_f32, f32::max)
    }

    pub fn fractions(&self) -> Vec<f32> {
        let peak = self.max();
        let values = self.values();
        if peak <= 0.0 {
            return vec![0.0; values.len()];
        }
        values.into_iter().map(|value| (value / peak).clamp(0.0, 1.0)).collect()
    }
```

- [ ] **Step 4: Capture the fixtures and extend the capture tests**

```bash
cat /proc/diskstats > celestina-rs/crates/hematita-core/tests/fixtures/proc-diskstats.txt
cat /proc/net/dev > celestina-rs/crates/hematita-core/tests/fixtures/proc-net-dev.txt
grep -cE ' (sd[a-z]+|nvme[0-9]+n[0-9]+|vd[a-z]+|mmcblk[0-9]+) ' celestina-rs/crates/hematita-core/tests/fixtures/proc-diskstats.txt
grep -c ':' celestina-rs/crates/hematita-core/tests/fixtures/proc-net-dev.txt
```

Note the whole-disk count (`DISKS`, expected 4 on the author's machine: nvme0n1, nvme1n1, sda, sdb) and the interface count minus `lo` (`INTERFACES`, expected 2). Append to `tests/captures.rs`:

```rust
use hematita_core::disk::parse_diskstats;
use hematita_core::network::parse_net_dev;

const DISKSTATS: &str = include_str!("fixtures/proc-diskstats.txt");
const NET_DEV: &str = include_str!("fixtures/proc-net-dev.txt");

#[test]
fn the_captured_diskstats_lists_the_whole_disks_only() {
    let disks = parse_diskstats(DISKSTATS).expect("the captured /proc/diskstats parses");
    assert_eq!(disks.len(), DISKS);
    assert!(disks.iter().all(|disk| !disk.name.contains('p') || disk.name.starts_with("sd")));
    assert!(disks.iter().any(|disk| disk.name == "nvme0n1"));
}

#[test]
fn the_captured_net_dev_lists_the_real_interfaces_without_loopback() {
    let interfaces = parse_net_dev(NET_DEV).expect("the captured /proc/net/dev parses");
    assert_eq!(interfaces.len(), INTERFACES);
    assert!(interfaces.iter().all(|interface| interface.name != "lo"));
}
```

Replace `DISKS` and `INTERFACES` with the literals you noted.

- [ ] **Step 5: Run the whole crate**

Run: `cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings`
Expected: 17 + 7 (rate) + 4 (disk) + 3 (network) + 4 (gpu) + 2 (history) unit tests and 5 capture tests pass; clean.

- [ ] **Step 6: Close H2-A**

Evidence `hematita/docs/evidence/2026-09-22-h2-core.md` from the template: the commands above with the printed test counts, `Limits`: no consumer yet, no Qt build. Ledger row H2-A → `done` with evidence link; roadmap row `done`; STATUS `Updated` and a sentence that the crate now parses disks, interfaces and the GPU. Inventory:

```bash
python3 "$SCRATCH/mkinv.py" H2-A \
  hematita/docs/inventories/2026-09-22-h2-resources/H2-A.numstat.tsv \
  hematita/docs/plans/active/2026-09-22-h2-resources.md \
  hematita/docs/evidence/2026-09-22-h2-core.md hematita/ROADMAP.md hematita/STATUS.md hematita/VALIDATION.md \
  hematita/docs/plans/active/README.md \
  celestina-rs/crates/hematita-core/src/lib.rs celestina-rs/crates/hematita-core/src/rate.rs \
  celestina-rs/crates/hematita-core/src/disk.rs celestina-rs/crates/hematita-core/src/network.rs \
  celestina-rs/crates/hematita-core/src/gpu.rs celestina-rs/crates/hematita-core/src/history.rs \
  celestina-rs/crates/hematita-core/tests/captures.rs \
  celestina-rs/crates/hematita-core/tests/fixtures/proc-diskstats.txt \
  celestina-rs/crates/hematita-core/tests/fixtures/proc-net-dev.txt
python3 scripts/check-staged-units.py hematita/docs/inventories/2026-09-22-h2-resources/H2-A.numstat.tsv
bash scripts/check-architecture-contract.sh && bash scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py
```

Commit:

```bash
git commit -m "$(cat <<'EOF'
hematita-maintenance: Add the disk, network and GPU parsers with named-counter rates

H2-A: NamedCounters turning cumulative byte counters into per-second rates;
/proc/diskstats whole devices with sysfs size, model and flags; /proc/net/dev
without loopback with link type, state and speed; amdgpu busy, memory and
active clock levels; ring fractions by peak; captures; H2 opened.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: The sampler learns every resource

**Files:**
- Modify: `hematita/src/sampler.rs` (rewrite of the types below; the thread, `INTERVAL`, `Sampler`, `Drop` stay)

**Interfaces:**
- Produces:
  - `pub enum ReasonKind { Unreadable, Malformed, NoRate }` with `as_str()` → `"unreadable" | "malformed" | "no-rate"`; `pub struct Reason { pub kind: ReasonKind, pub path: String }`.
  - `pub enum Section<T> { Available(T), Unavailable(Reason) }`.
  - `pub struct CpuReading { pub aggregate_percent: u8, pub core_percents: Vec<u8>, pub frequency_mhz: Option<u32> }`.
  - `pub struct DiskInfo { pub name: String, pub model: String, pub size_bytes: u64, pub rotational: bool }`, `pub struct DiskSection { pub info: DiskInfo, pub rate: Section<Option<[f64; 2]>> }` (`[read, write]` bytes/s; `None` = waiting).
  - `pub struct InterfaceInfo { pub name: String, pub wireless: bool, pub up: bool, pub speed_mbit: Option<u32> }`, `pub struct InterfaceSection { pub info: InterfaceInfo, pub rate: Section<Option<[f64; 2]>> }` (`[rx, tx]`).
  - `pub struct Snapshot { pub generation: u64, pub cpu: Section<Option<CpuReading>>, pub memory: Section<Memory>, pub gpu: Option<Section<GpuReading>>, pub disks: Vec<DiskSection>, pub interfaces: Vec<InterfaceSection>, pub identity: Option<Identity> }`.
  - `pub struct Identity { pub cpu_model: String, pub cpu_cores: usize, pub gpu_id: String }` (`gpu_id` like `1002:7550`, empty without amdgpu).

- [ ] **Step 1: Replace the types and the per-section readers**

Keep the file's header comment, `INTERVAL`, `Sampler`, `Drop` and `run`'s shape. Replace `Section`, `CpuReading`, `Snapshot`, `Identity`, `read_identity`, `read`, `sample_cpu`, `sample_memory` with:

```rust
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use hematita_core::cpu::{self, CpuSampler};
use hematita_core::disk::{self, SECTOR_BYTES};
use hematita_core::gpu::{self, AmdgpuFiles, GpuReading};
use hematita_core::memory::{self, Memory};
use hematita_core::network;
use hematita_core::rate::NamedCounters;

const STAT_PATH: &str = "/proc/stat";
const MEMINFO_PATH: &str = "/proc/meminfo";
const CPUINFO_PATH: &str = "/proc/cpuinfo";
const FREQUENCY_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq";
const DISKSTATS_PATH: &str = "/proc/diskstats";
const NET_DEV_PATH: &str = "/proc/net/dev";
const BLOCK_ROOT: &str = "/sys/block";
const NET_ROOT: &str = "/sys/class/net";
const DRM_ROOT: &str = "/sys/class/drm";

/// Why a section could not be read. The window composes the sentence; this
/// is data, not prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasonKind {
    Unreadable,
    Malformed,
    NoRate,
}

impl ReasonKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::Malformed => "malformed",
            Self::NoRate => "no-rate",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reason {
    pub kind: ReasonKind,
    pub path: String,
}

#[derive(Clone, Debug)]
pub enum Section<T> {
    Available(T),
    Unavailable(Reason),
}

#[derive(Clone, Debug)]
pub struct CpuReading {
    pub aggregate_percent: u8,
    pub core_percents: Vec<u8>,
    pub frequency_mhz: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskInfo {
    pub name: String,
    pub model: String,
    pub size_bytes: u64,
    pub rotational: bool,
}

#[derive(Clone, Debug)]
pub struct DiskSection {
    pub info: DiskInfo,
    /// `[read, write]` bytes per second; `None` until the second reading.
    pub rate: Section<Option<[f64; 2]>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceInfo {
    pub name: String,
    pub wireless: bool,
    pub up: bool,
    pub speed_mbit: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct InterfaceSection {
    pub info: InterfaceInfo,
    /// `[rx, tx]` bytes per second; `None` until the second reading.
    pub rate: Section<Option<[f64; 2]>>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub generation: u64,
    pub cpu: Section<Option<CpuReading>>,
    pub memory: Section<Memory>,
    /// `None` when the machine has no `amdgpu` card: absence, not failure.
    pub gpu: Option<Section<GpuReading>>,
    pub disks: Vec<DiskSection>,
    pub interfaces: Vec<InterfaceSection>,
    /// Carried by the first snapshot only.
    pub identity: Option<Identity>,
}

#[derive(Clone, Debug, Default)]
pub struct Identity {
    pub cpu_model: String,
    pub cpu_cores: usize,
    /// `vendor:device` of the AMD card, empty without one.
    pub gpu_id: String,
}

fn read(path: &Path) -> Result<String, Reason> {
    std::fs::read_to_string(path).map_err(|_| Reason {
        kind: ReasonKind::Unreadable,
        path: path.display().to_string(),
    })
}

fn malformed(path: &Path) -> Reason {
    Reason {
        kind: ReasonKind::Malformed,
        path: path.display().to_string(),
    }
}

/// The `amdgpu` card's device directory, if any: the first `cardN` whose
/// `device/driver` link ends in `amdgpu`.
fn amdgpu_device() -> Option<PathBuf> {
    let entries = std::fs::read_dir(DRM_ROOT).ok()?;
    let mut cards: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("card") && !name.contains('-'))
        })
        .collect();
    cards.sort();
    cards.into_iter().find_map(|card| {
        let device = card.join("device");
        let target = std::fs::read_link(device.join("driver")).ok()?;
        gpu::is_amdgpu_driver_link(&target.to_string_lossy()).then_some(device)
    })
}

#[must_use]
pub fn read_identity() -> Identity {
    let cpu_model = read(Path::new(CPUINFO_PATH))
        .ok()
        .and_then(|text| cpu::parse_model(&text))
        .unwrap_or_default();
    let cpu_cores = read(Path::new(STAT_PATH))
        .ok()
        .and_then(|text| cpu::parse_stat(&text).ok())
        .map_or(0, |stat| stat.cores.len());
    let gpu_id = amdgpu_device()
        .and_then(|device| {
            let vendor = read(&device.join("vendor")).ok()?;
            let id = read(&device.join("device")).ok()?;
            Some(format!(
                "{}:{}",
                vendor.trim().trim_start_matches("0x"),
                id.trim().trim_start_matches("0x")
            ))
        })
        .unwrap_or_default();
    Identity { cpu_model, cpu_cores, gpu_id }
}

fn sample_cpu(sampler: &mut CpuSampler) -> Section<Option<CpuReading>> {
    let path = Path::new(STAT_PATH);
    let stat = match read(path) {
        Ok(text) => match cpu::parse_stat(&text) {
            Ok(stat) => stat,
            Err(_) => {
                sampler.reset();
                return Section::Unavailable(malformed(path));
            }
        },
        Err(reason) => {
            // A machine whose counters went away is not a machine at 0 %.
            sampler.reset();
            return Section::Unavailable(reason);
        }
    };
    let sample = match sampler.sample(&stat) {
        Ok(sample) => sample,
        // A hot-plugged core restarts the rate; the next second answers.
        Err(cpu::CpuError::CoreCountChanged { .. }) => None,
        Err(_) => {
            return Section::Unavailable(Reason {
                kind: ReasonKind::NoRate,
                path: STAT_PATH.to_owned(),
            })
        }
    };
    // Frequency is optional: a VM or a locked governor has no such file.
    let frequency_mhz = read(Path::new(FREQUENCY_PATH))
        .ok()
        .and_then(|text| cpu::parse_frequency_khz(&text).ok())
        .and_then(|khz| u32::try_from(khz / 1000).ok());
    Section::Available(sample.map(|sample| CpuReading {
        aggregate_percent: sample.aggregate_percent,
        core_percents: sample.core_percents,
        frequency_mhz,
    }))
}

fn sample_memory() -> Section<Memory> {
    let path = Path::new(MEMINFO_PATH);
    match read(path) {
        Ok(text) => memory::parse_meminfo(&text)
            .map(Section::Available)
            .unwrap_or_else(|_| Section::Unavailable(malformed(path))),
        Err(reason) => Section::Unavailable(reason),
    }
}

fn sample_gpu(device: Option<&Path>) -> Option<Section<GpuReading>> {
    let device = device?;
    let file = |name: &str| read(&device.join(name));
    let texts = (
        file("gpu_busy_percent"),
        file("mem_busy_percent"),
        file("mem_info_vram_used"),
        file("mem_info_vram_total"),
        file("mem_info_gtt_used"),
        file("mem_info_gtt_total"),
        file("pp_dpm_sclk"),
        file("pp_dpm_mclk"),
    );
    let (busy, mem_busy, vram_used, vram_total, gtt_used, gtt_total, sclk, mclk) = match texts {
        (Ok(a), Ok(b), Ok(c), Ok(d), Ok(e), Ok(f), Ok(g), Ok(h)) => (a, b, c, d, e, f, g, h),
        (Err(r), ..) | (_, Err(r), ..) | (_, _, Err(r), ..) | (_, _, _, Err(r), ..)
        | (_, _, _, _, Err(r), ..) | (_, _, _, _, _, Err(r), ..)
        | (_, _, _, _, _, _, Err(r), _) | (_, _, _, _, _, _, _, Err(r)) => {
            return Some(Section::Unavailable(r))
        }
    };
    let files = AmdgpuFiles {
        busy_percent: &busy,
        memory_busy_percent: &mem_busy,
        vram_used: &vram_used,
        vram_total: &vram_total,
        gtt_used: &gtt_used,
        gtt_total: &gtt_total,
        sclk: &sclk,
        mclk: &mclk,
    };
    Some(match gpu::parse_amdgpu(&files) {
        Ok(reading) => Section::Available(reading),
        Err(_) => Section::Unavailable(malformed(&device.join("gpu_busy_percent"))),
    })
}

/// Every whole disk with its counters, as `[read, write]` bytes.
fn read_disks() -> Result<Vec<(String, [u64; 2])>, Reason> {
    let path = Path::new(DISKSTATS_PATH);
    let text = read(path)?;
    let stats = disk::parse_diskstats(&text).map_err(|_| malformed(path))?;
    Ok(stats
        .into_iter()
        .map(|stat| {
            (
                stat.name,
                [
                    stat.read_sectors.saturating_mul(SECTOR_BYTES),
                    stat.write_sectors.saturating_mul(SECTOR_BYTES),
                ],
            )
        })
        .collect())
}

fn disk_info(name: &str) -> DiskInfo {
    let root = Path::new(BLOCK_ROOT).join(name);
    let model = read(&root.join("device/model"))
        .map(|text| disk::clean_model(&text))
        .unwrap_or_default();
    let size_bytes = read(&root.join("size"))
        .ok()
        .and_then(|text| disk::parse_size_sectors(&text).ok())
        .map_or(0, |sectors| sectors.saturating_mul(SECTOR_BYTES));
    let rotational = read(&root.join("queue/rotational"))
        .ok()
        .and_then(|text| disk::parse_flag(&text).ok())
        .unwrap_or(false);
    DiskInfo { name: name.to_owned(), model, size_bytes, rotational }
}

fn sample_disks(counters: &mut NamedCounters<2>, elapsed: Duration) -> Vec<DiskSection> {
    let readings = match read_disks() {
        Ok(readings) => readings,
        Err(reason) => {
            counters.reset();
            // One unreadable file is every disk unreadable; the list keeps
            // the disks it can still name from sysfs so the rows do not vanish.
            return enumerate_block_devices()
                .into_iter()
                .map(|name| DiskSection {
                    info: disk_info(&name),
                    rate: Section::Unavailable(reason.clone()),
                })
                .collect();
        }
    };
    let rates = counters.sample(&readings, elapsed);
    readings
        .iter()
        .map(|(name, _)| DiskSection {
            info: disk_info(name),
            rate: Section::Available(
                rates.iter().find(|(rated, _)| rated == name).map(|(_, rate)| *rate),
            ),
        })
        .collect()
}

fn enumerate_block_devices() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(BLOCK_ROOT)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| disk::is_whole_device(name))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn interface_info(name: &str) -> InterfaceInfo {
    let root = Path::new(NET_ROOT).join(name);
    InterfaceInfo {
        name: name.to_owned(),
        wireless: root.join("wireless").is_dir(),
        up: read(&root.join("operstate")).is_ok_and(|text| network::parse_operstate(&text)),
        speed_mbit: read(&root.join("speed")).ok().and_then(|text| network::parse_speed_mbit(&text)),
    }
}

/// Interfaces whose link type is Ethernet (which Wi-Fi also reports).
fn is_shown_interface(name: &str) -> bool {
    read(&Path::new(NET_ROOT).join(name).join("type"))
        .ok()
        .and_then(|text| network::parse_link_type(&text).ok())
        .is_some_and(|kind| kind == network::ARPHRD_ETHER)
}

fn sample_interfaces(counters: &mut NamedCounters<2>, elapsed: Duration) -> Vec<InterfaceSection> {
    let path = Path::new(NET_DEV_PATH);
    let readings: Vec<(String, [u64; 2])> = match read(path)
        .and_then(|text| network::parse_net_dev(&text).map_err(|_| malformed(path)))
    {
        Ok(stats) => stats
            .into_iter()
            .filter(|stat| is_shown_interface(&stat.name))
            .map(|stat| (stat.name, [stat.rx_bytes, stat.tx_bytes]))
            .collect(),
        Err(reason) => {
            counters.reset();
            return Vec::from([InterfaceSection {
                info: InterfaceInfo { name: String::new(), wireless: false, up: false, speed_mbit: None },
                rate: Section::Unavailable(reason),
            }]);
        }
    };
    let rates = counters.sample(&readings, elapsed);
    readings
        .iter()
        .map(|(name, _)| InterfaceSection {
            info: interface_info(name),
            rate: Section::Available(
                rates.iter().find(|(rated, _)| rated == name).map(|(_, rate)| *rate),
            ),
        })
        .collect()
}
```

An `InterfaceSection` with an empty name is the "the whole file failed" row; the page shows one unavailable network row for it.

- [ ] **Step 2: Rewrite `run` to measure elapsed time and carry the new sections**

```rust
fn run(stop: &AtomicBool, publish: &dyn Fn(Snapshot)) {
    let mut cpu_sampler = CpuSampler::new();
    let mut disk_counters = NamedCounters::<2>::new();
    let mut interface_counters = NamedCounters::<2>::new();
    let gpu_device = amdgpu_device();
    let mut generation = 0u64;
    let mut last = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        generation += 1;
        let now = Instant::now();
        let elapsed = now.duration_since(last);
        last = now;
        let identity = (generation == 1).then(read_identity);
        publish(Snapshot {
            generation,
            cpu: sample_cpu(&mut cpu_sampler),
            memory: sample_memory(),
            gpu: sample_gpu(gpu_device.as_deref()),
            disks: sample_disks(&mut disk_counters, elapsed),
            interfaces: sample_interfaces(&mut interface_counters, elapsed),
            identity,
        });
        // Sleep in short slices so a close does not wait a whole interval.
        let mut slept = Duration::ZERO;
        while slept < INTERVAL && !stop.load(Ordering::Relaxed) {
            let slice = Duration::from_millis(100);
            thread::sleep(slice);
            slept += slice;
        }
    }
}
```

The first snapshot's `elapsed` is the time since thread start; `NamedCounters` has no previous reading then, so it rates nothing regardless.

No build in this task: the compile check happens with Task 10's single build. Keep the code `cargo fmt`-shaped by eye (four-space, trailing commas); `cargo fmt` runs in Task 10.

---

### Task 8: `publish.rs` — the adapter's pure decisions, tested

**Files:**
- Create: `hematita/src/publish.rs`
- Modify: `hematita/build.rs` (`.files([...])` gains `"src/publish.rs"` — it is plain Rust, so listing it is only for `rerun-if-changed`; add `println!("cargo::rerun-if-changed=src/publish.rs");` instead of the `.files` list, which is for bridges)
- Modify: `hematita/src/main.rs` (`mod publish;`)

**Interfaces:**
- Produces: `pub const ELEVATED_PERCENT: u8 = 80; pub const CRITICAL_PERCENT: u8 = 90;` `pub fn load_name(percent: u8) -> &'static str` (moved here from `resources.rs`); `pub fn accepts(generation: u64, last: u64) -> bool`; `pub fn fraction(percent: u8) -> f32`; `pub fn ticket(generation: u64) -> i32`; `pub enum Kind { Cpu, Memory, Gpu, Disk, Network }` with `as_str()`; `pub fn cpu_numbers(reading: &CpuReading, cores: usize) -> Vec<f64>`, `memory_numbers(&Memory)`, `gpu_numbers(&GpuReading)`, `disk_numbers(&DiskInfo, Option<[f64; 2]>)`, `network_numbers(&InterfaceInfo, Option<[f64; 2]>)` following the Kind contract; `pub fn throughput(rate: Option<[f64; 2]>) -> f32` (sum of both directions for the history ring).

- [ ] **Step 1: Write the file with tests first**

```rust
// hematita/src/publish.rs
//! What the adapter decides before it touches a Qt property.
//!
//! Everything here is a function over plain values, so it is tested without
//! a QObject: which snapshot is stale, what a percentage looks like as a
//! fraction, which numbers each kind of resource publishes and in what order
//! (the kind contract the page reads by index), and the only two policy
//! numbers Hematita has.

use hematita_core::gpu::GpuReading;
use hematita_core::memory::Memory;

use crate::sampler::{CpuReading, DiskInfo, InterfaceInfo};

/// Above this a value is worth noticing; above [`CRITICAL_PERCENT`] it is
/// worth interrupting for. The page maps these to appearance; the numbers are
/// policy and live here, not in the theme.
pub const ELEVATED_PERCENT: u8 = 80;
pub const CRITICAL_PERCENT: u8 = 90;

/// The state name the theme colours by.
#[must_use]
pub fn load_name(percent: u8) -> &'static str {
    if percent >= CRITICAL_PERCENT {
        "critical"
    } else if percent >= ELEVATED_PERCENT {
        "elevated"
    } else {
        "normal"
    }
}

/// A snapshot is applied only if it is newer than the last applied one: the
/// thread publishes in order, but the queue does not promise to.
#[must_use]
pub fn accepts(generation: u64, last: u64) -> bool {
    generation > last
}

#[must_use]
pub fn fraction(percent: u8) -> f32 {
    f32::from(percent.min(100)) / 100.0
}

/// The change ticket QML rebuilds on, folded into the positive half of an
/// `i32` because QML has no 64-bit integer.
#[must_use]
pub fn ticket(generation: u64) -> i32 {
    i32::try_from(generation % u64::from(u32::MAX / 2)).unwrap_or(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cpu,
    Memory,
    Gpu,
    Disk,
    Network,
}

impl Kind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Gpu => "gpu",
            Self::Disk => "disk",
            Self::Network => "network",
        }
    }
}

// The kind contract. The page reads these by index, so the order here is the
// order documented in the plan and nothing may be inserted in the middle.

#[must_use]
pub fn cpu_numbers(reading: &CpuReading, cores: usize) -> Vec<f64> {
    vec![
        f64::from(reading.aggregate_percent),
        reading.frequency_mhz.map_or(0.0, f64::from),
        cores as f64,
    ]
}

#[must_use]
pub fn memory_numbers(memory: &Memory) -> Vec<f64> {
    // Kibibytes never reach 2^53, so the doubles are exact.
    vec![
        memory.used_kib as f64,
        memory.total_kib as f64,
        memory.swap_used_kib as f64,
        memory.swap_total_kib as f64,
    ]
}

#[must_use]
pub fn gpu_numbers(reading: &GpuReading) -> Vec<f64> {
    vec![
        f64::from(reading.busy_percent),
        f64::from(reading.memory_busy_percent),
        reading.vram_used as f64,
        reading.vram_total as f64,
        reading.gtt_used as f64,
        reading.gtt_total as f64,
        reading.core_mhz.map_or(0.0, f64::from),
        reading.memory_mhz.map_or(0.0, f64::from),
    ]
}

#[must_use]
pub fn disk_numbers(info: &DiskInfo, rate: Option<[f64; 2]>) -> Vec<f64> {
    let [read, write] = rate.unwrap_or([0.0, 0.0]);
    vec![read, write, info.size_bytes as f64, if info.rotational { 1.0 } else { 0.0 }]
}

#[must_use]
pub fn network_numbers(info: &InterfaceInfo, rate: Option<[f64; 2]>) -> Vec<f64> {
    let [rx, tx] = rate.unwrap_or([0.0, 0.0]);
    vec![
        rx,
        tx,
        info.speed_mbit.map_or(0.0, f64::from),
        if info.wireless { 1.0 } else { 0.0 },
        if info.up { 1.0 } else { 0.0 },
    ]
}

/// What a throughput row's history records: both directions together.
#[must_use]
pub fn throughput(rate: Option<[f64; 2]>) -> f32 {
    rate.map_or(0.0, |[a, b]| (a + b) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_names_the_state_the_page_paints() {
        assert_eq!(load_name(0), "normal");
        assert_eq!(load_name(79), "normal");
        assert_eq!(load_name(ELEVATED_PERCENT), "elevated");
        assert_eq!(load_name(89), "elevated");
        assert_eq!(load_name(CRITICAL_PERCENT), "critical");
        assert_eq!(load_name(100), "critical");
    }

    #[test]
    fn only_a_newer_generation_is_applied() {
        assert!(accepts(2, 1));
        assert!(!accepts(1, 1));
        assert!(!accepts(1, 5));
    }

    #[test]
    fn fractions_and_tickets_stay_in_range() {
        assert_eq!(fraction(0), 0.0);
        assert_eq!(fraction(50), 0.5);
        assert_eq!(fraction(200), 1.0);
        assert_eq!(ticket(0), 0);
        assert_eq!(ticket(7), 7);
        assert!(ticket(u64::MAX) >= 0);
    }

    #[test]
    fn each_kind_publishes_its_numbers_in_contract_order() {
        let cpu = CpuReading { aggregate_percent: 23, core_percents: vec![1, 2], frequency_mhz: Some(4658) };
        assert_eq!(cpu_numbers(&cpu, 8), vec![23.0, 4658.0, 8.0]);
        let memory = Memory { used_kib: 10, total_kib: 20, swap_used_kib: 1, swap_total_kib: 2 };
        assert_eq!(memory_numbers(&memory), vec![10.0, 20.0, 1.0, 2.0]);
        let gpu = GpuReading {
            busy_percent: 61, memory_busy_percent: 15, vram_used: 5, vram_total: 10,
            gtt_used: 1, gtt_total: 4, core_mhz: Some(1568), memory_mhz: None,
        };
        assert_eq!(gpu_numbers(&gpu), vec![61.0, 15.0, 5.0, 10.0, 1.0, 4.0, 1568.0, 0.0]);
        let disk = DiskInfo { name: "sda".into(), model: "x".into(), size_bytes: 512, rotational: true };
        assert_eq!(disk_numbers(&disk, Some([3.0, 4.0])), vec![3.0, 4.0, 512.0, 1.0]);
        assert_eq!(disk_numbers(&disk, None), vec![0.0, 0.0, 512.0, 1.0]);
        let net = InterfaceInfo { name: "wlan0".into(), wireless: true, up: false, speed_mbit: None };
        assert_eq!(network_numbers(&net, Some([7.0, 1.0])), vec![7.0, 1.0, 0.0, 1.0, 0.0]);
        assert_eq!(throughput(Some([7.0, 1.0])), 8.0);
        assert_eq!(throughput(None), 0.0);
    }
}
```

- [ ] **Step 2: Wire the module**

`main.rs`: add `mod publish;` after `mod activation;`. `build.rs`: keep `.files(["src/activation.rs", "src/resources.rs"])` and add `println!("cargo::rerun-if-changed=src/publish.rs");` and `println!("cargo::rerun-if-changed=src/sampler.rs");` next to the QML loop (they were missing for `sampler.rs` too).

No build yet.

---

### Task 9: `HematitaResources` publishes lists

**Files:**
- Modify: `hematita/src/resources.rs` (rewrite)

**Interfaces:**
- Produces QML type `HematitaResources` with properties: `revision: i32`; `resourceKeys`, `resourceKinds`, `resourceLabels`, `resourceStates`, `resourceReasonKinds`, `resourceReasonPaths`, `resourceLoads`: `QStringList`; `resourceNumbers`, `resourceHistories`, `cpuCoreHistories`: `QVariant` (lists of lists of doubles); `cpuModel: QString`; `startFailed: bool`; invokable `start()`.
- Row order: `cpu`, `memory`, `gpu` (if the machine has one), disks by name, interfaces by name. Keys: `"cpu"`, `"memory"`, `"gpu"`, `"disk:<name>"`, `"net:<name>"`; the network file-failure row is `"net:"`.
- Labels are data, never product copy: CPU → model; memory → `""`; GPU → `gpu_id`; disk → model or name; network → interface name.
- States: `waiting` (no rate yet), `ready`, `unavailable`.

- [ ] **Step 1: Rewrite `resources.rs`**

```rust
//! The Performance page's state, as Qt properties.
//!
//! Rows travel as index-aligned lists plus a `revision` ticket rather than a
//! native model: CXX-Qt 0.9 cannot override `QAbstractListModel`'s virtuals
//! from Rust, and the suite already publishes list data this way. The page
//! rebuilds its rows when `revision` changes and never binds to a single list,
//! so it never sees one column from this second beside another from the last.
//!
//! Nothing here is prose a person reads: kinds, states and reasons are tokens
//! the page turns into Spanish through `qsTr()`.

use std::collections::HashMap;
use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QString, QStringList, QVariant};

use hematita_core::history::Ring;

use crate::publish::{self, Kind};
use crate::sampler::{Reason, Sampler, Section, Snapshot};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qvariant.h");
        // Lists of lists of doubles cross as a `QVariant`: it is the one shape
        // both qmllint and the engine resolve (see H1-D), and the page reads
        // them by row index.
        type QVariant = cxx_qt_lib::QVariant;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // revision — bumped once, after every list is in place
        // resource* — index-aligned rows: key, kind token, data label, state
        //   token, reason token and path, load token, kind-contract numbers,
        //   minute of history as fractions
        // cpuCoreHistories — one minute per core, fractions
        // cpuModel — the processor's name, for the CPU detail
        // startFailed — the sampling thread could not be created
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QStringList, resource_keys)]
        #[qproperty(QStringList, resource_kinds)]
        #[qproperty(QStringList, resource_labels)]
        #[qproperty(QStringList, resource_states)]
        #[qproperty(QStringList, resource_reason_kinds)]
        #[qproperty(QStringList, resource_reason_paths)]
        #[qproperty(QStringList, resource_loads)]
        #[qproperty(QVariant, resource_numbers)]
        #[qproperty(QVariant, resource_histories)]
        #[qproperty(QVariant, cpu_core_histories)]
        #[qproperty(QString, cpu_model)]
        #[qproperty(bool, start_failed)]
        type HematitaResources = super::HematitaResourcesRust;

        /// Starts the sampler, once. The window calls it when it is up.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaResources>);
    }

    impl cxx_qt::Threading for HematitaResources {}
}

/// One published row before it is split into columns.
struct Row {
    key: String,
    kind: Kind,
    label: String,
    state: &'static str,
    reason: Option<Reason>,
    load: &'static str,
    numbers: Vec<f64>,
    history: Vec<f32>,
}

impl Row {
    fn unavailable(key: String, kind: Kind, label: String, reason: Reason) -> Self {
        Self {
            key,
            kind,
            label,
            state: "unavailable",
            reason: Some(reason),
            load: "normal",
            numbers: Vec::new(),
            history: Vec::new(),
        }
    }
}

pub struct HematitaResourcesRust {
    revision: i32,
    resource_keys: QStringList,
    resource_kinds: QStringList,
    resource_labels: QStringList,
    resource_states: QStringList,
    resource_reason_kinds: QStringList,
    resource_reason_paths: QStringList,
    resource_loads: QStringList,
    resource_numbers: QVariant,
    resource_histories: QVariant,
    cpu_core_histories: QVariant,
    cpu_model: QString,
    start_failed: bool,
    /// One ring per row key; a key that leaves the machine takes its ring
    /// with it, a key that returns starts a fresh minute.
    rings: HashMap<String, Ring>,
    core_rings: Vec<Ring>,
    cpu_cores: usize,
    gpu_id: String,
    last_generation: u64,
    sampler: Option<Sampler>,
}

impl Default for HematitaResourcesRust {
    fn default() -> Self {
        Self {
            revision: 0,
            resource_keys: QStringList::default(),
            resource_kinds: QStringList::default(),
            resource_labels: QStringList::default(),
            resource_states: QStringList::default(),
            resource_reason_kinds: QStringList::default(),
            resource_reason_paths: QStringList::default(),
            resource_loads: QStringList::default(),
            resource_numbers: nested(&[]),
            resource_histories: nested(&[]),
            cpu_core_histories: nested(&[]),
            cpu_model: QString::default(),
            start_failed: false,
            rings: HashMap::new(),
            core_rings: Vec::new(),
            cpu_cores: 0,
            gpu_id: String::new(),
            last_generation: 0,
            sampler: None,
        }
    }
}

fn strings(values: impl IntoIterator<Item = String>) -> QStringList {
    let mut list = QStringList::default();
    for value in values {
        list.append(QString::from(value.as_str()));
    }
    list
}

fn doubles(values: &[f64]) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for value in values {
        list.append(QVariant::from(value));
    }
    QVariant::from(&list)
}

fn nested(rows: &[Vec<f64>]) -> QVariant {
    let mut list = QList::<QVariant>::default();
    for row in rows {
        list.append(doubles(row));
    }
    QVariant::from(&list)
}

fn widen(values: &[f32]) -> Vec<f64> {
    values.iter().map(|value| f64::from(*value)).collect()
}

impl qobject::HematitaResources {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().sampler.is_some() {
            return;
        }
        let qt = self.qt_thread();
        match Sampler::spawn(move |snapshot| {
            let _ = qt.queue(move |resources: Pin<&mut qobject::HematitaResources>| {
                resources.apply(snapshot);
            });
        }) {
            Ok(sampler) => self.as_mut().rust_mut().sampler = Some(sampler),
            Err(_) => self.as_mut().set_start_failed(true),
        }
    }

    fn apply(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        if !publish::accepts(snapshot.generation, self.rust().last_generation) {
            return;
        }
        self.as_mut().rust_mut().last_generation = snapshot.generation;

        if let Some(identity) = &snapshot.identity {
            let model = QString::from(identity.cpu_model.as_str());
            self.as_mut().set_cpu_model(model);
            let state = self.as_mut().rust_mut();
            state.cpu_cores = identity.cpu_cores;
            state.gpu_id = identity.gpu_id.clone();
            state.core_rings = (0..identity.cpu_cores).map(|_| Ring::new()).collect();
        }

        let rows = self.as_mut().rust_mut().rows_from(&snapshot);

        let keys = strings(rows.iter().map(|row| row.key.clone()));
        let kinds = strings(rows.iter().map(|row| row.kind.as_str().to_owned()));
        let labels = strings(rows.iter().map(|row| row.label.clone()));
        let states = strings(rows.iter().map(|row| row.state.to_owned()));
        let reason_kinds = strings(rows.iter().map(|row| {
            row.reason.as_ref().map_or(String::new(), |reason| reason.kind.as_str().to_owned())
        }));
        let reason_paths = strings(rows.iter().map(|row| {
            row.reason.as_ref().map_or(String::new(), |reason| reason.path.clone())
        }));
        let loads = strings(rows.iter().map(|row| row.load.to_owned()));
        let numbers = nested(&rows.iter().map(|row| row.numbers.clone()).collect::<Vec<_>>());
        let histories = nested(&rows.iter().map(|row| widen(&row.history)).collect::<Vec<_>>());
        let cores = nested(
            &self.rust().core_rings.iter().map(|ring| widen(&ring.values())).collect::<Vec<_>>(),
        );

        self.as_mut().set_resource_keys(keys);
        self.as_mut().set_resource_kinds(kinds);
        self.as_mut().set_resource_labels(labels);
        self.as_mut().set_resource_states(states);
        self.as_mut().set_resource_reason_kinds(reason_kinds);
        self.as_mut().set_resource_reason_paths(reason_paths);
        self.as_mut().set_resource_loads(loads);
        self.as_mut().set_resource_numbers(numbers);
        self.as_mut().set_resource_histories(histories);
        self.as_mut().set_cpu_core_histories(cores);
        // Last, so the page rebuilds once, with every column in place.
        let ticket = publish::ticket(snapshot.generation);
        self.as_mut().set_revision(ticket);
    }
}

impl HematitaResourcesRust {
    /// Builds the rows for one snapshot and advances the rings. Rings whose
    /// key is absent from this snapshot are dropped.
    fn rows_from(&mut self, snapshot: &Snapshot) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        // Rings advance only from the second snapshot on, so CPU (which has
        // no rate on the first) and everything else stay aligned.
        let advance = snapshot.generation >= 2;

        // CPU
        match &snapshot.cpu {
            Section::Available(Some(reading)) => {
                let history = self.push("cpu", publish::fraction(reading.aggregate_percent), advance);
                for (ring, percent) in self.core_rings.iter_mut().zip(&reading.core_percents) {
                    if advance {
                        ring.push(publish::fraction(*percent));
                    }
                }
                rows.push(Row {
                    key: "cpu".to_owned(),
                    kind: Kind::Cpu,
                    label: self.cpu_model_string(),
                    state: "ready",
                    reason: None,
                    load: publish::load_name(reading.aggregate_percent),
                    numbers: publish::cpu_numbers(reading, self.cpu_cores),
                    history,
                });
            }
            Section::Available(None) => rows.push(Row {
                key: "cpu".to_owned(),
                kind: Kind::Cpu,
                label: self.cpu_model_string(),
                state: "waiting",
                reason: None,
                load: "normal",
                numbers: Vec::new(),
                history: self.peek("cpu"),
            }),
            Section::Unavailable(reason) => rows.push(Row::unavailable(
                "cpu".to_owned(),
                Kind::Cpu,
                self.cpu_model_string(),
                reason.clone(),
            )),
        }
        seen.push("cpu".to_owned());

        // Memory
        match &snapshot.memory {
            Section::Available(memory) => {
                let percent = memory.used_percent();
                let history = self.push("memory", publish::fraction(percent), advance);
                rows.push(Row {
                    key: "memory".to_owned(),
                    kind: Kind::Memory,
                    label: String::new(),
                    state: "ready",
                    reason: None,
                    load: publish::load_name(percent),
                    numbers: publish::memory_numbers(memory),
                    history,
                });
            }
            Section::Unavailable(reason) => rows.push(Row::unavailable(
                "memory".to_owned(),
                Kind::Memory,
                String::new(),
                reason.clone(),
            )),
        }
        seen.push("memory".to_owned());

        // GPU (absent is not a row)
        if let Some(section) = &snapshot.gpu {
            match section {
                Section::Available(reading) => {
                    let history = self.push("gpu", publish::fraction(reading.busy_percent), advance);
                    rows.push(Row {
                        key: "gpu".to_owned(),
                        kind: Kind::Gpu,
                        label: self.gpu_id.clone(),
                        state: "ready",
                        reason: None,
                        load: publish::load_name(reading.busy_percent),
                        numbers: publish::gpu_numbers(reading),
                        history,
                    });
                }
                Section::Unavailable(reason) => rows.push(Row::unavailable(
                    "gpu".to_owned(),
                    Kind::Gpu,
                    self.gpu_id.clone(),
                    reason.clone(),
                )),
            }
            seen.push("gpu".to_owned());
        }

        // Disks
        for disk in &snapshot.disks {
            let key = format!("disk:{}", disk.info.name);
            let label = if disk.info.model.is_empty() {
                disk.info.name.clone()
            } else {
                disk.info.model.clone()
            };
            match &disk.rate {
                Section::Available(rate) => {
                    let history = self.push_throughput(&key, *rate, advance);
                    rows.push(Row {
                        key: key.clone(),
                        kind: Kind::Disk,
                        label,
                        state: if rate.is_some() { "ready" } else { "waiting" },
                        reason: None,
                        load: "normal",
                        numbers: publish::disk_numbers(&disk.info, *rate),
                        history,
                    });
                }
                Section::Unavailable(reason) => {
                    rows.push(Row::unavailable(key.clone(), Kind::Disk, label, reason.clone()));
                }
            }
            seen.push(key);
        }

        // Interfaces
        for interface in &snapshot.interfaces {
            let key = format!("net:{}", interface.info.name);
            match &interface.rate {
                Section::Available(rate) => {
                    let history = self.push_throughput(&key, *rate, advance);
                    rows.push(Row {
                        key: key.clone(),
                        kind: Kind::Network,
                        label: interface.info.name.clone(),
                        state: if rate.is_some() { "ready" } else { "waiting" },
                        reason: None,
                        load: "normal",
                        numbers: publish::network_numbers(&interface.info, *rate),
                        history,
                    });
                }
                Section::Unavailable(reason) => rows.push(Row::unavailable(
                    key.clone(),
                    Kind::Network,
                    interface.info.name.clone(),
                    reason.clone(),
                )),
            }
            seen.push(key);
        }

        self.rings.retain(|key, _| seen.contains(key));
        rows
    }

    fn cpu_model_string(&self) -> String {
        self.cpu_model.to_string()
    }

    /// Pushes a fraction into a row's ring and answers its values.
    fn push(&mut self, key: &str, value: f32, advance: bool) -> Vec<f32> {
        let ring = self.rings.entry(key.to_owned()).or_default();
        if advance {
            ring.push(value);
        }
        ring.values()
    }

    /// Pushes a throughput into a row's ring and answers it scaled by its
    /// own peak, which is the only sensible ceiling for bytes per second.
    fn push_throughput(&mut self, key: &str, rate: Option<[f64; 2]>, advance: bool) -> Vec<f32> {
        let ring = self.rings.entry(key.to_owned()).or_default();
        if advance && rate.is_some() {
            ring.push(publish::throughput(rate));
        }
        ring.fractions()
    }

    fn peek(&self, key: &str) -> Vec<f32> {
        self.rings.get(key).map_or_else(|| Ring::new().values(), Ring::values)
    }
}
```

Notes for the implementer: `QString::to_string()` exists in cxx-qt-lib (`Display`); if the borrow checker objects to `self.push(...)` while `reading` borrows `snapshot`, it does not — `snapshot` is a separate parameter. If `entry().or_default()` needs `Ring: Default`, it is already implemented. `resources.rs` no longer carries the `language-contract: product-copy` marker: delete that first line.

No build yet.

---

### Task 10: The page weaves rows; detail by kind; per-core grid; the follow-ups; the one build

**Files:**
- Modify: `hematita/qml/components/PerformancePage.qml`, `ResourceDetail.qml`, `HistoryGraph.qml`, `NavItem.qml`, `NavStrip.qml`, `hematita/qml/Main.qml`, `hematita/src/main.rs`, `hematita/build.rs`
- Create: `hematita/qml/components/CoreGrid.qml`

- [ ] **Step 1: Rewrite `PerformancePage.qml`**

```qml
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// Rendimiento: every resource on the left, the chosen one on the right. Rows
// are woven from the adapter's index-aligned lists on each revision, and the
// selection is a key, so a disk that comes and goes never moves it. Every
// word a person reads is composed here from tokens.
Item {
    id: page

    // `resources` is taken: QQuickItem already owns that name for its
    // non-visual children.
    required property HematitaResources metrics

    property string selectedKey: "cpu"
    // The woven rows: { key, kind, label, state, reasonKind, reasonPath,
    // load, numbers, history }.
    property var rows: []
    readonly property var selectedRow: page.rowFor(page.selectedKey)
    readonly property real listFraction: 0.32

    function rowFor(key) {
        for (let index = 0; index < page.rows.length; ++index)
            if (page.rows[index].key === key)
                return page.rows[index]
        return null
    }

    function weave() {
        const keys = page.metrics.resourceKeys
        const kinds = page.metrics.resourceKinds
        const labels = page.metrics.resourceLabels
        const states = page.metrics.resourceStates
        const reasonKinds = page.metrics.resourceReasonKinds
        const reasonPaths = page.metrics.resourceReasonPaths
        const loads = page.metrics.resourceLoads
        const numbers = page.metrics.resourceNumbers
        const histories = page.metrics.resourceHistories
        // Defensive: a short column would mean a publication error, and fewer
        // rows are better than rows with undefined fields.
        const count = Math.min(keys.length, kinds.length, labels.length, states.length,
                               reasonKinds.length, reasonPaths.length, loads.length,
                               numbers.length, histories.length)
        const woven = []
        for (let index = 0; index < count; ++index)
            woven.push({ key: keys[index], kind: kinds[index], label: labels[index],
                         state: states[index], reasonKind: reasonKinds[index],
                         reasonPath: reasonPaths[index], load: loads[index],
                         numbers: numbers[index], history: histories[index] })
        page.rows = woven
        if (page.rowFor(page.selectedKey) === null && woven.length > 0)
            page.selectedKey = woven[0].key
    }

    Connections {
        target: page.metrics
        function onRevisionChanged() { page.weave() }
    }

    Component.onCompleted: page.weave()

    // ── Words ──────────────────────────────────────────────────────────
    function nameFor(row) {
        switch (row.kind) {
        case "cpu": return qsTr("Procesador")
        case "memory": return qsTr("Memoria")
        case "gpu": return qsTr("Gráfica")
        case "disk": return row.label
        case "network": return row.label.length > 0 ? row.label : qsTr("Red")
        }
        return row.key
    }

    function subtitleFor(row) {
        switch (row.kind) {
        case "cpu": return row.label
        case "gpu": return row.label.length > 0 ? "AMD " + row.label : ""
        case "disk": return row.key.substring(5)
        case "network": return row.numbers.length > 3 && row.numbers[3] === 1 ? qsTr("Inalámbrica") : qsTr("Cable")
        }
        return ""
    }

    function reasonFor(row) {
        switch (row.reasonKind) {
        case "unreadable": return qsTr("No se pudo leer %1").arg(row.reasonPath)
        case "malformed": return qsTr("Contenido inesperado en %1").arg(row.reasonPath)
        case "no-rate": return qsTr("Sin variación entre dos lecturas de %1").arg(row.reasonPath)
        }
        return ""
    }

    function percentText(value) {
        return Math.round(value) + " %"
    }

    function gib(kib) {
        return (kib / 1048576).toLocaleString(Qt.locale(), "f", 1) + " GiB"
    }

    function bytesText(bytes) {
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return Math.round(bytes) + " B"
    }

    function rateText(bytesPerSecond) {
        return page.bytesText(bytesPerSecond) + "/s"
    }

    // The value beside the name in the side list.
    function valueFor(row) {
        if (row.state === "unavailable") return qsTr("No disponible")
        if (row.state === "waiting") return "—"
        const n = row.numbers
        switch (row.kind) {
        case "cpu": return page.percentText(n[0])
        case "memory": return page.gib(n[0]) + " / " + page.gib(n[1])
        case "gpu": return page.percentText(n[0])
        case "disk": return "↓ " + page.rateText(n[0]) + "  ↑ " + page.rateText(n[1])
        case "network": return "↓ " + page.rateText(n[0]) + "  ↑ " + page.rateText(n[1])
        }
        return ""
    }

    // Flat list of alternating label, value strings for the detail.
    function factsFor(row) {
        if (row === null || row.state !== "ready") return []
        const n = row.numbers
        switch (row.kind) {
        case "cpu":
            return [qsTr("Uso"), page.percentText(n[0]),
                    qsTr("Frecuencia"), n[1] > 0 ? (n[1] / 1000).toLocaleString(Qt.locale(), "f", 2) + " GHz" : "—",
                    qsTr("Núcleos"), String(n[2])]
        case "memory":
            return [qsTr("En uso"), page.gib(n[0]),
                    qsTr("Total"), page.gib(n[1]),
                    qsTr("Intercambio"), page.gib(n[2]) + " / " + page.gib(n[3])]
        case "gpu":
            return [qsTr("Uso"), page.percentText(n[0]),
                    qsTr("Memoria ocupada"), page.percentText(n[1]),
                    qsTr("VRAM"), page.bytesText(n[2]) + " / " + page.bytesText(n[3]),
                    qsTr("GTT"), page.bytesText(n[4]) + " / " + page.bytesText(n[5]),
                    qsTr("Reloj"), n[6] > 0 ? Math.round(n[6]) + " MHz" : "—",
                    qsTr("Reloj de memoria"), n[7] > 0 ? Math.round(n[7]) + " MHz" : "—"]
        case "disk":
            return [qsTr("Lectura"), page.rateText(n[0]),
                    qsTr("Escritura"), page.rateText(n[1]),
                    qsTr("Capacidad"), page.bytesText(n[2]),
                    qsTr("Tipo"), n[3] === 1 ? qsTr("Disco mecánico") : qsTr("Estado sólido")]
        case "network":
            return [qsTr("Recibido"), page.rateText(n[0]),
                    qsTr("Enviado"), page.rateText(n[1]),
                    qsTr("Velocidad"), n[2] > 0 ? Math.round(n[2]) + " Mbit/s" : "—",
                    qsTr("Estado"), n[4] === 1 ? qsTr("Activa") : qsTr("Inactiva")]
        }
        return []
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceSm

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: CelestinaTheme.spaceLg

            CelestinaSurface {
                Layout.preferredWidth: page.width * page.listFraction
                Layout.fillHeight: true
                role: CelestinaSurface.Panel
                padding: CelestinaTheme.spaceXs

                contentItem: ListView {
                    id: list
                    clip: true
                    spacing: CelestinaTheme.spaceXs
                    model: page.rows
                    delegate: ResourceRow {
                        required property var modelData
                        width: list.width
                        name: page.nameFor(modelData)
                        value: page.valueFor(modelData)
                        series: modelData.history
                        load: modelData.load
                        selected: modelData.key === page.selectedKey
                        onClicked: page.selectedKey = modelData.key
                    }
                }
            }

            ResourceDetail {
                Layout.fillWidth: true
                Layout.fillHeight: true
                title: page.selectedRow ? page.nameFor(page.selectedRow) : ""
                subtitle: page.selectedRow ? page.subtitleFor(page.selectedRow) : ""
                series: page.selectedRow ? page.selectedRow.history : []
                load: page.selectedRow ? page.selectedRow.load : "normal"
                facts: page.factsFor(page.selectedRow)
                coreHistories: page.selectedRow && page.selectedRow.kind === "cpu"
                               ? page.metrics.cpuCoreHistories : []
                notice: page.selectedRow && page.selectedRow.state === "unavailable"
                        ? page.reasonFor(page.selectedRow) : ""
            }
        }

        // The failure line lives in the column, under the two panels, never
        // over them. The sampling thread failing to start is the one failure
        // no row can carry.
        Text {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            visible: page.metrics.startFailed
            text: qsTr("No se pudo iniciar la lectura del sistema")
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }
    }
}
```

`ListView` inside a `Pane`'s `contentItem`: the pane sizes its contentItem to fill; the list's `width` is what the delegates use.

- [ ] **Step 2: Extend `ResourceDetail.qml`**

Add two required properties and the grid toggle. Replace the file's property block and the `HistoryGraph` element with:

```qml
    required property string title
    required property string subtitle
    required property var series
    required property string load
    // Flat list of alternating label, value strings.
    required property var facts
    // One minute per core when the CPU is shown; empty otherwise.
    required property var coreHistories
    // Why this resource cannot be read, or empty.
    required property string notice

    property bool showCores: false
```

and, in the header `RowLayout`, before the subtitle `Text`, an icon toggle that only appears for a resource with cores:

```qml
            CelestinaIconButton {
                visible: detail.coreHistories.length > 0
                iconName: detail.showCores ? "gauge" : "view-grid"
                helpText: detail.showCores ? qsTr("Ver una gráfica") : qsTr("Ver por núcleo")
                role: CelestinaButton.Ghost
                checkable: true
                checked: detail.showCores
                onToggled: detail.showCores = checked
            }
```

then the graph area:

```qml
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: detail.showCores && detail.coreHistories.length > 0 ? 1 : 0

            HistoryGraph {
                series: detail.series
                load: detail.load
            }

            CoreGrid {
                histories: detail.coreHistories
                load: detail.load
            }
        }

        Text {
            Layout.fillWidth: true
            visible: detail.notice.length > 0
            text: detail.notice
            wrapMode: Text.Wrap
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
        }
```

Add `import QtQuick.Controls` if `CelestinaIconButton` needs it (it is a `CelestinaButton`, itself a `Button` — the import is on the shared file, so no).

- [ ] **Step 3: Write `CoreGrid.qml`**

```qml
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// One small graph per core, in a grid that fills the detail. The number of
// columns follows the width so eight cores read as two rows of four and
// sixteen as four of four.
Item {
    id: grid

    // A list of minute-long fraction series, one per core.
    required property var histories
    required property string load

    readonly property int columns: Math.max(1, Math.min(grid.histories.length,
                                                        Math.floor(grid.width / (CelestinaTheme.controlHeightXl * 3))))

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Historial por núcleo")

    GridLayout {
        anchors.fill: parent
        columns: grid.columns
        columnSpacing: CelestinaTheme.spaceSm
        rowSpacing: CelestinaTheme.spaceSm

        Repeater {
            model: grid.histories.length

            HistoryGraph {
                required property int index
                Layout.fillWidth: true
                Layout.fillHeight: true
                series: grid.histories[index]
                load: grid.load
            }
        }
    }
}
```

- [ ] **Step 4: `HistoryGraph.qml` — token and enter motion**

Replace `strokeWidth: 2` with `strokeWidth: CelestinaTheme.borderHairline * 2`. Add the enter motion: on the `Shape`, `opacity: graph.series.length > 1 ? 1 : 0` and

```qml
        Behavior on opacity {
            NumberAnimation {
                duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeStandard
            }
        }
```

and drop the `visible:` line (opacity carries it; a hidden shape with opacity 0 costs nothing to skip).

- [ ] **Step 5: `NavItem.qml` and `NavStrip.qml`**

`NavItem.qml`: delete the three lines `checkable: true`, `checked: item.current`, `autoExclusive: true`. `NavStrip.qml`: add `Accessible.name: qsTr("Secciones")` under `Accessible.role`.

- [ ] **Step 6: `main.rs` — environment before the bus**

Move the `QT_QPA_PLATFORMTHEME` block above `if activation::hand_off()`, keeping both comments; `hand_off()` creates a bus connection with its own thread, and `set_var` must not race it.

- [ ] **Step 7: `Main.qml`**

Nothing structural changes: `PerformancePage { metrics: machine }` stays. Remove nothing else.

- [ ] **Step 8: `build.rs`**

Add `"qml/components/CoreGrid.qml",` after `ResourceDetail.qml` in `QML_FILES`.

- [ ] **Step 9: The one build cycle**

```bash
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets && cargo clippy --release --all-targets --locked -- -D warnings && cargo fmt --all --check)
hematita/scripts/verify-production.sh
```

Expected: the build succeeds; `publish` tests pass (4); clippy clean; verify green with qmllint at 0 warnings (the row may not rise; if a new warning appears, fix its cause — a common one is an unqualified access inside the `ListView` delegate, cured by qualifying through `page.`/`modelData` as done above) and `smoke: OK`. If the bridge refuses `QStringList` as a `qproperty`, it does not: Fluorita uses the same shape. If `cargo fmt` reorders anything in `sampler.rs`, let it.

- [ ] **Step 10: Offscreen behaviour check without a window**

```bash
QT_QPA_PLATFORM=offscreen timeout 6 hematita/target/release/hematita 2>&1 | grep -E 'Error|Warning|unavailable' ; echo "rc=${PIPESTATUS[0]}"
```
Expected: nothing printed, `rc=124`.

- [ ] **Step 11: Close H2-B**

Evidence `hematita/docs/evidence/2026-09-22-h2-resources-page.md`: commands, test counts, qmllint line, smoke line, verify exit; a `Kind contract` section repeating the five number layouts; `Limits`: no visual check (VAL-H2); the network file-failure row convention (`net:` with an empty name). Ledger row H2-B `done`, roadmap row, STATUS (`Updated`, the list is dynamic now, the failure path is per row, the `product-copy` marker is gone from Rust). Inventory with every changed path (`hematita/src/{main,sampler,publish,resources}.rs`, `hematita/build.rs`, `hematita/qml/Main.qml` only if changed, the five QML files, `hematita/Cargo.lock` only if changed, evidence, ROADMAP, STATUS, plan), `check-staged-units.py`, the three guards, then:

```bash
git commit -m "$(cat <<'EOF'
hematita-maintenance: Add every resource to the Performance page with per-row availability

H2-B: snapshot sections for the GPU, each whole disk and each interface with
typed reasons; publish.rs holding the adapter's pure decisions under test;
HematitaResources as index-aligned lists with a revision ticket; the page
weaving rows and composing every word through qsTr(); kind-driven detail;
the per-core grid; the failure line in the layout; NavItem, NavStrip,
HistoryGraph and main.rs follow-ups from H1.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

### Task 11: Implementation exit — H2-Z

**Files:**
- Modify: `hematita/Cargo.toml`, `hematita/Cargo.lock`, `docs/version-history.tsv`, `hematita/ROADMAP.md`, `hematita/STATUS.md`, `hematita/AGENTS.md`, both plan READMEs
- Move: the plan to `hematita/docs/plans/archive/`
- Create: `hematita/docs/evidence/2026-09-22-h2-production-completion.md`

- [ ] **Step 1: Bump and complete**

```bash
python3 scripts/version_tool.py bump hematita milestone --unit H2-Z --summary "Add every resource to the Performance page with the per-core grid"
python3 scripts/version_tool.py check
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
sha256sum hematita/target/release/hematita ~/.local/bin/hematita
```
Expected: 0.2.0 → 0.3.0; build (relink), verify (cached), deploy; both hashes equal.

- [ ] **Step 2: Close the documents**

- `ROADMAP.md`: `Status: idle`, checkpoint `none`, H2 rows `done`, `## H2 — closed 2026-09-22` naming the result and the completion evidence; the H3–H5 rows stay.
- `STATUS.md`: `Updated`, `Delivered as 0.3.0: H2`, next phase H3 planned.
- `AGENTS.md`: the "Absence is a state" bullet now reads that every row carries its own state and typed reason and the page composes the sentence.
- Plan: `Closed: 2026-09-22`, `Successor: H3`, `Status: done`, H2-Z row `done`; `git mv` to `docs/plans/archive/`; READMEs (active: no plan; archive: list H1 and H2).
- Evidence: the completion command, the sealed manifest path `hematita/target/production-artifact.toml`, installed paths, the hash comparison, `VAL-H2` pending. No literal `*` in metadata fields.

- [ ] **Step 3: Inventory and commit**

```bash
python3 "$SCRATCH/mkinv.py" H2-Z \
  hematita/docs/inventories/2026-09-22-h2-resources/H2-Z.numstat.tsv \
  hematita/docs/plans/archive/2026-09-22-h2-resources.md \
  hematita/docs/plans/active/2026-09-22-h2-resources.md \
  hematita/Cargo.toml hematita/Cargo.lock docs/version-history.tsv \
  hematita/ROADMAP.md hematita/STATUS.md hematita/AGENTS.md \
  hematita/docs/plans/active/README.md hematita/docs/plans/archive/README.md \
  hematita/docs/evidence/2026-09-22-h2-production-completion.md
python3 scripts/check-staged-units.py hematita/docs/inventories/2026-09-22-h2-resources/H2-Z.numstat.tsv
python3 scripts/version_tool.py check && bash scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py
git commit -m "$(cat <<'EOF'
hematita-milestone: Add every resource to the Performance page with the per-core grid

H2-Z: the implementation exit of H2 — release built, verified and deployed to
the author's prefix at 0.3.0; roadmap and status closed; the plan archived.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

If the generator rejects the deleted active-plan path, add its row by hand as `0<TAB><lines><TAB>deleted<TAB>hematita/docs/plans/active/2026-09-22-h2-resources.md` (the H1-Z inventory shows the exact shape) and rerun the guard.

---

## Self-review

**Spec coverage (H2):** §3 sources for disks (`/proc/diskstats`, `/sys/block`), network (`/proc/net/dev`, `/sys/class/net`), GPU (`amdgpu` sysfs incl. `pp_dpm_*`) → Tasks 3–5, 7. §4 modules `disk`, `network`, `gpu` (plus `rate`, the real shared intersection of disk and network sampling) → Tasks 2–5. §5 resource list with kind/name/value/subtitle/load/sparkline and detail → Tasks 8–10; per-core histories → Tasks 7, 9, 10. §6 CPU toggle between single graph and per-core grid → Task 10 (`CoreGrid`, icon toggle). §7 H2 row (disks, network, GPU, swap, per-core grid) → all; swap stays in memory's numbers. H1's booked follow-ups (per-resource typed reason via `qsTr()`, banner in layout, `apply` pure parts tested, NavItem, graph token and motion, `generation` → replaced by `revision` which the page uses, NavStrip name, env before bus, rings aligned) → Tasks 8–10.

**Placeholder scan:** `DISKS`/`INTERFACES` literals are captured at execution with instructions; no TBD/TODO.

**Type consistency:** `Section<T>`/`Reason`/`ReasonKind` defined in Task 7 and consumed in Task 9; `CpuReading.core_percents` restored in Task 7 and read in Task 9; `DiskInfo`/`InterfaceInfo` fields match between Tasks 7, 8, 9; `publish::{accepts, fraction, ticket, load_name, Kind, *_numbers, throughput}` match between Tasks 8 and 9; `Ring::{values, fractions}` from Tasks 6/H1 match Task 9; QML property names (`resourceKeys` … `cpuCoreHistories`, `cpuModel`, `startFailed`, `revision`) are the camelCase of Task 9's snake_case and are what Task 10 reads; `ResourceDetail`'s new `coreHistories`/`notice` required properties are supplied by `PerformancePage`; `CoreGrid { histories, load }` matches its use.
