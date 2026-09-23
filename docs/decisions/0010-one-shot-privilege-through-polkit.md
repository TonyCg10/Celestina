# ADR 0010: Hematita acts with privilege one call at a time, through polkit, and never holds it

- **Date:** 2026-09-22
- **Status:** accepted

## Context

Hematita's fifth phase adds systemd services with start, stop and restart, and
lets a person end a process that is not theirs. Both need privilege the
monitor does not have, and the suite's rule is that nothing needs standing
privilege: no daemon, no setuid helper.

Three facts decide the shape.

systemd already authorises through polkit. `StartUnit`, `StopUnit` and
`RestartUnit` on the system bus are guarded by
`org.freedesktop.systemd1.manage-units`; polkit asks the session's
authentication agent, the person answers, and the call proceeds or is
refused. The user's own manager on the session bus needs no authorisation at
all. Hematita therefore needs no privilege of its own to offer services: it
asks systemd, and systemd asks the person.

Ending a foreign process has no such broker. The kernel refuses a signal to
another user's process, and the only sanctioned way to send one is to run
`kill` as root through `pkexec`, which again asks the session's agent under
`org.freedesktop.policykit.exec`. A polkit action of Hematita's own would only
change the wording of the prompt and would add an installed policy file to
the artifact; it is not needed for correctness.

The author's session runs `polkitd` and, today, no authentication agent: the
shell that will provide one is in standby by the author's order. Without an
agent every privileged action fails at the prompt. That is a state Hematita
must name, not hide, and not work around.

## Decision

- Privilege is one-shot and polkit-mediated. Hematita never runs as root,
  never keeps an authorised connection, never caches a credential, ships no
  daemon, no setuid binary and no polkit action file.
- User units are managed over the session bus without authorisation. System
  units are listed over the system bus without authorisation and managed
  through `StartUnit`/`StopUnit`/`RestartUnit` with mode `replace`, letting
  polkit authorise each call.
- A foreign process is signalled by spawning `/usr/bin/pkexec /usr/bin/kill
  -TERM <pid>` or `-KILL <pid>` — a fixed argument shape, never a shell —
  from a worker thread, after the same re-validation of uid and start time
  that guards the user's own processes.
- Every privileged action reports a typed outcome the page turns into words:
  `done`, `no-agent` (no authentication agent answered), `denied` (the
  person cancelled or was not authorised), `failed`, `refused` (Hematita did
  not try). A missing agent is not an error of Hematita's and is not
  retried.
- Nothing in this decision touches the Celestina shell; when the shell
  provides an agent, Hematita's prompts start working without a change.

## Consequences

- The Services page works fully for user units today and degrades honestly
  for system units and foreign processes until the session has an agent.
- The artifact gains no installed file beyond the binary and its desktop
  entry; the security surface is polkit's and systemd's, not Hematita's.
- `pkexec` sets the target's environment; only `kill` runs, so nothing of
  Hematita's inherits root.

## Revisit when

- The session gains an agent and the prompts' wording proves inadequate: a
  Hematita polkit action with its own message would then be justified.
- systemd exposes pidfd-based signalling through the bus, or the kernel adds a
  sanctioned kill-by-pidfd for foreign processes, which would remove
  `pkexec`.
