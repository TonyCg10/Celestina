# Hematita status

- **Updated:** 2026-11-25
- **Active checkpoint:** none. `HEM-H1`, the hardening that follows the
  2026-09-26 monorepo audit, closed on 2026-11-25 with every row done; its
  [plan](docs/plans/archive/2026-09-26-hardening.md) is archived
- **Author validation pending:** `VAL-S1`, `VAL-S2` and `VAL-S3` (storage),
  in [VALIDATION.md](VALIDATION.md)
- **Follow-ups recorded by `HEM-H1-B`, not scheduled:** a deadline for
  opening a bus connection (the sampler's listing and a unit action open
  theirs with a blocking handshake that has none; a hung bus stalls the
  sampler's ticks, though no longer the window's shutdown); and, outside
  Hematita's prefix, `celestina-core::desktop_entry`'s adoption note, which
  still says no consumer calls `find` although Hematita now does (a
  `celestina-rs` unit updates it)
- **Author validation:** `VAL-H1` passed 2026-09-23 (remediation `VIS-1`
  validated through `VAL-VIS-2`); `VAL-H2` requested, not run; `VAL-H3` failed
  2026-09-23, remediation `VIS-1` validated through `VAL-VIS-2`; `VAL-H4`
  failed 2026-09-23, remediation `VIS-1` validated through `VAL-VIS-2`;
  `VAL-H5` requested, not run; `VAL-VIS-1` failed 2026-09-23 (remedied by
  `VIS-2`); `VAL-VIS-2` passed 2026-09-23 — the process table kept its scroll
  position across refreshes, applications opened folded, no flicker on
  refresh

## Current checkout truth

- Version 1.3.2 in the checkout (the last delivery, `HEM-H1-E`). The
  delivered checkpoints, their corrections and the per-unit records are in
  [the status history](docs/history/status-through-2026-11-25.md).
- Known limits recorded when storage was delivered (`S1`), still open: a
  btrfs subvolume is its own mount boundary rather than part of the
  filesystem it lives on; a scanned entry can still be replaced between the
  scan and an action (narrowed, not closed, by `S1-E`'s device-and-inode
  re-validation); and duplicate groups beyond the five-hundredth are counted
  but not listed. The wording is in the history's `S1` entries.
- Suite conventions ([ADR 0012](../docs/decisions/0012-suite-conventions.md),
  `CONV-1`): Hematita serves `org.celestina.Hematita` through
  `celestina_core::activation`: a second launch, or Siderita's «Abrir en» on a
  folder, browses that folder in the running window's storage section. The
  window follows the suite's appearance file (reduced motion, the text scale
  on the font tokens). Hematita accepts no drops.
- `CONV-1-A` (suite): HEM-H1-F's claim-first hand-off moved into
  `celestina_core::activation` (HEM-16 done by reference). The served
  interface is now the suite's `org.celestina.Application1`
  (`Activate()`, `Open(as paths)` with byte-exact path keys) instead of
  `org.celestina.Hematita`'s `Open(s)`; behaviour is unchanged: a second
  launch raises the window and browses the folder it was handed
  ([evidence](../docs/evidence/2026-10-09-shared-activation.md)).
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
- Residuals of `S2`: a rename of the analysed root while a deletion runs is
  not detected by the descriptor walk; a mount created between the
  read-only pass and the removal pass is caught only by the in-walk check,
  after siblings may have gone; the confirm worker's `Weak<Tree>` still
  lets `make_mut` copy once when a comparison is mid-flight; any prune or
  graft bumps the selection revision, so a graft landing under an open
  dialog refuses harmlessly; duplicate candidates inside a grafted folder
  are not offered again until a full rescan.
- Residuals of `S3`: Space, Delete and the D-Bus hand-off are untried on a
  real session (`VAL-S3`); the desktop entry declares no
  `MimeType=inode/directory`; an `Open` that arrives during a scan drops
  the analysis in progress.

## Blockers

None recorded.
