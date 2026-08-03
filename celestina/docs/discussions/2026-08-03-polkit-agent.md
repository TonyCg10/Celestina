# Polkit agent ownership

- **Opened:** 2026-08-03
- **Status:** open
- **Question:** Which external Polkit agent should the session use, and should first-party ownership ever be considered?

## Context

Noctalia removal eventually needs a maintained authentication agent. Selecting
or replacing one changes both package and security boundaries.

## Strongest case

An established external agent minimizes authentication code owned by Celestina
and can be evaluated independently before R8.

## Counter-case

External agents may fit the visual/session lifecycle poorly; a first-party path
would greatly increase security-sensitive scope.

## Alternatives

Retain the current agent, select a Niri-suitable external agent, or open a
separately authorized first-party investigation after a concrete failure.

## Falsifiers and evidence needed

Candidate inventory, real authorize/cancel/failure behavior and explicit
approval for any installation or first-party security work.

## Conclusion

Pending. It blocks only the corresponding R8 slice.
