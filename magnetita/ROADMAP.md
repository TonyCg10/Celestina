# Magnetita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** MAG-S1
- **Related author validation:** `VAL-MAG-01` through `VAL-MAG-07` in
  [VALIDATION.md](VALIDATION.md); they do not block implementation

`MAG-S1` is executing under
[its plan](docs/plans/active/2026-08-05-network-input-hardening.md). `MAG-M1`
remains the next settled reliability checkpoint and has no execution plan.

## MAG-S1 — Hostile network input at the daemon's boundaries

## Hypothesis and tangible outcome

Every defect the 2026-08-05 static audit raised against Magnetita is one
boundary trusting a value the peer chose, so validating each where the value
becomes typed removes the class rather than the instances. The tangible outcome
is a daemon a paired phone cannot turn into command execution, a mount cannot
be redirected away from the authenticated link, a handshake that ends on an
absolute deadline, a protocol floor the peer cannot argue its way below, and a
private key no other local user can read.

## Scope

- Validate the `kdeconnect.sftp` user, path and password at the decode boundary
  and mount only against the TLS-authenticated address (`MAG-C1`, `MAG-M6`).
- Bound the whole handshake with one absolute deadline and log admission
  exhaustion (`MAG-A1`).
- Floor the protocol at 8, take the peer identity only from the encrypted
  channel, restrict dialling to the standard port on local addresses, and
  decide trust before publishing a device (`MAG-A2`, `MAG-M2`, `MAG-M5`).
- Apply the existing bounded-subprocess discipline to the clipboard and the
  mount (`MAG-A3`).
- Create the private key owner-only and atomically; bound peer-chosen text and
  the notification map; render remote strings as plain text (`MAG-M7`).

## Exclusions

- Commit, version transition, production build, deployment, and any restart or
  inspection of the live `magnetitad`.
- `MAG-M1`'s app-side read/watch lifecycle.
- The `Revocations`/registry locks held across payload file I/O, which cannot
  be shortened without reordering the revocation barrier's locking policy.
- Any project other than Magnetita and its three registered crates.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| MAG-S1-A | done | none | The SFTP reply cannot become an sshfs option or redirect the mount | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |
| MAG-S1-B | done | none | One absolute handshake deadline from the crate's single owner of that recipe | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |
| MAG-S1-C | done | MAG-S1-B | Protocol floor, encrypted-only identity, bounded dial target, trust before publication | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |
| MAG-S1-D | done | none | Clipboard and mount subprocesses bounded and reaped like media's | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |
| MAG-S1-E | done | none | Owner-only atomic key, bounded peer text, plain-text rendering | [evidence](docs/evidence/2026-08-05-network-input-hardening.md) |

## Implementation exit

Close `MAG-S1` when every unit's tests pass alongside format, Clippy, the
workspace check, QML lint and the architecture contract, **and** the author has
requested the canonical `scripts/complete-production.sh` exit. The corrections
were authorized without a production build or deployment, so the units stay
`active` and the installed daemon still carries the uncorrected bytes until
that request arrives. `magnetita-core` changed, so closing will also require
`celestina/scripts/complete-production.sh`.

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
