# LVR-3 — late provider insertion

- **Opened:** 2026-08-05
- **Plan ID:** late-provider-insertion
- **Status:** active
- **Authorization:** the author requested the confirmed media defect be fixed,
  then authorized the Celestina-side corrections found by the read-only GPU
  loss audit while the live session remains exclusively on Noctalia
- **Scope:** celestina
- **Implementation checkpoint:** LVR-3
- **Author-validation checkpoint:** `VAL-R1-01` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The aggregate helper publishes valid media, but the panel does not observe a
provider key inserted after its first provider-map binding. Restarting the
helper makes media part of the replacement generation's initial populated map,
which is why the same player then appears.

## Tangible outcome

A provider key inserted after the panel's first provider-map binding becomes
visible without restarting the helper, and no rejected or terminating host
starts, overlaps or abandons automatic DDC work — with removal, later
reappearance and every other provider reading preserved.

## Scope

- Reproduce a provider map gaining `media` after initial component creation.
- Correct the existing host-to-panel state binding without adding a provider,
  protocol field or surface.
- Preserve removal, later reappearance and every other provider reading.
- Correct provider startup and shutdown paths that can overlap or abandon an
  active DDC child.
- Record the system evidence separately from the Celestina defect analysis.
- Defer every executable check, build, deployment and live validation until the
  author ends the long Noctalia-only observation.

## Exclusions

- MPRIS discovery redesign; the isolated helper already publishes correctly.
- Any new multimedia capability or visual redesign.
- The separate notification and held-child live reruns.
- Diagnosing Wi-Fi, changing the kernel, firmware, Mesa, DDC configuration,
  Noctalia or the live session.
- Claiming that Celestina caused the PCIe device loss; the retained evidence
  establishes correlation and correctable process defects, not causation.

## Build order

1. Make late provider insertion visible in the host-to-panel state binding, and
   prevent a rejected or terminating host from starting, overlapping or
   abandoning automatic DDC work (`LVR-3-A`).
2. Deliver the static-audit corrections and expose one revision-coupled
   provider lookup to both the compiled module and direct-directory QML tests
   (`LVR-3-B`).
3. Repair the two defects `LVR-3-B` introduced in that same path: address the
   escalation timer to the helper instance it was armed against, and let the
   handler that receives `exitStatus` own the restart delay (`LVR-3-C`).

## Implementation exit

The registered architecture guard, project verification, the canonical
Celestina production exit and the live transition matrix remain required, and
all of them are suspended for the duration of the safety hold below. No
Celestina executable, provider, build, test, deployment or activation may run
until the author explicitly ends the Noctalia-only observation, so the
implementation exit is reached only after that release.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LVR-3-A | `celestina:` | active | provider host/QML binding; host single-instance ordering; provider tool/brightness lifecycle; focused regressions; version and evidence records | pending | Make late provider insertion visible and prevent a rejected or terminating host from starting, overlapping or abandoning automatic DDC work | source regressions written; execution and canonical production exit suspended by the GPU safety hold | `VAL-R1-01`, `VAL-GPU-01` |
| LVR-3-B | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-B.numstat.tsv) | 52 files, +1758/-289 | Deliver 0.6.4 with the static-audit corrections and expose one revision-coupled provider lookup to both the compiled module and direct-directory QML tests; the initial singleton resolved as a type rather than a callable instance in the latter | 155 core tests, 43 shell Rust tests, Clippy, qmllint, 13 CTest targets and offscreen smoke pass; canonical verification is blocked only by unrelated Siderita architecture ratchets. The audit that specified these corrections is recorded in [static shell audit](../../evidence/2026-08-05-static-shell-audit.md) | `VAL-R1-01`, `VAL-GPU-01` |
| LVR-3-C | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-C.numstat.tsv) | 10 files, +161/-7 | Address the escalation timer to the helper instance it was armed against, so a replacement started inside the grace window is not killed mid-`ddcutil`; and let the handler that receives `exitStatus` own the restart delay, so the spacing an unclean exit earns is actually applied — with `FailedToStart` scheduled where it is, since Qt emits no `finished()` for a process that never ran | None: the GPU safety hold forbids building, testing or running this project. Reviewed by reading and recorded in [helper restart ownership evidence](../../evidence/2026-08-06-helper-restart-ownership.md) | `VAL-R1-01`, `VAL-GPU-01` |
| LVR-3-D | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-D.numstat.tsv) | 9 files, +129/-6 | Classify an oversized frame in the Niri adapter the way the provider helper already classifies it: report and skip it rather than end the session, so a frame the host would discard no longer tears down the compositor connection, empties the workspace strip and reconnects into the identical refusal | None: the GPU safety hold forbids building, testing or running this project. Reviewed by reading and recorded in [oversized frame evidence](../../evidence/2026-08-06-oversized-frame-is-skipped.md) | `VAL-R1-01` |
| LVR-3-E | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-E.numstat.tsv) | 8 files, +104/-15 | Gather the helper target's sources with a configure-time glob instead of a hand-written list that had drifted to ten of nineteen files and named none of the shell core, so the dependency set stops claiming something untrue | None: the GPU safety hold forbids configuring, building, testing or running this project. Reviewed by reading and recorded in [helper sources evidence](../../evidence/2026-08-06-helper-sources-gathered.md) | `VAL-R1-01` |

## Recorded trigger

Celestina 0.6.2 started while Firefox was already playing. The original helper
remained healthy but the panel showed no media. A second isolated helper
published the exact Firefox media payload immediately. Sending SIGTERM to the
original helper produced a clean replacement generation, after which the media
region appeared without changing playback.

## Safety hold

The author is running Noctalia alone to determine whether removing the complete
Noctalia to Celestina to Noctalia transition removes the random GPU loss. No
Celestina executable, provider, build, test, deployment or activation may run
until the author explicitly ends that observation. LVR-3-B may change only
repository text and source during the hold.

LVR-3-C repairs LVR-3-B. Both defects are in the path that exists to keep a DDC
child from being abandoned, and both made it do the opposite: one could kill a
healthy helper during its first `ddcutil detect`, and the other left the spacing
after an unclean exit unreachable. Like LVR-3-B, it changes only source and text
during the hold, and unlike LVR-3-B it carries no automated evidence at all —
nothing may be compiled or run against this project until the author ends the
observation.

LVR-3-D finishes what aligning the frame budgets started. The helper learned to
tell a write failure from a frame the host would discard; the Niri adapter did
not, so it read the second as the end of its session and reconnected into the
same refusal. Like the units before it, it changes only source and text and
carries no automated evidence, because nothing may be compiled or run against
this project until the author ends the observation.
