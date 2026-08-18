# The lock learns its wallpaper without ever waiting for it

- **Date:** 2026-08-17
- **Scope:** Celestina unit `LOCK-1-A`
- **Artifact:** `LockController::setBackdrop`/`sendBackdrop`, `LockBackdrop`, and
  the shell wiring in `celestina/src/main.cpp`
- **Environment:** `ctest --test-dir celestina/build -R lock-controller`, on the
  author's own machine; no session was touched
- **Plan:** [lock depth transition](../plans/archive/2026-08-17-lock-depth-transition.md)
- **Validation:** `VAL-LOCK-1`

## Procedure

`celestina-lock-controller-test` was run after adding two cases to it: one that
starts a lock and never gives it a backdrop at all, and one that hands it a mix
of absolute and relative paths.

## Result

### Confirmation is never gated on the hand-off

`aLockThatNeverReadsItsBackdropStillCovers` starts a fake lock that prints
`locked` and never touches its stdin. The controller still observes
`lockedChanged` and reports the session locked — the backdrop line is written
and the write channel closed without either blocking on or waiting for the
lock to read it, exactly as `LockController::sendBackdrop` requires of itself.

### Only absolute paths travel

`theBackdropCarriesOnlyAbsolutePaths` hands the controller a map with one
absolute and one relative entry. `backdropLine` drops the relative one before
it reaches the wire: the lock is a different process with a different working
directory, so a relative path there would name a different file than it did in
the shell, and a wrong picture behind a passphrase field is worse than the
canvas.

Both cases passed. The full suite ran as part of
`scripts/complete-production.sh`'s `ctest` pass, recorded in
[the depth-transition evidence](2026-08-17-lock-depth-transition.md).

## Limits

This covers the hand-off mechanism only: that a backdrop is never required to
cover the screen, and that only absolute paths are ever sent. It says nothing
about what a cover does with a path once it has one — that is
`LOCK-1-B`'s own evidence.
