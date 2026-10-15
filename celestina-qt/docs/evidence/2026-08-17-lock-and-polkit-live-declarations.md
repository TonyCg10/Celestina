# Two author declarations from real use: the lock, and polkit's first real test

- **Date:** 2026-08-17
- **Scope:** Celestina `VAL-R6` and the polkit-specific part of `VAL-R8`
- **Artifact:** `celestina-lock` and the polkit authentication agent, as built
  by R6 and R8
- **Environment:** the author's real Niri session; no session was touched by
  this record, which only writes down what the author reports already
  happened there
- **Plan:** [first-party session lock](../plans/archive/2026-08-14-first-party-session-lock.md),
  [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R6`, `VAL-R8`

## Procedure

The author was asked directly how each was tested, given `VAL-R6` had no
recorded case yet and polkit's own code comment says its first real test can
only happen once Noctalia's agent steps aside, since `polkitd` accepts one
agent per session.

## Result

**The lock.** Invoked directly from a terminal — `celestina-lock` or
`celestina msg lock` — rather than from a keybind: the live `config.kdl` binds
`Mod+Shift+A` to `swaylock` and `Mod+Shift+Escape` to `noctalia msg session
lock-and-suspend`, and carries no binding to Celestina's own lock yet. Every
output covered, a wrong passphrase leaving the session locked, and the correct
one returning it. Recorded as `VAL-R6` passed. What the author did not
separately exercise — killing the shell or the lock process while locked, an
output arriving mid-lock, and `lock-and-suspend`'s refusal path — stays on the
automated regressions' account, not an author-watched one, and ordinary daily
use through a keybind has not happened yet.

**Polkit.** The author performed a controlled handover to test it: stopped
Noctalia's polkit agent, let Celestina's register in its place, authenticated
through a real `pkexec` prompt, then restored Noctalia. This is the specific
gap the code comment on the agent's `Responsibility` entry names — the first
time the prompt could be exercised for real. It does not, by itself, make
`VAL-R8` passed: that section's own procedure and ROADMAP's R8 text tie
`VAL-R8` to living a full day without Noctalia and actually removing it,
which the author has not decided to do — Noctalia still owns the session.

## Limits

This is a record of what the author reports, not something reproduced or
observed by an agent. It does not cover a second monitor, assistive
technology, or any of the specific gaps named above for either surface.
