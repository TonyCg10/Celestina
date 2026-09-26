# Celestina Rust workspace implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** RS-H1
- **Related author validation:** product-level queues linked from
  [VALIDATION.md](VALIDATION.md); they do not block implementation

`CORE-M1` remains the next settled candidate after `RS-H1` and has no
active execution plan.

## RS-H1 — Shared owners after the monorepo audit

The falsifiable problem found by the audit: the same rule is implemented
several times with different answers. `file://` URIs are turned into paths
about seven times; `atomic_file::replace` has no mode, so private state comes
out world-readable and user media loses its mode; nothing owns
`XDG_RUNTIME_DIR`, and several copies fall back to world-writable `/tmp`;
readers of small state files bound nothing; and the `.desktop` scan is copied
three times, unbounded, partly on the Qt thread.

The boundary is `celestina-core`: each rule gains one tested owner, purely
added (ruling R-A3), so no consumer's behaviour changes until its own bug
unit adopts it. The tangible outcome is those owners with their tests, and a
workspace README, AGENTS and crate docs that match the checkout.

The plan is
[Shared owners after the monorepo audit](docs/plans/active/2026-09-26-hardening.md),
with the single unit `RS-H1-A`. It records `dotfiles-core`'s missing consumer
as an exclusion for the author's decision (ruling R-A4). The findings are in
[the shared crates audit record](../docs/evidence/2026-09-26-monorepo-audit-shared-crates.md).

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
| CORE-M1-C | planned | CORE-M1-B | Workspace contracts remain valid and Siderita's installed binary contains the changed core | Workspace verification, then `siderita/scripts/complete-production.sh` |

## Implementation exit

Close `CORE-M1` when the focused cancellation tests pass, no stale generation
publishes, shutdown joins cleanly, the canonical workspace artifact passes its
registered verification, and `siderita/scripts/complete-production.sh` deploys
the exact affected Siderita bytes to the author's normal test destination. Do
not add a pending Siderita interaction test to this checkpoint.

## Closed evidence

Completed CORE-0 through CORE-2 work and its evidence are archived in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
