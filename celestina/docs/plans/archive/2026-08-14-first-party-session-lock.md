# First-party session lock

- **Opened:** 2026-08-14
- **Closed:** 2026-08-14
- **Successor:** [polkit authentication agent](../active/2026-08-14-polkit-authentication-agent.md), which takes the checkpoint
- **Plan ID:** first-party-session-lock
- **Status:** done
- **Scope:** celestina
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

## What the lock turned out to need

The lock is its own process, which the plan did not say and the work made
unavoidable: Qt chooses one Wayland shell integration per process and every
surface this shell owns is a layer surface, so a lock surface cannot share it.
The isolation that falls out is worth having on its own — the shell crashing
leaves the session locked, and the lock crashing leaves it locked too, because
the guarantee lives in the compositor.

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
| R6-A | `celestina:` | done | [inventory](../../inventories/2026-08-14-first-party-session-lock/R6-A.numstat.tsv) | 11 files, +768/-4 | a separate process answers "authenticated" or "not", and only its exit code 0 unlocks | [verification boundary](../../evidence/2026-08-14-lock-verification-boundary.md) | `VAL-R6` |
| R6-B | `celestina:` | done | [inventory](../../inventories/2026-08-14-first-party-session-lock/R6-B.numstat.tsv) | 17 files, +924/-9 | every output is covered, and killing the lock leaves the session locked | [protocol record](../../evidence/2026-08-14-session-lock-protocol.md) | `VAL-R6` |
| R6-C | `celestina:` | done | [inventory](../../inventories/2026-08-14-first-party-session-lock/R6-C.numstat.tsv) | 8 files, +203/-37 | clock, prompt and failure state on the shell's own material, and no session content | [lock surface](../../evidence/2026-08-14-lock-surface-presentation.md) | `VAL-R6` |
| R6-E | `celestina:` | done | [inventory](../../inventories/2026-08-14-first-party-session-lock/R6-E.numstat.tsv) | 10 files, +64/-22 | the EGL hang is the nest serving one client, not a defect in the lock | [protocol record](../../evidence/2026-08-14-session-lock-protocol.md) | `VAL-R6` |
| R6-F | `celestina:` | done | [inventory](../../inventories/2026-08-14-first-party-session-lock/R6-F.numstat.tsv) | 8 files, +90/-10 | the handover model stops saying nothing provides a lock | [handover record](../../evidence/2026-08-14-handover-knows-about-the-lock.md) | `VAL-R6` |
| R6-D | `celestina:` | done | [inventory](../../inventories/2026-08-14-first-party-session-lock/R6-D.numstat.tsv) | 14 files, +692/-26 | suspend happens only after a confirmed lock, and is refused otherwise | [lock before sleep](../../evidence/2026-08-14-lock-before-sleep.md) | `VAL-R6` |
| R6-Z | `celestina:` | done | [inventory](../../inventories/2026-08-14-first-party-session-lock/R6-Z.numstat.tsv) | 20 files, +347/-200 | Close this plan on its implementation exit, record the author's validation of five earlier responsibilities, and hand the checkpoint to R8 | [closure record](../../evidence/2026-08-14-lock-plan-closure.md) | `VAL-R6` |

## What R6-Z closes, and what it does not

Every unit this plan named is built and has its own record: the verification
child, the protocol client, the surface, the sequencing, the two corrections
the work forced on its own earlier records. The implementation exit is met by
regressions, so the plan closes.

`VAL-R6` is not claimed and does not move. Nobody has typed a passphrase into
this lock, no lid has closed on it, and the one question the nest could not
answer — whether a real session running a shell *and* a lock hits the same
single-EGL-client limit — is still open. A plan that closed by declaring its
own author validation would be worth nothing; this one closes leaving the
question written down where it can be answered.

The checkpoint moves to `R8` and its polkit slice, whose plan was written and
authorized on the same day and has been waiting under `pending/` since.
