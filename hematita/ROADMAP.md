# Hematita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** H1
- **Related author validation:** `VAL-H1` in [VALIDATION.md](VALIDATION.md)
  (does not block)

## Hypothesis and tangible outcome

A window that samples `/proc` once per second on its own thread can show CPU
and memory with sixty seconds of history, through parsers tested on text
alone, without the machine noticing the monitor.

## Scope

| Phase | Outcome |
|---|---|
| H1 | Registered project, skeleton, navigation strip, Performance page with CPU and memory |
| H2 | Disks, network, GPU and swap; the per-core grid |
| H3 | Processes and applications |
| H4 | Sensors (all of hwmon) |
| H5 | Services and privileged actions |

## Exclusions

- Per-process network; non-AMD GPUs; history persistence; alerts; any shell
  integration.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| H1-A | done | none | crate stub, application skeleton, strip, scripts, documents | `scripts/smoke.sh`, guards |
| H1-B | done | H1-A | `hematita-core`: ratio, cpu, memory, history with captures | `cargo test -p hematita-core` |
| H1-C | done | H1-B | sampler, `HematitaResources`, activation, Performance page | `scripts/verify-production.sh` |
| H1-Z | planned | H1-C | implementation exit and 0.2.0 | `scripts/complete-production.sh` |

## Implementation exit

`scripts/complete-production.sh` succeeds: the release build, its verification
(crate tests, clippy, fmt, qmllint ratchet, smoke) and the deployment to the
author's prefix; the installed binary shows live CPU and memory graphs.

## Closed evidence

- H1-A: [skeleton](docs/evidence/2026-09-21-h1-skeleton.md)
- H1-B: [core](docs/evidence/2026-09-21-h1-core.md)
- H1-C: [performance page](docs/evidence/2026-09-21-h1-performance-page.md)
