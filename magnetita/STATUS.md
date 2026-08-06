# Magnetita status

- **Updated:** 2026-08-05
- **Implementation:** `MAG-S1` is active — the 2026-08-05 static audit's
  Magnetita findings are corrected in source and covered by tests, but not
  built, deployed or committed; `MAG-M1` remains planned
- **Author validation:** the original 1.0 daily set passed on the real phone;
  the hardening revalidation and the two new `MAG-S1` checks are pending in
  [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- `magnetitad` implements KDE Connect discovery, TCP/TLS trust, local pairing,
  storage mount, the daily plugins and `org.celestina.Devices1`.
- Siderita consumes the mount/device/media contract; the Celestina shell
  consumes phone/battery state; the standalone Magnetita app owns pairing,
  diagnostics and settings UI.
- Pairing v8 timestamp/code validation, identity binding, bounded admission,
  durable revocation, payload publication barriers and typed MPRIS actions are
  implemented and covered by unit/loopback evidence.
- App actions use one ordered owned worker. Snapshot reads/watchers remain
  detached best-effort work and still lack a fully deterministic shutdown path.
- Live phone evidence for the released daily set predates the 2026-07-29
  hardening; it is not reused as proof of the corrected paths.
- The `MAG-S1` corrections exist only in the worktree. The installed daemon
  still carries the argument-injection, handshake-deadline, protocol-floor,
  subprocess-bounding and key-permission defects, because the author asked for
  the corrections without a production build or deployment.

## Planned implementation debt

- Give the app's detached read/watch side explicit ownership, cancellation and
  deterministic join without regressing burst coalescing (`MAG-M1`).
- Keep packaging and resource diagnostics aligned with the canonical production
  artifact workflow. Service activation remains deploy-only.
- SMS, contacts, remote input, mDNS and newer protocol work stay conditional on
  a demonstrated need or reference-client requirement; they are not active.

## Blockers

`MAG-S1` cannot close until the author requests the canonical production exit;
that is a pending authorization, not a technical blocker. No other
implementation blocker is recorded. The real phone/network is required only
for the independent validation queue.

## Evidence boundary

CP0-CP4, dependency decisions and the earlier real-phone observations are in the
[archived roadmap](docs/history/roadmap-through-2026-08-03.md). On 2026-08-03
the exact app/daemon release bundle passed format, Clippy, client/core/net/daemon
tests, QML lint and isolated smoke; loopback tests were rerun outside the socket-
restricted sandbox and passed. See the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). The daemon
service and installed bytes were not touched. On 2026-08-05 the `MAG-S1`
corrections passed format, Clippy, the three crates' unit tests (207 tests, 0
failures), the workspace check, QML lint and the architecture contract; see the
[hardening evidence](docs/evidence/2026-08-05-network-input-hardening.md). No
build, deployment or service action was taken.

## Records

- [Implementation roadmap](ROADMAP.md)
- [Active plan MAG-S1](docs/plans/active/2026-08-05-network-input-hardening.md)
- [Author validation](VALIDATION.md)
- [Registry entry](../docs/projects.toml)
