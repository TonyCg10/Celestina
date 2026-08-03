# External locker for composed suspend

- **Opened:** 2026-08-03
- **Status:** open
- **Question:** Which existing Niri-compatible locker may Celestina compose for a safe lock-and-suspend path?

## Context

R3 can implement the typed command and refusal path without choosing or
installing a locker. The composed lock slice must not suspend until a confirmed
lock is active.

## Strongest case

A mature external locker narrows the security surface and lets Celestina own
only sequencing, timeout, failure reporting and rollback.

## Counter-case

An unsuitable locker may not provide a truthful readiness signal or may fail
under output hotplug, leaving safe suspend impossible.

## Alternatives

Keep the slice gated, approve a known installed locker after live evaluation,
or separately authorize the first-party investigation in `SHELL-D2`.

## Falsifiers and evidence needed

An approved candidate, its exact readiness/failure contract under Niri and
permission to install it if it is not already present.

## Conclusion

Pending. R3 work outside composed lock may proceed; no agent guesses or installs
a candidate.
