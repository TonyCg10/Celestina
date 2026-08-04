# R3 — session verbs

- **Opened:** 2026-08-03
- **Status:** active
- **Plan ID:** r3-session-verbs
- **Scope:** celestina
- **Implementation checkpoint:** R3
- **Author-validation checkpoint:** `VAL-R3` in [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

One versioned shell command path can apply keyboard-driven session changes and
publish their confirmed state while a reusable, reduced-motion-aware OSD shows
only values the provider actually reported.

## Tangible outcome

The production shell bundle accepts the implemented session verbs, reports
confirmed or failed outcomes, and renders a non-activating testable OSD path.

## Scope

In scope: typed volume, brightness, DPMS and session verbs; OSD surface;
bounded night-light lifecycle; caffeine/idle-inhibit state; a fail-closed
lock-and-suspend contract that refuses without a provider; opt-in configuration
and rollback instructions.

## Exclusions

Out of scope: selecting, installing or integrating a concrete locker while
SHELL-D1 remains open; applying live Niri configuration, suspending the session,
first-party PAM/lock UI, notifications and control-center work.
This plan records work but grants no authorization beyond the repository rules.

## Build order

1. Extend pure command/state policy and protocol tests.
2. Wire provider implementations and confirmed failure paths outside the GUI
   thread.
3. Add the OSD surface and QML composition over shared style contracts.
4. Add deterministic night-light and idle-inhibit lifecycles.
5. Add DPMS and the fail-closed lock refusal/extension seam.
6. Write reversible configuration instructions and run the automated exit.

## Implementation exit

- Protocol, bounds, timeout, refusal and lifecycle tests pass.
- CMake registration, QML lint and CTest pass.
- Rust format, Clippy and package tests pass with the lockfile unchanged.
- The architecture and documentation contracts pass.
- `scripts/complete-production.sh` builds once, verifies those exact bytes and
  updates the on-disk bundle that deploy/activate consume; no second
  compilation is required and the live session is never replaced.
- The author-test bundle is updated, but no live process or session is replaced,
  no package manager or system prefix is touched, no service is changed and no
  live configuration is edited by the automated exit.

R3 implementation closes on this evidence. The live OSD, monitor, gamma,
idle/DPMS and lock/suspend checks remain an independent `VAL-R3` run.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and
stable symbols are authoritative; line counts are a hand-off aid and may drift.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| R3-A | `celestina:` | planned | `celestina-shell-core`, shell command protocol | — | Typed session verbs and policy | Unit and protocol tests | None |
| R3-B | `celestina:` | planned | provider adapter, Qt host | — | Bounded providers and confirmed outcomes | Rust/CTest lifecycle cases | None |
| R3-C | `celestina:` | planned | surface manager, OSD QML, style consumer | — | OSD presentation and reduced motion | registration, lint, offscreen tests | `VAL-R3` |
| R3-D | `celestina:` | planned | night light, idle inhibit, DPMS, lock refusal seam | — | Deterministic session lifecycles without an assumed locker | failure/lifecycle tests | `VAL-R3` |
| R3-E | `celestina:` | planned | docs and opt-in config examples | — | Reversible handover instructions | documentation contract | `VAL-R3` |

## Decisions and rollback

The locker choice is intentionally open in
[`../../discussions/README.md`](../../discussions/README.md); that slice must not be
implemented by guessing or installing a package. When SHELL-D1 is applied, its
concrete composition receives a new bounded implementation unit rather than
being appended silently to this ledger. Each external lifecycle must stop
cleanly when the shell stops. The old Niri binds and Noctalia paths remain the
rollback until the author performs and accepts the separate validation.
