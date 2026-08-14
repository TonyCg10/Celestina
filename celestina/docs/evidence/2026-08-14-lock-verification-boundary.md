# The lock's verification boundary, and what it refuses

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R6-A`
- **Artifact:** Celestina 0.21.0, `celestina-lock-verify` and `LockAuthenticator`
- **Environment:** the repository's automated suite, plus the author's own
  Linux-PAM 1.7.2 stack for the three live checks below; no compositor, no lock
  surface and no real passphrase are involved
- **Plan:** [first-party session lock](../plans/active/2026-08-14-first-party-session-lock.md)
- **Validation:** `VAL-R6`

This unit builds the only thing in Celestina that may answer "authenticated",
and it is a separate process on purpose. What is recorded here is the boundary
around it: not that a correct passphrase works — that is `VAL-R6` and needs the
author's real one — but that everything which is not a correct passphrase fails
to open the session.

## Procedure

`celestina-lock-verify` links libpam directly and takes the passphrase on
stdin, never as an argument. `LockAuthenticator` spawns it and reads its exit
status, and has no other way to reach a verdict.

Seven offscreen regressions drive the parent against a stand-in verifier whose
exit code is dictated by the case: authenticated, refused, missing binary,
killed mid-answer, an exit code this shell does not define, and a second
attempt started while one is in flight. One more reads back what actually
reached the child, from the child's side.

Three checks then ran against the real PAM stack, all of them with a
deliberately wrong passphrase.

## Result

### The parent

All seven pass. The property they exist for is one-directional and holds:
**only exit code 0 becomes `Authenticated`**. A verifier that is absent, that
crashes, that returns an undefined code, or that is asked while another attempt
is running produces `Unavailable`, and a wrong passphrase produces `Refused`.
Neither unlocks anything.

The passphrase check is not an assertion but an observation from the other end
of the pipe: the stand-in recorded its own stdin and its own argument vector.
The secret appeared in the first and not the second, which is what keeps it out
of `/proc/<pid>/cmdline`, readable by every process on the machine.

### The real stack

- A wrong passphrase against `login` exits 1, refused.
- That refusal took **2.2 seconds**, which is `pam_unix`'s own failure delay.
  The stack really ran; nothing short-circuited it.
- A service name that does not exist exits 1, not 2. PAM falls through to
  `/etc/pam.d/other`, which is `pam_deny` on this machine. A typo in the
  service name locks the person out instead of letting them in — the direction
  to fail in, and the reason this is recorded rather than assumed.

## Limits

No correct passphrase has been tested, by design: a regression that needed one
would either hold a credential in this repository or be skipped forever.
Whether the author's own passphrase opens their own session is `VAL-R6`.

Nothing here touches a lock surface, because there is not one yet — `R6-B`
brings the protocol client, and until it exists this verifier answers a
question nobody is asking. The wipes on both sides of the pipe are written
through volatile views so a compiler may not delete them; that they defeat
every possible copy the allocator or the kernel made is not claimed.
