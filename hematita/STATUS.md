# Hematita status

- **Updated:** 2026-09-22
- **Delivered as 0.4.0:** H3 — the Processes page lists every process with
  live CPU, memory and IO, sortable columns, search, terminate and kill
  behind a confirming dialog; the Applications page groups the same rows
  under their application's icon; both the Performance list and the process
  table are reachable by keyboard; built, verified and deployed to the
  author's prefix
- **Delivered as 0.4.1:** `H3-E` — the whole-branch review's correction: the
  cursor follows the selection across every rebuild, the last action's
  outcome outlives the next reading, the signal path re-reads `/proc` before
  it acts, and the table is one Tab stop with keyboard folds and columns that
  fit the minimum window
- **Delivered as 0.5.0:** H4 — `hematita-core` reads every hwmon chip and
  channel into a typed value with its unit and kernel limits, captured from
  the author's processor, GPU and board chips; the sampler reads the value
  files every tick, `HematitaSensors` publishes them with the session's
  extremes, and the Sensors page shows every chip as a card, one row per
  channel, coloured by a thermal load graded against the chip's own `crit`.
  The six process-table items parked by `H3-E` are closed; built, verified
  and deployed to the author's prefix
- **Delivered as 0.5.1:** `H4-D` — the whole-branch review's correction: the
  Sensors page is its own single Tab stop whose arrows scroll card by card
  instead of forty-six rows each answering Tab, a tick that cannot read
  `/sys/class/hwmon` shows only its reason rather than the last good values
  beneath an error line, and the smoke fails unless the page publishes chips
- **Author validation:** `VAL-H1` requested, not run; `VAL-H2` requested, not
  run; `VAL-H3` requested, not run; `VAL-H4` requested, not run
- **Active phase:** none; the next phase is H5 (services and privileged
  actions), not yet opened

## Current checkout truth

- The project is registered and builds a release binary. The window shows the
  pill strip with four sections, and all four now have a page: Performance,
  Processes, Applications and Sensors.
- `hematita-core` provides `ratio`, `cpu`, `memory` and `history`: `/proc/stat`
  and `/proc/meminfo` parsing, a CPU sampler that turns two readings into a
  rate, cpufreq and model parsing, and a sixty-sample ring, all covered by
  unit tests and captures from the author's machine. As of `H2-A`, the crate
  also parses disks, interfaces and the GPU: `rate::NamedCounters` turns
  cumulative byte counters into per-second rates, `disk` reads
  `/proc/diskstats` and its `/sys/block` sysfs files for whole devices only,
  `network` reads `/proc/net/dev` (dropping loopback) and its `/sys/class/net`
  sysfs files, `gpu` reads the AMD `amdgpu` driver's sysfs files, and
  `history::Ring` gained `max`/`fractions` to scale a graph by its own peak.
  As of `H2-B` all of it is wired into the sampler and the page.
- A sampling thread reads every source once a second off the Qt thread and
  publishes one whole snapshot: CPU with its per-core percentages, memory,
  the AMD GPU when the machine has one, every whole disk and every Ethernet
  or Wi-Fi interface. `HematitaResources` applies it on the Qt thread as
  index-aligned lists with a `revision` ticket bumped last, keeps one history
  ring per row key, and drops a snapshot older than the one it already
  applied. The pure decisions — the 80/90 load thresholds, the staleness
  test, the ticket, and which numbers each kind publishes in what order —
  live in `publish.rs` under unit test, with no QObject involved. `H2-B`
  delivered per-row state: a source that cannot be read marks only its own
  row `unavailable` with a typed reason (`unreadable`, `malformed` or
  `no-rate`) and the path, and the page composes the Spanish sentence from
  those tokens through `qsTr()`; the one page-level failure left is the
  sampling thread refusing to start. No Rust file in the project carries
  product copy any more.
- The Performance page's list is dynamic: it weaves its rows from the
  adapter's lists on each `revision`, so the processor, memory, the GPU,
  every whole disk and every interface appear as the machine actually has
  them, each with its own sparkline and its own minute of history, and the
  selection is a key rather than an index so a disk that comes or goes never
  moves it. The detail is driven by the row's kind, and the processor gains a
  per-core grid behind an icon toggle. Histories cross to QML as a `QVariant`
  carrying a variant list: it is the one shape `qmllint` resolves, and this
  project suppresses no warnings. No row claims a number before its first
  rate arrives. As of `H2-C` the disks and the interfaces are ordered by
  name whether or not a read failed, and the list's model is the row count
  rather than the woven array, so a second that changes no resource leaves
  the delegates, the scroll position and the selection exactly where they
  were; only a resource appearing or disappearing rebuilds the list.
- A second `hematita` raises the running window over the session bus and exits
  without building one; no session bus is never fatal.
- Nobody has looked at the page on a real session yet: the evidence for H1-C
  and for H2-B is offscreen only, and appearance, keyboard and focus belong
  to `VAL-H1` and `VAL-H2`.
- The release binary is built, verified and installed at
  `~/.local/bin/hematita` at `0.4.0`, with its desktop entry and icon set
  registered under `~/.local/share`; the installed bytes match the checkout's
  `hematita/target/release/hematita` byte for byte. See the
  [production completion record](docs/evidence/2026-09-22-h3-production-completion.md).
- As of `H3-A` the side list is a list for the keyboard and for AT: it takes
  Tab, arrows move the current item, the selection and the current index are
  one value, and the view carries `Accessible.List` with a name, so an
  off-screen row is reachable without a pointer. The smoke now fails unless
  the first row publishes the CPU contract (`hematita-shape cpu 3 60`), the
  network subtitle waits for the row to be ready, `no-rate` is reserved for
  the error that means it, the core count comes from each second's reading,
  the static sysfs facts are read once per name, `sample_gpu` has no dead arm,
  and the capture asserts the exact disk set. Nobody has walked the list on a
  real session yet: that is `VAL-H2`.
- As of `H3-B` the crate also parses one process: `process` reads
  `/proc/PID/stat`, `/proc/PID/status`, `/proc/PID/cmdline` and
  `/proc/PID/io`, decodes the desktop application behind a systemd scope from
  `/proc/PID/cgroup` (`app-<launcher->id-<n>.scope` and
  `app-id[@instance].service`, including `dbus-:1.3-id@0`), and
  `ProcessSampler` turns successive per-PID tick readings into a percentage
  of the whole machine keyed on `(pid, start_ticks)` so a reused pid does not
  inherit another process's ticks. `passwd` maps uid to login name from
  `/etc/passwd`, skipping malformed lines rather than refusing the file.
  `process_view` decides the table's shape without Qt: a case-insensitive
  filter over name/pid/application, six sortable fields with ties broken by
  pid, and grouping filtered rows under their application in first-appearance
  order. All of it is under unit test and captured from the author's own
  session; none of it is wired into the sampler thread or any page yet — that
  is `H3-C`.
- As of `H3-C` it is wired. The sampler reads every process on every second
  tick (`PROCESS_TICKS`), caching per PID what does not change while it
  lives, and one process-global thread now publishes to both hub objects
  rather than one thread per object; the window joins it from
  `Component.onDestruction`. `HematitaProcesses` publishes the rows, the
  groups, the counts, the availability and the outcome of the last action as
  index-aligned lists plus a `revision`, and owns the only signal path:
  `terminate` and `kill` refuse any PID the latest snapshot does not show as
  the user's own, and refuse `0`, `1` and this process besides.
  `ProcessTable` serves both the Processes page and the Applications page,
  which differ only in `grouped` — the window sets it, because the two pages
  share one hub. Killing is asked first, in `KillDialog`. Every check is
  offscreen: no row was looked at, no signal was sent, and the application
  icons were never seen resolving. That is `VAL-H3`. See the
  [processes page evidence](docs/evidence/2026-09-22-h3-processes-page.md).
- As of `H3-D` the table answers the keyboard. Every column title is a button
  that Tab reaches and Space sorts by, with the shared focus ring; the list's
  current item and the selected pid write each other, so an arrow key moves
  the selection instead of an invisible cursor, and landing on an application
  row selects nothing. The last action's outcome is cleared when a new
  reading lands and when the selection moves, the IO counters are keyed by
  `(pid, start_ticks)` so a recycled PID cannot inherit them, a poisoned
  subscriber lock is now an error instead of a silent drop, and the
  application row no longer declares itself `checkable` against its own
  `expanded` binding. Still nobody has pressed a key: that is `VAL-H3`. See
  the [keyboard table evidence](docs/evidence/2026-09-22-h3-keyboard-table.md).
- `H3-Z` closed the checkpoint at `0.4.0`: the release binary is built,
  verified and deployed to the author's prefix. Nobody has looked at the
  Processes or Applications page on a real session yet, pressed a key on the
  table, or asked the kill dialog to act on a real process: `VAL-H1`,
  `VAL-H2` and `VAL-H3` all stay pending. See the
  [production completion record](docs/evidence/2026-09-22-h3-production-completion.md).
- As of `H3-E` the table's cursor is re-anchored wherever `entries` is
  rebuilt — every revision and every fold — so the selection no longer drifts
  a row per tick, and a selected process that died or was filtered out is let
  go rather than pointed at. `apply` no longer clears the action outcome, so
  a refusal or a failure stays readable until the selection moves or another
  action answers; `send_signal` re-reads `/proc/<pid>/stat` and
  `/proc/<pid>/status` immediately before the syscall and refuses unless the
  start time and the owner still match the snapshot's, which closes the gap a
  two-second-old snapshot leaves. The rows are `Qt.NoFocus` so the list is the
  table's one Tab stop, Space, Return, Left and Right fold an application
  under the cursor, the user column sorts by uid, the active column says its
  direction to a screen reader, the name column takes what the other columns
  leave and the two rate columns go below 760 logical pixels, `sampler::stop`
  drains its subscribers and lowers its flag so the hub can start again, the
  application row reports `Accessible.checkable`, and an unreadable `/proc` is
  announced before any note about a selected row. Deployed as `0.4.1`. Still
  nobody has pressed a key or sent a signal: `VAL-H3`. See the
  [table fixes evidence](docs/evidence/2026-09-22-h3-table-fixes.md).
- As of `H4-A` the crate also parses sensors: `sensors` reads one `hwmonN`
  directory's files into a `Chip` of `Channel`s, one per readable
  `<kind><n>_input` (a power channel prefers `_average`), each typed by
  `ChannelKind` (temperature, fan, voltage, power, current) with its own unit
  and kernel integer conversion, its label when the chip has one and its
  `max`/`crit` limits (a power channel's `cap` is its max), sorted by kind
  then index; a channel whose value file does not parse is skipped without
  disturbing the chip's other channels. All of it is under unit test and
  captured from the author's processor (`k10temp`), GPU (`amdgpu`) and board
  (`it8696`) chips; none of it is wired into the sampler thread or a page
  yet — that is `H4-B`. The deferred cgroup regression test from `H3-B`
  (a reverse-DNS application scope without an instance number) is in place
  too. See the [core evidence](docs/evidence/2026-09-22-h4-core.md).
- As of `H4-B` it is wired and the window is complete. The sampler reads
  `/sys/class/hwmon` on every tick — the labels and limits once per
  directory, the 46 `_input`/`_average` files of this machine's nine chips
  each second — and publishes a `sensors` section; an unreadable
  `/sys/class/hwmon` marks only that section. `publish::thermal_load` gives
  a temperature the same 80/90 thresholds as a fraction of the chip's own
  `crit`, and nothing else is ever other than `normal`. `HematitaSensors`
  publishes chips and channels as index-aligned lists with `revision` last,
  and is the only thing that remembers the session's extremes per channel.
  The Sensors page shows each chip as a `ListSection` card with one row per
  channel — label, value coloured by its load, the session's extremes and
  the kernel's limit — composing every word from tokens and showing the
  kernel's labels raw. The six process-table items parked by `H3-E` are
  closed: a successful action keeps its sentence through a rebuild, the
  header and the rows share one inset, a hidden table weaves nothing, the
  search is debounced by 150 ms, `layout()` is one pass, and
  `sampler::subscribe` registers inside the `handle` lock. Nobody has looked
  at the page: `VAL-H4`. See the
  [sensors page evidence](docs/evidence/2026-09-22-h4-sensors-page.md).
- As of `H4-C` the sensor reading and its gate are honest. `hwmonN` is an
  index, not an identity, so the cached labels and limits are re-validated
  against the chip's own `name` every tick and the session's extremes are
  keyed by that name too: a device given a freed index cannot inherit
  another's `crit` — which is what colours a temperature — or its extremes.
  A chip's rows are no longer torn down and rebuilt every second, so the
  keyboard can hold one. The smoke now walks every section one second apart,
  because a `StackLayout` builds only the page it shows and the three pages
  nobody selected had never been constructed in any headless run; all four
  now are, with no QML errors. `channelStates` is struck from the published
  contract rather than added: `discover` skips an unreadable channel, so the
  state it would carry is unreachable. Return applies the process search at
  once, and an untranslated chip name gets its ordinal. Still nobody has
  looked at the page: `VAL-H4`. See the
  [sensor gates evidence](docs/evidence/2026-09-22-h4-sensor-gates.md).
- `H4-Z` closed the checkpoint at `0.5.0`: the release binary is built,
  verified and deployed to the author's prefix. The Sensors page reads every
  hwmon chip the machine exposes and shows it as a card, one row per
  channel, with the value, the session's minimum and maximum, the kernel's
  own limit, and a thermal load graded against the chip's own `crit`; the
  headless smoke steps through Performance, Processes, Applications and
  Sensors one second apart and finds no QML error on any of them. Nobody has
  looked at the page on a real session yet: `VAL-H1` through `VAL-H4` all
  stay pending. See the
  [production completion record](docs/evidence/2026-09-22-h4-production-completion.md).
- As of `H4-D` the Sensors page is crossed the way the rest of the window is.
  It is a `ListView` of chip cards: one Tab stop, arrows that move card by
  card and scroll to what they reach, and a scroll bar reporting on it. The
  cards and the rows take no focus — forty-six focusable rows inside a
  surface with no current item were forty-six stops and a focus that could be
  stranded below the fold with nothing able to scroll to it — and each row
  still names its label, value, extremes and limit to a screen reader. A tick
  that cannot read `/sys/class/hwmon` now publishes empty lists and bumps its
  ticket, so the page shows only the reason; the session's extremes survive
  it, ready for the next good tick. The smoke prints and asserts the page's
  chip and channel counts, so a page that builds but publishes nothing fails
  the gate. Deployed as `0.5.1`. Still nobody has pressed a key on it:
  `VAL-H4`, which now asks for exactly that. See the
  [sensors fixes evidence](docs/evidence/2026-09-22-h4-sensors-fixes.md).

## Blockers

None recorded.
