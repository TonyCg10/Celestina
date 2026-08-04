# Siderita implementation roadmap

- **Status:** planned
- **Active implementation checkpoint:** none
- **Related author validation:** `VAL-SID-04` in
  [VALIDATION.md](VALIDATION.md); it does not block implementation

`SID-M1` is the next settled checkpoint and has no active execution plan.

## SID-M1 — Parent portal pickers on Wayland

## Hypothesis and tangible outcome

Importing the portal's `wayland:` parent handle through a bounded
`xdg-foreign` adapter will make each picker a transient child of its requester
without coupling the portal contract to QML ids or blocking D-Bus. The tangible
outcome is an inspectable picker lifecycle that accepts valid handles, degrades
on invalid/unsupported ones and still answers every request.

## Scope

- Parse and validate the portal parent handle without trusting arbitrary input.
- Add the smallest Qt/Wayland seam required to import the foreign parent and
  apply it before the picker maps.
- Preserve concurrent picker requests, cancellation and reply delivery.
- Degrade to the current free-floating picker when the protocol or handle is
  unavailable; never fail the file request solely because parenting failed.
- Add focused parser/lifecycle tests and update the portal contract/status.

## Exclusions

- Changing portal routing or the author's `portals.conf`.
- Installing/activating the backend during verification.
- Redesigning picker browsing, adding file operations or moving it into the main
  window.
- Manual requester/Wayland acceptance, tracked as `VAL-SID-04`.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| SID-M1-A | planned | none | Typed bounded parser and invalid-handle fallbacks | Focused Rust/C++ tests |
| SID-M1-B | planned | SID-M1-A | Minimal imported-parent lifecycle wired before map | Qt lifecycle test where headless support permits |
| SID-M1-C | planned | SID-M1-B | Portal remains compatible and the author's binary carries the verified bytes | `scripts/complete-production.sh` |

## Implementation exit

Close `SID-M1` when valid/invalid/unsupported handle paths are covered, portal
requests still answer and `scripts/complete-production.sh` builds, verifies and
deploys those exact bytes so the author's normal binary needs no rebuild. Do not
keep the checkpoint open for the real Wayland parent-child observation; that
result belongs to `VAL-SID-04`.

## Closed evidence

CP0-CP7, including both content integrations, are preserved in the
[roadmap history](docs/history/roadmap-through-2026-08-03.md).
