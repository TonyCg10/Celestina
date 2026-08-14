# Authorized plans waiting for their checkpoint

A plan lives in `active/` only while its checkpoint is the one this project's
ROADMAP names as active, and exactly one checkpoint is active at a time. These
plans are written, authorized and bounded, but their checkpoint has not opened
yet: they are not work in progress and no unit in them may be built from here.

Each moves to `active/` unchanged, keeping its `Plan ID`, when the ROADMAP
makes its checkpoint the active one — which is how the lock's own plan
left this directory.

| Plan | Checkpoint | Authorized by |
|---|---|---|
| [Polkit authentication agent](2026-08-14-polkit-authentication-agent.md) | R8 | [ADR 0005](../../decisions/0005-first-party-polkit-agent.md) |
