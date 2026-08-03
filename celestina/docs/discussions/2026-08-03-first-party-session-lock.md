# First-party session lock ownership

- **Opened:** 2026-08-03
- **Status:** open
- **Question:** Should Celestina ever own an `ext-session-lock` and PAM implementation?

## Context

Owning a lock screen means taking responsibility for a security-critical
Wayland protocol, authentication, failure containment and output lifecycle.

## Strongest case

First-party ownership could provide exact visual and lifecycle integration once
the simpler external composition has demonstrated a real limitation.

## Counter-case

The security and maintenance burden is much larger than a shell feature and an
error can expose an unlocked session.

## Alternatives

Keep an external locker permanently, contribute missing behavior upstream, or
implement only a non-authenticating visual surface around an external lock.

## Falsifiers and evidence needed

A reproduced external-locker limitation, threat model, PAM review boundary and
explicit author authorization for the security-sensitive scope.

## Conclusion

Pending and outside R3. R6 cannot start from this discussion alone.
