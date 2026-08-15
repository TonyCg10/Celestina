# The handover model stops saying nothing locks the screen

- **Date:** 2026-08-14
- **Scope:** Celestina unit `R6-F`
- **Artifact:** Celestina 0.25.2, `celestina-shell-core::handover`
- **Environment:** the repository's own tests and `scripts/handover-status.sh`,
  which is read-only and touches no session
- **Plan:** [first-party session lock](../plans/active/2026-08-14-first-party-session-lock.md)
- **Validation:** `VAL-R6`

## Procedure

`scripts/handover-status.sh` was run before and after correcting the
responsibility table, and the crate's own tests exercised the distinction the
correction introduces.

## Result

Before, with `R6` already delivered:

    [ ] screen lock — nothing in this shell provides it yet
    [ ] polkit authentication agent — nothing in this shell provides it yet

After:

    [~] screen lock — built in R6, but VAL-R6 has not been recorded
    [ ] polkit authentication agent — nothing in this shell provides it yet

The refusal to remove Noctalia is unchanged, and deliberately so: the lock
still blocks the handover. What changed is *why* — from "nobody built it" to
"nobody has watched it work", which are different problems with different
fixes, and the tool was reporting the first while the second was true.

The crate's tests now pin both halves: the agent is the only remaining
`NotImplemented`, and a separate case asserts the lock is not among them while
still blocking on `VAL-R6`. Seven handover tests pass.

## Limits

This corrects a description, not a capability. Nothing here says the lock
works on the author's machine — that is exactly the `VAL-R6` the table now
asks for.
