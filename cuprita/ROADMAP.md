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
| CUP-1-F | Exit: glass, keyboard and screen-reader pass; its landing publishes 1.0.0 |

## Exclusions

- Editing connection profiles, hotspots, mobile broadband, Bluetooth file
  transfer, audio graph routing, a tray or indicator, notifications, and any
  shell integration.

## Build order

Only CUP-1-F is open; its landing closes it.

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| CUP-1-A | done | none | crate stub, application skeleton, strip, scripts, documents (landed within AUD-1-P) | `scripts/smoke.sh`, guards |
| CUP-1-B | done | CUP-1-A | core models, traits, fakes; controllers over the fakes | `cargo test` |
| CUP-1-C | done | CUP-1-B | NetworkManager client and the Red page | `scripts/verify-production.sh` |
| CUP-1-D | done | CUP-1-B | BlueZ client, pairing agent and the Bluetooth page | `scripts/verify-production.sh` |
| CUP-1-E | done | CUP-1-B | PipeWire client and the Audio page | `scripts/verify-production.sh` |
| CUP-1-F | active | CUP-1-C, CUP-1-D, CUP-1-E | keyboard and accessibility pass, glass and motion check; its landing publishes 1.0.0 | `scripts/qml-tests.sh` (`tst_keyboard.qml`), [exit](docs/evidence/2026-10-08-exit.md) |

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary manages
network, Bluetooth and audio on the author's machine.

## Later

Not in CUP-1; picked from the units' evidence for what a person would notice.

- A WEP-only network reads «Protegida» like WPA and is refused on joining
  (CUP-1-C kept WEP inside `Psk`); it should say why before the dialog.
- Audio reflects an outside change within 2 s when `pw-mon` is missing (the
  watcher falls back to polling `wpctl status`).
- A stream whose link `pw-link` cannot name falls back to guessing its
  output from the node nickname, so the wrong output may be shown for it.
- A Bluetooth call that BlueZ never answers holds the section for up to
  90 s (one timeout per connection in zbus).
- One profile card per sound card instead of the design's single card
  (accepted in CUP-1-E); a machine with many cards gets a long page.
- `pw-mon` is not tied to Cuprita's death by `PR_SET_PDEATHSIG`; a killed
  Cuprita leaves it until its next write.

## Closed evidence

- CUP-1-B: [domain](docs/evidence/2026-10-08-domain.md)
- CUP-1-C: [network](docs/evidence/2026-10-08-network.md)
- CUP-1-A: [skeleton](docs/evidence/2026-10-08-skeleton.md)
- CUP-1-D: [Bluetooth](docs/evidence/2026-10-08-bluetooth.md)
- CUP-1-E: [audio](docs/evidence/2026-10-08-audio.md)
