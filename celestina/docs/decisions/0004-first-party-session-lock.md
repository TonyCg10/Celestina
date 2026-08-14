# ADR 0004: Own the session lock, and never own password verification

- **Date:** 2026-08-14
- **Status:** accepted

## Context

SHELL-D2 asked whether Celestina should ever own an `ext-session-lock` and PAM
implementation, and required a threat model, a PAM review boundary and explicit
author authorization before any such work could start. The author gave that
authorization on 2026-08-14 and asked for the implementation.

What the session does today, measured rather than assumed: `swaylock-effects`
is bound to `Mod+Shift+A`, and the composed lock-and-suspend path still calls
Noctalia. Both leave with Noctalia, so the shell that replaces it needs a lock
of its own or the session loses the ability to lock at all.

`ext-session-lock-v1` is what makes first-party ownership defensible where it
would otherwise be reckless: the compositor — not the client — is what stops
showing the session. A lock client that crashes, is killed, or never draws
leaves the compositor holding a locked, blank session. The dangerous direction
is closed by the protocol itself; only deliberately unlocking is ours.

## Decision

Celestina owns an `ext-session-lock-v1` client. It does not own password
verification.

- **Verification is PAM's, in a separate process.** The lock surface never
  links a PAM conversation into the process that holds the compositor state.
  A short-lived child performs the conversation and reports one bit plus a
  failure reason; it holds no surface, no protocol object and no shell state,
  so an authentication crash cannot take the lock down with it.
- **Unlock is the only privileged act.** `unlock_and_destroy` is reachable
  from exactly one place: a PAM conversation that returned success. There is
  no debug path, no session verb, no D-Bus method and no configuration value
  that can unlock a locked session.
- **Fail secure everywhere else.** Any error — the child dying, the protocol
  refusing, an output arriving, a surface failing to draw — leaves the session
  locked. There is no error path whose recovery is unlocking.
- **Every output, including the ones that arrive late.** A lock surface is
  created for each output the moment it appears, and an output that cannot be
  covered keeps the session locked rather than exposing itself.
- **The locked surface says as little as possible.** Time, the prompt, and the
  failure state. No notification bodies, no clipboard, no media metadata and
  no application content: a lock screen that renders someone's messages has
  defeated its own purpose.
- **Sleep is sequenced through logind.** The lock takes a delay inhibitor and
  releases it only once the lock is confirmed active, so lock-and-suspend
  cannot suspend an unlocked session. Suspend is refused, never forced,
  when the lock does not come up.
- **Nothing the person types is recorded.** The diagnostic journal, the
  provider channel and every log this shell owns are closed to the passphrase
  and to anything derived from it. The journal may record that an attempt
  happened and whether it failed.

## Consequences

- R6 opens as real implementation with this as its threat-model boundary.
- SHELL-D1 (which external locker to compose for suspend) is superseded: there
  is no external locker to choose once the shell owns the lock.
- `swaylock-effects` stays installed and bound as the recovery path until the
  first-party lock has passed live validation. Removing it is a separate
  author decision, not part of this one.
- The session gains a security-critical component that must be reviewed on
  every change to it, and whose failure mode is deliberately "locked out"
  rather than "let in".

## Revisit when

A reproduced failure leaves the author unable to unlock a machine they own, or
the protocol's fail-secure guarantee turns out not to hold on this compositor.
Either would justify returning to an external locker, which this decision does
not burn any bridge to.
