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
| CUP-1-G | The final review's fix wave, before the author's live checks |

## Exclusions

- Editing connection profiles, hotspots, mobile broadband, Bluetooth file
  transfer, audio graph routing, a tray or indicator, notifications, and any
  shell integration.

## Build order

CUP-1-A to CUP-1-F are done and 1.0.0 is published; CUP-1-G, the fix wave
from the final review, is open.

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| CUP-1-A | done | none | crate stub, application skeleton, strip, scripts, documents (landed within AUD-1-P) | `scripts/smoke.sh`, guards |
| CUP-1-B | done | CUP-1-A | core models, traits, fakes; controllers over the fakes | `cargo test` |
| CUP-1-C | done | CUP-1-B | NetworkManager client and the Red page | `scripts/verify-production.sh` |
| CUP-1-D | done | CUP-1-B | BlueZ client, pairing agent and the Bluetooth page | `scripts/verify-production.sh` |
| CUP-1-E | done | CUP-1-B | PipeWire client and the Audio page | `scripts/verify-production.sh` |
| CUP-1-F | done | CUP-1-C, CUP-1-D, CUP-1-E | keyboard and accessibility pass, glass and motion check; its landing published 1.0.0 | `scripts/qml-tests.sh` (`tst_keyboard.qml`), [exit](docs/evidence/2026-10-08-exit.md) |
| CUP-1-G | active | CUP-1-F | the final review's fixes: NetworkManager call timeout, Spanish notices for D-Bus and `wpctl` errors, no audio poll while `pw-mon` runs, passphrase length, WEP label | `cargo test -p cuprita-core`, `scripts/qml-tests.sh`, [final review](docs/evidence/2026-10-08-final-review.md) |

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary manages
network, Bluetooth and audio on the author's machine.

## Later

Not in CUP-1; picked from the units' evidence and the
[final review](docs/evidence/2026-10-08-final-review.md) for what a person
would notice.

- A stream whose link `pw-link` cannot name falls back to guessing its
  output from the node nickname, so the wrong output may be shown for it.
- A Bluetooth call that BlueZ never answers holds the section for up to
  90 s (one timeout per connection in zbus); during airplane mode this can
  delay the adapter's power-off by as much.
- Airplane mode lives in Cuprita's memory only: a restart forgets it, and
  NetworkManager or BlueZ changed elsewhere do not see it.
- BlueZ's `RequestAuthorization` (a device that pairs without a code) is
  answered by policy without asking the person: accepted for the device
  Cuprita is pairing or an already paired one, refused otherwise; a dialog
  for an unexpected request needs a design decision.
- When setting a paired device `Trusted` fails, it is only logged; the
  device then asks again on its next connection with no notice.
- An access point's signal is refreshed only on NetworkManager's change
  signals, not on a timer, so bars may lag between scans.
- The Tab path into a row's buttons and sliders is not walked by a test.
- `cuprita_core::agent::Agent` (the one-request slot) and the BlueZ agent's
  own pending slot both track the waiting request; the controller uses the
  first, so it is live code and stays until the two are merged.
- A numeric passkey request (`RequestPasskey`) shares the PIN token, so the
  field cannot offer a digits-only keyboard (`Qt.ImhDigitsOnly`) for it;
  that needs a token of its own through `cuprita-core`, the controller and
  `PairingDialog`.
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
