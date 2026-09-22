<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Hematita H3 — Processes and applications

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give Hematita its Processes and Applications pages — a sortable, searchable table of every process with CPU, memory and disk IO, terminate and kill for the user's own processes with a confirming dialog, and the same rows grouped under the desktop application that launched them — after first closing the list-operability gap H2 left on the Performance page.

**Architecture:** `hematita-core` gains a `process` module (pure parsers for `/proc/PID/{stat,status,cmdline,io,cgroup}`, a per-PID CPU sampler keyed by start time, the `app-*` scope decoder, a `passwd` parser and the sort/filter/group projection) — all text in, typed values out, fully tested. The sampler thread reads `/proc` every second tick on its own thread and ships a `ProcessSnapshot` inside the existing `Snapshot`. A new `HematitaProcesses` QObject publishes index-aligned lists plus a `revision` (the suite's list shape), owns the sort/filter/grouped state, and sends signals through `rustix` (safe Rust, already in the workspace) only to processes the current user owns. One `ProcessTable.qml` component serves both pages: flat for Processes, grouped with application icons for Applications. Application icons come from the desktop's icon theme through Qt's own `IconImage`, with a catalogue glyph as fallback.

**Tech Stack:** Rust 1.97.1 (pinned), cxx-qt 0.9.1 / cxx-qt-lib 0.9.1, `rustix = "1.1.4"` with the `process` feature (the pin `magnetitad` already carries), Qt 6.9+ QML, `celestina-style` symlinks.

**Spec:** [docs/superpowers/specs/2026-09-21-hematita-design.md](../specs/2026-09-21-hematita-design.md) §3 (process sources, application identity), §4 (`process` module), §5 (`processes.rs`), §6 (`ProcessPage`, `ApplicationsPage`), §7 (H3 row). Carry-overs: the H2 plan's list shape and the H1 plan's inventory generator and commit procedure ([2026-09-22-hematita-h2-resources.md](2026-09-22-hematita-h2-resources.md), [2026-09-21-hematita-h1-foundation.md](2026-09-21-hematita-h1-foundation.md)).

## Global Constraints

- **The Celestina shell is in standby.** Never read, reuse, modify or reference `celestina/` or `celestina-rs/crates/celestina-shell-core`.
- **Language contract:** identifiers, comments, docs, tests, script messages, commit subjects in English; product copy only as `qsTr()` literals in QML. No Rust file carries a user-visible string.
- **Build budget (author's rule):** three application build cycles in H3 — one at the end of H3-A, one at the end of H3-C (release build + release-profile tests/clippy + `verify-production.sh`), one inside `complete-production.sh` at H3-Z. H3-B is crate-only. Never `cargo` under `hematita/` outside those moments; never `cargo clean`; never a window on the live or nested session (appearance and interaction are `VAL-H3`).
- **Commits:** authorised per unit after review; `hematita:` prefix; inventories through the H1 plan's generator (scratchpad `mkinv.py`); evidence under `hematita/docs/evidence/`, inventories under `hematita/docs/inventories/2026-09-22-h3-processes/`. Hooks never bypassed; commit subjects start with a verb the hook accepts (`Add`, `Fix`, `Record`, `Update`, `Keep` — not `Book`).
- **Rust invariants:** no `unsafe`; no production `unwrap`/`expect`/`panic!`; typed errors; blocking IO never on the Qt thread; the sampler stops deterministically; snapshots applied whole, stale ones dropped; `rustix::process::kill_process` is the only signal path and is called only for a PID whose real uid equals the current user's, never for PID 1 or our own PID.
- **QML invariants:** every new file in `build.rs` `QML_FILES`; tokens only from `CelestinaTheme`; `required property`; no tooltips (`helpText` is an accessible name); keyboard and AT reachability; `reducedMotion` honoured; no `Canvas`; no lint suppressions; the `hematita` qmllint row is `0` and may not rise; icon-first actions with the shared hover circle; dialogs contain and restore focus (`CelestinaModalLayer`).
- **Constants once:** `INTERVAL` and `PROCESS_TICKS = 2` (processes are read every second tick) in `sampler.rs`; `HISTORY_SAMPLES` in `history.rs`; thresholds in `publish.rs`; `SECTOR_BYTES` in `disk.rs`; `CLK_TCK` is read once at startup from `sysconf` through `rustix::param::clock_ticks_per_second()`.
- **Process row contract (index-aligned lists on `HematitaProcesses`):** `processPids` (list of doubles), `processNames`, `processUsers`, `processCpuPercents`, `processMemoryKib`, `processReadRates`, `processWriteRates` (lists of doubles), `processApplications` (desktop id or `""`), `processGroupIndices` (index into the group lists, `-1` when flat or ungrouped), `processActionable` (list of `0`/`1`). Group contract: `groupIds`, `groupNames`, `groupIcons`, `groupCpuPercents`, `groupMemoryKib`, `groupCounts`. State: `sortField` (`cpu|memory|name|pid|read|write`), `sortAscending`, `filterText`, `grouped`, `revision`, `totalCount`, `shownCount`, `actionOutcome` (`""|done|refused|failed`), `actionPid`, `actionKind` (`terminate|kill`).
- **Reason for "not actionable":** a row whose uid is not ours is `actionable = 0`; the page says so in Spanish through `qsTr()` and H5 adds `pkexec`.

---

## File structure

| Path | Responsibility |
|---|---|
| `celestina-rs/crates/hematita-core/src/process.rs` | `/proc/PID` parsers: `stat`, `status`, `cmdline`, `io`, `cgroup`; `ApplicationScope`; `ProcessSampler` (CPU % per PID keyed by start time) |
| `celestina-rs/crates/hematita-core/src/passwd.rs` | `/etc/passwd` → uid to user name |
| `celestina-rs/crates/hematita-core/src/process_view.rs` | pure projection: filter by text, sort by field, group by application |
| `celestina-rs/crates/hematita-core/tests/fixtures/{proc-pid-stat.txt,proc-pid-status.txt,proc-pid-cgroup.txt,etc-passwd.txt}` | captures |
| `hematita/src/sampler.rs` | process sampling every `PROCESS_TICKS`, per-PID caches (cmdline, cgroup, uid), IO counters, `ProcessSnapshot` |
| `hematita/src/processes.rs` | `HematitaProcesses` QObject: lists + revision, sort/filter/grouped state, desktop-entry lookup cache, `terminate`/`kill` |
| `hematita/src/publish.rs` | unchanged shape; gains nothing (process projection lives in the crate) |
| `hematita/qml/components/ProcessTable.qml` | header + list + toolbar shared by both pages; `grouped` mode |
| `hematita/qml/components/ProcessHeader.qml` | sortable column titles |
| `hematita/qml/components/ProcessRow.qml` | one process row (Content family highlight) |
| `hematita/qml/components/ApplicationRow.qml` | one group row: icon, name, totals, expand chevron |
| `hematita/qml/components/KillDialog.qml` | the confirming dialog for kill |
| `hematita/qml/components/ProcessPage.qml`, `ApplicationsPage.qml` | thin pages over `ProcessTable` |
| `hematita/qml/components/PerformancePage.qml` | H3-A list operability |
| `hematita/scripts/smoke.sh` | H3-A shape assertion |
| `hematita/qml/Celestina{TextField,RowHighlight,ScrollBar,ModalLayer,InputShield,Shadow}.qml`, `Glass{Card,Surface}.qml` | new symlinks into `celestina-style` |
| `hematita/Cargo.toml` | `rustix` dependency, justified |

Ledger units:

| Unit | Kind | Content | Build |
|---|---|---|---|
| H3-A | `hematita-maintenance` | List operability and gates (booked in the roadmap): ListView focus/AT/currentIndex, smoke shape assertion, network subtitle, `no-rate` mapping, live core count, sysfs facts cached, capture test, dead arm | one |
| H3-B | `hematita-maintenance` | crate: `process`, `passwd`, `process_view`, captures | none |
| H3-C | `hematita-maintenance` | sampler process section, `HematitaProcesses`, `ProcessTable` and rows, `KillDialog`, both pages, symlinks | one |
| H3-Z | `hematita-milestone` | 0.4.0, `complete-production.sh`, documents closed, plan archived | one |

---

### Task 1: Open H3 in the documents

**Files:**
- Create: `hematita/docs/plans/active/2026-09-22-h3-processes.md`
- Modify: `hematita/ROADMAP.md`, `hematita/STATUS.md`, `hematita/VALIDATION.md`, `hematita/docs/plans/active/README.md`

No build; lands inside the H3-A commit.

- [ ] **Step 1: Write the plan ledger**

```markdown
# H3 — Processes and applications

- **Opened:** 2026-09-22
- **Plan ID:** h3-processes
- **Status:** active
- **Authorization:** the author asked to open the H3 plan on 2026-09-22
- **Scope:** hematita
- **Implementation checkpoint:** H3
- **Author-validation checkpoint:** `VAL-H3` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

Every process the kernel lists can be read, rated and grouped by the
application that launched it from `/proc` alone, on the sampler thread, and a
person can search, sort, terminate and kill their own processes from one
table that keeps its place while the numbers move.

## Tangible outcome

The installed 0.4.0 has a Processes page (search, sortable columns, terminate
and kill with a confirming dialog) and an Applications page (the same rows
under their application's icon and name), and the Performance list is
reachable by keyboard and screen reader.

## Scope

- `H3-A` — the list operability and gates booked by H2.
- `H3-B` — `hematita-core`: `process`, `passwd`, `process_view`, captures.
- `H3-C` — sampler process section, `HematitaProcesses`, the table, rows,
  dialog and both pages.
- `H3-Z` — implementation exit and 0.4.0.

## Exclusions

- Acting on other users' processes (H5, `pkexec`); services (H5); per-process
  network (never); process details beyond the columns (no tree, no threads,
  no open files).

## Build order

1. `H3-A`, then `H3-B`, `H3-C`, `H3-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds; the installed binary lists
processes with live CPU, memory and IO, sorts and filters them, groups them
by application, and terminates one of the user's own processes on request.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H3-A | `hematita:` | planned | `hematita/qml/`, `hematita/src/sampler.rs`, `hematita/scripts/smoke.sh`, `celestina-rs/crates/hematita-core/tests/`, documents | — | Performance list reachable by keyboard and AT with the selection bound to `currentIndex`; smoke asserting a row's numbers against the kind contract; network subtitle only when ready; `no-rate` mapping narrowed; live core count; static sysfs facts cached by name; capture test asserting the disk set; dead arm removed; H3 opened | `scripts/verify-production.sh` | `VAL-H2` |
| H3-B | `hematita:` | planned | `celestina-rs/crates/hematita-core/` | — | `/proc/PID` parsers, application scope decoding, per-PID CPU sampler, passwd, the filter/sort/group projection, captures | `cargo test -p hematita-core` | `VAL-H3` |
| H3-C | `hematita:` | planned | `hematita/src/`, `hematita/qml/`, `hematita/build.rs`, `hematita/Cargo.toml` | — | Process section in the snapshot with per-PID caches; `HematitaProcesses` lists, state and signals; `ProcessTable`, rows, `KillDialog`; Processes and Applications pages; style symlinks | `scripts/verify-production.sh` | `VAL-H3` |
| H3-Z | `hematita:` | planned | `hematita/`, `docs/version-history.tsv` | — | Implementation exit, 0.4.0, documents closed, plan archived | `scripts/complete-production.sh` | `VAL-H3` |
```

- [ ] **Step 2: Roadmap, STATUS, VALIDATION, README**

`ROADMAP.md`: `Status: active`, `Active implementation checkpoint: H3`; add rows `H3-A` (dependency H2-Z), `H3-B` (H3-A), `H3-C` (H3-B), `H3-Z` (H3-C) to the build-order table with the results above; replace `## H3 — planned first unit` with:

```markdown
## H3 — opened 2026-09-22

`H3-A` delivers the list operability and gates H2 booked; the process work
follows in `H3-B` and `H3-C`. Units and exit are in the
[active plan](docs/plans/active/2026-09-22-h3-processes.md).
```

`STATUS.md`: `Updated: 2026-09-22`, `Active phase: H3 (processes and applications), opened 2026-09-22`.

`VALIDATION.md` — append:

```markdown
## VAL-H3 — Processes and applications on the real session

- **Status:** pending
- **Related implementation:** H3
- **Requires:** the deployed Hematita 0.4.0 on the real session; a process of
  the author's to end (for example `sleep 600` in a terminal); one root
  process visible
- **Procedure:** open Procesos; type part of a name in the search and watch
  the table narrow; click each column title and confirm the order flips;
  select the `sleep` row and press Terminar; select another own process and
  press Matar, then cancel in the dialog with Escape, then confirm; select a
  root process and read the bar; walk the table by Tab and arrows and by
  screen reader; open Aplicaciones, confirm the running desktop applications
  appear with their icons and names, expand one and read its processes; leave
  the table open for a minute and confirm the selection and scroll position
  do not jump; check Hematita's own CPU while idle on Procesos
- **Pass condition:** the search narrows live; every column sorts both ways;
  Terminar ends `sleep` within a second; the dialog contains focus, Escape
  cancels, the confirm kills; a root process shows the not-actionable words
  and no dialog opens; every row and control is reachable by keyboard and
  named by the screen reader; every running desktop application has an icon
  (a missing icon is a `VAL` failure to record, not a crash); the table keeps
  its place across ticks; Hematita idles under 2 % CPU on Procesos
- **Result:** not run
- **Evidence:** none
```

`docs/plans/active/README.md`: the sentence "The active plan is H3 — processes and applications, which the project roadmap names as its active implementation checkpoint.", with the title as a Markdown link to `2026-09-22-h3-processes.md`.

- [ ] **Step 3: Documentation guard** — `bash scripts/check-documentation-contract.sh` → OK.

---

### Task 2: H3-A — the Performance list is operable, and the gates prove shape

**Files:**
- Modify: `hematita/qml/components/PerformancePage.qml`, `hematita/src/sampler.rs`, `hematita/scripts/smoke.sh`, `celestina-rs/crates/hematita-core/tests/captures.rs`

- [ ] **Step 1: The `ListView` becomes a real list**

In `PerformancePage.qml`, replace the `contentItem: ListView { … }` block with:

```qml
                contentItem: ListView {
                    id: list
                    clip: true
                    spacing: CelestinaTheme.spaceXs
                    model: page.rows.length
                    // The view is the keyboard's way in: Tab lands here, arrows
                    // move the selection, and the selection is the current
                    // item, so the screen reader follows it. Off-screen rows
                    // are reached by moving the current index, which scrolls.
                    activeFocusOnTab: true
                    keyNavigationEnabled: true
                    currentIndex: page.indexOf(page.selectedKey)
                    onCurrentIndexChanged: {
                        if (currentIndex >= 0 && currentIndex < page.rows.length)
                            page.selectedKey = page.rows[currentIndex].key
                    }
                    highlightFollowsCurrentItem: true
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Recursos")
                    delegate: ResourceRow {
                        id: resourceRow
                        required property int index
                        readonly property var row: page.rowAt(resourceRow.index)
                        width: list.width
                        name: page.nameFor(resourceRow.row)
                        value: page.valueFor(resourceRow.row)
                        series: resourceRow.row.history
                        load: resourceRow.row.load
                        selected: resourceRow.row.key === page.selectedKey
                        onClicked: page.selectedKey = resourceRow.row.key
                    }
                }
```

Add to the page's functions:

```qml
    function indexOf(key) {
        for (let index = 0; index < page.rows.length; ++index)
            if (page.rows[index].key === key)
                return index
        return -1
    }
```

`ResourceRow` keeps `focusPolicy: Qt.TabFocus` so a row can still be reached individually once realised; the view's own focus is what reaches the rest.

- [ ] **Step 2: Network subtitle only when ready**

In `subtitleFor`, the `network` case becomes:

```qml
        case "network": return row.state === "ready"
                               ? (row.numbers[3] === 1 ? qsTr("Inalámbrica") : qsTr("Cable"))
                               : ""
```

- [ ] **Step 3: `no-rate` only for the error that means it**

In `sampler.rs` `sample_cpu`, replace the `Err(_) =>` arm with two:

```rust
        Err(cpu::CpuError::NoElapsedTime) => {
            return Section::Unavailable(Reason {
                kind: ReasonKind::NoRate,
                path: STAT_PATH.to_owned(),
            })
        }
        Err(_) => return Section::Unavailable(malformed(path)),
```

- [ ] **Step 4: Live core count**

In `resources.rs` `rows_from`, the CPU `Available(Some(reading))` arm derives the count from the reading and resizes the rings when it changes:

```rust
                let cores = reading.core_percents.len();
                if self.core_rings.len() != cores {
                    self.core_rings = (0..cores).map(|_| Ring::new()).collect();
                    self.cpu_cores = cores;
                }
```

placed before the `zip` over `core_rings`, and `publish::cpu_numbers(reading, cores)`. The identity's `cpu_cores` stays as the initial value.

- [ ] **Step 5: Static sysfs facts cached by name**

In `sampler.rs`, add to `run`'s locals `let mut disk_facts: HashMap<String, DiskInfo> = HashMap::new();` and `let mut interface_facts: HashMap<String, InterfaceInfo> = HashMap::new();` (import `std::collections::HashMap`), pass them into `sample_disks`/`sample_interfaces`, and inside use `facts.entry(name.clone()).or_insert_with(|| disk_info(name)).clone()`; after building the rows, `facts.retain(|name, _| readings.iter().any(|(n, _)| n == name))`. For interfaces, `up` and `speed_mbit` change at runtime (a cable unplugged), so cache only `wireless` and the link-type check: keep `interface_info` reading `operstate` and `speed` every tick and cache the `is_shown_interface` verdict and `wireless` in a `HashMap<String, (bool, bool)>` (`shown`, `wireless`).

- [ ] **Step 6: Dead arm and capture test**

In `sample_gpu`, replace the `Ok(Err(_)) => return None` arm by making the conversion total: build `[String; 8]` directly with `let Ok(texts) = <[String; 8]>::try_from(texts) else { return Some(Section::Unavailable(malformed(&device.join(AMDGPU_FILES[0])))) };` — an eight-file read that does not yield eight strings is malformed, not absent.

In `tests/captures.rs`, tighten the diskstats test to the exact set:

```rust
    let names: Vec<&str> = disks.iter().map(|disk| disk.name.as_str()).collect();
    assert_eq!(names, vec!["nvme0n1", "nvme1n1", "sda", "sdb"]);
```

(replace the `DISKS` count assertion and the `contains('p')` line; keep the count constant if other tests use it).

- [ ] **Step 7: The smoke proves shape**

Add to `hematita/scripts/smoke.sh`, after the QML-error grep: an environment switch `HEMATITA_SMOKE_SHAPE=1` that `Main.qml` honours by printing one line after the third revision. In `Main.qml`, inside `HematitaResources { id: machine … }` add:

```qml
            // The smoke's shape gate: with HEMATITA_SMOKE_SHAPE set, print one
            // row's contract once, so a nested list that failed to convert is
            // a visible failure rather than an empty graph nobody saw.
            onRevisionChanged: {
                if (!window.smokeShape || machine.revision < 3 || window.shapePrinted)
                    return
                window.shapePrinted = true
                const numbers = machine.resourceNumbers
                const histories = machine.resourceHistories
                console.info("hematita-shape", machine.resourceKinds[0],
                             numbers.length > 0 ? numbers[0].length : -1,
                             histories.length > 0 ? histories[0].length : -1)
            }
```

with `required property bool smokeShape` and `property bool shapePrinted: false` on the window, set from `main.rs` like `reducedMotion` (`HEMATITA_SMOKE_SHAPE` present → true). In `smoke.sh`, export `HEMATITA_SMOKE_SHAPE=1` for the run, extend the timeout to 10 s, and after the error grep:

```sh
shape=$(grep -E '^hematita-shape ' "$log" | head -1 || true)
case "$shape" in
    "hematita-shape cpu 3 60") ;;
    *)
        echo "smoke: the first row did not publish the CPU contract (expected 'hematita-shape cpu 3 60'); got: '$shape'" >&2
        exit 1
        ;;
esac
```

`console.info` prints to stderr without a category prefix under `QT_ASSUME_STDERR_HAS_CONSOLE=1`; if the line arrives prefixed (`qml: hematita-shape …`), match `hematita-shape cpu 3 60$` with `grep -E` instead of the exact case.

- [ ] **Step 8: The one build cycle, evidence, close H3-A**

```bash
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets && cargo clippy --release --all-targets --locked -- -D warnings && cargo fmt --all --check)
(cd celestina-rs && cargo test -p hematita-core)
hematita/scripts/verify-production.sh
```

Expected: verify green, the smoke line now reads `smoke: OK — …` after the shape gate passed. Evidence `hematita/docs/evidence/2026-09-22-h3-list-operability.md` (commands, the shape line printed, qmllint 0, smoke, verify exit; Limits: keyboard walk not performed, `VAL-H2`). Ledger H3-A `done`, roadmap row, STATUS. Inventory `H3-A.numstat.tsv` with every changed path (plan, README, ROADMAP, STATUS, VALIDATION, evidence, `PerformancePage.qml`, `Main.qml`, `main.rs`, `sampler.rs`, `resources.rs`, `smoke.sh`, `captures.rs`), guards, commit:

```
hematita-maintenance: Fix the Performance list for keyboard and screen readers and gate the smoke on the row contract
```

---

### Task 3: `hematita-core::process` — the `/proc/PID` parsers

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/process.rs`
- Modify: `celestina-rs/crates/hematita-core/src/lib.rs` (`pub mod process;`)

**Interfaces:**
- `pub struct ProcessStat { pub pid: u32, pub comm: String, pub state: char, pub ppid: u32, pub cpu_ticks: u64, pub start_ticks: u64 }`; `pub fn parse_stat(text: &str) -> Result<ProcessStat, ProcessError>`
- `pub struct ProcessStatus { pub uid: u32, pub rss_kib: Option<u64>, pub threads: u32 }`; `pub fn parse_status(text: &str) -> Result<ProcessStatus, ProcessError>`
- `pub fn parse_cmdline(bytes: &[u8]) -> Vec<String>` (NUL-separated, lossy UTF-8, empty for a kernel thread)
- `pub struct ProcessIo { pub read_bytes: u64, pub write_bytes: u64 }`; `pub fn parse_io(text: &str) -> Result<ProcessIo, ProcessError>`
- `pub struct ApplicationScope { pub desktop_id: String, pub unit: String }`; `pub fn parse_cgroup(text: &str) -> Option<ApplicationScope>`
- `pub struct ProcessSampler` with `sample(&mut self, readings: &[(u32, u64, u64)], elapsed: Duration, clock_ticks: u64, cores: usize) -> Vec<(u32, f32)>` (`(pid, start_ticks, cpu_ticks)` in; percent of the whole machine out; a PID with a different start time is a new process)
- `pub enum ProcessError { NoCommand { line: String }, TooFewFields { line: String }, UnreadableNumber { field: &'static str }, MissingField(&'static str) }`

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-rs/crates/hematita-core/src/process.rs
//! One process, as `/proc/PID` describes it.
//!
//! `stat` is the awkward one: the command name sits in parentheses and may
//! itself contain spaces and parentheses, so the line is split at the *last*
//! `)` and everything after it is fields. CPU is a rate between two readings
//! keyed by the process's start time, because PIDs are reused and a new
//! process wearing an old PID must not inherit the old one's ticks.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessStat {
    pub pid: u32,
    pub comm: String,
    pub state: char,
    pub ppid: u32,
    /// user + system ticks since the process started.
    pub cpu_ticks: u64,
    /// Ticks after boot when the process started: its identity across PID reuse.
    pub start_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessStatus {
    /// The real uid — the first of the four `Uid:` values.
    pub uid: u32,
    /// `VmRSS` in kibibytes; kernel threads have none.
    pub rss_kib: Option<u64>,
    pub threads: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessIo {
    pub read_bytes: u64,
    pub write_bytes: u64,
}

/// The desktop application a process was launched under, from its systemd
/// scope: `app-<launcher->id-<n>.scope` or `app-id@instance.service`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationScope {
    /// The `.desktop` basename without the suffix, e.g. `com.slack.Slack`.
    pub desktop_id: String,
    /// The whole unit name, e.g. `app-flatpak-com.slack.Slack-1877727323.scope`.
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessError {
    NoCommand { line: String },
    TooFewFields { line: String },
    UnreadableNumber { field: &'static str },
    MissingField(&'static str),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCommand { line } => write!(formatter, "stat has no parenthesised command: {line}"),
            Self::TooFewFields { line } => write!(formatter, "stat line is too short: {line}"),
            Self::UnreadableNumber { field } => write!(formatter, "{field} is not a number"),
            Self::MissingField(field) => write!(formatter, "status has no {field}"),
        }
    }
}

impl std::error::Error for ProcessError {}

/// Parses `/proc/PID/stat`.
///
/// # Errors
///
/// No parenthesised command, too few fields after it, or a non-numeric field.
pub fn parse_stat(text: &str) -> Result<ProcessStat, ProcessError> {
    todo!()
}

/// Parses `/proc/PID/status` for the uid, resident size and thread count.
///
/// # Errors
///
/// Missing `Uid:` or `Threads:`, or a non-numeric value.
pub fn parse_status(text: &str) -> Result<ProcessStatus, ProcessError> {
    todo!()
}

/// Splits `/proc/PID/cmdline` on NUL. Lossy on purpose: a command line is
/// shown, never executed.
#[must_use]
pub fn parse_cmdline(bytes: &[u8]) -> Vec<String> {
    todo!()
}

/// Parses `/proc/PID/io` for the two byte counters that reach the disk.
///
/// # Errors
///
/// Missing `read_bytes:`/`write_bytes:` or a non-numeric value.
pub fn parse_io(text: &str) -> Result<ProcessIo, ProcessError> {
    todo!()
}

/// The application scope of `/proc/PID/cgroup`, if the process runs under
/// one. Recognised shapes:
/// `app-com.anthropic.Claude-893609.scope`, `app-flatpak-com.slack.Slack-1877727323.scope`,
/// `app-blueman@autostart.service`, `app-dbus-:1.3-org.freedesktop.portal@0.service`.
#[must_use]
pub fn parse_cgroup(text: &str) -> Option<ApplicationScope> {
    todo!()
}

/// Turns successive per-PID tick readings into CPU percentages of the whole
/// machine (every core together, so the column sums to the CPU row).
#[derive(Debug, Default)]
pub struct ProcessSampler {
    previous: HashMap<u32, (u64, u64)>,
}

impl ProcessSampler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `readings` are `(pid, start_ticks, cpu_ticks)`. A PID seen before with
    /// the same start time is rated; one with another start time is a new
    /// process and starts over; PIDs absent from `readings` are forgotten.
    pub fn sample(
        &mut self,
        readings: &[(u32, u64, u64)],
        elapsed: Duration,
        clock_ticks: u64,
        cores: usize,
    ) -> Vec<(u32, f32)> {
        todo!()
    }

    pub fn reset(&mut self) {
        self.previous.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "1967299 (zsh) S 908913 1967299 1967299 0 -1 4194304 283 887 0 0 5 7 0 0 12 -8 1 0 67467262 11894784 1108 18446744073709551615 0 0 0 0 0 0 2 4 134283265 1 0 0 17 3 0 0 0 0 0 0 0 0 0 0 0 0 0\n";
    const STATUS: &str = "Name:\tzsh\nState:\tS (sleeping)\nUid:\t1000\t1000\t1000\t1000\nVmRSS:\t    4736 kB\nThreads:\t1\n";

    #[test]
    fn stat_splits_at_the_last_parenthesis_and_reads_the_fields_after_it() {
        let stat = parse_stat(STAT).expect("readable stat");
        assert_eq!(stat.pid, 1967299);
        assert_eq!(stat.comm, "zsh");
        assert_eq!(stat.state, 'S');
        assert_eq!(stat.ppid, 908913);
        assert_eq!(stat.cpu_ticks, 12);
        assert_eq!(stat.start_ticks, 67467262);
    }

    #[test]
    fn a_command_with_spaces_and_parentheses_does_not_break_the_split() {
        let stat = parse_stat("42 (Web Content (x)) R 1 1 1 0 -1 0 0 0 0 0 3 4 0 0 0 0 1 0 100 0 0 0\n")
            .expect("readable stat");
        assert_eq!(stat.comm, "Web Content (x)");
        assert_eq!(stat.cpu_ticks, 7);
        assert_eq!(stat.start_ticks, 100);
    }

    #[test]
    fn a_stat_without_a_command_or_with_too_few_fields_is_refused() {
        assert!(matches!(parse_stat("42 zsh S\n"), Err(ProcessError::NoCommand { .. })));
        assert!(matches!(parse_stat("42 (zsh) S 1 2\n"), Err(ProcessError::TooFewFields { .. })));
        assert!(matches!(
            parse_stat("42 (zsh) S x 1 1 0 -1 0 0 0 0 0 3 4 0 0 0 0 1 0 100 0 0 0\n"),
            Err(ProcessError::UnreadableNumber { field: "ppid" })
        ));
    }

    #[test]
    fn status_reads_the_real_uid_the_resident_size_and_the_threads() {
        let status = parse_status(STATUS).expect("readable status");
        assert_eq!(status, ProcessStatus { uid: 1000, rss_kib: Some(4736), threads: 1 });
        let kernel = parse_status("Name:\tkthreadd\nUid:\t0\t0\t0\t0\nThreads:\t1\n").expect("readable");
        assert_eq!(kernel.rss_kib, None);
        assert_eq!(parse_status("Name:\tx\n"), Err(ProcessError::MissingField("Uid")));
    }

    #[test]
    fn cmdline_splits_on_nul_and_is_empty_for_a_kernel_thread() {
        assert_eq!(parse_cmdline(b"/usr/bin/zsh\0-c\0echo\0"), vec!["/usr/bin/zsh", "-c", "echo"]);
        assert_eq!(parse_cmdline(b""), Vec::<String>::new());
    }

    #[test]
    fn io_reads_the_two_disk_counters() {
        let io = parse_io("rchar: 1\nwchar: 2\nread_bytes: 4096\nwrite_bytes: 8192\n").expect("readable io");
        assert_eq!(io, ProcessIo { read_bytes: 4096, write_bytes: 8192 });
        assert_eq!(parse_io("rchar: 1\n"), Err(ProcessError::MissingField("read_bytes")));
    }

    #[test]
    fn cgroup_names_the_application_behind_every_scope_shape() {
        let prefix = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/";
        let scope = |unit: &str| parse_cgroup(&format!("{prefix}{unit}\n"));
        assert_eq!(
            scope("app-com.anthropic.Claude-893609.scope").map(|s| s.desktop_id),
            Some("com.anthropic.Claude".to_owned())
        );
        assert_eq!(
            scope("app-flatpak-com.slack.Slack-1877727323.scope").map(|s| s.desktop_id),
            Some("com.slack.Slack".to_owned())
        );
        assert_eq!(
            scope("app-blueman@autostart.service").map(|s| s.desktop_id),
            Some("blueman".to_owned())
        );
        assert_eq!(
            scope("app-dbus-:1.3-org.freedesktop.impl.portal.desktop.celestina@0.service").map(|s| s.desktop_id),
            Some("org.freedesktop.impl.portal.desktop.celestina".to_owned())
        );
        assert_eq!(
            scope("app-niri-kitty-4242.scope").map(|s| s.desktop_id),
            Some("kitty".to_owned())
        );
        let unit = scope("app-com.anthropic.Claude-893609.scope").expect("a scope");
        assert_eq!(unit.unit, "app-com.anthropic.Claude-893609.scope");
        assert_eq!(parse_cgroup("0::/user.slice/user-1000.slice/user@1000.service/session.slice/pipewire.service\n"), None);
        assert_eq!(parse_cgroup("0::/system.slice/sshd.service\n"), None);
        assert_eq!(parse_cgroup(""), None);
    }

    #[test]
    fn cpu_percent_is_the_share_of_the_whole_machine_between_two_readings() {
        let mut sampler = ProcessSampler::new();
        assert!(sampler.sample(&[(7, 100, 50)], Duration::from_secs(1), 100, 8).is_empty());
        // 50 more ticks in one second at 100 ticks/s on 8 cores: 50 / 800.
        let rates = sampler.sample(&[(7, 100, 100)], Duration::from_secs(1), 100, 8);
        assert_eq!(rates, vec![(7, 6.25)]);
    }

    #[test]
    fn a_reused_pid_with_another_start_time_starts_over() {
        let mut sampler = ProcessSampler::new();
        sampler.sample(&[(7, 100, 50)], Duration::from_secs(1), 100, 1);
        assert!(sampler.sample(&[(7, 900, 10)], Duration::from_secs(1), 100, 1).is_empty());
        let rates = sampler.sample(&[(7, 900, 60)], Duration::from_secs(1), 100, 1);
        assert_eq!(rates, vec![(7, 50.0)]);
    }

    #[test]
    fn degenerate_inputs_rate_nothing_or_saturate() {
        let mut sampler = ProcessSampler::new();
        sampler.sample(&[(7, 1, 0)], Duration::from_secs(1), 100, 1);
        assert!(sampler.sample(&[(7, 1, 10)], Duration::ZERO, 100, 1).is_empty());
        assert!(sampler.sample(&[(7, 1, 20)], Duration::from_secs(1), 0, 1).is_empty());
        let rates = sampler.sample(&[(7, 1, 20_000)], Duration::from_secs(1), 100, 1);
        assert_eq!(rates, vec![(7, 100.0)]);
        // Backwards ticks (should not happen) rate zero rather than wrapping.
        let rates = sampler.sample(&[(7, 1, 5)], Duration::from_secs(1), 100, 1);
        assert_eq!(rates, vec![(7, 0.0)]);
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cd celestina-rs && cargo test -p hematita-core process` → 10 failures.

- [ ] **Step 3: Implement**

```rust
pub fn parse_stat(text: &str) -> Result<ProcessStat, ProcessError> {
    let line = text.trim_end();
    let open = line.find('(').ok_or_else(|| ProcessError::NoCommand { line: line.to_owned() })?;
    let close = line.rfind(')').ok_or_else(|| ProcessError::NoCommand { line: line.to_owned() })?;
    if close < open {
        return Err(ProcessError::NoCommand { line: line.to_owned() });
    }
    let pid = line[..open]
        .trim()
        .parse::<u32>()
        .map_err(|_| ProcessError::UnreadableNumber { field: "pid" })?;
    let comm = line[open + 1..close].to_owned();
    // After the command: state ppid pgrp session tty tpgid flags minflt
    // cminflt majflt cmajflt utime stime cutime cstime priority nice
    // num_threads itrealvalue starttime …  (fields 3.. in proc(5) numbering)
    let fields: Vec<&str> = line[close + 1..].split_whitespace().collect();
    if fields.len() < 20 {
        return Err(ProcessError::TooFewFields { line: line.to_owned() });
    }
    let number = |index: usize, field: &'static str| -> Result<u64, ProcessError> {
        fields[index]
            .parse::<u64>()
            .map_err(|_| ProcessError::UnreadableNumber { field })
    };
    let state = fields[0].chars().next().unwrap_or('?');
    let ppid = u32::try_from(number(1, "ppid")?).map_err(|_| ProcessError::UnreadableNumber { field: "ppid" })?;
    let utime = number(11, "utime")?;
    let stime = number(12, "stime")?;
    let start_ticks = number(19, "starttime")?;
    Ok(ProcessStat {
        pid,
        comm,
        state,
        ppid,
        cpu_ticks: utime.saturating_add(stime),
        start_ticks,
    })
}

pub fn parse_status(text: &str) -> Result<ProcessStatus, ProcessError> {
    let field = |name: &'static str| -> Option<&str> {
        text.lines()
            .find_map(|line| line.strip_prefix(name).and_then(|rest| rest.strip_prefix(':')))
            .map(str::trim)
    };
    let uid = field("Uid")
        .ok_or(ProcessError::MissingField("Uid"))?
        .split_whitespace()
        .next()
        .ok_or(ProcessError::MissingField("Uid"))?
        .parse::<u32>()
        .map_err(|_| ProcessError::UnreadableNumber { field: "Uid" })?;
    let threads = field("Threads")
        .ok_or(ProcessError::MissingField("Threads"))?
        .parse::<u32>()
        .map_err(|_| ProcessError::UnreadableNumber { field: "Threads" })?;
    let rss_kib = match field("VmRSS") {
        Some(value) => Some(
            value
                .split_whitespace()
                .next()
                .and_then(|digits| digits.parse::<u64>().ok())
                .ok_or(ProcessError::UnreadableNumber { field: "VmRSS" })?,
        ),
        None => None,
    };
    Ok(ProcessStatus { uid, rss_kib, threads })
}

pub fn parse_cmdline(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

pub fn parse_io(text: &str) -> Result<ProcessIo, ProcessError> {
    let field = |name: &'static str| -> Result<u64, ProcessError> {
        text.lines()
            .find_map(|line| line.strip_prefix(name).and_then(|rest| rest.strip_prefix(':')))
            .ok_or(ProcessError::MissingField(name))?
            .trim()
            .parse::<u64>()
            .map_err(|_| ProcessError::UnreadableNumber { field: name })
    };
    Ok(ProcessIo { read_bytes: field("read_bytes")?, write_bytes: field("write_bytes")? })
}

pub fn parse_cgroup(text: &str) -> Option<ApplicationScope> {
    let path = text
        .lines()
        .find_map(|line| line.strip_prefix("0::"))?;
    let unit = path
        .rsplit('/')
        .find(|component| component.starts_with("app-") && (component.ends_with(".scope") || component.ends_with(".service")))?;
    let mut id = unit.strip_prefix("app-")?;
    let is_service = id.ends_with(".service");
    id = id.strip_suffix(".scope").or_else(|| id.strip_suffix(".service"))?;
    if is_service {
        // `id@instance` and `dbus-:1.3-id@0`
        id = id.split('@').next()?;
        if let Some(rest) = id.strip_prefix("dbus-") {
            id = rest.split_once('-').map_or(rest, |(_, name)| name);
        }
    } else {
        // `[launcher-]id-<digits>`: the trailing number is the instance.
        if let Some((head, tail)) = id.rsplit_once('-') {
            if !tail.is_empty() && tail.bytes().all(|byte| byte.is_ascii_digit()) {
                id = head;
            }
        }
        // A launcher prefix is a plain word before a dotted or plain id:
        // `flatpak-com.slack.Slack`, `niri-kitty`. A reverse-DNS id has no
        // launcher when its first segment already contains a dot.
        if let Some((head, tail)) = id.split_once('-') {
            if !head.contains('.') && !tail.is_empty() {
                id = tail;
            }
        }
    }
    if id.is_empty() {
        return None;
    }
    Some(ApplicationScope { desktop_id: id.to_owned(), unit: unit.to_owned() })
}

impl ProcessSampler {
    pub fn sample(
        &mut self,
        readings: &[(u32, u64, u64)],
        elapsed: Duration,
        clock_ticks: u64,
        cores: usize,
    ) -> Vec<(u32, f32)> {
        let capacity = elapsed.as_secs_f64() * clock_ticks as f64 * cores.max(1) as f64;
        let mut rates = Vec::new();
        let mut current = HashMap::with_capacity(readings.len());
        for &(pid, start, ticks) in readings {
            if capacity > 0.0 {
                if let Some(&(previous_start, previous_ticks)) = self.previous.get(&pid) {
                    if previous_start == start {
                        let delta = ticks.saturating_sub(previous_ticks) as f64;
                        let percent = (delta / capacity * 100.0).clamp(0.0, 100.0);
                        rates.push((pid, percent as f32));
                    }
                }
            }
            current.insert(pid, (start, ticks));
        }
        self.previous = current;
        rates
    }
}
```

- [ ] **Step 4: Run tests, fmt, clippy** — `cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings` → pass, clean.

---

### Task 4: `hematita-core::passwd` and `process_view`

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/passwd.rs`, `celestina-rs/crates/hematita-core/src/process_view.rs`
- Modify: `lib.rs` (`pub mod passwd; pub mod process_view;`)

**Interfaces:**
- `passwd::parse(text: &str) -> HashMap<u32, String>` (uid → login name; malformed lines skipped).
- `process_view::ProcessRow { pub pid: u32, pub name: String, pub uid: u32, pub cpu_percent: f32, pub memory_kib: u64, pub read_rate: f64, pub write_rate: f64, pub application: Option<String>, pub actionable: bool }`
- `process_view::SortField { Cpu, Memory, Name, Pid, Read, Write }` with `from_token(&str) -> Option<Self>` and `as_str()`.
- `process_view::project(rows: &[ProcessRow], filter: &str, field: SortField, ascending: bool) -> Vec<usize>` (indices into `rows`, filtered by case-insensitive substring on name, pid text or application, then sorted; ties by pid ascending).
- `process_view::Group { pub id: String, pub member_indices: Vec<usize>, pub cpu_percent: f32, pub memory_kib: u64 }`; `process_view::group(rows: &[ProcessRow], order: &[usize]) -> Vec<Group>` (groups in first-appearance order of `order`; rows without an application form no group).

- [ ] **Step 1: Write both files test-first**

```rust
// celestina-rs/crates/hematita-core/src/passwd.rs
//! `/etc/passwd`, for the one thing the table needs from it: a login name
//! for a uid. Anything else on a line is ignored; a malformed line is skipped
//! rather than refused, because one bad entry must not blank every user.

use std::collections::HashMap;

#[must_use]
pub fn parse(text: &str) -> HashMap<u32, String> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split(':');
            let name = fields.next()?;
            let _password = fields.next()?;
            let uid = fields.next()?.parse::<u32>().ok()?;
            (!name.is_empty()).then(|| (uid, name.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn names_are_keyed_by_uid_and_bad_lines_are_skipped() {
        let users = parse("root:x:0:0:root:/root:/bin/bash\ntoni:x:1000:1000::/home/toni:/bin/zsh\nbroken\n:x:5:5::/:/bin/false\n");
        assert_eq!(users.get(&0).map(String::as_str), Some("root"));
        assert_eq!(users.get(&1000).map(String::as_str), Some("toni"));
        assert_eq!(users.len(), 2);
    }
}
```

```rust
// celestina-rs/crates/hematita-core/src/process_view.rs
//! What the table shows, decided without Qt: which rows survive the search,
//! in what order, and how they gather under their applications.

use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub uid: u32,
    pub cpu_percent: f32,
    pub memory_kib: u64,
    pub read_rate: f64,
    pub write_rate: f64,
    pub application: Option<String>,
    pub actionable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortField {
    Cpu,
    Memory,
    Name,
    Pid,
    Read,
    Write,
}

impl SortField {
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "cpu" => Some(Self::Cpu),
            "memory" => Some(Self::Memory),
            "name" => Some(Self::Name),
            "pid" => Some(Self::Pid),
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Name => "name",
            Self::Pid => "pid",
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

/// Indices of the rows that match `filter` (case-insensitive substring of the
/// name, the PID as text or the application id; empty matches all), sorted by
/// `field`; ties break by PID ascending so the order is stable across ticks.
#[must_use]
pub fn project(rows: &[ProcessRow], filter: &str, field: SortField, ascending: bool) -> Vec<usize> {
    let needle = filter.trim().to_lowercase();
    let mut order: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            needle.is_empty()
                || row.name.to_lowercase().contains(&needle)
                || row.pid.to_string().contains(&needle)
                || row.application.as_deref().is_some_and(|app| app.to_lowercase().contains(&needle))
        })
        .map(|(index, _)| index)
        .collect();
    order.sort_by(|&a, &b| {
        let (left, right) = (&rows[a], &rows[b]);
        let primary = match field {
            SortField::Cpu => left.cpu_percent.partial_cmp(&right.cpu_percent).unwrap_or(Ordering::Equal),
            SortField::Memory => left.memory_kib.cmp(&right.memory_kib),
            SortField::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            SortField::Pid => left.pid.cmp(&right.pid),
            SortField::Read => left.read_rate.partial_cmp(&right.read_rate).unwrap_or(Ordering::Equal),
            SortField::Write => left.write_rate.partial_cmp(&right.write_rate).unwrap_or(Ordering::Equal),
        };
        let primary = if ascending { primary } else { primary.reverse() };
        primary.then_with(|| left.pid.cmp(&right.pid))
    });
    order
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub id: String,
    pub member_indices: Vec<usize>,
    pub cpu_percent: f32,
    pub memory_kib: u64,
}

/// Gathers the rows in `order` under their application, groups in the order
/// their first member appears. Rows with no application form no group.
#[must_use]
pub fn group(rows: &[ProcessRow], order: &[usize]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for &index in order {
        let Some(application) = rows[index].application.as_deref() else {
            continue;
        };
        let row = &rows[index];
        match groups.iter_mut().find(|group| group.id == application) {
            Some(group) => {
                group.member_indices.push(index);
                group.cpu_percent += row.cpu_percent;
                group.memory_kib = group.memory_kib.saturating_add(row.memory_kib);
            }
            None => groups.push(Group {
                id: application.to_owned(),
                member_indices: vec![index],
                cpu_percent: row.cpu_percent,
                memory_kib: row.memory_kib,
            }),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, name: &str, cpu: f32, memory: u64, app: Option<&str>) -> ProcessRow {
        ProcessRow {
            pid,
            name: name.to_owned(),
            uid: 1000,
            cpu_percent: cpu,
            memory_kib: memory,
            read_rate: 0.0,
            write_rate: f64::from(pid),
            application: app.map(str::to_owned),
            actionable: true,
        }
    }

    fn rows() -> Vec<ProcessRow> {
        vec![
            row(10, "kitty", 1.0, 500, Some("kitty")),
            row(20, "Firefox", 30.0, 9000, Some("firefox")),
            row(30, "cargo", 30.0, 3000, None),
            row(40, "Web Content", 5.0, 4000, Some("firefox")),
        ]
    }

    #[test]
    fn the_filter_matches_name_pid_and_application_case_insensitively() {
        let rows = rows();
        assert_eq!(project(&rows, "fire", SortField::Pid, true), vec![1, 3]);
        assert_eq!(project(&rows, "30", SortField::Pid, true), vec![2]);
        assert_eq!(project(&rows, "WEB", SortField::Pid, true), vec![3]);
        assert_eq!(project(&rows, "", SortField::Pid, true), vec![0, 1, 2, 3]);
        assert_eq!(project(&rows, "nothing", SortField::Pid, true), Vec::<usize>::new());
    }

    #[test]
    fn sorting_honours_the_field_and_direction_and_breaks_ties_by_pid() {
        let rows = rows();
        assert_eq!(project(&rows, "", SortField::Cpu, false), vec![1, 2, 3, 0]);
        assert_eq!(project(&rows, "", SortField::Cpu, true), vec![0, 3, 1, 2]);
        assert_eq!(project(&rows, "", SortField::Name, true), vec![2, 1, 0, 3]);
        assert_eq!(project(&rows, "", SortField::Memory, false), vec![1, 3, 2, 0]);
        assert_eq!(project(&rows, "", SortField::Write, false), vec![3, 2, 1, 0]);
    }

    #[test]
    fn groups_follow_first_appearance_and_sum_their_members() {
        let rows = rows();
        let order = project(&rows, "", SortField::Cpu, false);
        let groups = group(&rows, &order);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].id, "firefox");
        assert_eq!(groups[0].member_indices, vec![1, 3]);
        assert_eq!(groups[0].cpu_percent, 35.0);
        assert_eq!(groups[0].memory_kib, 13000);
        assert_eq!(groups[1].id, "kitty");
    }

    #[test]
    fn sort_tokens_round_trip() {
        for field in [SortField::Cpu, SortField::Memory, SortField::Name, SortField::Pid, SortField::Read, SortField::Write] {
            assert_eq!(SortField::from_token(field.as_str()), Some(field));
        }
        assert_eq!(SortField::from_token("bogus"), None);
    }
}
```

- [ ] **Step 2: Run tests, fmt, clippy** — all pass (these files ship with their implementation; RED is not required for code whose tests were written in the same step — say so in the evidence).

---

### Task 5: Captures and the H3-B unit

**Files:**
- Create: `tests/fixtures/proc-pid-stat.txt`, `proc-pid-status.txt`, `proc-pid-cgroup.txt`, `etc-passwd.txt`
- Modify: `tests/captures.rs`

- [ ] **Step 1: Capture from the running session**

```bash
F=celestina-rs/crates/hematita-core/tests/fixtures
cat /proc/self/stat > $F/proc-pid-stat.txt
cat /proc/self/status > $F/proc-pid-status.txt
cat /proc/self/cgroup > $F/proc-pid-cgroup.txt
grep -E '^(root|toni):' /etc/passwd > $F/etc-passwd.txt
```

The captures are of the shell that ran the command; the cgroup capture carries an `app-…` scope only when the shell was launched under the desktop (the author's terminal is), so check `grep -c app- $F/proc-pid-cgroup.txt` prints 1 before writing the test below; if it prints 0, capture `/proc/<pid of a desktop app>/cgroup` instead (pick a PID from `grep -l app- /proc/[0-9]*/cgroup | head -1`).

- [ ] **Step 2: Capture tests**

```rust
use hematita_core::passwd;
use hematita_core::process::{parse_cgroup, parse_stat, parse_status};

const PID_STAT: &str = include_str!("fixtures/proc-pid-stat.txt");
const PID_STATUS: &str = include_str!("fixtures/proc-pid-status.txt");
const PID_CGROUP: &str = include_str!("fixtures/proc-pid-cgroup.txt");
const PASSWD: &str = include_str!("fixtures/etc-passwd.txt");

#[test]
fn the_captured_process_files_parse_and_agree_on_the_pid() {
    let stat = parse_stat(PID_STAT).expect("the captured stat parses");
    let status = parse_status(PID_STATUS).expect("the captured status parses");
    assert!(stat.pid > 1);
    assert_eq!(status.uid, 1000);
    assert!(status.threads >= 1);
    assert!(status.rss_kib.is_some());
}

#[test]
fn the_captured_cgroup_names_a_desktop_application() {
    let scope = parse_cgroup(PID_CGROUP).expect("the shell ran under an app scope");
    assert!(!scope.desktop_id.is_empty());
    assert!(scope.unit.starts_with("app-"));
}

#[test]
fn the_captured_passwd_names_root_and_the_author() {
    let users = passwd::parse(PASSWD);
    assert_eq!(users.get(&0).map(String::as_str), Some("root"));
    assert_eq!(users.get(&1000).map(String::as_str), Some("toni"));
}
```

- [ ] **Step 3: Whole crate green, close H3-B**

`cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings`. Evidence `2026-09-22-h3-core.md`; ledger/roadmap/STATUS; inventory `H3-B.numstat.tsv` (lib.rs, process.rs, passwd.rs, process_view.rs, captures.rs, four fixtures, evidence, ROADMAP, STATUS, plan); guards; commit:

```
hematita-maintenance: Add the process parsers, the application scope decoder and the table projection
```

---

### Task 6: The sampler reads processes

**Files:**
- Modify: `hematita/src/sampler.rs`, `hematita/Cargo.toml`, `hematita/Cargo.lock`

**Interfaces:**
- `pub const PROCESS_TICKS: u64 = 2;`
- `pub struct ProcessReading { pub pid: u32, pub name: String, pub uid: u32, pub cpu_percent: Option<f32>, pub memory_kib: u64, pub io_rate: Option<[f64; 2]>, pub application: Option<String> }`
- `pub struct ProcessSnapshot { pub own_uid: u32, pub clock_ticks: u64, pub readings: Vec<ProcessReading>, pub users: HashMap<u32, String> }` (users read once at start; refreshed never — a new user during a session is rare and shows as a number).
- `Snapshot.processes: Option<Section<ProcessSnapshot>>` — `None` on ticks that did not read processes.

- [ ] **Step 1: Dependency**

`hematita/Cargo.toml`:

```toml
# Sending SIGTERM/SIGKILL and reading the clock tick rate without `unsafe`:
# rustix wraps the syscalls in safe Rust and is already pinned in the
# workspace by magnetitad at this exact version.
rustix = { version = "1.1.4", features = ["process", "param"] }
```

`cargo update -p rustix --precise 1.1.4` is unnecessary if the lock already holds 1.1.4 via the workspace; otherwise `cargo generate-lockfile` is not allowed (it would move every crate) — use `cargo update -p rustix --precise 1.1.4` and nothing else. Do this edit before the single build so the lock lands in the same build cycle.

- [ ] **Step 2: The process section**

```rust
use std::collections::HashMap;

use hematita_core::process::{self, ProcessSampler};
use hematita_core::passwd;

/// Processes are read every second tick: two thousand PIDs are two thousand
/// directories, and a table nobody reads at a glance does not need the
/// cadence a graph does.
pub const PROCESS_TICKS: u64 = 2;

const PROC_ROOT: &str = "/proc";
const PASSWD_PATH: &str = "/etc/passwd";

#[derive(Clone, Debug)]
pub struct ProcessReading {
    pub pid: u32,
    pub name: String,
    pub uid: u32,
    /// `None` until the second reading of this PID.
    pub cpu_percent: Option<f32>,
    pub memory_kib: u64,
    /// `[read, write]` bytes per second; `None` for another user's process
    /// (the kernel refuses `io`) or before the second reading.
    pub io_rate: Option<[f64; 2]>,
    pub application: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProcessSnapshot {
    pub own_uid: u32,
    pub readings: Vec<ProcessReading>,
    pub users: HashMap<u32, String>,
}

/// What does not change while a process lives, read once per (pid, start).
#[derive(Clone, Debug)]
struct ProcessFacts {
    start_ticks: u64,
    name: String,
    uid: u32,
    application: Option<String>,
}

struct ProcessState {
    sampler: ProcessSampler,
    io: NamedCounters<2>,
    facts: HashMap<u32, ProcessFacts>,
    users: HashMap<u32, String>,
    own_uid: u32,
    clock_ticks: u64,
}

impl ProcessState {
    fn new() -> Self {
        let users = read(Path::new(PASSWD_PATH)).map(|text| passwd::parse(&text)).unwrap_or_default();
        let own_uid = rustix::process::getuid().as_raw();
        let clock_ticks = u64::from(rustix::param::clock_ticks_per_second());
        Self {
            sampler: ProcessSampler::new(),
            io: NamedCounters::new(),
            facts: HashMap::new(),
            users,
            own_uid,
            clock_ticks,
        }
    }
}

/// The name shown for a process: the first word of its command line when it
/// has one (the binary's basename), else the kernel's `comm`.
fn display_name(comm: &str, cmdline: &[String]) -> String {
    cmdline
        .first()
        .and_then(|argument| argument.rsplit('/').next())
        .filter(|name| !name.is_empty())
        .map_or_else(|| comm.to_owned(), str::to_owned)
}

fn sample_processes(state: &mut ProcessState, elapsed: Duration, cores: usize) -> Section<ProcessSnapshot> {
    let root = Path::new(PROC_ROOT);
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => {
            state.sampler.reset();
            state.io.reset();
            return Section::Unavailable(Reason { kind: ReasonKind::Unreadable, path: PROC_ROOT.to_owned() });
        }
    };
    let mut ticks = Vec::new();
    let mut io_readings = Vec::new();
    let mut partial = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<u32>().ok()) else {
            continue;
        };
        let dir = entry.path();
        // A process may vanish between readdir and read: skip it silently.
        let Ok(stat_text) = read(&dir.join("stat")) else { continue };
        let Ok(stat) = process::parse_stat(&stat_text) else { continue };
        let facts = match state.facts.get(&pid) {
            Some(facts) if facts.start_ticks == stat.start_ticks => facts.clone(),
            _ => {
                let Ok(status_text) = read(&dir.join("status")) else { continue };
                let Ok(status) = process::parse_status(&status_text) else { continue };
                let cmdline = std::fs::read(dir.join("cmdline")).map(|bytes| process::parse_cmdline(&bytes)).unwrap_or_default();
                let application = read(&dir.join("cgroup")).ok().and_then(|text| process::parse_cgroup(&text)).map(|scope| scope.desktop_id);
                let facts = ProcessFacts {
                    start_ticks: stat.start_ticks,
                    name: display_name(&stat.comm, &cmdline),
                    uid: status.uid,
                    application,
                };
                state.facts.insert(pid, facts.clone());
                facts
            }
        };
        // Resident size changes every tick; it is the one status field read again.
        let memory_kib = read(&dir.join("status"))
            .ok()
            .and_then(|text| process::parse_status(&text).ok())
            .and_then(|status| status.rss_kib)
            .unwrap_or(0);
        if facts.uid == state.own_uid {
            if let Ok(io) = read(&dir.join("io")).and_then(|text| process::parse_io(&text).map_err(|_| malformed(&dir.join("io")))) {
                io_readings.push((pid.to_string(), [io.read_bytes, io.write_bytes]));
            }
        }
        ticks.push((pid, stat.start_ticks, stat.cpu_ticks));
        partial.push((pid, facts, memory_kib));
    }
    let live: Vec<u32> = partial.iter().map(|(pid, _, _)| *pid).collect();
    state.facts.retain(|pid, _| live.contains(pid));
    let cpu: HashMap<u32, f32> = state.sampler.sample(&ticks, elapsed, state.clock_ticks, cores).into_iter().collect();
    let io: HashMap<String, [f64; 2]> = state.io.sample(&io_readings, elapsed).into_iter().collect();
    let readings = partial
        .into_iter()
        .map(|(pid, facts, memory_kib)| ProcessReading {
            pid,
            name: facts.name,
            uid: facts.uid,
            cpu_percent: cpu.get(&pid).copied(),
            memory_kib,
            io_rate: io.get(&pid.to_string()).copied(),
            application: facts.application,
        })
        .collect();
    Section::Available(ProcessSnapshot { own_uid: state.own_uid, readings, users: state.users.clone() })
}
```

Two `status` reads per tick for a known PID would be wasteful; read `status` once per PID per tick and use it for both facts (when new) and `memory_kib` — restructure the block so `status_text` is read once before the `facts` match. The `elapsed` passed to `sample_processes` is the time since the *previous process read* (keep a separate `last_process` `Instant` in `run`), not since the last tick.

In `run`: `let mut processes = ProcessState::new(); let mut last_process = Instant::now();` and, in the loop, `processes: if generation % PROCESS_TICKS == 0 { let now = Instant::now(); let elapsed = now.duration_since(last_process); last_process = now; Some(sample_processes(&mut processes, elapsed, core_count)) } else { None }` where `core_count` is the last CPU reading's `core_percents.len()` (keep a `let mut core_count = 1usize;` updated from `sample_cpu`'s result when available). Add `pub processes: Option<Section<ProcessSnapshot>>` to `Snapshot`. `ProcessSnapshot.users` cloning every two seconds is a few hundred strings; acceptable, or wrap in `Arc<HashMap<…>>` — do the `Arc`.

---

### Task 7: `HematitaProcesses`

**Files:**
- Create: `hematita/src/processes.rs`
- Modify: `hematita/src/main.rs` (`mod processes;`), `hematita/build.rs` (`.files([…, "src/processes.rs"])`), `hematita/src/resources.rs` (forward the process section)

Rather than a second sampler, `HematitaResources` keeps the single thread and hands each `Snapshot.processes` to a `HematitaProcesses` the window registers with it: add `#[qinvokable] fn attach_processes(self: Pin<&mut HematitaResources>, processes: *mut HematitaProcesses)` — CXX-Qt raw QObject pointers across bridges are awkward; instead use a signal: `HematitaResources` gains `#[qsignal] fn processes_published(self: Pin<&mut Self>, generation: i32)` and a `#[qproperty(QVariant, process_payload)]`… that is also awkward. **Decision:** `HematitaProcesses` owns its own reader: a second `Sampler`-like thread is exactly the duplication the architecture forbids. So the single sampler thread stays, and the window wires the two objects in QML: `HematitaResources` exposes the raw process section as properties too? No — the clean shape is one Rust-side hub. Implement: `HematitaProcesses::start()` is not called from QML; instead `HematitaResources::start()` creates the `Sampler` and, on each snapshot, queues to itself; `HematitaResources` holds a `processes: Option<cxx_qt::CxxQtThread<qobject::HematitaProcesses>>`? A `CxxQtThread` handle is obtained from the *other* object's `qt_thread()`, which QML cannot hand over. **Final decision (simplest that stays honest):** `HematitaProcesses` is the process hub and `HematitaResources` stays the resource hub, and the sampler thread publishes to *both* through two closures: `Sampler::spawn(publish_resources, publish_processes)`. QML calls `machine.start()` and `processes.start()`; whichever starts first spawns the thread and stores a shared `Arc<Mutex<Option<CxxQtThread<HematitaProcesses>>>>`… still cross-object.

Take the plain route instead: **one sampler thread per hub object is forbidden; so the sampler becomes a process-global singleton** in `sampler.rs`: `pub fn ensure_started()` spawns the thread once (`OnceLock<Sampler>`), and each hub registers a `Box<dyn Fn(&Snapshot) + Send>` subscriber through `pub fn subscribe(callback)` guarded by a `Mutex<Vec<…>>`. `Drop` for the singleton runs at process exit (the thread is joined by `std::process::exit` never — so the singleton keeps a stop flag and the window's `Component.onDestruction` calls `sampler::stop()` which sets the flag and joins). `HematitaResources::start()` and `HematitaProcesses::start()` both call `sampler::subscribe(move |snapshot| { let _ = qt.queue(...) })` with their own `qt_thread()` and `ensure_started()`.

- [ ] **Step 1: Make the sampler a shared singleton**

In `sampler.rs`:

```rust
use std::sync::{Mutex, OnceLock};

type Subscriber = Box<dyn Fn(&Snapshot) + Send + 'static>;

struct Hub {
    subscribers: Mutex<Vec<Subscriber>>,
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

static HUB: OnceLock<Hub> = OnceLock::new();

fn hub() -> &'static Hub {
    HUB.get_or_init(|| Hub {
        subscribers: Mutex::new(Vec::new()),
        stop: Arc::new(AtomicBool::new(false)),
        handle: Mutex::new(None),
    })
}

/// Registers a subscriber and starts the thread if it is not running.
///
/// # Errors
///
/// The OS refused to create the thread.
pub fn subscribe(callback: impl Fn(&Snapshot) + Send + 'static) -> std::io::Result<()> {
    let hub = hub();
    if let Ok(mut subscribers) = hub.subscribers.lock() {
        subscribers.push(Box::new(callback));
    }
    let mut handle = hub.handle.lock().map_err(|_| std::io::Error::other("sampler lock poisoned"))?;
    if handle.is_none() {
        let stop = Arc::clone(&hub.stop);
        *handle = Some(
            thread::Builder::new()
                .name("hematita-sampler".to_owned())
                .spawn(move || run(&stop, &|snapshot| {
                    if let Ok(subscribers) = hub().subscribers.lock() {
                        for subscriber in subscribers.iter() {
                            subscriber(&snapshot);
                        }
                    }
                }))?,
        );
    }
    Ok(())
}

/// Asks the thread to stop and waits for it. Called once, when the window
/// goes away; a second call is a no-op.
pub fn stop() {
    let hub = hub();
    hub.stop.store(true, Ordering::Relaxed);
    if let Ok(mut handle) = hub.handle.lock() {
        if let Some(handle) = handle.take() {
            let _ = handle.join();
        }
    }
}
```

`run`'s `publish` parameter becomes `&dyn Fn(Snapshot)` as today; subscribers receive `&Snapshot` and clone what they need. Remove `pub struct Sampler` and its `Drop`. `HematitaResources` drops its `sampler: Option<Sampler>` field for a `started: bool`, calls `sampler::subscribe(move |snapshot| { let snapshot = snapshot.clone(); let _ = qt.queue(move |r| r.apply(snapshot)); })`, and `set_start_failed(true)` on `Err`. Add `#[qinvokable] fn shutdown(self: Pin<&mut HematitaResources>)` calling `sampler::stop()`, and in `Main.qml` `Component.onDestruction: machine.shutdown()`.

- [ ] **Step 2: `processes.rs`**

```rust
//! The Processes and Applications pages' state, as Qt properties.
//!
//! Rows are index-aligned lists plus a `revision`, like the resources. The
//! projection — what survives the search, in which order, under which
//! application — is decided in `hematita-core::process_view`; this object
//! holds the state that projection reads, applies it to the latest process
//! snapshot, and is the only place a signal is sent from.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QString, QStringList, QVariant};

use celestina_core::desktop_entry::{self, DesktopEntry};
use hematita_core::process_view::{self, ProcessRow, SortField};

use crate::sampler::{self, ProcessReading, ProcessSnapshot, Reason, Section, Snapshot};

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
        // revision — bumped once per applied projection
        // sortField / sortAscending / filterText / grouped — the state QML sets
        // process* — index-aligned rows (see the plan's row contract)
        // group* — index-aligned groups, only when grouped
        // totalCount / shownCount — before and after the filter
        // available / reasonKind / reasonPath — the last read succeeded, or why not
        // actionOutcome / actionPid / actionKind — the last terminate or kill
        #[qobject]
        #[qml_element]
        #[qproperty(i32, revision)]
        #[qproperty(QString, sort_field)]
        #[qproperty(bool, sort_ascending)]
        #[qproperty(QString, filter_text)]
        #[qproperty(bool, grouped)]
        #[qproperty(QVariant, process_pids)]
        #[qproperty(QStringList, process_names)]
        #[qproperty(QStringList, process_users)]
        #[qproperty(QVariant, process_cpu_percents)]
        #[qproperty(QVariant, process_memory_kib)]
        #[qproperty(QVariant, process_read_rates)]
        #[qproperty(QVariant, process_write_rates)]
        #[qproperty(QStringList, process_applications)]
        #[qproperty(QVariant, process_group_indices)]
        #[qproperty(QVariant, process_actionable)]
        #[qproperty(QStringList, group_ids)]
        #[qproperty(QStringList, group_names)]
        #[qproperty(QStringList, group_icons)]
        #[qproperty(QVariant, group_cpu_percents)]
        #[qproperty(QVariant, group_memory_kib)]
        #[qproperty(QVariant, group_counts)]
        #[qproperty(i32, total_count)]
        #[qproperty(i32, shown_count)]
        #[qproperty(bool, available)]
        #[qproperty(QString, reason_kind)]
        #[qproperty(QString, reason_path)]
        #[qproperty(QString, action_outcome)]
        #[qproperty(i32, action_pid)]
        #[qproperty(QString, action_kind)]
        #[qproperty(bool, start_failed)]
        type HematitaProcesses = super::HematitaProcessesRust;

        #[qinvokable]
        fn start(self: Pin<&mut HematitaProcesses>);

        /// Re-projects the last snapshot with the current state. QML calls it
        /// after changing the sort, the filter or the grouping.
        #[qinvokable]
        fn refresh(self: Pin<&mut HematitaProcesses>);

        /// SIGTERM to one of the user's own processes.
        #[qinvokable]
        fn terminate(self: Pin<&mut HematitaProcesses>, pid: i32);

        /// SIGKILL to one of the user's own processes.
        #[qinvokable]
        fn kill(self: Pin<&mut HematitaProcesses>, pid: i32);
    }

    impl cxx_qt::Threading for HematitaProcesses {}
}

pub struct HematitaProcessesRust {
    revision: i32,
    sort_field: QString,
    sort_ascending: bool,
    filter_text: QString,
    grouped: bool,
    process_pids: QVariant,
    process_names: QStringList,
    process_users: QStringList,
    process_cpu_percents: QVariant,
    process_memory_kib: QVariant,
    process_read_rates: QVariant,
    process_write_rates: QVariant,
    process_applications: QStringList,
    process_group_indices: QVariant,
    process_actionable: QVariant,
    group_ids: QStringList,
    group_names: QStringList,
    group_icons: QStringList,
    group_cpu_percents: QVariant,
    group_memory_kib: QVariant,
    group_counts: QVariant,
    total_count: i32,
    shown_count: i32,
    available: bool,
    reason_kind: QString,
    reason_path: QString,
    action_outcome: QString,
    action_pid: i32,
    action_kind: QString,
    start_failed: bool,
    started: bool,
    last_generation: u64,
    latest: Option<Arc<ProcessSnapshot>>,
    own_uid: u32,
    /// desktop id → (name, icon), resolved once per id from the XDG dirs.
    entries: HashMap<String, Option<(String, String)>>,
}
```

`Default` sets `sort_field: "cpu"`, `sort_ascending: false`, empty lists via `doubles(&[])`/`QStringList::default()`, `available: true`, everything else zero/empty. Move `strings`/`doubles` helpers from `resources.rs` into a small `src/lists.rs` (`pub fn strings`, `pub fn doubles`, `pub fn nested`) used by both — that is the real shared intersection.

Methods:

```rust
impl qobject::HematitaProcesses {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        let outcome = sampler::subscribe(move |snapshot: &Snapshot| {
            let Some(section) = &snapshot.processes else { return };
            let generation = snapshot.generation;
            let section = section.clone();
            let _ = qt.queue(move |processes: Pin<&mut qobject::HematitaProcesses>| {
                processes.apply(generation, section);
            });
        });
        if outcome.is_err() {
            self.as_mut().set_start_failed(true);
        }
    }

    fn apply(mut self: Pin<&mut Self>, generation: u64, section: Section<ProcessSnapshot>) {
        if generation <= self.rust().last_generation {
            return;
        }
        self.as_mut().rust_mut().last_generation = generation;
        match section {
            Section::Available(snapshot) => {
                let own_uid = snapshot.own_uid;
                self.as_mut().rust_mut().latest = Some(Arc::new(snapshot));
                self.as_mut().rust_mut().own_uid = own_uid;
                self.as_mut().set_available(true);
                self.as_mut().set_reason_kind(QString::default());
                self.as_mut().set_reason_path(QString::default());
                self.as_mut().refresh();
            }
            Section::Unavailable(Reason { kind, path }) => {
                self.as_mut().set_available(false);
                self.as_mut().set_reason_kind(QString::from(kind.as_str()));
                self.as_mut().set_reason_path(QString::from(path.as_str()));
            }
        }
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        let Some(snapshot) = self.rust().latest.clone() else { return };
        let field = SortField::from_token(&self.rust().sort_field.to_string()).unwrap_or(SortField::Cpu);
        let ascending = self.rust().sort_ascending;
        let filter = self.rust().filter_text.to_string();
        let grouped = self.rust().grouped;
        let own_uid = snapshot.own_uid;

        let rows: Vec<ProcessRow> = snapshot.readings.iter().map(|reading| row_of(reading, own_uid)).collect();
        let order = process_view::project(&rows, &filter, field, ascending);
        let groups = if grouped { process_view::group(&rows, &order) } else { Vec::new() };

        // Group index per shown row, -1 when flat or ungrouped.
        let mut group_of: HashMap<usize, i64> = HashMap::new();
        for (group_index, group) in groups.iter().enumerate() {
            for &member in &group.member_indices {
                group_of.insert(member, group_index as i64);
            }
        }
        // Grouped layout lists members group by group; flat keeps `order`.
        let shown: Vec<usize> = if grouped {
            groups.iter().flat_map(|group| group.member_indices.iter().copied()).collect()
        } else {
            order.clone()
        };

        let user_name = |uid: u32| snapshot.users.get(&uid).cloned().unwrap_or_else(|| uid.to_string());
        self.as_mut().set_process_pids(doubles(&shown.iter().map(|&i| f64::from(rows[i].pid)).collect::<Vec<_>>()));
        self.as_mut().set_process_names(strings(shown.iter().map(|&i| rows[i].name.clone())));
        self.as_mut().set_process_users(strings(shown.iter().map(|&i| user_name(rows[i].uid))));
        self.as_mut().set_process_cpu_percents(doubles(&shown.iter().map(|&i| f64::from(rows[i].cpu_percent)).collect::<Vec<_>>()));
        self.as_mut().set_process_memory_kib(doubles(&shown.iter().map(|&i| rows[i].memory_kib as f64).collect::<Vec<_>>()));
        self.as_mut().set_process_read_rates(doubles(&shown.iter().map(|&i| rows[i].read_rate).collect::<Vec<_>>()));
        self.as_mut().set_process_write_rates(doubles(&shown.iter().map(|&i| rows[i].write_rate).collect::<Vec<_>>()));
        self.as_mut().set_process_applications(strings(shown.iter().map(|&i| rows[i].application.clone().unwrap_or_default())));
        self.as_mut().set_process_group_indices(doubles(&shown.iter().map(|&i| group_of.get(&i).copied().unwrap_or(-1) as f64).collect::<Vec<_>>()));
        self.as_mut().set_process_actionable(doubles(&shown.iter().map(|&i| if rows[i].actionable { 1.0 } else { 0.0 }).collect::<Vec<_>>()));

        let mut names = Vec::new();
        let mut icons = Vec::new();
        for group in &groups {
            let (name, icon) = self.as_mut().rust_mut().entry_for(&group.id);
            names.push(name);
            icons.push(icon);
        }
        self.as_mut().set_group_ids(strings(groups.iter().map(|group| group.id.clone())));
        self.as_mut().set_group_names(strings(names));
        self.as_mut().set_group_icons(strings(icons));
        self.as_mut().set_group_cpu_percents(doubles(&groups.iter().map(|group| f64::from(group.cpu_percent)).collect::<Vec<_>>()));
        self.as_mut().set_group_memory_kib(doubles(&groups.iter().map(|group| group.memory_kib as f64).collect::<Vec<_>>()));
        self.as_mut().set_group_counts(doubles(&groups.iter().map(|group| group.member_indices.len() as f64).collect::<Vec<_>>()));
        self.as_mut().set_total_count(i32::try_from(rows.len()).unwrap_or(i32::MAX));
        self.as_mut().set_shown_count(i32::try_from(shown.len()).unwrap_or(i32::MAX));
        let next = self.rust().revision.wrapping_add(1).max(1);
        self.as_mut().set_revision(next);
    }

    pub fn terminate(self: Pin<&mut Self>, pid: i32) {
        self.signal(pid, rustix::process::Signal::TERM, "terminate");
    }

    pub fn kill(self: Pin<&mut Self>, pid: i32) {
        self.signal(pid, rustix::process::Signal::KILL, "kill");
    }

    fn signal(mut self: Pin<&mut Self>, pid: i32, signal: rustix::process::Signal, kind: &str) {
        let outcome = match self.rust().owned_pid(pid) {
            None => "refused",
            Some(target) => match rustix::process::kill_process(target, signal) {
                Ok(()) => "done",
                Err(_) => "failed",
            },
        };
        self.as_mut().set_action_pid(pid);
        self.as_mut().set_action_kind(QString::from(kind));
        self.as_mut().set_action_outcome(QString::from(outcome));
    }
}

impl HematitaProcessesRust {
    /// The PID as a target, only if the latest snapshot shows it as ours and
    /// it is neither init nor this process.
    fn owned_pid(&self, pid: i32) -> Option<rustix::process::Pid> {
        let pid_u32 = u32::try_from(pid).ok()?;
        if pid_u32 <= 1 || pid_u32 == std::process::id() {
            return None;
        }
        let latest = self.latest.as_ref()?;
        let reading = latest.readings.iter().find(|reading| reading.pid == pid_u32)?;
        (reading.uid == self.own_uid).then(|| rustix::process::Pid::from_raw(pid))?
    }

    /// The application's name and icon from its `.desktop` file, resolved
    /// once per id. A missing entry answers the id as the name and no icon.
    fn entry_for(&mut self, desktop_id: &str) -> (String, String) {
        if let Some(cached) = self.entries.get(desktop_id) {
            return cached.clone().unwrap_or_else(|| (desktop_id.to_owned(), String::new()));
        }
        let found = desktop_entry::application_dirs().into_iter().find_map(|dir| {
            let path = dir.join(format!("{desktop_id}.desktop"));
            let text = std::fs::read_to_string(&path).ok()?;
            let entry: DesktopEntry = desktop_entry::parse(&format!("{desktop_id}.desktop"), &text)?;
            Some((entry.name, entry.icon))
        });
        self.entries.insert(desktop_id.to_owned(), found.clone());
        found.unwrap_or_else(|| (desktop_id.to_owned(), String::new()))
    }
}

fn row_of(reading: &ProcessReading, own_uid: u32) -> ProcessRow {
    ProcessRow {
        pid: reading.pid,
        name: reading.name.clone(),
        uid: reading.uid,
        cpu_percent: reading.cpu_percent.unwrap_or(0.0),
        memory_kib: reading.memory_kib,
        read_rate: reading.io_rate.map_or(0.0, |[read, _]| read),
        write_rate: reading.io_rate.map_or(0.0, |[_, write]| write),
        application: reading.application.clone(),
        actionable: reading.uid == own_uid,
    }
}
```

`entry_for` reads `.desktop` files on the Qt thread — a handful of small files, once per application id, cached forever. That is the same trade Siderita's icon resolution makes and is acceptable for H3; note it in the evidence. `hematita/Cargo.toml` gains `celestina-core = { path = "../celestina-rs/crates/celestina-core" }` with the comment "The .desktop parser the suite already shares; Hematita reads an application's name and icon from it." Flatpak exports (`/var/lib/flatpak/exports/share`, `~/.local/share/flatpak/exports/share`) are in `XDG_DATA_DIRS` on this machine (verify with `echo $XDG_DATA_DIRS`); if not, they are the author's environment to fix, not Hematita's.

Add a test module in `processes.rs` for `row_of` (actionable follows the uid) and for `owned_pid` refusing 0, 1 and `std::process::id()` — construct a `HematitaProcessesRust::default()` with a `latest` built by hand.

- [ ] **Step 3: `resources.rs` and `lists.rs`**

Move `strings`, `doubles`, `nested`, `widen` to `src/lists.rs` (`pub`), import them in both hubs; `resources.rs` uses `sampler::subscribe` and gains `shutdown()`. `build.rs` `.files(["src/activation.rs", "src/resources.rs", "src/processes.rs"])`, rerun lines for `lists.rs`.

---

### Task 8: The table, the rows, the dialog, the pages

**Files:**
- Create symlinks in `hematita/qml/`: `CelestinaTextField.qml`, `CelestinaRowHighlight.qml`, `CelestinaScrollBar.qml`, `CelestinaModalLayer.qml`, `CelestinaInputShield.qml`, `CelestinaShadow.qml`, `GlassSurface.qml`, `GlassCard.qml` (all `../../celestina-style/<name>`), and add each to `QML_FILES`.
- Create: `ProcessHeader.qml`, `ProcessRow.qml`, `ApplicationRow.qml`, `ProcessTable.qml`, `KillDialog.qml`, `ProcessPage.qml`, `ApplicationsPage.qml`
- Modify: `Main.qml`, `build.rs`

- [ ] **Step 1: `ProcessHeader.qml`**

```qml
pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.hematita 1.0

// The column titles, each a sort control. Controls family: they lift, they
// do not paint the accent ramp. The active column shows its direction.
Item {
    id: header

    required property string sortField
    required property bool sortAscending
    // Column widths, shared with the rows so titles sit over values.
    required property var columns

    signal sortRequested(string field)

    implicitHeight: CelestinaTheme.controlHeightSm

    Row {
        anchors.fill: parent

        Repeater {
            model: header.columns

            Item {
                id: cell
                required property var modelData
                readonly property bool active: header.sortField === cell.modelData.field
                width: cell.modelData.width
                height: header.height

                Accessible.role: Accessible.Button
                Accessible.name: cell.modelData.title
                Accessible.onPressAction: header.sortRequested(cell.modelData.field)

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceXs / 2
                    radius: CelestinaTheme.radiusSm
                    color: mouse.pressed ? CelestinaTheme.pressedWash
                         : mouse.containsMouse ? CelestinaTheme.surfaceHover
                         : CelestinaTheme.clear
                }

                Row {
                    anchors.left: parent.left
                    anchors.leftMargin: CelestinaTheme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: CelestinaTheme.spaceXs
                    layoutDirection: cell.modelData.numeric ? Qt.RightToLeft : Qt.LeftToRight
                    width: cell.width - CelestinaTheme.spaceSm * 2

                    Text {
                        text: cell.modelData.title
                        color: cell.active ? CelestinaTheme.text : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        font.weight: CelestinaTheme.weightDemiBold
                    }
                    CelestinaIcon {
                        width: CelestinaTheme.iconSm
                        height: width
                        opacity: cell.active ? 1 : 0
                        name: header.sortAscending ? "view-sort-ascending" : "view-sort-descending"
                        tone: CelestinaIcon.Primary
                    }
                }

                MouseArea {
                    id: mouse
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: header.sortRequested(cell.modelData.field)
                }
            }
        }
    }
}
```

- [ ] **Step 2: `ProcessRow.qml`**

```qml
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// One process. Content family: the shared row highlight paints hover,
// press and selection; the cells only write text.
AbstractButton {
    id: row

    required property var columns
    required property string name
    required property string user
    required property string pid
    required property string cpu
    required property string memory
    required property string read
    required property string write
    required property bool actionable
    required property bool selected
    // Indented under its application in the grouped layout.
    property bool nested: false

    implicitHeight: CelestinaTheme.controlHeightSm
    hoverEnabled: true
    focusPolicy: Qt.TabFocus

    Accessible.role: Accessible.ListItem
    Accessible.name: row.name + ", " + row.pid + ", " + row.cpu + ", " + row.memory
    Accessible.selected: row.selected

    background: CelestinaRowHighlight {
        family: CelestinaRowHighlight.Content
        hovered: row.hovered
        pressed: row.down
        selected: row.selected
        focused: row.visualFocus
    }

    contentItem: Item {
        opacity: row.actionable ? 1 : CelestinaTheme.mutedContentOpacity

        Row {
            anchors.fill: parent
            anchors.leftMargin: row.nested ? CelestinaTheme.space2xl : 0

            Repeater {
                model: row.columns

                Text {
                    id: cell
                    required property var modelData
                    width: cell.modelData.width - (row.nested && cell.modelData.field === "name" ? CelestinaTheme.space2xl : 0)
                    height: row.height
                    leftPadding: CelestinaTheme.spaceSm
                    rightPadding: CelestinaTheme.spaceSm
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: cell.modelData.numeric ? Text.AlignRight : Text.AlignLeft
                    elide: Text.ElideRight
                    text: row[cell.modelData.field]
                    color: cell.modelData.field === "name" ? CelestinaTheme.text : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    font.features: CelestinaTheme.fontFeaturesTabular
                }
            }
        }
    }
}
```

`row[cell.modelData.field]` reads the row's own property named by the column (`name`, `user`, `pid`, `cpu`, `memory`, `read`, `write`) — the columns carry the same `field` tokens the header sorts by, except `user` which does not sort (the header's model omits it from sorting by making its `field` `"user"` and `ProcessTable` ignoring a `sortRequested("user")`).

- [ ] **Step 3: `ApplicationRow.qml`**

```qml
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import org.celestina.hematita 1.0

// One application in the grouped layout: its icon from the desktop's icon
// theme, its name, how many processes, and their CPU and memory together.
// The row expands and collapses its processes.
AbstractButton {
    id: row

    required property string appName
    required property string iconName
    required property int count
    required property string cpu
    required property string memory
    required property bool expanded

    implicitHeight: CelestinaTheme.rowHeight
    hoverEnabled: true
    focusPolicy: Qt.TabFocus
    checkable: true
    checked: row.expanded

    Accessible.role: Accessible.ListItem
    Accessible.name: row.appName + ", " + (row.count === 1 ? qsTr("1 proceso") : qsTr("%1 procesos").arg(row.count)) + ", " + row.cpu
    Accessible.checked: row.expanded

    background: CelestinaRowHighlight {
        family: CelestinaRowHighlight.Content
        hovered: row.hovered
        pressed: row.down
        selected: false
        focused: row.visualFocus
    }

    contentItem: Item {
        Row {
            anchors.fill: parent
            anchors.leftMargin: CelestinaTheme.spaceSm
            spacing: CelestinaTheme.spaceMd

            CelestinaIcon {
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.iconSm
                height: width
                name: row.expanded ? "chevron-down" : "chevron-right"
                tone: CelestinaIcon.Secondary
            }

            // The desktop's icon for the application, through Qt's theme
            // lookup. Not a catalogue glyph: this is the application's own
            // face. When the theme has none, the catalogue's window glyph.
            Item {
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.glyphTile
                height: width

                IconImage {
                    id: themed
                    anchors.fill: parent
                    name: row.iconName
                    sourceSize: Qt.size(width, height)
                    visible: status === Image.Ready
                }
                CelestinaIcon {
                    anchors.fill: parent
                    visible: !themed.visible
                    name: "app-window"
                    tone: CelestinaIcon.Secondary
                }
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - CelestinaTheme.glyphTile - CelestinaTheme.iconSm - parent.spacing * 2 - CelestinaTheme.spaceSm - totals.width

                Text {
                    text: row.appName
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideRight
                    width: parent.width
                }
                Text {
                    text: (row.count === 1 ? qsTr("1 proceso") : qsTr("%1 procesos").arg(row.count))
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowSecondary
                }
            }

            Text {
                id: totals
                anchors.verticalCenter: parent.verticalCenter
                text: row.cpu + "   " + row.memory
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                font.features: CelestinaTheme.fontFeaturesTabular
                rightPadding: CelestinaTheme.spaceSm
            }
        }
    }
}
```

`IconImage` accepts an absolute path in `name`? No: when `iconName` starts with `/`, set `source: "file://" + row.iconName` instead of `name` — implement with `name: row.iconName.startsWith("/") ? "" : row.iconName` and `source: row.iconName.startsWith("/") ? "file://" + row.iconName : ""`. The count text uses the two-form `qsTr` pair shown (Qt's `%n` plural form needs a translator to render; the suite composes with `arg(`).

- [ ] **Step 4: `KillDialog.qml`**

```qml
import QtQuick
import org.celestina.hematita 1.0

// Killing is not undoable, so it is asked. The question owns the focus while
// it is up; Escape and a click outside both mean "no".
CelestinaModalLayer {
    id: layer

    required property Item backdrop
    property int pid: 0
    property string processName: ""

    signal confirmed(int pid)

    z: 90
    dismissOnEscape: true
    dismissOnOutsideClick: true
    onDismissRequested: layer.shown = false
    onShownChanged: if (shown) cancelButton.forceActiveFocus()

    function ask(pid, name) {
        layer.pid = pid
        layer.processName = name
        layer.shown = true
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(420, layer.width - CelestinaTheme.space3xl)
        height: buttons.y + buttons.height + CelestinaTheme.spaceLg
        backdropSource: layer.backdrop

        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Matar proceso")

        MouseArea { anchors.fill: parent }

        Text {
            id: question
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: CelestinaTheme.spaceLg
            anchors.top: parent.top
            anchors.topMargin: CelestinaTheme.spaceLg
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            text: qsTr("¿Matar «%1» (%2)? El proceso no podrá guardar nada.").arg(layer.processName).arg(layer.pid)
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody

            Accessible.role: Accessible.StaticText
            Accessible.name: question.text
        }

        Row {
            id: buttons
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.top: question.bottom
            anchors.topMargin: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceSm

            CelestinaButton {
                id: cancelButton
                text: qsTr("Cancelar")
                onClicked: layer.shown = false
            }
            CelestinaButton {
                text: qsTr("Matar")
                role: CelestinaButton.Destructive
                onClicked: { layer.confirmed(layer.pid); layer.shown = false }
            }
        }
    }
}
```

- [ ] **Step 5: `ProcessTable.qml`**

```qml
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.hematita 1.0

// The table both pages share: a bar with the search and the actions, the
// column titles, and the rows. `grouped` puts each application's row above
// its processes and lets it fold them.
Item {
    id: table

    required property HematitaProcesses processes
    required property bool grouped
    // What the kill dialog blurs beneath itself.
    required property Item backdrop

    property int selectedPid: -1
    property var rows: []
    property var groups: []
    // Application ids folded shut.
    property var collapsed: ({})

    readonly property var columns: [
        { field: "name", title: qsTr("Nombre"), width: Math.max(160, table.width * 0.34), numeric: false },
        { field: "user", title: qsTr("Usuario"), width: 90, numeric: false },
        { field: "pid", title: qsTr("PID"), width: 80, numeric: true },
        { field: "cpu", title: qsTr("CPU"), width: 80, numeric: true },
        { field: "memory", title: qsTr("Memoria"), width: 110, numeric: true },
        { field: "read", title: qsTr("Lectura"), width: 110, numeric: true },
        { field: "write", title: qsTr("Escritura"), width: 110, numeric: true }
    ]

    function percentText(value) { return value.toLocaleString(Qt.locale(), "f", 1) + " %" }
    function bytesText(bytes) {
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return Math.round(bytes) + " B"
    }
    function rateText(v) { return v > 0 ? table.bytesText(v) + "/s" : "—" }

    function weave() {
        const p = table.processes
        const count = Math.min(p.processPids.length, p.processNames.length, p.processUsers.length,
                               p.processCpuPercents.length, p.processMemoryKib.length,
                               p.processReadRates.length, p.processWriteRates.length,
                               p.processApplications.length, p.processGroupIndices.length,
                               p.processActionable.length)
        const woven = []
        for (let i = 0; i < count; ++i)
            woven.push({ pid: p.processPids[i], name: p.processNames[i], user: p.processUsers[i],
                         cpu: p.processCpuPercents[i], memory: p.processMemoryKib[i],
                         read: p.processReadRates[i], write: p.processWriteRates[i],
                         application: p.processApplications[i], group: p.processGroupIndices[i],
                         actionable: p.processActionable[i] === 1 })
        const groupCount = Math.min(p.groupIds.length, p.groupNames.length, p.groupIcons.length,
                                    p.groupCpuPercents.length, p.groupMemoryKib.length, p.groupCounts.length)
        const wovenGroups = []
        for (let g = 0; g < groupCount; ++g)
            wovenGroups.push({ id: p.groupIds[g], name: p.groupNames[g], icon: p.groupIcons[g],
                               cpu: p.groupCpuPercents[g], memory: p.groupMemoryKib[g], count: p.groupCounts[g] })
        table.rows = woven
        table.groups = wovenGroups
        table.entries = table.layout()
    }

    // The visible sequence: in the flat layout every row; grouped, each
    // application followed by its rows unless folded. Each entry is
    // { kind: "group"|"process", index }.
    property var entries: []
    function layout() {
        const out = []
        if (!table.grouped) {
            for (let i = 0; i < table.rows.length; ++i)
                out.push({ kind: "process", index: i })
            return out
        }
        for (let g = 0; g < table.groups.length; ++g) {
            out.push({ kind: "group", index: g })
            if (table.collapsed[table.groups[g].id])
                continue
            for (let i = 0; i < table.rows.length; ++i)
                if (table.rows[i].group === g)
                    out.push({ kind: "process", index: i })
        }
        return out
    }

    function toggleGroup(id) {
        const next = Object.assign({}, table.collapsed)
        if (next[id]) delete next[id]; else next[id] = true
        table.collapsed = next
        table.entries = table.layout()
    }

    function selectedRow() {
        for (let i = 0; i < table.rows.length; ++i)
            if (table.rows[i].pid === table.selectedPid)
                return table.rows[i]
        return null
    }

    function outcomeText() {
        const p = table.processes
        if (p.actionOutcome === "") return ""
        const verb = p.actionKind === "kill" ? qsTr("Matar") : qsTr("Terminar")
        switch (p.actionOutcome) {
        case "done": return qsTr("%1: señal enviada al proceso %2").arg(verb).arg(p.actionPid)
        case "refused": return qsTr("%1: el proceso %2 no es tuyo; actuar sobre él llega con la fase de servicios").arg(verb).arg(p.actionPid)
        case "failed": return qsTr("%1: el sistema rechazó la señal para el proceso %2").arg(verb).arg(p.actionPid)
        }
        return ""
    }

    Connections {
        target: table.processes
        function onRevisionChanged() { table.weave() }
    }
    Component.onCompleted: {
        table.processes.grouped = table.grouped
        table.processes.refresh()
        table.weave()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceSm

        // ── Bar ────────────────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: CelestinaTheme.spaceSm

            CelestinaTextField {
                id: search
                Layout.preferredWidth: 260
                shape: CelestinaTextField.Search
                placeholderText: qsTr("Buscar por nombre, PID o aplicación")
                Accessible.name: placeholderText
                onTextChanged: { table.processes.filterText = text; table.processes.refresh() }
            }

            Text {
                text: table.processes.shownCount === table.processes.totalCount
                      ? qsTr("%1 procesos").arg(table.processes.totalCount)
                      : qsTr("%1 de %2 procesos").arg(table.processes.shownCount).arg(table.processes.totalCount)
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                font.features: CelestinaTheme.fontFeaturesTabular
            }

            Item { Layout.fillWidth: true }

            CelestinaCapsule {
                CelestinaIconButton {
                    iconName: "circle-stop"
                    helpText: qsTr("Terminar el proceso seleccionado")
                    role: CelestinaButton.Ghost
                    enabled: table.selectedRow() !== null && table.selectedRow().actionable
                    onClicked: table.processes.terminate(table.selectedPid)
                }
                CelestinaIconButton {
                    iconName: "x"
                    helpText: qsTr("Matar el proceso seleccionado")
                    role: CelestinaButton.Ghost
                    enabled: table.selectedRow() !== null && table.selectedRow().actionable
                    onClicked: killDialog.ask(table.selectedPid, table.selectedRow().name)
                }
            }
        }

        Text {
            Layout.fillWidth: true
            visible: text.length > 0
            text: {
                const row = table.selectedRow()
                if (row !== null && !row.actionable)
                    return qsTr("El proceso %1 pertenece a %2; terminarlo o matarlo llega con la fase de servicios").arg(row.pid).arg(row.user)
                if (!table.processes.available)
                    return qsTr("No se pudo leer %1").arg(table.processes.reasonPath)
                return table.outcomeText()
            }
            color: table.processes.available && table.processes.actionOutcome !== "failed" ? CelestinaTheme.textMuted : CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }

        // ── Header ─────────────────────────────────────────────────────
        ProcessHeader {
            Layout.fillWidth: true
            columns: table.columns
            sortField: table.processes.sortField
            sortAscending: table.processes.sortAscending
            onSortRequested: function(field) {
                if (field === "user") return
                if (table.processes.sortField === field)
                    table.processes.sortAscending = !table.processes.sortAscending
                else {
                    table.processes.sortField = field
                    table.processes.sortAscending = field === "name" || field === "pid"
                }
                table.processes.refresh()
            }
        }

        // ── Rows ───────────────────────────────────────────────────────
        CelestinaSurface {
            Layout.fillWidth: true
            Layout.fillHeight: true
            role: CelestinaSurface.Panel
            padding: CelestinaTheme.spaceXs

            contentItem: ListView {
                id: list
                clip: true
                model: table.entries.length
                activeFocusOnTab: true
                keyNavigationEnabled: true
                Accessible.role: Accessible.List
                Accessible.name: table.grouped ? qsTr("Aplicaciones") : qsTr("Procesos")

                delegate: Loader {
                    id: slot
                    required property int index
                    readonly property var entry: index < table.entries.length ? table.entries[index] : { kind: "process", index: -1 }
                    width: list.width
                    sourceComponent: slot.entry.kind === "group" ? groupRow : processRow
                }

                Component {
                    id: processRow
                    ProcessRow {
                        readonly property var data: table.rows[slot.entry.index] || { pid: -1, name: "", user: "", cpu: 0, memory: 0, read: 0, write: 0, actionable: false }
                        columns: table.columns
                        name: data.name
                        user: data.user
                        pid: String(data.pid)
                        cpu: table.percentText(data.cpu)
                        memory: table.bytesText(data.memory * 1024)
                        read: table.rateText(data.read)
                        write: table.rateText(data.write)
                        actionable: data.actionable
                        selected: data.pid === table.selectedPid
                        nested: table.grouped
                        onClicked: table.selectedPid = data.pid
                    }
                }

                Component {
                    id: groupRow
                    ApplicationRow {
                        readonly property var data: table.groups[slot.entry.index]
                        appName: data.name
                        iconName: data.icon
                        count: data.count
                        cpu: table.percentText(data.cpu)
                        memory: table.bytesText(data.memory * 1024)
                        expanded: !table.collapsed[data.id]
                        onClicked: table.toggleGroup(data.id)
                    }
                }
            }
        }
    }

    KillDialog {
        id: killDialog
        anchors.fill: parent
        backdrop: table.backdrop
        onConfirmed: function(pid) { table.processes.kill(pid) }
    }
}
```

`slot` inside the `Component`s: a `Loader`'s `sourceComponent` items see the Loader as their parent context, so `slot.entry` resolves. If qmllint reports unqualified access inside the components, pass the entry through a `property var entry` on the Loader's `item` via `onLoaded` instead — do not suppress.

- [ ] **Step 6: The pages and `Main.qml`**

`ProcessPage.qml`:

```qml
import QtQuick
import org.celestina.hematita 1.0

Item {
    id: page
    required property HematitaProcesses processes
    required property Item backdrop
    ProcessTable {
        anchors.fill: parent
        processes: page.processes
        grouped: false
        backdrop: page.backdrop
    }
}
```

`ApplicationsPage.qml` is the same with `grouped: true`. Both pages share one `HematitaProcesses`, and `grouped` on it is set by whichever page is visible: in `Main.qml`, `onCurrentSectionChanged` sets `processes.grouped = window.currentSection === 2` and calls `processes.refresh()`; `ProcessTable.Component.onCompleted` stops setting `grouped` (delete those two lines) — the window owns it.

`Main.qml`: declare `HematitaProcesses { id: processes }` beside `machine`; replace the `Repeater` placeholders with `ProcessPage { processes: processes; backdrop: window.contentItem }`, `ApplicationsPage { … }` and one remaining placeholder `Item` for Sensors (H4); `Component.onCompleted` adds `processes.start()`; `Component.onDestruction: machine.shutdown()`.

- [ ] **Step 7: `build.rs`** — add the eight symlinks and the seven components to `QML_FILES` (before `Main.qml`).

- [ ] **Step 8: The one build cycle; close H3-C**

```bash
cd hematita && cargo fmt --all
hematita/scripts/build-production.sh
(cd hematita && cargo test --release --locked --all-targets && cargo clippy --release --all-targets --locked -- -D warnings && cargo fmt --all --check)
hematita/scripts/verify-production.sh
```

Then an offscreen behaviour check without a window: run the binary offscreen for 8 s with `HEMATITA_SMOKE_SHAPE=1` and confirm the shape line still prints and no `TypeError`/`ReferenceError`/`Unable to assign`/`Cannot read property` appears (the process table is constructed on startup even though hidden in the `StackLayout`).

Evidence `2026-09-22-h3-processes-page.md`: commands and outputs; the row and group contracts; the `.desktop` read on the Qt thread as an accepted trade; `Limits`: no visual check, no signal sent (VAL-H3 sends one), the icon-theme lookup through `IconImage` unverified (VAL-H3). Ledger/roadmap/STATUS; inventory `H3-C.numstat.tsv` (every changed path incl. `Cargo.toml`, `Cargo.lock`, symlinks, new QML, `build.rs`, `main.rs`, `sampler.rs`, `resources.rs`, `processes.rs`, `lists.rs`, `Main.qml`); guards; commit:

```
hematita-maintenance: Add the Processes and Applications pages with terminate and kill for own processes
```

---

### Task 9: Implementation exit — H3-Z

As the H2 plan's Task 11, with: `python3 scripts/version_tool.py bump hematita milestone --unit H3-Z --summary "Add the Processes and Applications pages"`, `complete-production.sh`, hashes, roadmap `idle`/`none` with `## H3 — closed 2026-09-22` and H4 as the next checkpoint, STATUS `Delivered as 0.4.0: H3`, AGENTS.md gains a bullet: "Signals leave `processes.rs` only, through `rustix`, only to a PID the latest snapshot shows as the user's own, never to PID 1 or ourselves", plan archived (`Successor: H4`), both READMEs, evidence `2026-09-22-h3-production-completion.md` (with the keyed-digest note), inventory `H3-Z.numstat.tsv` including the deleted active path, guards, commit:

```
hematita-milestone: Add the Processes and Applications pages
```

---

## Self-review

**Spec coverage (H3):** §3 process sources and application identity → Tasks 3, 6, 7 (`.desktop` via `celestina-core::desktop_entry`). §4 `process` module (`parse_stat`, `parse_status`, `parse_io`, `parse_cgroup`, `ProcessSampler`, scope decoding, grouping) → Tasks 3–4 (grouping lives in `process_view`, named for what it is). §5 `processes.rs` roles, sort/filter/grouped state in Rust, terminate/kill only for own processes with `actionable` and the UI saying why → Tasks 7–8; "row changes diffed so the table is not rebuilt each second" → satisfied by the integer model and index reads (only a count change resets), stated in the plan header. §6 `ProcessPage` (search, flat/grouped, sortable headers, single selection, icon actions, confirming dialog) and `ApplicationsPage` (grouped with icon, collapsible) → Task 8. §7 H3 row → all. H2's booked H3-A → Task 2. Deviation recorded: `QAbstractListModel` is not available from CXX-Qt 0.9, so lists + revision, as in H2.

**Placeholder scan:** none; captures replace nothing by hand except confirming the cgroup capture carries a scope.

**Type consistency:** `ProcessStat`/`ProcessStatus`/`ProcessIo`/`ApplicationScope`/`ProcessSampler::sample` (Task 3) ↔ `sample_processes` (Task 6); `ProcessRow`/`SortField`/`project`/`group` (Task 4) ↔ `refresh` (Task 7); `ProcessReading`/`ProcessSnapshot`/`Snapshot.processes`/`sampler::subscribe`/`stop` (Tasks 6–7) ↔ both hubs; QML property names are the camelCase of Task 7's snake_case and are what `ProcessTable.weave()` reads; `ProcessHeader.columns`/`ProcessRow.columns` share the `columns` array; `KillDialog.ask(pid, name)`/`confirmed(pid)` match `ProcessTable`; `ApplicationRow` props match the group weave.
