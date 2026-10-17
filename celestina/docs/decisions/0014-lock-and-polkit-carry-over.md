# ADR 0014: Lock and polkit carry ADR 0004 and 0005 over

- **Date:** 2026-10-16
- **Status:** accepted

## Context

The halted Qt shell recorded its session lock and its polkit agent in its own
ADRs, `celestina-qt/docs/decisions/0004-first-party-session-lock.md` and
`0005-first-party-polkit-agent.md`. Those are history under the halt in the
root `AGENTS.md`; they cannot be ported unread. Lock and polkit are the two
privileged duties of the parity program. See the design's §4.4.

## Decision

Lock (CEL-2-H) and polkit (CEL-2-I) carry those two decisions over as this
project's own, adapted and re-reviewed, and each lands with:

1. its own design, written when the unit is next;
2. a threat model and an explicit list of what the unit never does;
3. a review by a model other than the implementer, at high reasoning;
4. a recovery path that stays installed until the author's live validation
   passes (`swaylock` for the lock; polkit needs none, because without an
   agent actions fail closed);
5. a failure mode of "locked out" or "denied", never "let in".

Neither unit is planned or implemented by this record.

## Consequences

- The two units are gated by design and review, not by the parity schedule
  alone; they may land later than their neighbours without changing this
  record.
- Rehearsal of either unit happens in the nested niri only, never in the
  author's session.

## Revisit when

The author decides that a privileged duty may be taken over without a recovery
path, or the carried-over decisions are superseded by this project's own
design of CEL-2-H or CEL-2-I.
