# Magnetita implementation roadmap

- **Status:** planned
- **Active implementation checkpoint:** none
- **Related author validation:** `VAL-MAG-01` through `VAL-MAG-05` in
  [VALIDATION.md](VALIDATION.md); they do not block implementation

`MAG-M1` is the next settled reliability checkpoint and has no active execution
plan.

## MAG-M1 — Deterministic app read/watch lifecycle

## Hypothesis and tangible outcome

Owning the app's best-effort D-Bus reads and watcher lifecycle will let Magnetita
close deterministically without losing coalescing or blocking Qt. The tangible
outcome is a client that can be created, flooded with refreshes and destroyed
repeatedly with every owned worker joined and no stale snapshot applied.

## Scope

- Inventory every read/watch thread and callback currently detached from the
  app QObject lifecycle.
- Replace detached ownership with a bounded cancellation/shutdown contract.
- Preserve one ordered action worker and at most one coalesced follow-up read.
- Reject callbacks after the owning QObject begins shutdown.
- Add repeated create/burst/close tests and update lifecycle documentation.

## Exclusions

- Changing the KDE Connect wire protocol or `org.celestina.Devices1` semantics.
- Re-pairing, deleting trust state or restarting the live daemon.
- New plugins or Android-side work.
- Real phone, mount, artwork and Wayland acceptance.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-M1-A | planned | none | Complete ownership map and failing close/burst regression | Focused app lifecycle test |
| MAG-M1-B | planned | MAG-M1-A | Cancelable, joined read/watch lifecycle | Repeated create/burst/close test |
| MAG-M1-C | planned | MAG-M1-B | App, daemon and D-Bus consumers remain compatible; installed bytes are current | `scripts/complete-production.sh` |

## Implementation exit

Close `MAG-M1` when every app-owned thread has a deterministic termination path,
refresh coalescing still delivers the newest confirmed snapshot, post-shutdown
callbacks are rejected and the exact production artifacts pass
`scripts/complete-production.sh`, including deployment to the author's normal
test destination without a second build. If an implementation unit changes
`magnetita-core`, it also runs `celestina/scripts/complete-production.sh` so the
installed shell bundle carries the same shared contract without activating the
live session. Do not wait for a real phone session to close the checkpoint.

## Closed evidence

The released CP0-CP4 implementation and 2026-07-29 hardening record are
preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
