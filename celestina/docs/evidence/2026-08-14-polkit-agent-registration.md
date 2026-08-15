# The agent polkitd calls, and who else is allowed to

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R8-P-B`
- **Artifact:** Celestina 0.28.0, `PolkitAgent`
- **Environment:** the repository's automated suite on the author's machine,
  against a stand-in authority on the session bus. The real polkitd was not
  registered with, no action was authorized and no password was typed.
- **Plan:** [polkit authentication agent](../plans/active/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

A stand-in authority owns polkit's own name on the session bus, records every
registration, and can be withdrawn and brought back to stand for a restart.
The agent was registered against it, driven through requests, cancellation and
concurrency, and asked to serve identities it has no way to ask.

The real polkitd is deliberately not used. It lives on the system bus, accepts
one agent per session, and registering against it from a regression would take
the session's agent slot away from whatever the author is running while the
suite happens to be executing. What is proven here is the conversation with an
authority; the author's own polkit is `VAL-R8`.

## Result

Nine regressions, all passing, and the suite at 22/22.

- **Registration names this session and this object.** polkitd matches both
  when it decides who to call, so both are asserted rather than the fact that
  a call was made.
- **A restarted authority finds the agent again.** The registration is
  reported as lost when polkit's name goes away — nothing is pretended about
  the interval — and repeated when it comes back. A polkitd upgrade otherwise
  leaves the session unable to authorize anything until the shell restarts,
  which is a failure nobody would connect to the upgrade that caused it.
- **An authority that refuses leaves no registration**, and `attach` says so.
- **A request reaches a prompt with polkitd's own strings**, and only the
  helper's success finishes it as authorized.
- **Two requests are answered independently.** An answer given to one cookie
  ends that one and leaves the other waiting; a wrong answer to the second
  ends it unauthorized.
- **Cancellation and dismissal both end the request without a verdict**, and
  an answer arriving for a request that is over is dropped rather than held
  for whatever comes next.
- **An identity this session cannot ask is refused before any prompt exists.**
  polkitd may offer only another person's account or a group; prompting anyway
  would be asking the person in front of the screen for a password that cannot
  help them.

### Only polkit may ask for a password

This object sits on a bus every process on the machine can reach, and a prompt
asking for a password is exactly what somebody would want to forge. A forged
`BeginAuthentication` could never produce an authorization — the cookie would
not be one polkitd issued, so the helper would refuse — but it could produce a
convincing prompt, and harvesting a password does not require the prompt to
work. The call is therefore refused unless its sender owns polkit's own name.

### A crash this unit inherited rather than invented

The identity case segfaulted the first time it ran. `sendErrorReply` writes
into the D-Bus call the object is answering, and when the method is called
in-process there is none: it dereferences a connection that is not there. This
is the same defect the August audit recorded in the session verbs' `Suspend`,
found there by reading and here by a regression. Every refusal in this class
now goes through one function that checks `calledFromDBus()` first.

## Limits

**The shell does not register this agent yet, and that is deliberate.** There
is no prompt surface until `R8-P-C`, so a registered agent would receive real
requests it could show to nobody — a `pkexec` that hangs instead of failing is
worse than the machine's current behaviour, which is to fail immediately with
no graphical agent at all. The wiring lands with the surface.

One attempt per request. polkit's own agents let a person retype after a
mistake; here a wrong password ends the request and the action must be started
again. Whether that is acceptable is a question for the surface, not for this
seam.

Nothing here has met the real polkitd, a real action, or a real password.
