# Selenita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** SEL-1
- **Related author validation:** none yet; the SEL-1 units add their entries
  to [VALIDATION.md](VALIDATION.md) (they do not block)

## Hypothesis and tangible outcome

A capture tool built on `grim`, `slurp`, niri's IPC and the ScreenCast portal,
in the suite's conventions, can take screenshots and screen recordings out of
the terminal: one window with the capture, the recording and the history.

## Scope

| Unit | Outcome |
|---|---|
| EXT-1-B (`suite`) | Registration, with the skeleton, the icon and the document set |
| SEL-1-A | Screenshots of the screen, a window or a region, with delay, destinations, history and the niri key-binding flags |
| SEL-1-B | Screen recording to MP4 through the portal and GStreamer, with sound and `--stop` |
| SEL-1-C | The keyboard and accessibility pass, and 1.0.0 |

## Exclusions

- Editing a capture beyond what Fluorita already does, recording one window
  alone (the portal exposes outputs), OCR, and anything in the halted shell.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| SEL-1-A | done | EXT-1-B | capture and history cards, delay, destinations, flags | [screenshots](docs/evidence/2026-10-09-screenshots.md) |
| SEL-1-B | done | SEL-1-A | recording card, portal, pipeline, sound, `--record`, `--stop` | [recording](docs/evidence/2026-10-09-recording.md) |
| SEL-1-C | planned | SEL-1-B | keyboard, accessibility, 1.0.0 | `scripts/complete-production.sh` |

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary captures
and records with every tool of the scope.

## Closed evidence

- [The skeleton](docs/evidence/2026-10-09-skeleton.md) (EXT-1-B).
- [Screenshots, delay, destinations and the history](docs/evidence/2026-10-09-screenshots.md) (SEL-1-A).
- [Screen recording through the portal and GStreamer](docs/evidence/2026-10-09-recording.md) (SEL-1-B).
