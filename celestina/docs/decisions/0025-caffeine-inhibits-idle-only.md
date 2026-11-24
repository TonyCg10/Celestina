# ADR 0025: Caffeine inhibits idling only

- **Date:** 2026-11-23
- **Status:** accepted
- **Accepted:** by the author on 2026-11-24, after trying the change live on
  1.13.4 (caffeine on, a suspend asked for by hand went through, locked)
- **Supersedes:** the `what` of decision D6 of
  [the energy plan](../evidence/2026-10-16-energy-plan.md) (`idle:sleep`).
  The rest of D6 stands: the inhibitor is a descriptor the worker holds, not
  a child process.

## Context

CEL-2-B made caffeine a logind inhibitor `Inhibit("idle:sleep", "Celestina",
..., "block")` (decision D6 of the energy plan), on the reasoning that
caffeine promises the session neither idles nor suspends while it is on. In
the author's first live use (celestina 1.13.2, 2026-11-23) the `sleep` half
also refused a suspend asked for by hand: with caffeine on, `systemctl
suspend` was blocked. The author decided that this is not what caffeine is
for.

Why it refused: since systemd 257 a `block` lock also binds the user who
holds it; the earlier behaviour, in which the holder's own requests pass, is
now the mode `block-weak` (`org.freedesktop.login1(5)`; this machine runs
systemd 262). The refusal therefore came from logind itself, and `idle:sleep`
refused every suspend of the same user that logind checks against
inhibitors:

- `systemctl suspend`;
- the shell's own `Suspend` call from `celestina lock suspend`, which left
  the session locked but awake;
- the power key, which this machine maps to suspend.

It never refused the lid: logind ignores inhibitors for the lid switch by
default (`LidSwitchIgnoreInhibited=yes`).

## Decision

Caffeine stops only what inactivity would do by itself. The logind inhibitor
is `Inhibit("idle", "Celestina", ..., "block")`; it no longer names `sleep`.
The shell's own idle chain keeps honouring caffeine as before
(`IdleChain::set_caffeine`): no screen-off and no lock-and-suspend by
inactivity while it is on. A suspend a person asks for (`systemctl suspend`,
the power key, `celestina lock suspend`) now goes through, and it locks first
whenever the lock duty is on, as ADR 0022 requires.

## Alternatives

- **`block-weak` on `sleep`:** gives nothing on a single-user machine. Every
  suspend asked for by the same user passes a weak lock, a second idle
  daemon's included, so it would stop nobody this session has.
- **Keeping `idle:sleep`:** refused by the author; it is the behaviour this
  record replaces.

## Consequences

- `systemd-inhibit --list` shows caffeine's `Celestina` lock for `idle` only.
  The lock duty's own `Celestina` lock for `sleep` in mode `delay` is another
  one and stays.
- The case no longer covered is an idle daemon that does not follow logind's
  `idle` lock and suspends the machine on its own schedule: under the old
  rule logind refused its suspend, and now nothing does. logind's idle action
  and the shell's idle chain stay held off.
- With the lock duty off, a suspend asked for by hand now goes through
  unlocked while caffeine is on, as it does without caffeine. Under the old
  rule it was refused.
- An idle suspend in flight is withdrawn when caffeine is switched on. If the
  chain already handed out its lock-and-suspend step and the lock has not
  confirmed yet, `IdleChain::set_caffeine(true)` answers
  `IdleAction::Resumed`, as input coming back does, and the lock policy drops
  the idle chain's request and never a person's own `lock suspend`. The
  session still locks, and that idle period does not ask to suspend again.
  The old `sleep` half refused that suspend by accident and was the only
  thing that did.
- `VAL-CEL2-CAFFEINE` gains the step that a manual suspend works while
  caffeine is on.

## Revisit when

The author wants a second, stronger mode that also refuses a manual suspend
(a presentation mode, for example); it would be a switch of its own, not a
change of what caffeine means.
