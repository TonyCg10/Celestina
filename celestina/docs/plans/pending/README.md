# Authorized plans waiting for their checkpoint

A plan lives in `active/` only while its checkpoint is the one this project's
ROADMAP names as active, and exactly one checkpoint is active at a time. These
plans are written, authorized and bounded, but their checkpoint has not opened
yet: they are not work in progress and no unit in them may be built from here.

Each moves to `active/` unchanged, keeping its `Plan ID`, when the ROADMAP
makes its checkpoint the active one — which for both of these means PANEL-1's
own ledger units are closed first.

| Plan | Checkpoint | Authorized by |
|---|---|---|
| [First-party session lock](2026-08-14-first-party-session-lock.md) | R6 | [ADR 0004](../../decisions/0004-first-party-session-lock.md) |
| [Polkit authentication agent](2026-08-14-polkit-authentication-agent.md) | R8 | [ADR 0005](../../decisions/0005-first-party-polkit-agent.md) |
