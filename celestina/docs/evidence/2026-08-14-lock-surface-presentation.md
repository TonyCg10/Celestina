# What a locked screen is allowed to show

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R6-C`
- **Artifact:** Celestina 0.24.0, `qml/LockScreen.qml`
- **Environment:** nested niri (`dev-session.sh`), one `winit` output at
  3840x2160 scale 1.5, `QT_QUICK_BACKEND=software`, which the nest needs for a
  second EGL client and the lock itself does not; no live session
- **Plan:** [first-party session lock](../plans/active/2026-08-14-first-party-session-lock.md)
- **Validation:** `VAL-R6`

## Procedure

The lock was started against the nested session and photographed with `grim`
while covering the output.

## Result

The cover shows the time, the date, and a prompt on the shell's own dense
content material — and nothing else. ADR 0004's list of what a lock screen may
not render (notification bodies, media titles, clipboard, window list) is not
implemented anywhere in this file, which is the only way that rule can be kept:
by the content not existing rather than by a flag that hides it.

### The material is opaque, and that is the design

`ext-session-lock` means the compositor has stopped showing the session, so
there is nothing behind this surface to see through. A translucent material
here would be glass over a void. The prompt therefore sits on
`ContentSurface` with `externalBackdropReady: false`, which is the shell's
existing no-compositor-sample fallback: a readable fill, reached through the
same component every card in this shell uses rather than a colour invented
for the lock.

### A defect the screenshot caught

The first render read **"Friday 14 de August"** — English weekday and month
names joined by a Spanish preposition, because the date was formatted with a
hand-built string against whatever locale the process inherited. The panel's
own clock had already solved this and says why: a shell spawned from a
C-locale service prints an English date into a Spanish surface. The lock now
asks for `es_ES` explicitly, exactly as `Clock.qml` does, and reads
**"viernes 14 de agosto"**.

That is worth recording rather than quietly fixing: the lock is a separate
process, and every convention the shell holds by construction is one this
program has to be given deliberately.

## Limits

Nothing here was typed into. The field's behaviour on a real attempt — the
clearing after a refusal, the disabled state while checking, the wording for
each verdict — is wired to `LockAuthenticator`'s verdicts and covered by that
unit's regressions, but no passphrase has been entered on this surface by a
person. That is `VAL-R6`.

The clock ticks once a second while the surface is visible and renders to the
minute. Whether that is the right cost on a machine left locked for hours is
not measured here.
