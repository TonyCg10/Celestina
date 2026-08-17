# The prompt that holds the keyboard, or does not appear

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R8-P-C`
- **Artifact:** Celestina 0.29.0, `PolkitPromptController` and
  `qml/PolkitPrompt.qml`
- **Environment:** the repository's automated suite, plus one offscreen run of
  the shell itself against this machine's real polkitd. No action was
  authorized, no password was typed, and the author's running session was not
  changed.
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

The rule this unit exists to hold is that there is no lesser prompt. A
password typed into a surface that does not hold the keyboard can be read by
whatever does, so a shell that cannot take an exclusive grab refuses the
request rather than collecting the password anyway.

## Procedure

The decision — may this shell ask for a password right now — was exercised as
a function of its own, and the surface was exercised as QML against a stand-in
source. The shell was then started once, offscreen, to see what the real
polkitd does with its registration.

## Result

Seven controller regressions and four surface regressions, all passing, and
the suite at 23/23.

### The refusal is a decision, not a side effect

`promptRefusal` is a free function so the case that matters can be asked
directly: with everything else ready and no layer shell, the answer is
`NoKeyboardGrab`. A regression that only drove the controller under
`offscreen` would have passed while proving nothing — the QML component is not
loadable there either, so it would have refused for that reason and never
reached the question about the keyboard. Each refusal carries its own distinct
reason, which the test asserts rather than assumes: "no layer shell" is a
session that will never prompt and "already showing" is one that will prompt
again in a moment.

End to end under a platform that cannot grab, the request is dismissed rather
than left open. polkitd hears a cancellation and the action fails at once,
which is what a machine with no graphical agent does anyway.

### The surface shows polkitd's words and nothing else

The message, the action id and the identity all arrive on screen unedited. The
action id is shown deliberately and in full: a message can claim anything, and
`org.freedesktop.policykit.exec` is the one string a hostile caller cannot
dress up. PAM's own prompt replaces this shell's placeholder as soon as it
arrives, the field never echoes, what is typed leaves through `respond` and
nowhere else, an empty field is not an answer, and a problem from PAM outranks
a pending notice.

The surface takes `KeyboardInteractivityExclusive`, which no other surface in
this shell uses. On-demand — what the launcher and the menus take — asks for
the keyboard on a click and gives it up when something else takes it, and a
password field that can quietly lose the keyboard mid-word types the rest of
the password into whatever took it.

An echoing prompt (`PAM_PROMPT_ECHO_ON`, typically a username) is refused
rather than shown with the characters hidden. A person who cannot see what
they are typing into a username field will type it wrong and be told their
password was rejected.

### What the real polkitd said

    polkit.agent  state=refused  session=3

    org.freedesktop.PolicyKit1.Error.Failed:
    An authentication agent already exists for the given subject

The plan assumed this machine "has no graphical agent at all". It has one:
Noctalia, which is still the author's session shell, runs a `polkit-agent`
plugin — its files are in `~/.config/noctalia/plugins/polkit-agent/` — and
holds the session's single agent slot.

So the refusal is the design working. polkitd accepts one agent per session
and Celestina does not fight for the slot; it reports that something else
holds it and leaves it there. It also means `VAL-R8` cannot be run while
Noctalia is running: the real test is a `pkexec` against a real password with
Celestina as the session's shell, which is the transition the author has
already named as the step after this one.

### One defect this unit found in itself

A controller built without a QML engine segfaulted rather than disabling
itself: `QQmlComponent` with a null engine is not a component that fails to
load, it is a crash. It now reports "no QML engine" and prompts for nothing,
which is what every other unreachable-prompt path already did.

## Limits

Nothing on a real compositor. The exclusive grab, the card's appearance, the
Escape and click-outside paths and the surface's behaviour when polkitd
cancels mid-prompt have all been exercised offscreen or in QML, where there is
no layer shell to grab with. Whether the grab is actually held — and whether
another surface can steal it — is `VAL-R8`, on a session where Celestina is
the shell.

No password has been typed into this prompt and no action has been authorized
by it.

The shell registers its agent at startup and gives the slot back when it exits
cleanly. A shell killed outright leaves polkitd to notice its bus name
disappear, which polkitd does on its own.
