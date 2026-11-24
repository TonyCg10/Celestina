# ADR 0026: The lock and the polkit agent are on by default, and fail closed

- **Date:** 2026-11-24
- **Status:** proposed
- **Amends:** [ADR 0024](0024-cutover-defaults.md): Decision 1's "`lock` stays
  **off**… `polkit` stays **off**… each flips in a unit of its own", Decision
  2's "every duty but the clipboard" and the last Consequences bullet. Both
  flips are made in one unit, by the author's decision of 2026-11-24. Also
  [ADR 0021](0021-polkit-agent.md), Decision 16's "The duty is `off` by
  default", and [ADR 0022](0022-session-lock.md), H-11's "off by default".

## Context

ADR 0024 made every shipped duty default to `on` except the two privileged
ones: `lock` (the session lock, ADR 0022) stayed `off` until the author's
`VAL-CEL2-LOCK-*` rows passed, and `polkit` (the polkit authentication agent,
ADR 0021) stayed `off` until `VAL-CEL2-POLKIT-LIVE` passed, each to flip in a
unit of its own.

On 2026-11-24 the author reported, from his own niri session on celestina
1.13.5, these rows as passed: `VAL-CEL2-LOCK-PAM`, `VAL-CEL2-LOCK-LIVE`,
`VAL-CEL2-LOCK-RECOVERY`, `VAL-CEL2-LOCK-SUSPEND` and `VAL-CEL2-POLKIT-LIVE`.
The polkit row included cancelling a prompt three times in a row with no rise
in `faillock`, the property CEL-2-I-CANCEL fixed (the amendment of ADR 0021).
His session has run both duties `on`, set explicitly in `shell.json`, since the
cutover of 2026-11-24. This record rests on that report: the state of every row
in [`VALIDATION.md`](../../VALIDATION.md) is written by the author, not by an
agent, and this record claims no row beyond those five.

Five lock rows remain `pending`: `VAL-CEL2-LOCK-HOTPLUG`, `-NOTICES`,
`-COEXIST`, `-BACKDROP` and `-CLOCK`. The author decided the same day that
they do not gate the default. Their own text supports the distinction: they
check the lock's behaviour at its edges (an output plugged while locked, the
notices and the fade, another locker holding the session, the wallpaper under
the lock, the clock and the date), not whether a person can lock, unlock and
recover, which are the five that passed. A failure in one of them opens a
corrective unit, as the documentation governance requires for every manual
failure.

## Decision

1. **Both defaults flip, in one unit.** With no `duties` key, or with the
   `lock` or `polkit` key absent from the object, both duties read as **on**
   (`Duties::default()`). This amends ADR 0024's "each flips in a unit of its
   own": the author decided on 2026-11-24 to make both in one unit.
2. **Both fail closed, like the clipboard.** A value of `lock` or `polkit` that
   is neither `"on"` nor `"off"` (`false`, `"ON"`, `"disabled"`, `7`, `null`,
   an array) reads as **off**. The two keys keep their existing deserializer,
   which reads off unless the file says `"on"`; they do not move to the
   fail-open one the other shipped duties use, and those keep failing open.
   The reason is that these duties are privileged. A person who tried to write
   `"off"` and got it wrong must not end up with a session lock that can cover
   the screen and, with no PAM service installed, cannot be unlocked, nor with
   an agent that registers for the session and takes the keyboard for password
   prompts. The trade-off, stated plainly: a typo costs, at most, a missing
   feature the person can see and fix (no lock on idle, or no polkit prompt,
   in which case polkit itself refuses, its safe baseline); failing open could
   cost a person the use of their own session. A typo in the other direction
   (`"on"` misspelled) therefore turns the duty off and says nothing; this
   record accepts that.
3. **An unclear `duties` value.** What a `duties` value that is not an object
   reads as, and what an object serde cannot read falls back to
   (`Duties::unclear()`), is the defaults with `clipboard`, `lock` and
   `polkit` **off**: the three duties that fail closed.
4. **A protected start is unchanged.** `Duties::all_off()` stays as ADR 0024
   Decision 3 made it: a `shell.json` that cannot be read starts nothing, the
   lock and the polkit agent included. As that decision also has it, only a
   start is protected: a file that breaks while the shell runs keeps the
   values in force, so two duties that were on stay on until a clean read
   says otherwise; and a missing file (a dangling link included) or a session
   with no configuration home is not protected and runs on the defaults,
   both on.
5. **An explicit value is honoured.** An `"off"` (or `"on"`) already in a file
   wins. The shell serializes every duty when it writes `shell.json`, so a file
   an earlier version wrote carries explicit values for both keys, and the flip
   reaches only files with no `lock` or `polkit` key: a fresh install and a
   hand-written file.
6. **Polkit registers, never forces.** With the duty on by default the shell
   asks to be the session's agent at start. If another agent holds the session
   the registration is refused and not retried, as ADR 0021 decided, and
   nothing changes for that agent. polkitd keeps whichever agent registered
   first, so on a session where another agent starts after the shell it is
   that agent which is refused: `"polkit": "off"` leaves the session to it.
7. **The lock's behaviour does not change.** The flip changes a default and
   nothing the lock does; in particular decision 6 of the lock design stands
   (see Consequences).

## Alternatives

- **Failing open for both** (an unreadable value reads as `on`, as for the
  other shipped duties): rejected for the reason given in Decision 2.
- **One unit for each flip,** as ADR 0024 planned: amended; the evidence for
  both is the same report and the same session.
- **Waiting for the five pending lock rows:** rejected by the author's
  decision of 2026-11-24, for the reason in Context.

## Consequences

- **A machine without the PAM service gets a lock nobody can leave.** A
  missing `/etc/pam.d/celestina-lock` does not make the lock unavailable
  (decision 6 of the lock design, ADR 0022): the shell logs a warning and
  posts a desktop notice once when the duty starts, and the lock still covers
  and denies every attempt. With the default on, that warning appears at every
  start on such a machine, and whatever starts the lock there locks a session
  that cannot be unlocked from the lock itself: the `lock` verbs, the idle
  chain's second step, and any sleep logind announces (a lid closed, the
  power key, `systemctl suspend`), which the duty covers before the machine
  sleeps. This record documents that and does not change it. Installing the PAM
  service ([the README's section](../../README.md#installing-the-locks-pam-service))
  is therefore a **prerequisite of the default**, not an optional step. The way
  back is the recovery path of the README ("If the lock does not let you in")
  and of [the handover](../handover.md); the alternative is `"lock": "off"`.
- **A session that chose nothing now locks and suspends when idle.** With
  `idle` on since the cutover and `lock` on now, a start with no `shell.json`
  turns the screens off after 20 s of idle and locks and suspends after
  240 s. Before this record the second step was refused for want of a
  locker, so nothing suspended. `"idle": "off"` or `"lock": "off"` keeps the
  machine from suspending on idle; caffeine holds the chain while it is on.
  The author's own session does not change: its file names both `"on"`.
- **An explicit `"off"` already in a file stays off** (Decision 5). A person
  who has written `"lock": "off"` or `"polkit": "off"` sees no change.
- **A protected start still starts nothing** (Decision 4): while `shell.json`
  cannot be read, no lock and no polkit agent run.
- **Every script that writes a `shell.json` keeps naming every duty
  explicitly,** `lock` and `polkit` included (ADR 0024 Decision 5, enforced by
  `celestina/tests/duty_files.rs` against the serialized `Duties::default()`
  keys), so a nest never starts the lock or the agent a script did not ask for.
- **`swaylock` stays** installed and bound as the independent recovery path.
  Retiring it is the author's, in a separate change (ADR 0022, H-12).
- **ADR 0024's text is not rewritten.** It carries a dated note pointing here;
  ADR 0021's and ADR 0022's statements that their duty is `off` by default are
  superseded by Decision 1 and carry the same kind of note.
- The documents that said the two duties are off by default (the README, the
  handover, `STATUS.md`, `ROADMAP.md`) are corrected in the same unit.

## Revisit when

A remaining lock row fails in the author's session; a second person installs
the shell without the PAM service, which reopens whether a missing PAM file
should make the lock unavailable to the idle chain, against the author's
decision 6; or the author wants an unreadable value of `lock` or `polkit` to
warn instead of reading as off silently.
