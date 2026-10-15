# Shell performance audit — idle cost of the running prototype

- **Date:** 2026-08-12
- **Scope:** Celestina host, niri adapter and provider adapter, as running in
  the nested development session at commit `f47727f` plus the uncommitted
  droplet-blur work
- **Artifact:** Celestina 0.14.1 build tree (`celestina/build`), not the
  deployed bundle
- **Environment:** nested Niri (`dev-session.sh`) on the author's live desktop,
  one `winit` output, shell idle and visible, no interaction during any
  measurement window
- **Validation:** feeds `VAL-SHELL-02`, which remains deferred until numeric
  ceilings are accepted

## Procedure

Two passes. Runtime measurements were taken read-only from `/proc` on the
live nested processes — CPU jiffies, context switches, `smaps_rollup`,
`io::write_bytes`, thread inventories — over 30 and 60 second idle windows.
No input was injected and nothing was restarted. The static pass followed the
hot paths those numbers pointed at: the diagnostics journal, the provider poll
loops, the QML capture and animation routes.

## Result

### Idle baseline

| process | PSS | threads | CPU (60 s idle) | wakeups/s | disk writes |
|---|---|---|---|---|---|
| celestina host | 278.9 MiB | 28 | 0.13 % | 17.7 | 0 B/s |
| provider adapter | 138.0 MiB | 19 | 0.12 % | 10.1 | **290 KB/s** |
| niri adapter | 1.3 MiB | 4 | 0.00 % | 0.0 | 0 B/s |

The rendering side is genuinely quiescent: zero minor faults and zero disk
writes across the idle window, two `QSGRenderThread`s (panel and wallpaper
windows), and the 17 wakeups/s are the clock's second hand plus provider
frames arriving. CPU at idle is excellent on both sides.

### Findings, most severe first

**1. The diagnostics journal writes ~25 GB/day to the SSD at idle.**
`io::write_bytes` on the provider adapter is 290 KB/s while its journal file
grows only ~2.3 KB/s — roughly 126× write amplification. The cause is
composition: the audio provider polls by spawning `wpctl` every 2 seconds,
every subprocess emits three `Critical` events (`process.spawn`,
`process.started`, `process.exit`), and `Critical` flushes immediately, so
each tiny append dirties and writes back a page. 500 recent journal lines are
164 spawn triples against 3 DDC operations — the journal is almost entirely
routine poll bookkeeping recorded at the level reserved for freeze forensics.
DIAG-1's intent was "an operation started and this file ends" for the crash
window; a poll that succeeds 43,000 times a day is not that.

**Corrected.** The level now follows what the child can touch rather than the
fact that it is a child. `ddcutil` — the reason the level exists, because it
reaches the I²C buses of a card that has been lost from the bus — keeps
`Critical` for its whole lifecycle. So does every anomaly for any program:
a failed spawn, a timeout, a cancellation, a broken wait, a kill-and-reap
(the `AUD-1-C` overlap), and an exit that failed. Only the ordinary
spawn/started/exit of a program that cannot reach the card drops to `Info`,
which still writes the line and merely lets it travel in the buffer the sink
already has.

Measured after the change on a fresh nest, same idle conditions: **0 B/s** of
`write_bytes` over 45 seconds, against 289,724 B/s before, with 594 `info` and
23 `critical` lines recorded in that window. Nothing stopped being recorded.

**2. Poll-by-subprocess is the helper's steady state: ~1.4 children/s, forever.**
Counted from one 35-minute idle run of the nest: 3,007 spawns, from five
pollers — `wpctl` 3,512 invocations, `nmcli` 2,052, `bluetoothctl` 2,052,
`powerprofilesctl` 684, `ip` 684. Every one is a fork, an exec, a pipe and a
reap, and before finding 1 each also cost three synchronous disk flushes.
Measured CPU is harmless (0.12 %); what this pays is the write path and the
wakeup floor, and it is the single largest source of both. Native
subscriptions — PipeWire for audio, D-Bus signals for NetworkManager and
BlueZ — would remove almost all of it, and each is a real project rather than
a tweak. The cheap half is backing off pollers whose surface is closed.

**3. The provider's 143 MiB RSS is one-time, not a leak, and not yet attributed.**
`VmPeak` 710 MiB, `VmHWM` 268 MiB, settled RSS 143 MiB. RSS moved 4 KB across
100 seconds and 250 subprocess spawns, so nothing accumulates. The virtual
peak is mostly reservation: five 64 MiB glibc arenas with **zero** resident
pages, plus 19 thread stacks. The resident cost is three mappings — 62 MiB and
29 MiB anonymous, both fully touched, and a 41 MiB heap.

This audit first attributed that to wallpaper decoding, and the journal
disproves it: this run recorded no wallpaper event at all, so that path never
executed. The real allocation site is unidentified. It is stable, bounded and
idle-quiet, so it is a sizing question rather than a defect, and it is left
open rather than guessed at — fixing an allocation whose owner is unknown is
how a real bound gets removed by accident.

**4. The host has no memory ceiling and 279 MiB PSS deserves one.** Heap is
51 MiB, the rest is GL driver, scenegraph textures and the wallpaper at output
size — nothing looks leaked, and idle minor faults are zero, but `VAL-SHELL-02`
was deferred exactly because no numeric ceilings exist. These measurements are
a proposed baseline: host ≤ 300 MiB PSS, provider ≤ 80 MiB settled, idle CPU
≤ 0.5 %, idle disk 0 B/s except a bounded journal.

**5. The nested session runs real DDC.** The nest's own journal shows one
`ddc.detected` with `ddc.start`/`ddc.end` pairs at startup: `dev-restart.sh`
sets no `CELESTINA_DDC`, so every nest launch probes the live buses the
desktop is using — the same class of contact `PANEL-1-M` removed from the
smoke. The author does use the nest to exercise brightness, so off-by-default
is their call, not this audit's; recorded so the next bus-contention incident
does not have to rediscover it.

**6. Small, acceptable, listed for completeness.** The falling-drop region is
republished per animation frame, bounded to ~25 updates per opening and
deduplicated in C++ before reaching the compositor. Toast and OSD surfaces
use a live in-scene capture while visible; they are transient and small. A
media progress tick bumps the global provider revision every second while
music plays, re-evaluating every `ProviderReading` binding on every open
surface; after the identity-keyed row lists this is label updates only.

## Limits

Everything above is the idle profile of one nested output. Frame rate, GPU
cost of the blur stack, and interaction paths (menu opening under load, the
fall animation's frame budget) were not measured: the nest exposes no frame
counters and this audit injects no input by rule. The 279 MiB host figure
includes the nest's window sizes, not the author's three real outputs, where
wallpaper textures are larger. Ceilings remain proposals until the author
accepts numbers under `VAL-SHELL-02`.
