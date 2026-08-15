# What the first real prompt looked like, and the two defects it showed

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-F`
- **Artifact:** Celestina 0.29.3, `qml/PolkitPrompt.qml`
- **Environment:** the author's real session, Celestina 0.29.2 as the session
  shell, stock niri 26.04; a real `pkexec` raised the prompt
- **Plan:** [polkit authentication agent](../plans/active/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

The author ran a real authorization on the transitioned session — the first
time this surface met a person — and reported what they saw. Both defects were
reproduced in the offscreen suite before being fixed.

## Result

### The card was cut to its header

The live prompt showed the title and the identity and nothing else: no
message, no action id, no password field. The card's height sums its column's
`implicitHeight`, and both content sections declared `height` alone — so each
contributed zero, and the card sized itself to the one child that had an
implicit height. The sections now declare `implicitHeight`, and a regression
pins the real thing: the password field and the action id must end inside the
card, with the failure message naming both numbers.

### A click on empty space threw the authorization away

The surface followed the overlays' outside-click-dismisses convention, and for
a password prompt that convention is wrong: this surface holds the keyboard
exclusively, a request is spent by its dismissal, and a stray click cost the
author their attempt. A click outside now only returns focus to the field; the
ways out are the ones a person means — Escape, or answering.

## Limits

The same live run surfaced visual defects that are not this surface's:
saturated colours, the dense material appearing where the veil should be, and
menu animation inconsistencies. Those trace to the session compositor, not the
shell — the real session runs stock niri with none of the nest's blur profile,
and the author's `config.kdl` gained the global profile in the same sitting.
The patched compositor the dense material needs is built and not yet the
session's; that decision remains the author's.

No successful authentication has been observed on this surface yet: the card
was cut before the field existed to type into. That observation is still owed
to `VAL-R8`.
