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

## Amendment (2026-11-23): lazy conversation

Unit `CEL-2-I-CANCEL`, a bug found in the author's real session on 1.13.2.

### What happened and why

Cancelling the prompt card (Escape or the cancel control) counted as a failed
authentication in `pam_faillock`: the third cancel in a row locked the account
for ten minutes, for login, `sudo` and the lock screen alike. Cause, verified
in polkit's `polkitagenthelper-pam.c` (`conversation_function`) and in this
host's PAM stack (`/usr/lib/pam.d/polkit-1` includes `/etc/pam.d/system-auth`,
whose `pam_unix.so` line is `[success=1 default=bad]` and is followed by
`pam_faillock.so authfail`):

1. The agent opened the helper's socket as soon as the card had the keyboard
   (decision 20), and so PAM, running as root inside the helper, sat at its
   first `PAM_PROMPT_ECHO_OFF` in `fgets` for as long as the person read the
   card.
2. Any close of the socket while PAM waits for input is an end of input:
   `PAM_CONV_ERR` from the conversation function, which `pam_unix` turns into
   `default=bad`, and `pam_faillock authfail` records a failure.

Decision 23 ("two attempts per prompt, below `deny = 3`") assumed that only a
wrong password counts. Every dismissal counted too, and it was three of those.

### Decision

25. **The conversation with the helper is lazy: no socket is opened until the
    person submits a non-empty secret.**
    - The helper's thread is still started when the card is ready (the
      keyboard proven and the guard window passed, decision 20), but it idles
      holding no connection. The card is ready to type at that moment, with
      its own generic password label (`copy::POLKIT_PASSWORD_PROMPT`), and does
      not wait for a PAM prompt.
    - At the first submit the secret moves to the thread (decision 21,
      unchanged). The thread connects, writes the account and the cookie,
      shows PAM's notices (`PAM_TEXT_INFO`, `PAM_ERROR_MSG`) and, at the
      **first** `PAM_PROMPT_ECHO_OFF`, writes the retained secret at once and
      drops it. The card is not told of that prompt and stays in "checking".
      `PAM_PROMPT_ECHO_ON` is still refused (`EchoOnPrompt`).
    - A **second** `PAM_PROMPT_ECHO_OFF` in the same conversation (a second
      factor) is shown to the person with PAM's own words and answered live.
    - After a wrong answer below the cap the helper has already ended: the
      thread closes its socket, publishes the failure and goes back to idle.
      The card asks again with the attempts left, and the next submit opens
      the next conversation (same cookie). The exchange keeps counting the
      attempts across conversations. The helper's silence deadline does not
      run while the thread idles: no line is owed.
    - Escape, the cancel control, the idle timeout, the loss of the keyboard,
      polkitd restarting, `CancelAuthentication`, the duty switched off or the
      shell quitting, **before any submit**, touch PAM not at all: no
      connection ever existed, so none can be closed.
    - A cancel raised **between the connection and the first prompt** (PAM has
      not yet read the secret, so closing now would be the same end of input)
      is held back until the secret is written, for at most `CONNECT_GRACE`
      (5 s), and then the socket closes. Nothing is published after the
      cancel, held back or not, so the loop's reply is not delayed. The
      shell's own shutdown flag is not held back.
    - A cancel raised while the connection is still being made is held back
      like any other between connect and prompt; one raised before the
      connection started opens nothing, and the secret is dropped unwritten.
    - After the secret was written, closing the socket no longer changes the
      verdict: it is PAM's, already in flight.
    - **The non-dumpable window outlives a held-back secret** (N-6, S-9). The
      thread keeps the typed answer until PAM's first prompt or the grace, so
      the loop no longer closes the window when it cancels the thread: it
      lends the window's hold to the thread's reaper
      (`Helper::cancel_holding`, `DumpableWindow::hold`), which drops it when
      the thread has ended. The loop never waits for it (N-22).
    - **The window and its holds are process-wide.** The flag is the
      process's, so its counter and the saved previous value are too: every
      window `DumpableWindow::process()` makes, whichever start of the duty
      made it, shares one, so a hold that outlives the window of an earlier
      start (the duty switched off and on while a thread still holds a
      secret) neither restores `Dumpable` under a later request nor leaves
      the process non-dumpable for good. The previous value comes back once,
      when the last window or hold anywhere is gone. A window over a fake
      flag, which only tests use, has a counter of its own.
    - **If the thread does not end within the reaper's bound** (about
      `CANCEL_WAIT + CONNECT_GRACE + WRITE_TIMEOUT`, 12 s, a practical bound
      rather than a strict one: a connect has no timeout), the reaper gives
      up and the window **fails closed**: the hold is leaked on purpose, an
      error is journaled, and the process stays non-dumpable until it exits.
      The bound is of the reaper's wait, not of the secret's life: a thread
      stuck in a call keeps the secret as long as the call lasts, and the
      window now stays shut for all of that time.
    - **An ending is published only after the retained secret is dropped**, so
      the loop never closes the window on a thread that still holds it.
    - Decision 20's text is read with this one: the connection opens at the
      first submit, which presupposes the keyboard proof and the guard window.
    - The dismissal back-off (three prompts ended with no attempt in a row)
      is unchanged: a cancel before any submit is a dismissal and counts, a
      cancel after a submit is an attempt and does not. The notice about the
      cool-down is still lost while another program holds
      `org.freedesktop.Notifications`; that case is now one `warn` line per
      occurrence in the journal, tested.

### What a cancel after the secret does, and the faillock effect

A cancel after a correct secret was sent (or one held back and then released
after it was written) is still answered `Cancelled` to polkitd, never
normally. In polkit main as read on 2026-11-23,
`src/polkitbackend/polkitbackendinteractiveauthority.c`,
`authentication_agent_begin_cb`: `gained_authorization` starts `FALSE` (line
2145) and takes `session->is_authenticated` only in the branch where the
agent's reply is not an error (line 2162); an error reply, which for
`Cancelled` also sets `was_dismissed` (line 2156), leaves it `FALSE`, and that
is the value handed to the session's callback (line 2176), even if the helper
had already told polkitd of a success. The agent's rule that only
`Ending::Authenticated` replies normally, and only after the helper's
`SUCCESS` reached the loop, therefore gives a denied request in that case,
which is the right outcome for a person who cancelled.

The effect on `pam_faillock` follows from the stack above and is not
affected by the reply. A **correct** password that was sent counts as a
success there (`pam_faillock authsucc` resets the counter), even though the
request ends denied. A **wrong** password that was sent counts as a failure,
which is right: a wrong password was really sent.

### Residuals (they can still count in `pam_faillock`)

- A cancel while PAM waits for the answer to a **second** prompt (a second
  factor): the person is the one holding up PAM, the socket must close, and
  that is an end of input. This host asks for no second factor.
- The **shell shutting down** (its own flag, `celestina quit` as the process
  ends) between the connection and the first prompt, or at a second prompt.
- A **grace overrun**: PAM did not ask within `CONNECT_GRACE` after a cancel.
- A submit that is itself a wrong password counts, as it always did.

### Revisit when

- The first hidden prompt of the PAM stack is not the password (a one-time
  code or a PIN asked first, `pam_u2f` with a PIN, `forward_pass`). The first
  `PAM_PROMPT_ECHO_OFF` receives the submitted password without the person
  having read PAM's text, so such a stack would count a failure and the card
  would be calling a PIN a password. Not this host: `system-auth` asks
  `pam_unix` first. The failures that now appear only after typing and
  submitting (the helper's socket not trusted, `pam_faillock preauth` already
  refusing) were found when the card opened before; that is accepted.
- PAM's stack on this host asks a second `PAM_PROMPT_ECHO_OFF` or a fingerprint
  prompt: then the second-factor residual is no longer theoretical.
- Polkit's helper stops treating an end of input as a failed conversation, or
  stops being a socket-activated process.
