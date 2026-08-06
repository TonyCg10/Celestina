# MAG-S1 — hostile network input at the daemon's boundaries

- **Opened:** 2026-08-05
- **Plan ID:** network-input-hardening
- **Status:** active
- **Authorization:** the author requested the Magnetita corrections raised by
  the read-only static suite audit be implemented, and explicitly excluded
  commit, production build, deployment and any change to the live service
- **Scope:** magnetita
- **Implementation checkpoint:** MAG-S1
- **Author-validation checkpoint:** `VAL-MAG-06` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

Every finding below is one boundary trusting a value the peer chose. A packet
field became a process argument, a mount host, a map key, a rich-text label; a
socket timeout stood in for a deadline; a peer-declared version decided whether
its own identity had to be proved. Validating each of those where the value
becomes typed — and never above it — removes the class, not the instances.

## Tangible outcome

A paired phone, a compromised app on it, or anyone able to send a UDP datagram
on the LAN can no longer execute a command as the desktop user, redirect the
mount, hold an admission permit indefinitely, pin its certificate under another
device's id, occupy the real phone's registry slot, stall the link pump on a
subprocess, or read `privateKey.pem`. Each is covered by a test in its own unit.

## Scope

Audit identifiers from
[`../../../../docs/evidence/2026-08-05-static-suite-audit.md`](../../../../docs/evidence/2026-08-05-static-suite-audit.md):

- **MAG-C1** — `sshfs` argument injection, and the peer-chosen mount host.
- **MAG-A1** — the handshake's missing absolute deadline, and silent admission
  exhaustion.
- **MAG-A2 with MAG-M2 and MAG-M5** — the peer-selected protocol downgrade, the
  UDP-announced identity, the unrestricted dial target, and registry
  publication before the trust check.
- **MAG-A3 with the mount half of MAG-M1** — unbounded subprocesses on the link
  pump thread.
- **MAG-M7** — the private key's world-readable window and non-atomic write.
- **MAG-M6** — bounding peer-chosen text at decode, and refusing to render it
  as markup.

## Exclusions

- Commit, version transition, version history, production build, verification,
  deployment, and any restart or inspection of the live `magnetitad`.
- `MAG-M1`'s app-side read/watch lifecycle, which keeps its own checkpoint.
- The `Revocations`/registry locks held across file I/O in
  `payload_handlers.rs::with_live_paired_device`. Reducing that critical
  section changes the lock ordering the revocation barrier depends on, and the
  author did not authorize reordering that policy. Recorded, not attempted.
- MAG-B6 and the session-bus trust model, which the audit did not schedule.
- Anything outside `magnetita/` and the three registered crates.

## Build order

1. Close the reachable code-execution path first (MAG-C1): it is the only
   finding an already-paired phone can turn into a shell.
2. Then the network-exposure and protocol-binding findings, which share one
   idea — the peer does not get to choose what proves its identity.
3. Then the daily-robustness findings, which are corrections to disciplines the
   codebase already implements correctly elsewhere and merely did not reuse.

## Implementation exit

```sh
cd celestina-rs
cargo fmt -p magnetita-core -p magnetita-net -p magnetitad
cargo clippy -p magnetita-core -p magnetita-net -p magnetitad --all-targets
cargo test -p magnetita-core -p magnetita-net -p magnetitad
qmllint magnetita/qml/components/*.qml
```

The canonical `scripts/complete-production.sh` exit is **not** part of this
plan's execution: the author asked for the corrections without a production
build or deployment, so the units stay `active` until that is requested.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-S1-A | `magnetita:` | done | [inventory](../../inventories/2026-08-05-network-input-hardening/MAG-S1-A.numstat.tsv) | 34 files, +1987/-387 | Validate the SFTP user, path and password at the decode boundary and drop the peer-supplied `ip`, so a mount argument can no longer become an `ssh` option or point at a host of the phone's choosing; give the whole handshake one absolute deadline and log an exhausted admission pool; floor the protocol at version 8 and take the peer's identity only from the encrypted re-exchange, dialling only a local address on the announced port and publishing a device entry only after the trust check; extract the bounded-subprocess discipline from the media worker and apply it to the clipboard and the mount; write the private key 0600 and atomically; bound peer-supplied text and the notification map; and render remote strings as plain text | `cargo test`, `cargo clippy`, `cargo fmt` for the three crates and the application — recorded in [network input hardening evidence](../../evidence/2026-08-05-network-input-hardening.md) | `VAL-MAG-HARDENING` |

Every unit stays `active`. A `done` unit requires an exact immutable inventory
and the single commit that carries it, and the author has not requested a
commit.

## Recorded limitation

`celestina_core::atomic_file::replace` is the suite's owner of atomic state
publication and is used here for `certificate.pem`. It cannot be used for
`privateKey.pem`: it creates its temporary at the process umask, which is
precisely the world-readable window MAG-M7 describes. `cert.rs` therefore keeps
its own sibling-then-rename with `mode(0o600)` at creation, and names the
reason at the call site. Adding a mode-restricted variant to `celestina-core`
is the better resolution and belongs to that crate's owner, outside this plan's
authorized paths.

The roadmap's five build-order steps deliver as one ledger unit. They were found
and fixed against each other in the same files — the mount is both the injection
sink and a subprocess to bound, the certificate module holds both the
verification key and the private-key write — so exclusive per-step inventories
would claim a boundary a single commit cannot produce.
