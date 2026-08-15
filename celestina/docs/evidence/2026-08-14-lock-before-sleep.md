# Nothing sleeps an uncovered screen

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R6-D`
- **Artifact:** Celestina 0.23.0, `LockController` and the session lock verbs
- **Environment:** the repository's automated suite; logind is not called and
  no machine was suspended
- **Plan:** [first-party session lock](../plans/archive/2026-08-14-first-party-session-lock.md)
- **Validation:** `VAL-R6`

The rule this unit exists to hold is one sentence: the session suspends only
behind a confirmed lock. Every case below drives the sequence to a point where
a careless implementation would suspend anyway, and asserts a refusal instead.

## Procedure

`LockController` was driven against stand-in lock programs whose behaviour is
dictated per case — one that covers, one that never confirms, one that dies
before covering, and a path that names nothing runnable. logind is deliberately
not called: a regression that really suspended this machine could be run once.
What is checked is the decision that precedes that call.

## Result

Five regressions, all passing, and the suite at 20/20.

- **Started is not covered.** A running lock process that has not printed its
  confirmation leaves `isLocked()` false. Treating a live process as a locked
  session is exactly the race this separates.
- **The confirmation is the only thing that locks.** `locked` on the lock's own
  stdout is what flips the state; nothing else does.
- **A lock that never confirms never suspends.** The controller was left for
  two and a half seconds — long past any "give up and sleep" timer a
  well-meaning implementation might add — and no suspend was attempted. There
  is no elapsed time that makes sleeping an uncovered screen safe, so there is
  no timeout in this path at all.
- **A lock that dies before covering refuses**, with a reason, rather than
  falling through to a suspend.
- **A lock binary that is not there refuses.**

### One design defect the tests found

The first run of the missing-binary case passed for the wrong reason: with
`CELESTINA_LOCK` naming something unrunnable, the lookup fell through to the
binary sitting next to the shell and started the real lock. Silent fallback is
wrong here — "which program may cover this session" is not a question to answer
by guessing — so an explicit setting is now authoritative even when it is
wrong, and the same correction was made to the verifier's lookup in `R6-A`,
where a fallback could have quietly changed which process is allowed to say
"authenticated".

### What the sleep inhibitor does

A logind `delay` inhibitor is held for `sleep` and released only when a lock is
confirmed, so a lid, an idle timer or `systemctl suspend` waits for the cover
too. `delay` rather than `block` deliberately: a shell that could veto sleep
outright is a shell that can wedge a laptop shut. If the lock cannot start
before sleep, logind's own timeout expires and the machine sleeps uncovered —
this shell says so in the journal rather than pretending it handled it, because
it cannot both refuse to wedge the machine and guarantee the cover.

## Limits

No suspend was performed and no inhibitor was taken against a real logind here.
Whether a real lid close, a real idle timeout and a real `lock-and-suspend`
sequence behave as this describes is `VAL-R6`, on the author's own machine.

`R6-B`'s record first described the lock as unable to start under EGL. That
was the nest, not the lock: with the nest's own shell stopped, so the lock is
the only EGL client, it starts on the GPU and locks. These verbs are therefore
expected to work rather than to exercise only their refusal path — but a real
session runs a shell *and* a lock, which is the same two-client shape the nest
could not serve, so whether that constraint is the nest's alone is a `VAL-R6`
question.
