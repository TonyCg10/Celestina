# Cuprita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** CUP-1
- **Related author validation:** `VAL-C`, `VAL-D` and `VAL-E` in
  [VALIDATION.md](VALIDATION.md) (do not block)

## Hypothesis and tangible outcome

One window can replace nm-applet, Blueman and pavucontrol when each backend
sits behind a trait with a fake, so the whole surface is testable in the
session and the author's live check is only the last mile.

## Scope

| Unit | Outcome |
|---|---|
| AUD-1-P (`suite`) | Registration, with the skeleton and the 0.1.0 baseline |
| CUP-1-A | Skeleton: crate stub, window with the three-section strip and empty pages, scripts, documents |
| CUP-1-B | `cuprita-core`: models, the three traits, fakes, pure logic; controllers and models over the fakes |
| CUP-1-C | Red: the NetworkManager client and its page |
| CUP-1-D | Bluetooth: the BlueZ client, the pairing agent and its page |
| CUP-1-E | Audio: the binding spike, the PipeWire client and its page |
| CUP-1-F | Exit: glass, keyboard and screen-reader pass; 1.0.0 |

## Exclusions

- Editing connection profiles, hotspots, mobile broadband, Bluetooth file
  transfer, audio graph routing, a tray or indicator, notifications, and any
  shell integration.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| CUP-1-A | active | none | crate stub, application skeleton, strip, scripts, documents | `scripts/smoke.sh`, guards |
| CUP-1-B | active | CUP-1-A | core models, traits, fakes; controllers over the fakes | `cargo test` |
| CUP-1-C | planned | CUP-1-B | NetworkManager client and the Red page | `scripts/verify-production.sh` |
| CUP-1-D | planned | CUP-1-B | BlueZ client, pairing agent and the Bluetooth page | `scripts/verify-production.sh` |
| CUP-1-E | planned | CUP-1-B | PipeWire client and the Audio page | `scripts/verify-production.sh` |
| CUP-1-F | planned | CUP-1-C, CUP-1-D, CUP-1-E | implementation exit and 1.0.0 | `scripts/complete-production.sh` |

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary manages
network, Bluetooth and audio on the author's machine.

## Closed evidence

None yet.
