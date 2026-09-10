# MAG-P5 — Commands, trackpad and keyboard

- **Opened:** 2026-09-09
- **Plan ID:** remote-control
- **Status:** active
- **Authorization:** the author said "sigue con todo" on 2026-09-09 with
  `MAG-P4`'s exit met
- **Scope:** magnetita, magnetitad, magnetita-mobile, magnetita-peer
- **Implementation checkpoint:** MAG-P5
- **Author-validation checkpoint:** `VAL-MAG-13` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

Remote control adds no peer-chosen string to any shell or compositor: the
phone runs desktop-registered command ids, and its motion, buttons, keys
and text become events of a virtual device the daemon owns.

## Tangible outcome

Registered commands in the desktop app's settings, published to the phone
by id and name and run under the daemon's subprocess discipline; a
`uinput` pointer and keyboard fed by the phone's control screen.

## Scope

- `MAG-P5-A` — the command registry, its `Devices1` methods, the desktop
  app's settings section, publication and bounded runs.
- `MAG-P5-B` — the virtual pointer and keyboard through `uinput`, with a
  rate governor and decode at the boundary; motion by datagram.
- `MAG-P5-C` — the phone's control screen, under `magnetita-android`'s
  `AND-3`; this unit records the pairing of the two.

## Exclusions

- Presenter mode, absolute-position tablet, gamepad.
- Any test against the author's live session: the virtual device is only
  opened and closed in the daemon's own ignored test.

## Build order

1. `MAG-P5-A` and `-B`, then `-C`.

## Implementation exit

The peer's synthetic input reaches the daemon's sink in the loopback test
and the virtual device opens on this host; commands run only by registered
id; the `uinput` grant is recorded in `HOST-HYGIENE.md`.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| MAG-P5-A | `magnetita:` | done | [inventory](../../inventories/2026-09-09-remote-control/MAG-P5-A.numstat.tsv) | 15 files, +727/-5 | Registered commands published and run by id; the settings section | [record](../../evidence/2026-09-09-commands-own-wire.md) | `VAL-MAG-13` |
| MAG-P5-B | `magnetita:` | done | [inventory](../../inventories/2026-09-09-remote-control/MAG-P5-B.numstat.tsv) | 25 files, +1149/-89 | The virtual pointer and keyboard; `MAG-P4` archived and `MAG-P5` opened | [record](../../evidence/2026-09-09-input-own-wire.md) | `VAL-MAG-13` |
| MAG-P5-C | `magnetita:` | planned | `docs/` | — | The phone's control screen paired with `AND-3-A` | record | `VAL-MAG-13` |
