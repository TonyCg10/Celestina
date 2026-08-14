# First-party session lock

- **Opened:** 2026-08-14
- **Plan ID:** first-party-session-lock
- **Status:** pending
- **Scope:** project
- **Implementation checkpoint:** R6
- **Author-validation checkpoint:** VAL-R6

## Hypothesis

Celestina can lock its own session through `ext-session-lock-v1` such that no
reachable code path unlocks it except a PAM conversation that returned success,
and every other outcome — a crashed helper, an output arriving mid-lock, a
refused protocol object, a killed shell — leaves the session locked.

## Tangible outcome

`celestina msg session lock` covers every output with a lock surface that
accepts a passphrase and returns to the session only on a real PAM success;
`lock-and-suspend` refuses to suspend when the lock did not come up; and
killing the shell while locked leaves the compositor locked rather than
exposing the session.

## Scope

- An `ext-session-lock-v1` client: acquiring the lock, one surface per output,
  surfaces for outputs that appear while locked, and `unlock_and_destroy`
  reachable from exactly one call site.
- A verification child process: PAM conversation, one verdict on its pipe, no
  compositor state, no passphrase in its output or its exit code.
- The logind delay inhibitor and the lock-then-suspend sequence, including the
  refusal when the lock is not confirmed.
- The locked surface: clock, prompt, attempt state and failure reason. Nothing
  else is rendered on it.
- Wiring the existing `lock` and `lock-and-suspend` session verbs to this
  instead of to Noctalia.

## Exclusions

- Fingerprint, smartcard and any second factor.
- Unlocking as a different user, or any user switching.
- Removing `swaylock-effects` or changing the author's keybindings: the
  recovery path stays until `VAL-R6` passes, and its removal is a separate
  author decision.
- Idle-to-lock policy, which the delivered idle path already owns; this plan
  supplies the lock it calls, not a new timer.
- Any lock-surface content beyond the four things named in Scope.

## Build order

1. The verification child alone, with no compositor involvement: spawn, PAM
   conversation, verdict, refusal on every error. Testable headless.
2. The lock client's protocol lifecycle against a real compositor: acquire,
   surface per output, hotplug, and deliberate non-unlock on every failure.
3. The locked surface's own presentation, on the shared glass material.
4. The logind inhibitor and the suspend sequencing, including the refusal.
5. The session verbs moved onto it.

## Implementation exit

An offscreen regression proves the verification child reports failure for a
wrong passphrase, refuses when the helper cannot start, and never emits the
passphrase on any stream. A protocol regression proves a surface exists for
every output including one added while locked, and that every failure branch
leaves `unlock_and_destroy` uncalled. A sequencing regression proves suspend is
refused when the lock is not confirmed active. Real keyboards, real monitors
and a real suspend are `VAL-R6` and do not block this exit.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| R6-A | `celestina:` | planned | verification child: PAM conversation, verdict pipe | — | a separate process answers "authenticated" or "not", and never carries the passphrase further | offscreen regression over success, failure and unavailable-helper | `VAL-R6` |
| R6-B | `celestina:` | planned | `ext-session-lock-v1` client lifecycle and per-output surfaces | — | every output is covered, late outputs included, and no failure path unlocks | protocol regression incl. hotplug and forced-failure branches | `VAL-R6` |
| R6-C | `celestina:` | planned | the locked surface's presentation | — | clock, prompt and failure state on the shell's glass, and no session content | offscreen surface regression | `VAL-R6` |
| R6-D | `celestina:` | planned | logind inhibitor, lock-then-suspend sequence, session verbs | — | suspend happens only after a confirmed lock, and is refused otherwise | sequencing regression over confirmed and failed lock | `VAL-R6` |
