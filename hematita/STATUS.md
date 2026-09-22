# Hematita status

- **Updated:** 2026-09-21
- **Delivered as 0.2.0:** H1 — the window opens with the navigation strip and
  a live Performance page reading CPU and memory once a second, built,
  verified and deployed to the author's prefix
- **Author validation:** `VAL-H1` requested, not run
- **Next phase:** H2 (disks, network, GPU and swap; the per-core grid),
  planned, not opened

## Current checkout truth

- The project is registered and builds a release binary. The window shows the
  pill strip with four sections; only Performance has a page, and the other
  three say so.
- `hematita-core` provides `ratio`, `cpu`, `memory` and `history`: `/proc/stat`
  and `/proc/meminfo` parsing, a CPU sampler that turns two readings into a
  rate, cpufreq and model parsing, and a sixty-sample ring, all covered by
  unit tests and captures from the author's machine.
- A sampling thread reads every source once a second off the Qt thread and
  publishes one whole snapshot; `HematitaResources` applies it as typed
  properties, owns the 80/90 load thresholds and the two history rings, and
  drops a snapshot older than the one it already applied. A source that cannot
  be read names itself in the page instead of freezing its number.
- The Performance page shows the processor and memory in a side list with
  sparklines and the selected one large, with its minute graph and its facts.
  Both histories cross to QML as a `QVariant` carrying a variant list: it is
  the one shape `qmllint` resolves, and this project suppresses no warnings.
  Neither reading claims a number before its first sample arrives.
- A second `hematita` raises the running window over the session bus and exits
  without building one; no session bus is never fatal.
- Nobody has looked at the page on a real session yet: the evidence for H1-C is
  offscreen only, and appearance, keyboard and focus belong to `VAL-H1`.
- The release binary is built, verified and installed at
  `~/.local/bin/hematita`, with its desktop entry and icon set registered
  under `~/.local/share`; the installed bytes match
  `hematita/target/release/hematita` by `sha256sum`. See the
  [production completion record](docs/evidence/2026-09-21-h1-production-completion.md).

## Blockers

None recorded.
