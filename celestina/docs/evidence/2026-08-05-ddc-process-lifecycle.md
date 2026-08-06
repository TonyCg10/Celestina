# Evidence: 2026-08-05 DDC process lifecycle correction

- **Date:** 2026-08-05
- **Status:** implementation active; executable evidence suspended
- **Scope:** celestina — the startup, shutdown and bounded-tool paths that can
  start, overlap or abandon automatic DDC work
- **Trigger:** the GPU-loss audit found automatic overlapping DDC work and
  concrete Celestina ownership defects
- **Environment:** static source review only. No formatter, compiler, unit
  test, integration test, production build, smoke, deployment or live
  activation ran; the live session remained exclusively on Noctalia
- **Safety condition:** Noctalia alone owns the live session until the author
  explicitly ends the observation
- **Artifact:** none — no binary was produced or may be produced during the
  hold. The outputs are the source corrections listed below and this record

## Procedure

Read-only review of the Celestina host and provider-helper lifecycle paths
named below, entered from the GPU-loss audit's finding of automatic overlapping
DDC work. The corrections were then written as source and text only, under the
safety condition above; nothing was executed.

## Result

### Celestina findings

The Qt host constructed `ShellProvidersClient` before attempting to own
`org.celestina.Shell`. A second host that was destined to defer could therefore
start the aggregate helper and its automatic `ddcutil detect --brief` before
the D-Bus rejection.

The brightness worker discarded its thread handle and had no cancellation
condition. Signal and stdin shutdown paths called `process::exit(0)`, which
does not run Rust destructors. The bounded-tool path killed and waited for the
direct child on timeout, but returned from a `try_wait()` error without doing
either. The host owned only the helper QProcess and could not clean up a DDC
child abandoned by that helper.

These paths make an orphan or overlap structurally possible. The retained
journal lacks historical PPIDs, so it does not prove that a particular crash
contained such an orphan.

### Corrective intent

- Claim the session name before constructing any provider, tray or surface
  owner that can start external work.
- Replace abrupt helper exits with one shared shutdown request.
- Retain and join the brightness worker.
- Make bounded tools observe shutdown, kill and reap their direct child, and
  never abandon that child after a wait error.
- Wait until the active DDC child and command worker have drained before
  returning from the helper.
- Preserve the established detached-launch contract for applications the user
  explicitly launches; those are not provider probes.

The direct `ddcutil` child is the concrete ownership boundary: ddcutil is not
observed to create a decoder or long-lived descendant in this path. The generic
runner still does not claim process-tree ownership for arbitrary programs, so
future providers that spawn process trees require an explicit process-group
contract rather than inheriting this DDC fix by assumption.

### Source corrections written during the hold

- `src/main.cpp` now claims the shell name before constructing Niri, provider
  or tray adapters.
- `src/shellservice.{h,cpp}` allows the accepted host to wire Niri after that
  claim without weakening the D-Bus interface.
- `src/provider_adapter/brightness.rs` retains the worker in an RAII owner;
  normal shutdown and every later initialization error request cancellation and
  join the thread.
- `src/provider_adapter/tools.rs` checks cancellation while a DDC child is
  active and kills and reaps it on cancellation, timeout or `try_wait` error.
- `src/provider_adapter/main.rs` shares one shutdown request between signals,
  stdin loss, the command worker and the brightness worker; it no longer calls
  `process::exit()`. `CELESTINA_DISABLE_DDC` provides the process-local safety
  boundary required by later isolation phases without changing other providers.
- `src/shellprovidersclient.cpp` gives the orderly helper path three seconds
  before escalating to TERM/KILL.

A focused source regression was added for cancellation of a bounded direct
child. It is intentionally unexecuted while the safety hold remains active.

The correction does not claim to fix amdgpu, firmware or hardware. It removes
Celestina-owned process races that are invalid independently of the GPU root
cause.

## Limits

- The retained journal lacks historical PPIDs, so this record does not prove
  that a particular crash contained an orphaned DDC child.
- The correction makes no claim about amdgpu, firmware or hardware.
- Deferred evidence: no formatter, compiler, unit test, integration test,
  production build, smoke, deployment or live activation may run during the
  Noctalia-only observation. Static source review is the only evidence
  permitted now. The registered architecture guard, project verification,
  canonical production exit and live transition matrix remain required after
  the author releases the hold.
