# LVR-3 — late provider insertion

- **Opened:** 2026-08-05
- **Plan ID:** late-provider-insertion
- **Status:** done
- **Closed:** 2026-08-07
- **Successor:** `UX-1` is active in
  [`../active/2026-08-07-network-bluetooth-indicator-menus.md`](../active/2026-08-07-network-bluetooth-indicator-menus.md)
- **Authorization:** the author requested the confirmed media defect be fixed,
  authorized the Celestina-side corrections found by the read-only GPU loss
  audit, ended the safety hold, completed the controlled transitions and
  declared the phase closed
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

## Scope extension — 2026-08-07 controlled transitions

The author ended the Noctalia-only observation on 2026-08-07 and ran two
controlled Noctalia → Celestina → Noctalia transitions. `VAL-GPU-01` passed and
`VAL-R1-01`'s first-generation media case passed, which is what this plan was
opened for. The same run recorded seven further defects, and the author asked for
them to be consolidated here rather than split across plans, because every one
of them is the same class of fault this plan already owns: a provider or a
surface lifecycle treating one unlucky observation as the truth.

The scope therefore grows, before any of it is edited, to:

- publish the Bluetooth adapter's own state so a powered adapter with nothing
  connected stays visible, without inventing state when the query fails;
- keep the last confirmed network link across a transient probe failure, with a
  bounded expiry and an explicit confirmed-offline path, without raising the
  shared tool deadline;
- give every overlay exactly the initial properties it declares;
- let output hotplug request one coalesced DDC rediscovery from the single
  worker that already owns the only `ddcutil` child;
- dismiss every transient surface on a click outside its own bounds;
- find and correct the loss between a registered tray item and a rendered one;
- drive media from MPRIS signals rather than a `playerctl` poll.

All seven are corrected in `LVR-3-F`. Two of them are corrected by design
rather than from a reproduction, and the difference is recorded rather than
blurred:

- **Overlay dismissal.** What Niri does with a focused `LayerOverlay` surface
  when the panel behind it is clicked was never established, and no build can
  establish it. The correction removes the question instead of answering it:
  every transient surface now covers its own output, so a click outside the
  card is inside the surface and the surface answers it. The panel button that
  opened an overlay is behind the overlay while it is up, so it cannot receive
  a click, cannot re-enter `toggle()`, and focus returns exactly once.
- **The missing tray items.** Read-only inspection of this session's bus on
  2026-08-07 showed all four registered items answering `GetAll` correctly, and
  nothing in the host's parsing, icon resolution or drawer filtering that would
  drop Slack and Solaar while keeping `nm-applet` and Blueman. The exact reason
  those two were lost is therefore still not known. What *is* established by
  reading the host is that the loss was possible and silent: a `GetAll` that
  failed was dropped without a word, and an item with no properties was never
  published at all. That chain is closed, and the live rerun stays required.

Media keeps one primary path: the `playerctl` poll is gone, not disabled
alongside a second route.

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
4. Classify an oversized Niri frame as skippable rather than as the end of the
   session (`LVR-3-D`).
5. Gather the helper target's sources instead of listing them (`LVR-3-E`).
6. Correct every defect the 2026-08-07 controlled transitions recorded, as one
   0.6.8 delivery (`LVR-3-F`): the Bluetooth adapter's own state, the last
   confirmed network link, each overlay's declared properties, a coalesced DDC
   rediscovery on output hotplug, dismissal on a click outside any transient
   surface, a tray item that is never dropped for failing to describe itself,
   and media driven by MPRIS signals rather than a `playerctl` timer.

   One unit rather than seven because an inventory's boundaries are exclusive
   and these changes share `session.rs`, `ControlCentre.qml`, `CMakeLists.txt`,
   the overlay contract test and the version declarations. They are also one
   PATCH: a single delivery answering a single validation run.
7. Correct the two cases the author's rerun of `LVR-3-F` still failed
   (`LVR-3-G`): a network reading that repetition alone could still retire, and
   the tray items the author did not find. Both land in the same 0.6.8 batch —
   the previous unit is uncommitted, so this is one PATCH from `HEAD`, not two.

## Implementation exit

The registered architecture guard, project verification, the canonical
Celestina production exit and the live transition matrix remain required. They
were suspended for the duration of the safety hold below; the author ended that
observation on 2026-08-07 and `VAL-GPU-01` passed, so from `LVR-3-F` onward the
exit runs normally: `bash scripts/check-architecture-contract.sh`, the project's
registered `verify-production.sh`, `python3 scripts/version_tool.py check`,
`python3 scripts/check-staged-units.py` and the canonical
`celestina/scripts/complete-production.sh`. Completion updates the on-disk
bundle; it never activates Celestina or replaces Noctalia.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LVR-3-A | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-A.numstat.tsv) | 4 files, +312/-37 | Close the stale ledger state for the provider-insertion and DDC-lifecycle implementation delivered in `9002970` after its deferred canonical and live evidence completed | [live validation closure](../../evidence/2026-08-07-lvr3-validation-closure.md) | `VAL-R1-01`, `VAL-GPU-01` |
| LVR-3-B | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-B.numstat.tsv) | 52 files, +1758/-289 | Deliver 0.6.4 with the static-audit corrections and expose one revision-coupled provider lookup to both the compiled module and direct-directory QML tests; the initial singleton resolved as a type rather than a callable instance in the latter | 155 core tests, 43 shell Rust tests, Clippy, qmllint, 13 CTest targets and offscreen smoke pass; canonical verification is blocked only by unrelated Siderita architecture ratchets. The audit that specified these corrections is recorded in [static shell audit](../../evidence/2026-08-05-static-shell-audit.md) | `VAL-R1-01`, `VAL-GPU-01` |
| LVR-3-C | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-C.numstat.tsv) | 10 files, +161/-7 | Address the escalation timer to the helper instance it was armed against, so a replacement started inside the grace window is not killed mid-`ddcutil`; and let the handler that receives `exitStatus` own the restart delay, so the spacing an unclean exit earns is actually applied — with `FailedToStart` scheduled where it is, since Qt emits no `finished()` for a process that never ran | None: the GPU safety hold forbids building, testing or running this project. Reviewed by reading and recorded in [helper restart ownership evidence](../../evidence/2026-08-06-helper-restart-ownership.md) | `VAL-R1-01`, `VAL-GPU-01` |
| LVR-3-D | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-D.numstat.tsv) | 9 files, +129/-6 | Classify an oversized frame in the Niri adapter the way the provider helper already classifies it: report and skip it rather than end the session, so a frame the host would discard no longer tears down the compositor connection, empties the workspace strip and reconnects into the identical refusal | None: the GPU safety hold forbids building, testing or running this project. Reviewed by reading and recorded in [oversized frame evidence](../../evidence/2026-08-06-oversized-frame-is-skipped.md) | `VAL-R1-01` |
| LVR-3-E | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-E.numstat.tsv) | 8 files, +104/-15 | Gather the helper target's sources with a configure-time glob instead of a hand-written list that had drifted to ten of nineteen files and named none of the shell core, so the dependency set stops claiming something untrue | None: the GPU safety hold forbids configuring, building, testing or running this project. Reviewed by reading and recorded in [helper sources evidence](../../evidence/2026-08-06-helper-sources-gathered.md) | `VAL-R1-01` |
| LVR-3-F | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-F.numstat.tsv) | 36 files, +2691/-315 | Correct every defect the 2026-08-07 controlled transitions recorded, as one 0.6.8 delivery. Four readings stop treating one unlucky observation as the truth — the Bluetooth adapter's own state, the last confirmed network link, each overlay's declared properties, and a coalesced DDC rediscovery on output hotplug. Every transient surface then becomes the whole output so a click outside it is its own to answer, an item the tray registry lists is never dropped for failing to describe itself, and media is driven by MPRIS owner and property signals instead of a `playerctl` timer | 178 shell-core tests, 34 provider unit tests and six tests across three integration binaries, Clippy and `cargo fmt` clean, QML lint, CTest 14/14 with the new `celestina-overlay-contract` target, the architecture, language and documentation guards, and the canonical production exit; recorded in [the 2026-08-07 corrections](../../evidence/2026-08-07-one-poll-is-not-the-truth.md) | `VAL-R1-01`, `VAL-R1-02`, `VAL-R5` |
| LVR-3-G | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-G.numstat.tsv) | 14 files, +2245/-110 | Read the routing table first and on its own, so a confirmed absence of a default route retires a link without waiting on `nmcli`, and state the offline streak once — consecutive, with an unreadable poll keeping the link and resetting the run. Then walk the tray's whole D-Bus path against a private bus, which reproduced the live loss on its first run and found it: a registry read rebuilt the registration list wholesale from a snapshot older than registrations it had already learned | 181 shell-core tests, 46 helper unit tests, 6 integration tests, Clippy, `cargo fmt`, QML lint, CTest 15/15 with the new `celestina-tray-watcher` target, every guard and the canonical production exit; recorded in [what a probe did not see](../../evidence/2026-08-07-what-a-probe-did-not-see.md) | `VAL-R1-NET`, `VAL-R1-TRAY` |
| LVR-3-H | `celestina:` | done | [inventory](../../inventories/2026-08-05-late-provider-insertion/LVR-3-H.numstat.tsv) | 14 files, +452/-264 | Archive the completed plan, reconcile the already-delivered static hardening and activate the bounded UX-1 successor | [administrative closure](../../evidence/2026-08-07-lvr3-archive.md) | `VAL-R1-01`, `VAL-R1-NET`, `VAL-R1-DDC`, `VAL-R5-BT`, `VAL-R1-TRAY` |

## Recorded trigger

Celestina 0.6.2 started while Firefox was already playing. The original helper
remained healthy but the panel showed no media. A second isolated helper
published the exact Firefox media payload immediately. Sending SIGTERM to the
original helper produced a clean replacement generation, after which the media
region appeared without changing playback.

## Safety hold — ended 2026-08-07

`VAL-GPU-01` passed on 2026-08-07: a long Noctalia-only observation and two
controlled handovers, one without DDC and one with DDC, hotplug, brightness and
media, all completed without `device lost from bus!`. That is strong negative
reproduction evidence, not proof that a lower-probability driver or transition
fault cannot recur, so the DDC invariants this plan defends are unchanged:
one owning worker, global exclusion and serialization, coalesced operations, a
bounded timeout, deterministic cancellation, a killed and reaped child, no
orphans and no frequent DDC polling. `LVR-3-F`'s rediscovery request is written against every one
of them.

The hold's original terms are kept below unchanged, because the units delivered
under it carry no automated evidence and the reason must stay legible.

The author was running Noctalia alone to determine whether removing the complete
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

The final live validation and administrative closure are recorded in
`LVR-3-A` and `LVR-3-H`; this archived plan is no longer current instruction.
