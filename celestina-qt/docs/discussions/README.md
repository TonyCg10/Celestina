# Celestina open discussions

These are material product questions whose answer changes a later shell slice.
They are not implementation tasks and grant no package, session or repository
authorization.

| ID | Status | Question | Blocks |
|---|---|---|---|
| [SHELL-D5](2026-08-08-shell-visual-design.md) | open | What visual and interaction language should make Celestina feel coherent and usable? | UX-2 |

Closed on 2026-08-14, each with the author's explicit authorization for the
security-sensitive scope it names:

- SHELL-D1 (which external locker to compose) — superseded by
  [ADR 0004](../decisions/0004-first-party-session-lock.md): there is no
  external locker to choose once the shell owns the lock.
- SHELL-D2 (first-party session lock) —
  [ADR 0004](../decisions/0004-first-party-session-lock.md): own the lock,
  never own password verification.
- SHELL-D3 (Polkit agent) —
  [ADR 0005](../decisions/0005-first-party-polkit-agent.md): own the prompt,
  and nothing behind it.
- SHELL-D4 (running-app dock) —
  [ADR 0003](../decisions/0003-no-running-app-dock.md): no dock.

Conclude a discussion only with the evidence it names. Then add or supersede a
record under `../decisions/` and update the affected roadmap/plan before marking
the discussion `applied`.
