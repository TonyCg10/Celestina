# DIAG-1 — a journal that survives the freeze

- **Opened:** 2026-08-08
- **Plan ID:** diagnostic-journal
- **Status:** done
- **Closed:** 2026-08-08
- **Successor:** `UX-2` remains planned; no implementation plan is active
- **Scope:** celestina
- **Implementation checkpoint:** DIAG-1
- **Author-validation checkpoint:** `VAL-DIAG-1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

An AMD Radeon RX 9070 XT (Navi 48) has repeatedly ended in
`amdgpu 0000:03:00.0: device lost from bus!` while Celestina was running. The
most recent occurrence happened with Celestina inside a **nested Niri session**
started by `scripts/dev-session.sh`. That nest separated the surfaces and
nothing else: the GPU, the VCN block, the DDC/I²C buses and the session bus were
all still shared with the host session. The Noctalia → Celestina → Noctalia
handover is therefore no longer a necessary condition, and neither is a live
layer-shell surface.

**This plan does not assume Celestina is the cause.** The evidence to date is
coincidence, and coincidence is not causation. The problem is narrower and
answerable: after a freeze and a physical reset, *nobody can currently
reconstruct what Celestina was doing in the seconds before it*. That is a defect
in this shell's own observability, and it is worth fixing whether or not the
shell turns out to be innocent.

Today the record is not reconstructable because:

- the host emits scattered Qt messages;
- the helpers use `eprintln!`;
- several external processes discard `stderr` entirely;
- `CELESTINA_PROVIDER_TRACE` is optional and off;
- nothing correlates host, helpers, workers, commands, external processes and
  surfaces into one ordering;
- a run started from a terminal is not guaranteed to reach journald at all;
- and a freeze plus a physical reset can lose whatever was only ever in a
  buffer.

## Tangible outcome

Every Celestina process writes a structured, bounded, always-on JSONL journal
under `$XDG_STATE_HOME/celestina/diagnostics/`, correlated by one `run_id` the
host generates and passes to its helpers. After a freeze and a reset, a
read-only script collects those files plus the relevant kernel and user
`journalctl` lines into one deterministic bundle.

The journal records **classes of event, identities and timings** — never
clipboard content, notification bodies, media metadata, window titles, launched
commands, secrets or external payloads.

## Scope

- Pure event, level, field, redaction, bound, rotation and serialization policy
  in `celestina-shell-core`.
- A non-blocking bounded sink for the Rust helpers, with an explicit drop policy,
  a loss counter, critical-event flushing and a deterministic bounded drain at
  shutdown.
- The host's own journal writer and its instrumentation, including generating the
  `run_id` and handing it to both helpers through their environment.
- Instrumentation of the event classes adjacent to the observed failure: host
  and helper lifecycle, helper restarts and backoff, external processes, DDC,
  Niri connectivity and actions, D-Bus ownership and shutdown. Media remains a
  passive MPRIS consumer and is identifiable through provider publication and
  process absence; private payloads and pointer-level transient-UI events are
  deliberately not logged.
- A compact mirror to stderr / Qt logging so journald captures what it can, with
  an environment switch that quiets only the mirror and never the file.
- `celestina/scripts/diagnostic-report.sh`, read-only.
- Documentation of the format, location, limits, privacy, collection procedure,
  read-only inspection commands and the limits of what a journal can prove.

## Exclusions

- Any investigation of, or change to, the GPU, DDC behaviour, amdgpu, the
  kernel, Niri, systemd, Noctalia or configuration outside this repository.
- Any claim about causation. The journal is an instrument, not a verdict.
- Wi-Fi, which is explicitly out of scope.
- Starting, activating, restarting or running Celestina, and any smoke that could
  reach a real GPU, DDC bus, real Wayland or the live session.
- A remote or networked sink. Nothing here sends anything anywhere.

## Build order

1. **DIAG-1-A — Own the journal as pure policy.** Levels, components, the event
   record and its field vocabulary, bounded reasons, the redaction rules, one
   JSON line per event, the file and directory limits, and the deterministic
   rotation decision. Domain tests only; nothing touches a filesystem.
2. **DIAG-1-B — A sink that cannot hurt what it observes.** The bounded queue,
   the explicit drop policy and its loss event, the background writer, rotation,
   private permissions, recovery from a truncated, corrupt or unwritable path,
   the critical-event flush and the bounded drain at shutdown. Tested against
   temporary directories and fake failures.
3. **DIAG-1-C — Instrument the helpers.** The Niri adapter and the aggregate
   provider runtime: process lifecycle, external process start/exit/timeout/
   cancel/kill/reap, DDC detect/read/write with its bus and output identities and
   its serialization proof, Niri connection and GPU-adjacent actions, and clean
   helper shutdown.
4. **DIAG-1-D — Instrument the host.** The `run_id`, its propagation into both
   helpers' environment, host startup and classified arguments, D-Bus name
   acquisition and refusal, both helpers' start/generation/backoff/death and the
   bounded shutdown.
5. **DIAG-1-E — Deliver.** The read-only report script, the documentation, the
   version bump, the guards and the canonical production exit.

## Implementation exit

- Every emitted line is valid JSON on one line, with the declared field names
  identical across the host and both helpers.
- No test starts Celestina, runs `ddcutil`, touches hardware, opens real Wayland
  or queries a real MPRIS player. Fake processes and temporary directories only.
- Hostile fixtures containing clipboard text, notification bodies, media
  metadata, window titles, launched commands and secrets produce journal lines
  that contain none of them.
- A journal that cannot be written never blocks, never terminates and never
  changes what a provider or the host does.
- Dropped events are counted and published rather than silently lost.
- Shutdown gives the writer a bounded drain interval and never lets an
  unresponsive filesystem hold process exit open. A writer that misses the
  deadline retains process-lifetime storage and is ended by process exit.
- `bash scripts/check-architecture-contract.sh`, the language and documentation
  guards, both Rust suites, `qmllint` and CTest pass.

### Production exit

`scripts/complete-production.sh` built, verified and deployed celestina 0.8.0.
Its smoke ran only `--pick-output` with Qt `offscreen`, scratch XDG directories
and a nonexistent session bus; that path starts neither helper and reaches no
DDC, real MPRIS or live Wayland surface. The live session was not activated.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| DIAG-1-A | `celestina:` | done | [exact inventory](../../inventories/2026-08-08-diagnostic-journal/DIAG-1-A.numstat.tsv) | 26 files, +4682/-52 | Deliver the bounded journal, crash-relevant instrumentation, read-only collector and verified 0.8.0 bundle | [evidence](../../evidence/2026-08-08-diagnostic-journal.md) | `VAL-DIAG-1` |

## Atomic delivery boundary

`WSG-1` and `DIAG-1` were completed in one worktree without an intermediate
commit. Their inventories therefore divide whole paths in one atomic delivery:
WSG owns its independent domain and QML paths; DIAG owns every shared integration
path and the release metadata. No claim is made that an intermediate checkout
with only WSG existed.

## Why this is a separate plan

`WSG-1` closed in celestina 0.8.0 before this plan opened, and the two share
nothing: one answers a strip that became unreadable, the other answers a record
that cannot be reconstructed. Mixing the journal into `WSG-1`'s units would have
put an unrelated change inside a delivered checkpoint's inventories.
