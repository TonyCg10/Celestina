# The cut card, reproduced by the assistant instead of reported twice

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-G`
- **Artifact:** Celestina 0.29.4, `qml/PolkitPrompt.qml` and
  `PolkitPromptController`
- **Environment:** the author's real session on Celestina 0.29.3. A `pkexec`
  was raised by the assistant, photographed with `grim` on every output, and
  cancelled by killing the `pkexec` process — nothing was typed into the
  session.
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

`R8-P-F` shipped a fix for the cut card and the author reported the prompt
unchanged. Rather than a third blind fix, the prompt was reproduced and
photographed: `pkexec true`, `grim` per output, `pkill pkexec`.

## Result

### The photograph said two things the report could not

The card was still its header alone — so `R8-P-F`'s `implicitHeight` change
had treated a symptom — and the prompt had opened on DP-1, an output running
`niri-blackout`, while the author works on another. The author had already
said prompts open on the wrong monitor; the photograph added that the wrong
monitor was a deliberately blacked-out one.

### The real cause of the cut

`MenuSection` is a backdrop plate: it fills its parent and sits at `z: -1`.
Every overlay uses it as a *sibling* inside an `Item` that owns the geometry —
and the prompt used it as a container, so both sections stretched over the
whole column behind the header and the card measured nothing but its header.
The prompt now uses the same anatomy as every other overlay, and the
regression asserts reading order in mapped coordinates — header above message
above field — which is the check that would have failed all along, where the
bottom-of-card check `R8-P-F` added passed because everything overlapped
inside it.

### The wrong monitor

`QCursor::pos()` is not an answer on Wayland: a layer-shell client cannot ask
where the pointer is, and Qt reports a stale or zero position, which lands on
whatever output owns the origin. The compositor knows where the person is —
the output holding the focused workspace — and the host now wires that from
`NiriClient` into the prompt controller, with the primary screen as the only
fallback.

## Limits

Reproduced offscreen and photographed once live before the fix; the fixed
prompt has not yet been photographed on the real session. The author's other
reports — membrane and animation inconsistency, the dense material where the
veil should be — are outside this unit and remain open; the dense strength
still requires the patched compositor, which is not the session's.
