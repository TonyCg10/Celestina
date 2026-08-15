# The lock plan closes, and five responsibilities become validated

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R6-Z`
- **Artifact:** the R6 plan, `celestina/VALIDATION.md` and the checkpoint
- **Environment:** the repository's own records and
  `scripts/handover-status.sh`; no session was touched and nothing was rebuilt
- **Plan:** [first-party session lock](../plans/archive/2026-08-14-first-party-session-lock.md)
- **Validation:** `VAL-R6`

## Procedure

The plan's six units were checked against their inventories and evidence
records, the author's validation declarations of 2026-08-14 were written into
`VALIDATION.md` beside the earlier results rather than over them, and
`scripts/handover-status.sh` was read afterwards.

## Result

### The plan closes on its implementation exit

`R6-A` through `R6-F` are all `done` with an inventory and a dated record
each. The exit this plan set — a refusal for a wrong passphrase, a cover for
every output, an uncalled `unlock_and_destroy` on every failure branch, and a
refused suspend without a confirmed lock — is met by regressions that run
without a person.

### `VAL-R6` does not move

Nothing here was validated by the author. No passphrase has been typed into
the lock, no lid has closed on it, and the nest's one-EGL-client limit leaves
open whether a real session running both a shell and a lock behaves the same.
The plan closes with that question written down rather than absorbed.

### Five responsibilities the author declared validated

On 2026-08-14 the author declared the launcher and clipboard history, the
notifications, the control centre and session menu, and the wallpaper and
portal values validated in daily use. Those sections were holding at
`deferred`, `failed` and `partial`.

Each declaration sits beside the earlier result, not in place of it, because
the old findings are what makes the new status mean anything. One of them is
now demonstrably fixed rather than merely re-declared: `VAL-R4` failed on
Escape not closing the notification centre once focus left its inner list, and
Escape is now a window-context `Shortcut`, so it closes from wherever focus
sits. That was read in the source, not re-observed, and the record says so.

What no declaration covers is named in it: the screen-reader paths, a real
`Terminal=true` entry, a configured weather location, a forced provider-write
failure, the paired-phone path and the Niri colour-include comparison remain
untested.

### The handover

    [x] launcher and clipboard history — R2
    [x] notifications — R4
    [x] control centre and session menu — R5
    [x] wallpaper and session look — R7
    [~] screen lock — built in R6, but VAL-R6 has not been recorded
    [ ] polkit authentication agent — nothing in this shell provides it yet

Two responsibilities still block removing Noctalia, and the checkpoint now
moves to the second of them.

## Limits

This record changes documents, not behaviour. No code was modified, nothing
was rebuilt, and every declaration above rests on the author's own use of the
shell rather than on a procedure anybody could repeat from this file.
