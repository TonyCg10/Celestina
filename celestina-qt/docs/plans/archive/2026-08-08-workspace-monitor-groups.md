# WSG-1 — workspace groups survive their monitor

- **Opened:** 2026-08-08
- **Plan ID:** workspace-monitor-groups
- **Status:** done
- **Closed:** 2026-08-08
- **Successor:** [DIAG-1 — a journal that survives the freeze](2026-08-08-diagnostic-journal.md)
- **Scope:** celestina
- **Implementation checkpoint:** WSG-1
- **Author-validation checkpoint:** `VAL-WSG-1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The author's session declares five workspaces on each of three monitors. When
two of those monitors are off — which is the state the live session is in on
2026-08-08 — Niri moves all fifteen onto the survivor and the panel draws
fifteen equal pills in a row. The strip becomes unreadable at exactly the moment
the person has the least screen to read it on.

The compositor cannot answer this. `niri msg workspaces` publishes the output a
workspace **is on** and has no field for the one it was configured for, so the
displaced state is indistinguishable from a session that genuinely has fifteen
workspaces on one monitor. If the grouping is to exist it has to be remembered
from a moment that could see it, or declared by the person, and the panel has to
be honest about which.

## Tangible outcome

A strip showing workspaces from more than one monitor draws the group holding
the focus in full and every other group as one capsule carrying its monitor's
name, its count and its urgency. Moving the focus into a collapsed group expands
it and collapses the one that had been open. Clicking a capsule is a focus
request like any other; the group opens because the focus arrived, not because
the capsule was clicked.

A strip whose workspaces all belong to one monitor — every monitor connected,
each panel showing its own five — renders exactly as it does today, with no
capsule and no added chrome.

## Scope

- Pure grouping policy and the learned/declared home memory in
  `celestina-shell-core`, with the rule that an observation may only teach what
  it is in a position to know.
- Durable persistence of the learned memory under the shell's own state
  directory, written before it is relied on.
- An additive workspace field on the existing Niri snapshot carrying the home,
  computed by the adapter that already owns `niri-ipc` types.
- A declaration route in the shell's own settings, so a memory that learned a
  layout the person has since changed can be repaired without deleting a file.
- The collapsible strip in QML, including urgency on a collapsed capsule,
  keyboard and assistive routes, visible focus and the reduced-motion path.
- Domain, protocol, QML contract and offscreen tests in the same units.

## Exclusions

- Reading, parsing or reacting to the author's Niri configuration. `open-on-output`
  is not available over IPC and this shell does not read another program's files.
- Changing which workspace the compositor puts where, or asking it to.
- The shell-wide visual language. This checkpoint gives the strip one bounded
  behaviour it does not have; it does not restyle the panel, and it does not
  pre-empt any direction SHELL-D5 may settle on. If UX-2 later restyles the
  strip, it restyles this behaviour rather than replacing it.
- A settings surface for editing declarations by pointer. The declaration is a
  settings value in this checkpoint; giving it a control centre row is separate.
- Any live activation without a separate explicit request.

## Build order

1. **WSG-1-A — Own the grouping as pure policy.** Add the learned/declared home
   memory, the refusal rules that keep a displaced observation from teaching,
   and the grouping that expands exactly one group. Domain tests only; nothing
   is wired to a provider, a snapshot or a surface.
2. **WSG-1-B — Publish and persist the home.** Compute each workspace's home in
   the Niri adapter and add it to the snapshot row additively, preserving every
   existing field. Persist the learned memory durably under the shell's state
   directory, writing only when something new was learned. A memory that cannot
   be read or written degrades the grouping to today's behaviour instead of
   failing the strip.
3. **WSG-1-C — Collapse the strip.** Render groups and capsules from the
   published home, with urgency and occupancy surviving the collapse, the focus
   request on a capsule, visible focus, assistive names and a reduced-motion
   route for the expansion.
4. **WSG-1-D — Deliver.** Bump the registered MINOR version, append the history
   row, run the registered guards and the canonical production exit, deploy the
   verified bytes without activation, and record only the live cases the author
   actually performs.

## Implementation exit

- The snapshot's existing workspace fields are unchanged and the added field is
  optional to every current consumer.
- No QML file decides where a workspace belongs; it presents the home the
  adapter published.
- Tests cover the single-output frame that must teach nothing, the displaced
  frame that must not overwrite a known home, a declaration overruling what was
  learned, urgency surviving a collapse, and a strip of one group rendering as
  it does today.
- A missing, unreadable or corrupt memory file leaves the strip working.
- `bash scripts/check-architecture-contract.sh`, the registered project verify
  script, `python3 scripts/version_tool.py check`, exact staged-unit checks and
  `scripts/complete-production.sh` pass before delivery.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| WSG-1-A | `celestina:` | done | [inventory](../../inventories/2026-08-08-workspace-monitor-groups/WSG-1-A.numstat.tsv) | 11 files, +1882/-93 | Remember each workspace's monitor and render the independent grouped-strip domain and QML paths; shared integration and release paths are owned by the atomic DIAG-1 companion inventory | [evidence](../../evidence/2026-08-08-workspace-monitor-groups.md) | `VAL-WSG-1` |

## Unit boundary

`WSG-1-A` is the whole checkpoint: the pure module, the adapter and host
publication, the strip and its two extracted components, the tests, the version
and the canonical production exit. The four steps of the build order above are
the order the work happened in, not four deliveries.

It does not own the roadmap's transition to the next checkpoint or the active-plan
index, which belong to the plan that succeeds this one.

### Recorded at declaration — 2026-08-08

`WSG-1-A`'s code was written before this ledger existed, at the author's
direction and during a design iteration session. It is recorded here as it
stands rather than rewritten to look like it came second.

### Atomic companion inventory — 2026-08-08

`WSG-1` closed without a commit and `DIAG-1` was then implemented in the same
worktree. The two units therefore close as one atomic batch. This inventory owns
the independent workspace-domain, QML, QML-test and development-session paths.
The DIAG-1 inventory owns every whole path that carries changes from both units,
including `CMakeLists.txt`, the Niri adapter and client, shared module
registration, documentation state and release metadata. This is a path-level
delivery boundary, not a claim that an intermediate WSG-only checkout existed.

### Recorded at closure — 2026-08-08

The checkpoint was finished in a later session under an explicit bounded
authorization from the author, after that session found the `WSG-1-B` step
already implemented in the worktree while the ledger still declared `WSG-1-A`
the only authorized unit. Nothing was reverted; the ledger was brought up to
what the worktree already contained and then carried to delivery.

Two gaps found while closing were repaired rather than inherited: the persisted
memory had a `SCHEMA_VERSION` that nothing enforced, and the declared route
named in `Scope` had no settings value behind it.

The four declared steps became **one** delivered ledger unit because they are one
atomic batch with one evidence record, and because two of the files cannot be
split between them: `celestina/CMakeLists.txt` carries both the new QML
registration and the version declaration, and an inventory boundary owns a whole
path. Recording four inventories would have meant either four near-identical
evidence records or a file assigned to a unit that did not earn it.

### Why this is not UX-2

`UX-2` is the shell-wide visual and interaction language and is gated behind
[SHELL-D5](../../discussions/2026-08-08-shell-visual-design.md) being applied
through an accepted decision. This checkpoint is not that. It answers one
observed defect in the author's live session with one bounded behaviour, and it
adds no token, no shared component and no anatomy that a later direction would
have to undo. The strip it produces is the strip UX-2 would go on to restyle.
