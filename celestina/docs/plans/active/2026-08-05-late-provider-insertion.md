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

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LVR-3-A | `celestina:` | active | provider host/QML binding; host single-instance ordering; provider tool/brightness lifecycle; focused regressions; version and evidence records | pending | Make late provider insertion visible and prevent a rejected or terminating host from starting, overlapping or abandoning automatic DDC work | source regressions written; execution and canonical production exit suspended by the GPU safety hold | `VAL-R1-01`, `VAL-GPU-01` |

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
