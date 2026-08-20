# Magnetita status

- **Updated:** 2026-08-19
- **Implementation:** `MAG-S1` (hostile network input) and `MAG-R1` (the
  one-button wireless mirror) are delivered, committed and deployed. `MAG-R2`
  (the mirror without discovery) is committed but **not deployed**. `MAG-M1`
  remains planned and unimplemented
- **Author validation:** the original 1.0 daily set passed on the real phone,
  before the 2026-07-29 hardening. **Every one of `VAL-MAG-01` through
  `VAL-MAG-04` and `VAL-MAG-06` through `VAL-MAG-09` is still pending**, so no
  corrected path has an author pass against the phone. This, not the code, is
  Magnetita's largest open risk

## Current checkout truth

- The checkout is clean of Magnetita work: `MAG-S1`, `MAG-R1` and `MAG-R2` are
  all committed.
- The installed daemon carries `MAG-S1`, `MAG-R1` and `MAG-R2`'s port pinning
  and remembered endpoint, but **not** `MAG-R2`'s stale-advertisement fallback:
  `verify-production.sh` runs a workspace-wide `cargo fmt --all --check` and
  unrelated in-flight `grafita-core` work was failing it when that correction
  landed. Redeploying is the only thing standing between the tree and the
  installed bytes.
- The mirror was observed working end to end against the real S25U, including
  pairing on six digits alone and pinning to the fixed port. What has never been
  observed is a mirror after a *phone* reboot (`VAL-MAG-09`).

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
- `SendFileUri` on
  `org.celestina.Devices1`. It names the file by the percent-encoded `file://`
  URI the portal and the clipboard already speak, decodes it by bytes with
  `celestina_core::percent`, and refuses a URI that is not a local `file://` one
  or whose escapes are malformed with a typed reason. `Command::SendFile` now
  carries a `PathBuf`, so a filename that is not valid UTF-8 reaches
  `serve_file` unaltered. `SendFile` itself is unchanged and stays for
  compatibility: it is a published interface and altering the meaning of its
  argument would break any other caller. Siderita's send-to-phone menu item is the
  first consumer, under its own `SID-G7-G`. Committed and deployed; no live file
  transfer has been observed since — that is `VAL-MAG-HARDENING`. See the
  [byte-exact send evidence](docs/evidence/2026-08-06-byte-exact-send-to-phone.md).
- The `MAG-S1` corrections are committed and installed. They have never been
  exercised against the phone: `VAL-MAG-06` is what would prove the corrected
  boundaries hold, and it has not been run.

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

On 2026-08-06 `MAG-S1-B` passed format, Clippy and the unit tests for the
`celestina-rs` workspace with `celestina-shell-core` excluded, which the
author's hardware-safety hold puts out of bounds and which this unit does not
depend on. Nothing was exercised over a real bus: the new method's decode and
its refusals are proven, the delivery of a file to a phone is not. See the
[byte-exact send evidence](docs/evidence/2026-08-06-byte-exact-send-to-phone.md).

## Records

- [Implementation roadmap](ROADMAP.md)
- [Active plan MAG-S1](docs/plans/active/2026-08-05-network-input-hardening.md)
- [Author validation](VALIDATION.md)
- [Registry entry](../docs/projects.toml)
