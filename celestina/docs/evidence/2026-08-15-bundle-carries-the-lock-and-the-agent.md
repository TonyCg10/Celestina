# The bundle a transition would install, and what it was missing

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-E`
- **Artifact:** Celestina 0.29.2, the production bundle and the handover model
- **Environment:** `complete-production.sh` on the author's machine, deploying
  to `~/.local`; the running session was not changed
- **Plan:** [polkit authentication agent](../plans/active/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

The production pipeline was run to completion before the transition, and the
installed bundle was compared against what the shell actually spawns.

## Result

### Three programs the shell needs were not being installed

`celestina-lock`, `celestina-lock-verify` and `celestina-polkit-converse` are
separate processes on purpose — that is what ADR 0004 and ADR 0005 are about —
and the deployed bundle contained none of them. A transitioned session would
have had a shell that reports it cannot lock and cannot answer an
authorization prompt: honest, and useless.

They are now part of the artifact, the deploy and the status report, together
with the lock's shared facade and its Wayland shell-integration plugin tree,
whose directory shape Qt reads and which therefore travels whole. The launcher
points the lock at that tree so it never inherits a build path.

    installed: OK ~/.local/libexec/celestina/celestina-lock
    installed: OK ~/.local/libexec/celestina/celestina-lock-verify
    installed: OK ~/.local/libexec/celestina/celestina-polkit-converse
    installed: OK ~/.local/libexec/celestina/libcelestina-lock-session.so

### And the shell preferred the build tree over its own bundle

`LockController` looked for the lock at the path the build compiled in before
looking beside itself. On a deployed shell that is the wrong file: whatever
was last built in the source tree, at whatever version. Beside the shell now
wins, and the build path remains only for a shell run straight out of the
build directory.

This is the same class of defect as the deploy that once overwrote a running
shell's binaries: the development tree and the installed bundle are two
different things, and any path that silently prefers the first is a shell
running code nobody installed.

### The handover model knows the agent exists

    [~] screen lock — built in R6, but VAL-R6 has not been recorded
    [~] polkit authentication agent — built in R8, but VAL-R8 has not been recorded

Nothing on the list is unbuilt any more. Two of its tests said otherwise by
construction and now say the true thing instead: removal is refused until both
validations are recorded, and the report names which one closes each gap.

## Limits

The bundle was installed, not run as a session. Nothing here says the
transitioned shell starts, locks, or prompts — that is the transition itself,
and `VAL-R6` and `VAL-R8` are what it produces.
