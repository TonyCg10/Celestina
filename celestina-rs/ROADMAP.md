# Celestina Rust workspace implementation roadmap

- **Status:** planned
- **Active implementation checkpoint:** none
- **Related author validation:** product-level queues linked from
  [VALIDATION.md](VALIDATION.md); they do not block implementation

`CORE-M1` is the next settled candidate and has no active execution plan.

## CORE-M1 — Supersede running scans

## Hypothesis and tangible outcome

When a newer Siderita scan is enqueued, cancelling the scan already in progress
will stop obsolete filesystem work without allowing a stale result to publish or
breaking deterministic shutdown. The tangible result is a tested executor that
converges on the newest request under a deliberately slow scan.

## Scope

- Cancel the running `siderita-core` scan when a newer request supersedes it.
- Preserve generation rejection, bounded queueing and deterministic join.
- Add focused tests for in-flight cancellation, newest-result publication and
  shutdown while cancellation is pending.
- Update the affected public contract and current status.

## Exclusions

- Public API/versioning policy before an accepted decision exists.
- A Qt QObject inside `siderita-qt`; UI adapters remain application-owned.
- Transactional dotfiles application or speculative shared config/IPC crates.
- Real-session interaction, which belongs to Siderita validation.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| CORE-M1-A | planned | none | Reproduction proving obsolete in-flight work survives today | Focused failing executor test |
| CORE-M1-B | planned | CORE-M1-A | Running scan receives cancellation when superseded | `cargo test -p siderita-core --locked` |
| CORE-M1-C | planned | CORE-M1-B | Workspace contracts and artifact remain valid | `scripts/build-production.sh` then `scripts/verify-production.sh` |

## Implementation exit

Close `CORE-M1` when the focused cancellation tests pass, no stale generation
publishes, shutdown joins cleanly and the canonical workspace artifact passes
`scripts/verify-production.sh`. Do not add a pending Siderita interaction test
to this checkpoint.

## Closed evidence

Completed CORE-0 through CORE-2 work and its evidence are archived in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
