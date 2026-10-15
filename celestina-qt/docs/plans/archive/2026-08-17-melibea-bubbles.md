# BUBBLE-1 — native minimized windows become shell bubbles

- **Opened:** 2026-08-17
- **Closed:** 2026-08-18
- **Plan ID:** melibea-bubbles
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** BUBBLE-1
- **Author-validation checkpoint:** VAL-BUBBLE-1
- **Predecessor:** [LIVE-1 live session repairs](../archive/2026-08-17-live-session-repairs.md)
- **Successor:** none

## Hypothesis

A minimized window is useful only when leaving the layout makes it easier to
recover than a scratchpad or hidden workspace. A compact overlapping icon
group in the shell, backed by compositor-authoritative minimized state, should
make that lifecycle visible without reintroducing a taskbar.

## Tangible outcome

Celestina shows Melibea's ordered native minimized windows as one compact
bubble group in the panel. Opening that group reveals a keyboard-, pointer-
and assistive-technology-accessible selector. Every row identifies the
application and window, restores it through Melibea, and offers an explicit
close action. The row disappears only after the subscribed authoritative state
does.

## Scope

One implementation unit owns the complete vertical slice.

- **BUBBLE-1-A — subscribed state, actions and presentation.** The aggregate
  provider helper owns the bounded Unix-socket client and reconnect loop. Pure
  protocol decoding and ordered-state reduction live in
  `celestina-shell-core`. The panel owns only the compact overlapping visual,
  and the existing overlay lifecycle owns the selector surface.
- The same unit releases the active checkpoint slot left by the already
  delivered `LIVE-1` plan and records this checkpoint in the governing
  documents.

## Exclusions

- No window previews. Wayland and Melibea v1 expose identity and title, not
  another client's pixels.
- No hidden workspace, off-screen storage, shell-owned lifetime or optimistic
  removal.
- No new compositor minimization semantics or Melibea protocol version in this
  unit. Deployment reapplies Celestina's existing per-layer blur-strength
  patch to the already accepted native-minimization checkout so the combined
  session binary does not regress dense glass.
- No coordinated window-to-bubble trajectory. The exact shell anchor needed
  by that animation belongs to Melibea M7.
- No taskbar, running-application inventory or inference from ordinary Niri
  windows. Only native minimized state becomes a bubble.

## Build order

1. Decode Melibea v1 and reduce snapshots plus ordered incremental revisions
   under bounded tests.
2. Add one cancellable reconnecting worker to the existing aggregate provider
   helper and route restore/close through the existing request ledger.
3. Render the compact group and selector, including keyboard focus, close,
   empty/unavailable state and reduced motion.
4. Exercise the complete slice against a disposable nested Niri/Melibea
   session, then build, verify and deploy the exact production bundle.
5. Only after every byte is ready, activate the updated Niri and Celestina
   session once and verify the real contract.

## Implementation exit

- Pure tests prove snapshot replacement, sequential changes, revision gaps,
  incompatible messages, bounded hostile input and unavailable-to-snapshot
  recovery.
- Provider tests prove reconnect, a snapshot-first subscription and that an
  accepted restore or close settles only after the window leaves the
  authoritative list.
- QML tests prove the overlapping compact group, selector navigation,
  restore/close pointer and keyboard routes, plain producer text, empty state,
  accessible names and reduced motion.
- The common architecture guard, Rust format/Clippy/tests, QML lint, affected
  CTest coverage and `scripts/complete-production.sh` pass against the exact
  deployed bundle.
- Nested Niri proves minimize, bubble appearance, selector restore and close
  without touching the live session. The combined production Niri accepts the
  real configuration and retains the per-layer blur contract. Real-session
  perceptual and assistive-technology acceptance remains separate in
  `VAL-BUBBLE-1`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Intended change | Diffstat | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| BUBBLE-1-A | `celestina:` | done | [exact inventory](../../inventories/2026-08-17-melibea-bubbles/BUBBLE-1-A.numstat.tsv) | Integrate native minimized-window bubbles into the shell without giving Celestina authority over window state | 51 files, +4397/-93 | [delivery evidence](../../evidence/2026-08-18-melibea-bubbles.md) | `VAL-BUBBLE-1` |
