# Evidence: 2026-08-06 which helper a timer was armed against

- **Date:** 2026-08-06
- **Scope:** `LVR-3-C`; plan
  [late-provider-insertion](../plans/archive/2026-08-05-late-provider-insertion.md);
  findings `H1` and `H2` of the
  [light monorepo audit](../../../docs/evidence/2026-08-06-light-monorepo-audit.md),
  both introduced by `LVR-3-B`
- **Environment:** source and text only. The GPU safety hold stands, so nothing
  was compiled, tested, built, deployed or run — the plan scopes this unit to
  exactly that, as it did `LVR-3-B`
- **Artifact:** none, and none may be produced during the hold

## What was wrong

`LVR-3-B` replaced an immediate `kill()` with TERM-then-KILL so a helper would
reap its DDC child before dying. Both halves of that change miss.

**The escalation is not addressed to anyone.** The deferred lambda captures
`this` and asks `m_process` whether it is running, but `QProcess` is reused for
every replacement, so the question it asks cannot distinguish the instance it
was armed against from the healthy one that replaced it. `gracefulShutdownMs` is
3000 and the restart backoff begins at 250 and doubles, so the first several
replacements start well inside the window and take the `kill()`. A helper's
first act is `ddcutil detect`, and SIGKILL skips the cancellation chain
entirely — abandoning a child on the monitor bus, which is the shape that
preceded both retained GPU losses and the reason this path was written.

**The spacing after an unclean exit never engages.** `m_uncleanExit` is known
only to `helperStopped`, but `helperError` also called `scheduleRestart()`, and
that returns early once the timer is active. Whichever signal Qt delivered first
settled the delay, so `qMax(m_restartDelayMs, abandonedChildLifetimeMs)` was
effectively unreachable for a crash: the mitigation written to remove the DDC
overlap did not remove it.

## What changed

- `src/shellprovidersclient.h`, `src/shellprovidersclient.cpp` — a
  `m_helperGeneration` names the running instance, incremented in
  `startHelper()`. The escalation captures the generation it was armed with and
  returns without killing anything when it no longer matches.
- `src/shellprovidersclient.cpp` — `helperError` no longer schedules a restart
  for an instance that has already stopped. `helperStopped` is the only handler
  that receives `exitStatus`, so it is the only one that can tell an exit that
  ran the helper's own shutdown from one that abandoned a child, and it now owns
  that decision alone.
- `src/shellprovidersclient.cpp` — with one deliberate exception:
  `QProcess::FailedToStart` is scheduled from `helperError`, because Qt emits no
  `finished()` for a process that never ran. Without that branch this change
  would have left a helper that fails to start never restarted at all, and the
  shell permanently without providers. There is also nothing to space out in
  that case: a process that did not run abandoned no child.

## Procedure

None. No command was run against this project.

```text
The GPU safety hold forbids running any Celestina executable, provider,
build, test, deployment or activation. This unit is source and text.
```

## Result

Not verified, and that is the honest state of it. The change was reviewed by
reading: the generation is `quint64`, a type the header already uses; the lambda
capture is by value; `startHelper()` is the single place a new instance begins;
and the three exits from `helperError` were traced against Qt's documented
signal behaviour for `FailedToStart`, a crash and a read/write failure on a
still-running process.

## Limits

This code has not seen a compiler. The correctness argument for `H2` also rests
on a premise that cannot be settled under the hold — the order in which Qt
emits `errorOccurred` and `finished` for a crash — but the repair does not
depend on that order: it removes the race rather than betting on which side
wins.

Whether the shell survives a helper that must be escalated, and whether a crash
now really waits out the abandoned-child window, are observations for
`VAL-R1-01` and the lifecycle rerun once the author ends the hold. Nothing here
may be deployed before that.
