# Hematita — design

- **Date:** 2026-09-21
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Product:** the suite's resource monitor, replacing Mission Center
- **Identifiers:** project `hematita`, crate `hematita-core`, commit prefix
  `hematita`, desktop id `org.celestina.Hematita`

## 1. Goal and scope

Hematita shows what the machine is doing and lets the user act on it. It
replaces every part of Mission Center the author uses, in this order:

1. resource graphs: CPU (aggregate and per core), memory and swap, disks,
   network interfaces, GPU;
2. a process list: sortable, searchable, with terminate and kill;
3. a per-application view: processes grouped by the desktop application that
   launched them, with the application's icon;
4. sensors: everything hwmon exposes — temperatures, fans, voltages, power —
   grouped by chip;
5. systemd services with start and stop, delivered last.

Out of scope for every phase: per-process network throughput (the kernel does
not expose it without root or eBPF), non-AMD GPUs (the machine has an
`amdgpu` card; other vendors are a later decision), history persistence across
runs, alerts or notifications, and any integration with the Celestina shell,
which is in standby by the author's order.

## 2. Constraints the design honours

- Root `AGENTS.md`: pure domain in `celestina-rs/`, Qt adaptation in
  `hematita/src`, presentation in `hematita/qml`, tokens from
  `celestina-style`, every QML file registered in `build.rs`, no blocking IO on
  the Qt thread, typed errors, no production `unwrap`, each dependency
  justified.
- Style contract (`celestina-style/DESIGN.md`): grouped rounded cards, one
  accent, GPU-backed `Shape` for strokes, no per-frame `Canvas`, motion within
  100–500 ms honouring `reducedMotion`.
- Author rules: icon-first actions with the shared hover circle, no tooltips,
  Spanish product copy through `qsTr()`.
- Template: Grafita is the smallest application and its layout (crate +
  `src` + `qml` + `build.rs` + production scripts + ledger) is copied
  structurally, never its code.

## 3. Data sources (verified on the author's machine, 2026-09-21)

| Source | Path | Notes |
|---|---|---|
| CPU ticks | `/proc/stat` | aggregate `cpu` line plus `cpuN` lines; idle = idle + iowait |
| CPU frequency | `/sys/devices/system/cpu/cpuN/cpufreq/scaling_cur_freq` | kHz |
| CPU model | `/proc/cpuinfo` | first `model name` |
| Memory, swap | `/proc/meminfo` | used = `MemTotal` − `MemAvailable`; swap from `SwapTotal`/`SwapFree` (zram shows here) |
| Disks | `/proc/diskstats` | whole devices only (no partitions); sectors × 512; model and size from `/sys/block/DEV/` |
| Network | `/proc/net/dev` | rx/tx bytes per interface; `lo` dropped |
| GPU | `/sys/class/drm/cardN/device/` | `gpu_busy_percent`, `mem_busy_percent`, `mem_info_vram_used/total`, `mem_info_gtt_used/total`, `pp_dpm_sclk`, `pp_dpm_mclk`; only when `driver` resolves to `amdgpu` |
| Sensors | `/sys/class/hwmon/hwmonN/` | `name`, then `tempX_input`, `fanX_input`, `inX_input`, `powerX_average`, optional `*_label`; units: m°C, rpm, mV, µW |
| Processes | `/proc/PID/{stat,status,cmdline,io,cgroup}` | `io` is readable only for own processes; `cgroup` line `app-<id>-<n>.scope` names the launching application |
| Application identity | XDG `.desktop` lookup by the `app-` scope id | icon and display name; a scope with no `.desktop` groups under its command name |

Absence is a state, not an error: no GPU, no hwmon chip, no readable `io`
render as "not available" and the rest of the snapshot still publishes.

## 4. `celestina-rs/crates/hematita-core`

Pure Rust. No Qt, no threads, no filesystem access of its own: every function
takes the text (or directory listing) a caller read and returns typed values
or a typed error naming what was unreadable. Modules:

- `cpu`: `parse_stat(&str) -> Result<CpuStat, CpuError>` (aggregate and per
  core ticks), `CpuSampler` holding the previous reading and answering `None`
  for the first sample, `parse_frequency`, `parse_model`.
- `memory`: `parse_meminfo(&str) -> Result<Memory, MemoryError>` with
  `used_kib`, `total_kib`, `swap_used_kib`, `swap_total_kib`, `used_percent()`.
- `disk`: `parse_diskstats(&str)`, `DiskSampler` producing read/write bytes
  per second between two readings, saturating on counters that go backwards.
- `network`: `parse_net_dev(&str)`, `NetworkSampler` producing rx/tx bytes
  per second per interface.
- `gpu`: `AmdgpuReading` built from the individual sysfs file contents;
  `parse_dpm_level` for the active `*` entry in `pp_dpm_*`.
- `sensors`: `discover(chips: impl Iterator<Item = ChipListing>) -> Vec<Chip>`
  where a `ChipListing` is the file names and contents a caller enumerated;
  `Channel { kind: Temperature|Fan|Voltage|Power, label, value }` with units
  converted to °C, rpm, V, W.
- `process`: `parse_stat`, `parse_status`, `parse_io`, `parse_cgroup`;
  `ProcessSampler` turning two tick readings into CPU percent per PID;
  `ApplicationScope::from_cgroup` extracting the `app-` id; `group_by_scope`.
- `history`: `Ring<const N: usize>` of `f32` samples, `HISTORY_SAMPLES = 60`,
  the one place that number lives.
- `error`: one enum per module, all implementing `std::error::Error`, each
  variant carrying the field or path that failed.

Every module has tests with fixtures captured from the author's machine under
`tests/fixtures/` plus hostile cases: empty input, too few fields, unreadable
numbers, counters moving backwards, a chip with no labels, a PID that vanished
between reads. The crate holds no policy: thresholds, sort orders and units of
display belong to the adapter.

Dependencies: none beyond `std`. The manifest says so.

## 5. `hematita/src` — the CXX-Qt adapter

- `sampler.rs`: one named thread. Every `INTERVAL` (1 s, a single constant)
  it reads every source, feeds the crate, and builds an immutable `Snapshot`
  with a monotonic `generation`. A failed section becomes
  `Section::Unavailable(reason)` while the others publish. The snapshot is
  moved to the Qt thread with `qt_thread().queue`; a snapshot older than the
  last applied one is dropped. Shutdown is a flag plus `join`, so the window
  closes deterministically.
- `resources.rs` (`HematitaResources`, `QObject`): the Performance page state.
  A resource list for the side column (`kind`, `name`, `currentValue`,
  `subtitle`, `load`, `sparkline`) and the selected resource's detail
  (`history`, per-core histories, key/value rows). Owns the only policy
  numbers: `ELEVATED_PERCENT = 80`, `CRITICAL_PERCENT = 90`, mapped to a
  `load` string (`normal`, `elevated`, `critical`) the theme colours.
- `processes.rs` (`HematitaProcesses`, `QAbstractListModel`): roles `pid`,
  `name`, `user`, `cpuPercent`, `memoryKib`, `readBytesPerSecond`,
  `writeBytesPerSecond`, `application`, `actionable`. Sort field, direction
  and filter text are properties; sorting and filtering run in Rust. Two
  layouts: flat, and grouped by application with collapsible groups. Actions
  `terminate(pid)` (SIGTERM) and `kill(pid)` (SIGKILL) act only on processes
  owned by the current user; `actionable` is false otherwise and the UI says
  why. Row changes are diffed against the previous snapshot so the table is
  not rebuilt each second.
- `sensors.rs` (`HematitaSensors`, `QAbstractListModel`): chip, channel kind,
  label, value, unit, session minimum and maximum.
- `activation.rs`: single instance on the session bus, same shape as
  Grafita's.
- `main.rs`: registers `org.celestina.hematita`, starts the sampler, loads
  `Main.qml`.

## 6. `hematita/qml` — the surface

Layout A from the brainstorm: a centred pill navigation strip at the top; the
Performance page has the resource list on the left and the selected
resource's detail on the right; every other page is a table or a card list.

- `Main.qml`: window, hosts `NavStrip` and a `StackLayout` of pages. It
  coordinates and owns nothing else.
- `components/NavStrip.qml`: icon-over-label items inside a
  `CelestinaCapsule`; the active item wears a lighter pill (the Samsung
  Gallery reference the author supplied). Keyboard: a radio group — arrows
  move the selection, `Tab` leaves the strip. Exposes `Accessible.role`
  `PageTabList`, item names and `checked` state. Local to Hematita until a
  second consumer exists.
- `components/PerformancePage.qml`, `ResourceRow.qml`,
  `ResourceDetail.qml`: the side list (name, current value, sparkline) and
  the detail (big graph, key/value grid). CPU adds a toggle between the single
  graph and a per-core grid of small graphs.
- `components/HistoryGraph.qml`: the only graph. A `Shape` with a gradient
  fill and a stroke, drawing a normalised series Rust publishes once per
  sample. Colour follows `load` through theme tokens (accent, warning,
  danger). Enter motion only, `reducedMotion` respected.
- `components/ProcessPage.qml`: search field, flat/grouped toggle, sortable
  headers, single row selection, terminate and kill as icon actions in the
  bar. Kill opens a confirming dialog that contains and restores focus.
- `components/ApplicationsPage.qml`: the grouped layout with the app icon and
  collapsible process rows.
- `components/SensorsPage.qml`: `ListSection` cards per chip, one row per
  channel with value, minimum and maximum.
- `components/ServicesPage.qml`: phase H5 only; not registered before that.

All product copy is Spanish through `qsTr()`. No tooltips. Colours, radii,
spacing, type and motion come from `CelestinaTheme` tokens.

## 7. Delivery phases

Each phase is a `ROADMAP.md` checkpoint with a ledger unit, inventory and
evidence, closed by `complete-production.sh`.

| Phase | Outcome |
|---|---|
| H1 | Project skeleton (crate, app, `build.rs`, style links, `.desktop`, icon, `docs/projects.toml` entry, production scripts, baseline version row) and the Performance page with CPU and memory |
| H2 | Disks, network, GPU and swap in the side list with details; the per-core grid |
| H3 | Process model and page, search, sort, terminate/kill of own processes, the Applications page |
| H4 | hwmon discovery and the Sensors page |
| H5 | Services over the system bus with start/stop (systemd authorises via the user's polkit agent) and terminate/kill of foreign processes via `pkexec`; preceded by its own written decision because it touches privilege |

Removing Mission Center from the system is the author's action and no phase
performs it.

## 8. Verification

- Crate: fixture-driven tests per parser and sampler plus hostile cases; a
  ring-buffer test.
- Adapter: tests for process sorting/filtering and for snapshot diffing.
- `hematita/scripts/smoke.sh`: launches the release binary on the offscreen
  platform, constructs every registered page and exits; same shape as
  Grafita's smoke.
- Guards before every commit: `scripts/check-architecture-contract.sh`,
  production qmllint, the language contract, `version_tool.py check`.
- `VALIDATION.md` (author only): graph smoothness at one-second sampling,
  Hematita's own idle cost, the strip and the tables by keyboard and screen
  reader, the kill dialog on the real session.

## 9. Decisions recorded here

- Hand-written `/proc` and `/sys` parsers over `sysinfo`/`procfs`: full cost
  control, fixture-testable, zero runtime dependencies; a library would still
  leave hwmon and amdgpu to us.
- No daemon: privileged actions are one-shot (`pkexec`, systemd D-Bus with
  polkit); nothing needs standing privilege.
- The shell's existing `/proc` parser is not reused: the shell is in standby
  and Hematita owns its own domain crate.
- `NavStrip` stays local until a second application needs it, per the
  extraction rule.
