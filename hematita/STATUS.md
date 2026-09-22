# Hematita status

- **Updated:** 2026-09-22
- **Delivered as 0.3.0:** H2 — the Performance page lists the processor (with
  a per-core grid), memory with swap, the AMD GPU, every whole disk and every
  network interface, each with live graphs, built, verified and deployed to
  the author's prefix
- **Author validation:** `VAL-H1` requested, not run; `VAL-H2` requested, not
  run
- **Active phase:** none; `H3` (processes and applications) is planned, not
  yet opened

## Current checkout truth

- The project is registered and builds a release binary. The window shows the
  pill strip with four sections; only Performance has a page, and the other
  three say so.
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
  `~/.local/bin/hematita` at `0.3.0`, with its desktop entry and icon set
  registered under `~/.local/share`; the installed bytes match the checkout's
  `hematita/target/release/hematita` byte for byte. See the
  [production completion record](docs/evidence/2026-09-22-h2-production-completion.md).
- The side list's off-screen rows are not yet keyboard-reachable; `H3-A`
  books it.

## Blockers

None recorded.
