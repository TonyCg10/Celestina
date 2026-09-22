# Hematita status

- **Updated:** 2026-09-21
- **Implementation:** H1 is active; the skeleton builds and opens the window
  with the navigation strip, and `hematita-core` now parses CPU, memory and
  history from `/proc`
- **Author validation:** `VAL-H1` requested, not run

## Current checkout truth

- The project is registered and builds a release binary. The window shows the
  pill strip with four sections; only Performance gains a page in H1.
- `hematita-core` provides `ratio`, `cpu`, `memory` and `history`: `/proc/stat`
  and `/proc/meminfo` parsing, a CPU sampler that turns two readings into a
  rate, cpufreq and model parsing, and a sixty-sample ring, all covered by
  unit tests and captures from the author's machine. No consumer wires this
  into the window yet.

## Blockers

None recorded.
