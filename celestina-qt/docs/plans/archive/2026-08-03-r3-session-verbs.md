# R3 — session verbs

- **Opened:** 2026-08-03
- **Status:** done
- **Plan ID:** r3-session-verbs
- **Closed:** 2026-08-04
- **Successor:** none; the roadmap is idle until an R4 notifications plan is opened
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
| R3-A | `celestina:` | done | [inventory](../../inventories/2026-08-03-r3-session-verbs/R3-A.numstat.tsv) | 41 files, +3096/-105 | Typed session verbs and bounded level policy in the pure core; volume, mute and brightness carried to their providers and confirmed by a later reading; a corner OSD raised by published readings; night light and idle inhibit held by an owned child that is released on shutdown and failure; DPMS through Niri; a lock that refuses because no provider exists; and the optional bindings with their rollback | [R3 session verbs](../../evidence/2026-08-04-r3-session-verbs.md) | `VAL-R3` |
| R3-Z | `celestina:` | done | [inventory](../../inventories/2026-08-03-r3-session-verbs/R3-Z.numstat.tsv) | 10 files, +203/-117 | Close the checkpoint: run the registered exit, record the verified and deployed bundle, and archive this plan | [R3 completion](../../evidence/2026-08-04-r3-completion.md) | `VAL-R3` |

The six build-order steps closed as one unit because they deliver one milestone
in one commit. Splitting them further would have required one exclusive
inventory *and* one exclusive evidence record each, which for a single
verification run means five near-identical records — fragmentation for
appearance rather than for review.

R3-Z is administrative: it carries the registered production exit and the
archive move, which is why it is a second unit rather than an edit to the
delivery R3-A already inventoried and committed.

## Decisions and rollback

R3-D holds night light and the idle inhibitor by owning somebody else's
process — `wlsunset` at a fixed 2700 K and `systemd-inhibit --mode=block` — so
the published state is whether this helper still has that child, checked every
time it is asked and released on shutdown, on failure and when the tool cannot
start at all. They share one module because they share one lifecycle. DPMS is
composed through Niri, whose synchronous answer is the outcome rather than a
helper's acceptance. Locking is refused: the refusal site is the seam a locker
provider is wired into when SHELL-D1 is applied.

R3-C draws the OSD level as a meter over the shared track tokens instead of a
`CelestinaSlider`. Its surface takes neither pointer nor keyboard, so a control
that looked draggable would offer an interaction the surface cannot accept. The
OSD is also raised by published readings rather than by command outcomes, which
is what keeps it from announcing a request the device never carried out.

The locker choice is intentionally open in
[`../../discussions/README.md`](../../discussions/README.md); that slice must not be
implemented by guessing or installing a package. When SHELL-D1 is applied, its
concrete composition receives a new bounded implementation unit rather than
being appended silently to this ledger. Each external lifecycle must stop
cleanly when the shell stops. The old Niri binds and Noctalia paths remain the
rollback until the author performs and accepts the separate validation.
