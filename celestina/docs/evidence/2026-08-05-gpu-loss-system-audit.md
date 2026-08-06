# Evidence: 2026-08-05 GPU PCIe loss audit

- **Date:** 2026-08-05
- **Status:** read-only audit complete; causation unresolved
- **Scope:** the two retained boots that ended with `device lost from bus!`,
  their preceding DDC activity, and the competing explanations for the PCIe
  endpoint loss
- **Environment:** AMD Radeon RX 9070 XT (Navi 48) on kernel
  `linux-cachyos 7.1.5-1-cachyos`, with AMD firmware `20260622-1`, Mesa
  `26.1.6`, libdrm `2.4.134` and ddcutil `2.2.7`; affected boot IDs
  `0b9e7a92...` and `824f56efef794c53be4ff62c3838177e`
- **Mutation boundary:** no code, configuration, service or live-session state
  was changed while collecting this evidence
- **Artifact:** none — this is a read-only audit. Its outputs are this record
  and the isolation hold and deferred phase matrix stated below

## Procedure

The retained journals for the affected boots were read and their `ddcutil`
diagnostics grouped by systemd scope and argument shape, then placed on the
same timeline as the amdgpu fence timeouts, SMU reads, output connect/disconnect
events and the abrupt boot endings. The author supplied the session actions that
the journal cannot show. The installed graphics stack versions and the upstream
Linux 7.1.6 changelog were then consulted for candidate explanations, together
with the primary references listed at the end.

## Result

### Conclusion boundary

Both retained boots with a confirmed `device lost from bus!` were preceded by
DDC processes whose argument shape matches Celestina. One loss began while a
DDC detection was still active. This is a strong correlation across the two
available positive cases, not proof that Celestina or DDC caused the PCIe
endpoint to disappear.

Celestina has independently confirmed startup and shutdown defects that can
start automatic DDC work in a host that will be rejected, fail to join the DDC
worker, and terminate the helper without destructing an active child. Those are
valid product defects even if the ultimate fault is in amdgpu, firmware, Mesa,
the PCIe link or hardware.

### Confirmed crash 1

Boot `0b9e7a92...` ended abruptly at 2026-08-05 14:16:10 EDT.

| Time | Retained journal evidence |
|---|---|
| 14:12:22 | DP-1 disconnected while PIDs 200767 and 200768 ran `ddcutil detect --brief` and PID 200953 read VCP 10; DDC errors and lock contention followed. |
| 14:12:24 | DP-1 reconnected. |
| 14:14:20 | PIDs 201685 and 201688 ran concurrent detections on bus 8. |
| 14:14:22 | PID 201886 read display 2 and met lock contention. |
| 14:14:23-25 | DP-1 disconnected and reconnected. |
| 14:14:50 | PIDs 205176, 205223, 205231 and 205343 overlapped reads and detections with DDC errors. |
| 14:15:49 | PID 206062 read display 1; PID 206127 began a detection that continued emitting errors until about 14:16:01. |
| 14:15:50.638 | First fence timeout on `vcn_unified_0`. |
| 14:15:51.311 | `gfx` and then `sdma0` timed out. |
| 14:15:54.413 | First `device lost from bus!`; SMU responses became `0xFFFFFFFF`. |
| 14:16:01 | Further `gfx` failure while PID 206127 was still reporting DDC errors. |

The boot contains 83 distinct `ddcutil` PIDs that emitted diagnostics: four in
Noctalia's scope with its `--noconfig` and `--bus` argument shape, three in a
Claude scope with Celestina-shaped arguments, 26 in one Kitty scope and 50 in
another Kitty scope. Successful silent executions are not recoverable from the
journal and historical entries contain no `_PPID` field.

### Confirmed crash 2

Boot `824f56efef794c53be4ff62c3838177e` ran from 2026-08-05 14:18:20 to
14:37:57 EDT.

| Time | Retained journal evidence |
|---|---|
| 14:18:36 | Noctalia started. |
| 14:19:39 | PIDs 8455 and 8560 ran concurrent Celestina-shaped detections. |
| 14:22:26 | PID 10871 detected displays; PID 11174 read display 1. |
| 14:25:44-45 | PIDs 15775, 15871 and 15976 ran concurrent detections. |
| 14:26:28 | PID 16720 detected; PIDs 17026 and 17269 read displays 1 and 3. |
| 14:30:04-05 | PIDs 22076, 22147, 22167 and 22303 overlapped reads and detections; these are the last retained DDC errors. |
| 14:30:29-31 | DP-1 disconnected and reconnected. |
| 14:32:41-43 | DP-1 disconnected and reconnected again. |
| 14:36:18 | Noctalia restarted. |
| 14:36:19-20 | DP-1 disconnected and reconnected. |
| 14:37:40.517 | First `gfx` fence timeout, followed by `sdma1`. |
| 14:37:45.275 | `device lost from bus!`; SMU returned `0xFFFFFFFF`. VCN power-gating failures followed the loss. |
| 14:37:46-47 | Further `gfx` and `sdma0` failures, an out-of-bounds `write_frame` report and VCN SRAM load failure. |
| 14:37:57 | `flip_done timed out`; the boot ended abruptly. |

All 14 DDC PIDs that emitted diagnostics belonged to `kitty-2003-0.scope` and
used Celestina-shaped arguments. The last visible DDC error preceded the first
GPU timeout by about 7 minutes 35 seconds. Noctalia's restart preceded it by
about 82 seconds.

The author confirmed that Noctalia was stopped, Celestina was used without a
manual brightness action, Celestina was stopped, Noctalia was restored and the
machine failed while idle minutes later. Terminal output was not stored in the
journal, so exact host/helper parentage cannot be reconstructed from that boot.

### Other retained boots

- `8754f430...` ended abruptly without a GPU-loss signature. Its last retained
  event was a DDC error, but it is not a confirmed instance of this crash.
- `b365f189...` ended abruptly without a GPU or DDC signature and is not a
  confirmed instance.
- A normal S3 resume at 13:02:13 in the first affected boot explains its earlier
  MODE1 reset and ring reinitialization; it is not a third crash.

The two confirmed GPU-loss boots are therefore both positive for preceding
Celestina-shaped DDC activity. The sample contains no retained clean pre-DDC
control and is too small to prove causation.

### Competing explanations

#### Confirmed

- The endpoint stopped responding over PCIe and amdgpu reported SMU reads of
  `0xFFFFFFFF`.
- Repeated and sometimes concurrent DDC work, lock contention, DDC/I2C errors
  and output churn preceded both confirmed losses.
- Existing run-time power mitigations did not prevent the last crash.
- The first crash began with VCN, then `gfx`/`sdma`; the second began with
  `gfx`/`sdma` and reported VCN failures only after device loss.
- Celestina media reads and controls MPRIS through `playerctl`; it does not
  launch a decoder or directly use VA-API, Vulkan, OpenGL or VCN.
- Noctalia's mpvpaper assignment file was empty during the retained boots, so
  its supervisor had no configured video child to launch.

#### Strong correlations

- Active DDC work and display churn immediately preceded the first loss.
- A complete shell transition preceded the second loss.
- Every retained affected boot used kernel 7.1.5 with GFX12 MES active.

#### Unproven

- That a DDC transaction caused the PCIe loss rather than exposing a kernel,
  firmware, display-core, PCIe or hardware defect.
- That a historical `ddcutil` process was orphaned or that both shells held the
  same DDC bus at the decisive instant.
- That media playback or VCN initiated either loss.
- That Celestina is necessary for the crash.

### Installed-software context

The affected system had AMD firmware `20260622-1`, Mesa `26.1.6`, libdrm
`2.4.134` and ddcutil `2.2.7`. Kernel 7.1.5 had been installed after 7.1.6 on
2026-08-04, and every retained affected boot used 7.1.5.

The upstream Linux 7.1.6 changelog contains relevant but not dispositive GFX12
fixes: MES-enabled kernel and user queue EOP interrupts could be routed
incorrectly in the fence path, and masked DCN vblank/pageflip events could
produce `flip_done` timeouts. No exact upstream report matching Navi 48 plus
DDC plus PCIe device loss was found in the primary sources inspected. That
absence is not evidence that no such regression exists.

## Isolation hold

The live session now remains exclusively on Noctalia with the kernel, monitors
and existing mitigations held constant. Celestina and its providers must not be
started during this observation. Noctalia still has ddcutil enabled, so this
phase isolates Celestina and the handover sequence, not DDC as a whole.

If the loss recurs, Celestina is disproved as a necessary condition. If a long
observation exceeds the prior failure window without a loss, that is strong
evidence against the handover sequence but still not proof of the exact layer
that failed.

## Deferred isolation matrix

No later phase starts until the author explicitly ends the current phase. A
GPU loss aborts the phase; the affected boot journal is preserved before any
other variable changes.

| Phase | Fixed configuration | Isolated question |
|---|---|---|
| A | Noctalia only; current kernel, monitors, mitigations and Noctalia DDC unchanged | Is Celestina or the handover necessary? |
| B | Celestina with DDC disabled and no playback | Is the base shell sufficient? |
| C | Celestina with DDC disabled and MPRIS playback active | Does media usage expose the failure without Celestina DDC? |
| D | Corrected Celestina with one DDC owner and no playback | Does DDC alone expose it after lifecycle repair? |
| E | Noctalia to Celestina to Noctalia, with Celestina DDC disabled | Does the transition alone expose it? |
| F | A separately authorized kernel comparison | Does a kernel containing the relevant GFX12 fixes change the result? |

Because the failure is severe and random, these are observation phases rather
than a transition stress loop. Each phase must exceed the earlier failure
window by a margin the author accepts. Absence of concurrent or surviving DDC
PIDs, output churn and fence timeouts is required before advancing.

## Limits

- Causation is unresolved. It is not established that a DDC transaction caused
  the PCIe loss rather than exposing a kernel, firmware, display-core, PCIe or
  hardware defect, that a historical `ddcutil` process was orphaned, that both
  shells held the same DDC bus at the decisive instant, that media playback or
  VCN initiated either loss, or that Celestina is necessary for the crash.
- The sample is two positive boots with no retained clean pre-DDC control, and
  is too small to prove causation.
- Historical journal entries contain no `_PPID` field and successful silent
  `ddcutil` executions are not recoverable, so host/helper parentage cannot be
  reconstructed. Terminal output from the second boot was not stored.
- No exact upstream report matching Navi 48 plus DDC plus PCIe device loss was
  found in the primary sources inspected; that absence is not evidence that no
  such regression exists.

## Primary external references

- Linux 7.1.6 changelog:
  <https://cdn.kernel.org/pub/linux/kernel/v7.x/ChangeLog-7.1.6>
- Linux release archive: <https://cdn.kernel.org/pub/linux/kernel/v7.x/>
- ddcutil 2.2.7 release:
  <https://github.com/rockowitz/ddcutil/releases/tag/v2.2.7>
- Rust process exit contract:
  <https://doc.rust-lang.org/std/process/fn.exit.html>
- Rust child ownership contract:
  <https://doc.rust-lang.org/std/process/struct.Child.html>
- Qt QProcess lifecycle contract: <https://doc.qt.io/qt-6/qprocess.html>
