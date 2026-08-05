# LVR-1 — live validation remediation

- **Opened:** 2026-08-04
- **Plan ID:** live-validation-remediation
- **Status:** done
- **Scope:** celestina
- **Implementation checkpoint:** LVR-1
- **Author-validation checkpoint:** `VAL-R1-01`, `VAL-R2-02`, `VAL-R4`,
  `VAL-SHELL-03` and `VAL-COPY-01` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)
- **Closed:** 2026-08-05
- **Successor:** none; the author reruns the linked validation cases

## Hypothesis

The live failures are bounded contract mismatches at existing seams: MPRIS
process timing and panel presentation, clipboard empty-state focus ownership,
notification row shape versus the host decoder, runtime application identity,
and untranslated product copy. They can be corrected without weakening input
bounds, creating a second provider channel or changing session ownership.

## Tangible outcome

The same live checks show media for a valid browser MPRIS player, keep the
clipboard dismissible after clearing it, accept a bounded notification through
the complete helper-to-host path without withdrawing unrelated readings, start
without the recorded accessibility or portal identity diagnostics, and present
every exposed shell surface in Spanish.

## Scope

In scope: the five failed validation paths recorded in
[the live evidence](../../evidence/2026-08-04-live-validation-failures.md);
targeted characterization and boundary tests; the registered production exit;
and rerunning only the author-validation cases affected by a completed unit.

## Exclusions

Out of scope: implementing a screen locker or Polkit agent; removing Noctalia;
changing Niri configuration; turning the control centre into a network or
Bluetooth manager; configuring a weather location for the author; enlarging the
launcher result cap without separate product evidence; or completing validation
paths that were not run before the stop.

## Build order

1. Reproduce the Firefox MPRIS absence with per-call timing and raw provider
   frames, then preserve the bounded-IO contract while making valid players
   visible and absent readings safe in QML.
2. Keep clipboard focus and dismissal available in the empty state and expose
   a visible, accessible per-entry delete action without removing the existing
   keyboard and context-menu paths.
3. Characterize the notification JSON at the Rust/C++ boundary, choose one
   bounded compatible representation for action rows, and prevent one invalid
   provider payload from silently erasing unrelated confirmed state.
4. Attach wallpaper accessibility semantics to a valid item and install or
   register the shell's application identity through the production artifact
   contract.
5. Translate each affected surface as one complete Spanish product-copy pass,
   updating assertions and accessibility text in the same unit.

## Implementation exit

- Each unit adds a regression test at the boundary that failed live; the R4
  test must traverse the C++ host decoder, not stop at helper JSON.
- Invalid external data remains bounded and cannot crash, block or silently
  stale another provider.
- QML lint, CTest, Rust format, Clippy and package tests pass for every changed
  boundary.
- The architecture, documentation, language, accessibility and version guards
  pass as applicable.
- A product unit bumps the registered SemVer source and appends its history row
  before `scripts/complete-production.sh` builds, verifies and deploys the exact
  production bytes without activating the live shell.
- The author reruns the linked validation case only after its corrective unit
  is complete and the Noctalia rollback remains available.

## Change and commit ledger

The five recorded failures shared several QML files, and an inventory may not
overlap another except on this plan. They are therefore one unit rather than
five: the alternative was assigning shared files arbitrarily to whichever unit
touched them first, which would have made every diffstat a fiction.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LVR-1-A | `celestina:` | done | [inventory](../../inventories/2026-08-04-live-validation-remediation/LVR-1-A.numstat.tsv) | 45 files, +1806/-267 | Publish notification actions as a bounded flat sibling list the host decodes; stop an unreadable frame from clearing unrelated providers; keep the media widget from being clipped off the bar; guard the audio widget's accessible text; keep the emptied clipboard dismissible and give it a delete button that actually receives its click; attach wallpaper accessibility to an Item and seal the desktop entry as a registered artifact; and finish the Spanish product copy | [live validation remediation](../../evidence/2026-08-05-live-validation-remediation.md) | `VAL-R1-01`, `VAL-R2-02`, `VAL-R4`, `VAL-SHELL-03`, `VAL-COPY-01` |

## The clipboard button needed a second correction

Making the button visible was not enough: the row's own `MouseArea` filled the
row and answered the presses aimed at it, so the button did nothing. Neither
declaration order nor `z` moved the press — a filling area wins it either way —
so the row became its own component with the two input areas disjoint, and a
test that clicks it for real replaced one that called `remove(index)` and passed
through the bug.

## Delivery order

This unit needs one change outside its own prefix: `docs/projects.toml` registers
`celestina.desktop` as a production input and a sealed artifact. The registry is
suite scope, so it is a separate `suite:` delivery and it must land **first** —
a Celestina commit arriving alone would have `deploy-production.sh` copying
bytes the manifest does not seal, which is exactly what the artifact contract
forbids. The inventory below therefore claims only Celestina's own paths and the
version-history row.

## Decisions and rollback

The first notification exposed two contracts, not permission to make either
looser. Notification actions must remain bounded, while malformed input must
not impersonate helper death and erase otherwise valid provider state. The
implementation unit must record which boundary owns each rule.

The recorded media hypothesis — a `playerctl` timeout — was measured and found
false: the calls answer in 3-5 ms and the helper publishes a valid player. The
widget was being clipped out of the panel by a workspace strip allowed to claim
the whole flank. No timeout was changed, because no measurement justified
changing one.

No live surface is activated by the production exit. During author validation,
the rollback remains stopping Celestina and restoring Noctalia as the watcher
and notification server; exact ownership is verified with `busctl` rather than
inferred from which bar is visible.
