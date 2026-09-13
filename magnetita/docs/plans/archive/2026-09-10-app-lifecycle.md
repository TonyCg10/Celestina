# MAG-M1 — Deterministic app read/watch lifecycle

- **Opened:** 2026-09-10
- **Closed:** 2026-09-13
- **Plan ID:** app-lifecycle
- **Status:** done
- **Authorization:** the author said "sigamos en lo que queda del roadmap"
  on 2026-09-10 with `MAG-P7` closed as far as `VAL-MAG-14` allows
- **Scope:** magnetita
- **Implementation checkpoint:** MAG-M1
- **Author-validation checkpoint:** none; the roadmap closes `MAG-M1` on
  its own tests and the production exit
- **Successor:** MAG-D1

## Hypothesis

Owning the app's best-effort D-Bus reads and its signal watchers under one
owner per QObject lets Magnetita close deterministically without losing
coalescing or blocking Qt: every worker is joined on drop, a result that
arrives after shutdown began is dropped instead of applied, and a burst of
signals still costs at most one follow-up read.

## Tangible outcome

A client that can be created, flooded with refreshes and destroyed
repeatedly with every owned worker joined and no stale snapshot applied,
proven by a test that closes an owner under sixteen workers twenty times.

## Scope

- `MAG-M1-A` — the ownership map of every thread the app spawned and the
  regression: `lifecycle::Owned` with its guard, `lifecycle::Reload` for
  the coalescing, and the test that closes under a burst.
- `MAG-M1-B` — the cancelable, joined lifecycle in the models: the device
  model's reads, watches, mirror snapshot and pairing window, the messages
  model's refresh and the commands model's refresh and edits run under
  their owner; the bus watches return when the owner closes.
- `MAG-M1-C` — the app, the daemon and the D-Bus consumers unchanged in
  contract; the installed bytes current through
  `scripts/complete-production.sh`.

## Exclusions

- Changing `org.celestina.Devices1` or `Mirror1`.
- The daemon; the Android application.

## Build order

1. `MAG-M1-A`, `-B`, `-C`.

## Implementation exit

Every app-owned thread has a deterministic termination path, refresh
coalescing still delivers the newest confirmed snapshot, post-shutdown
callbacks are rejected, and the production artifacts pass
`scripts/complete-production.sh`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-M1-A | `magnetita:` | done | [inventory](../../inventories/2026-09-10-app-lifecycle/MAG-M1-A.numstat.tsv) | 5 files, +335/-0 | The owner, the guard, the coalescer and the close-under-burst test | [record](../../evidence/2026-09-10-app-lifecycle.md) | none |
| MAG-M1-B | `magnetita:` | done | [inventory](../../inventories/2026-09-10-app-lifecycle/MAG-M1-B.numstat.tsv) | 17 files, +431/-237 | The three models' reads, watches and edits under their owner; `MAG-P7` archived and `MAG-M1` opened | [record](../../evidence/2026-09-10-owned-models.md) | none |
| MAG-M1-C | `magnetita:` | done | [inventory](../../inventories/2026-09-10-app-lifecycle/MAG-M1-C.numstat.tsv) | 4 files, +115/-8 | Contracts unchanged; the installed bytes current | [record](../../evidence/2026-09-10-app-lifecycle-exit.md) | none |
