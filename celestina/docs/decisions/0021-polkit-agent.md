# ADR 0021: The shell is the session's polkit agent

- **Date:** 2026-10-17
- **Status:** proposed
- **Number:** a draft written by the CEL-2-I implementation; provisional until landing. It
  was drafted as 0016, renumbered to 0017 when the weather ADR (CEL-2-K) landed
  as 0016, and to 0021 when the connectivity ADRs (CEL-2-F, 0017 to 0019) and
  the phone and bubbles ADR (CEL-2-J, 0020) landed. Unlanded worktrees may have
  taken 0021 in turn, in which case the landing renumbers this file and the index row, and
  nothing else changes.

## Context

ADR 0014 carried the halted shell's polkit decision (the Qt agent, its ADR 0005)
over to this project on condition of its own design, a threat model, a list of
what the unit never does, a review by another model, a recovery path and a
failure mode of "denied". The design
[`2026-10-17-celestina-polkit-agent-design.md`](../../../docs/superpowers/specs/2026-10-17-celestina-polkit-agent-design.md)
is that design; the evidence is
[`2026-10-17-cel-2-i.md`](../evidence/2026-10-17-cel-2-i.md). The decisions
below are its §13 (numbered 16 to 24, continuing the parity program's list that
ends at 15) as the implementation ended. Without any agent, polkit refuses every
action that needs interaction, which is the baseline the unit can always return
to.

## Decision

16. **The shell is the session's polkit agent when the `polkit` duty is on,
    inside its own process**, owning the prompt and nothing behind it. The
    password is verified only by `polkit-agent-helper-1`, which also reports the
    verdict to polkitd. The duty is `off` by default.
17. **The agent speaks the helper's socket protocol itself**, without
    `libpolkit-agent`, FFI or a child process; the socket variant only
    (`/run/polkit/agent-helper.socket`, one constant that no setting or
    environment variable can change).
18. **Another agent holding the session means the shell refuses**: it logs once,
    never retries and never evicts.
19. **The unit never grants, never remembers and never logs the secret**, and
    every failure is a denial. A normal reply to `BeginAuthentication` exists
    only after the helper's `SUCCESS`.
20. **Keyboard ownership is proven and kept.** No helper connection opens before
    the card has the keyboard and the guard window has passed; a lost keyboard
    wipes the buffer and cancels after a grace.
21. **The secret is a fixed-capacity zeroizing byte buffer** in a toolkit
    `SecretField`, moved to the helper's thread and never copied, over a toolkit
    whose key events no longer print their text.
22. **The agent's bus thread acts on one ordered stream and does not use
    `#[interface]`.** A reading thread takes every message out of the stream
    from the start and keeps only what the bus thread can act on, so a flood of
    unicast signals cannot stop the answer to one of the agent's own calls from
    being read. The sender is checked on the header, before the body and before
    any logging; only authentic broadcasts from the bus driver update the owner
    cache. **The loop never waits for these threads:** a finished prompt hands
    its helper thread, and a duty switched off hands the agent, to a reaper
    thread, and the next agent registers only after the previous reaper is
    done. A reaper also waits for the one its agent was started behind, so
    the chain holds through quick switches.
23. **Two attempts per prompt**, below this host's `pam_faillock` limit
    (`deny = 3`).
24. **Launch-environment control is out of scope** for the polkit agent.

## Consequences

- The shell gains a privileged duty. It is gated by the guards in
  `celestina/tests/polkit_guards.rs`, the tests listed in the evidence record,
  the review by another model and the author's `VAL-CEL2-POLKIT-LIVE`; the
  duty's default flips to `on` only in CEL-2-M, and only if that row has passed.
- Recovery needs no new tool: switch the duty off or stop the shell, then
  re-enable Noctalia's agent; the helper, its socket and `polkitd` are untouched.
- A change to polkit's helper protocol makes the Rust conversation fail to
  "denied" until updated; there is no spawn fallback to the setuid helper.
- A prompt on Wayland has no trusted path: a look-alike surface, a virtual
  keyboard and a hostile compositor are residuals recorded in the design (§6.2),
  narrowed by the keyboard proof and the notice, and not closed.
- Notices about an unshown prompt, a helper that cannot be used, and the
  cool-down are lost while another program owns the notifications name
  (Noctalia); the request is still cancelled.
- The display sanitiser is a polkit-local module until a suite unit converges it
  with the shared text sanitiser.

## Revisit when

- Polkit changes the helper protocol, or the helper stops being socket-activated.
- The author wants the requesting process or pkexec's full command line in the
  card, or an input method in the secret field.
- The author's accounts leave `/etc/passwd` (NSS-only users).
- A maintained standalone agent becomes something the author prefers, or a
  separate agent process becomes worth a second deployable.
- Launch-environment control stops being out of scope.
