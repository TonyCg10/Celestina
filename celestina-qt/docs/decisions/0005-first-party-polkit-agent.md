# ADR 0005: Own the Polkit agent's prompt, and nothing behind it

- **Date:** 2026-08-14
- **Status:** accepted

## Context

SHELL-D3 asked which external Polkit agent the session should use and whether
first-party ownership should ever be considered. It required a candidate
inventory and explicit approval for security-sensitive work. The author gave
that authorization on 2026-08-14 and asked for the implementation.

The inventory is short and decides the question by itself: this machine has
`polkit`, `polkitd` and `polkit-agent-helper-1`, and **no graphical
authentication agent at all**. Whatever agent the session has today comes with
Noctalia and leaves with it. So the choice is not "ours versus theirs" — it is
"ours versus none", and none means every action needing authorization fails
silently or falls back to a terminal.

The reason a first-party agent is defensible here is that the dangerous half is
not ours to write. Polkit ships `polkit-agent-helper-1`, a setuid helper that
performs the PAM conversation and reports the result to `polkitd` itself. An
agent's real job is to show a prompt and pass what was typed to that helper
over a pipe. That is the same boundary `libpolkit-agent-1` draws.

## Decision

Celestina registers as an `org.freedesktop.PolicyKit1.AuthenticationAgent` for
its own session, and implements the prompt only.

- **Verification is the system helper's.** The agent spawns
  `polkit-agent-helper-1`, writes the cookie and the response on its stdin, and
  reads its verdict. Celestina performs no PAM conversation, opens no
  `/etc/shadow`, and never decides whether an authentication succeeded — it
  reports what the helper said.
- **The response is passed and dropped.** It goes from the prompt to the
  helper's pipe and nowhere else: not to the diagnostic journal, not to the
  provider channel, not to a property another surface can read, not to disk,
  not into an error message.
- **The prompt is its own surface, and it grabs the keyboard.** An
  authorization prompt that another window can cover or read keystrokes past is
  not an authorization prompt. It is refused rather than shown degraded if the
  grab cannot be taken.
- **It says exactly what is being authorized, from polkit.** The action id, the
  message and the requesting identity as `polkitd` gave them — never text this
  shell composed, which could describe the wrong action.
- **Every failure denies.** A helper that will not spawn, a cancelled
  conversation, a malformed request, a lost D-Bus name: all end as "not
  authorized". No path treats an error as success.
- **It registers for this session only**, never as the system-wide agent.

## Consequences

- The R8 Polkit slice opens as real implementation with this boundary.
- The session gains graphical authorization it currently does not have once
  Noctalia leaves.
- Celestina's attack surface grows by one D-Bus-facing prompt. It does not grow
  by an authentication implementation, which is the part this decision refuses
  to own.
- If the agent is not running, actions needing authorization fail as they do
  today with no agent installed. That is a visible loss of capability, never a
  silent grant.

## Revisit when

A maintained standalone agent becomes something the author would rather run, or
the helper protocol this depends on changes shape. Neither would require
undoing anything: the prompt is separable from the shell it lives in.
