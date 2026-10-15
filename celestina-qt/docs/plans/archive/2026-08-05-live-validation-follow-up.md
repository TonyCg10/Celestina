# LVR-2 — live validation follow-up

- **Opened:** 2026-08-05
- **Plan ID:** live-validation-follow-up
- **Status:** done
- **Closed:** 2026-08-05
- **Successor:** none; the author reruns the linked validation cases against 0.6.2
- **Authorization:** the author authorized bounded corrective implementation on
  2026-08-05, excluding new feature work
- **Scope:** celestina
- **Implementation checkpoint:** LVR-2
- **Author-validation checkpoint:** `VAL-R1-01`, `VAL-R3`, `VAL-R4`, and
  `VAL-R7` in [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

The follow-up failures are independent lifecycle and interaction defects, not
one provider-frame regression: media misses only the helper's initial
generation, notification dismissal depends on which child owns focus, held
children outlive the helper that claims to own them, and portal registration
instructions omit backend selection in a session with an explicit preference
file.

## Tangible outcome

The first provider generation publishes an already-playing MPRIS source,
Escape dismisses the notification centre from every focus position, every
`wlsunset` or `systemd-inhibit` child is terminated with its helper, and the
appearance-portal procedure produces Celestina's public values with an exact
rollback.

## Scope

- Media startup publication in the aggregate provider helper.
- Notification-centre focus containment and root Escape handling.
- Held-child ownership and deterministic provider-helper shutdown.
- Appearance-portal registration and backend-selection documentation.
- Focused automated regressions and the canonical Celestina production exit.

## Exclusions

- Screen lock and Polkit, which remain gated by SHELL-D1/SHELL-D2 and SHELL-D3.
- Changing the author's Niri colour configuration; that validation was omitted
  by explicit choice.
- Weather configuration, paired-phone notification delivery, resource ceilings
  and assistive-technology validation.
- Launcher result-cap redesign without a separate product decision.

## Build order

1. Characterize media publication on the first helper generation with a fake
   already-playing MPRIS source, then correct only the lost bootstrap edge.
2. Move notification-centre dismissal to the surface-level focus boundary and
   prove Escape from list, button, empty and reopened states.
3. Characterize repeated helper start/stop with active holds, then make child
   termination deterministic without weakening confirmed-state semantics.
4. Correct the portal procedure and prove install, public read, rollback and
   restoration while leaving Siderita's FileChooser backend untouched.

## Implementation exit

```sh
bash scripts/check-architecture-contract.sh
celestina/scripts/complete-production.sh
python3 scripts/version_tool.py check
python3 scripts/check-staged-units.py
```

The media, dismissal and inhibitor corrections form one product bug delivery
and therefore require the next Celestina PATCH transition. Documentation-only
portal clarification may remain maintenance only if it lands independently.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| LVR-2-A | `celestina:` | done | [`../../inventories/2026-08-05-live-validation-follow-up/LVR-2-A.numstat.tsv`](../../inventories/2026-08-05-live-validation-follow-up/LVR-2-A.numstat.tsv) | 19 files, +707/-110 | Consolidate the recorded media bootstrap, notification Escape, held-child lifecycle and appearance-portal documentation defects without adding capability | [follow-up and completion evidence](../../evidence/2026-08-05-live-validation-follow-up.md) | `VAL-R1-01`, `VAL-R3`, `VAL-R4`, `VAL-R7` |

## Recorded trigger

The complete observation matrix, commands, external configuration changes,
rollbacks and limits are in
[the 2026-08-05 follow-up evidence](../../evidence/2026-08-05-live-validation-follow-up.md).
