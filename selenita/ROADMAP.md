# Selenita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** SEL-3
- **Related author validation:** VAL-SEL-SHOT, VAL-SEL-REC,
  VAL-SEL-PREVIEW and VAL-SEL-FIXES pending in
  [VALIDATION.md](VALIDATION.md) (they do not block)

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
| SEL-2-A | The corner preview after every capture and recording: drag the file out, click to edit in Fluorita, `Adopt` for the edited copy; no main window after a key-binding launch |
| SEL-3-A | The fixes after the author's first live use of the preview: a sharp preview, the screen key binding on the focused monitor, recordings into `Grabaciones`, the `Adopt` rule's folders named by `selenita_core::names` |

## Exclusions

- Editing a capture beyond what Fluorita already does, recording one window
  alone (the portal exposes outputs), OCR, and anything in the halted shell.
- For the preview, everything the
  [capture preview design](../docs/superpowers/specs/2026-10-10-capture-preview-design.md)'s
  §1 lists as out of scope, and placing the window itself (niri's rule
  does).

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| SEL-1-A | done | EXT-1-B | capture and history cards, delay, destinations, flags | [screenshots](docs/evidence/2026-10-09-screenshots.md) |
| SEL-1-B | done | SEL-1-A | recording card, portal, pipeline, sound, `--record`, `--stop` | [recording](docs/evidence/2026-10-09-recording.md) |
| SEL-1-C | done | SEL-1-B | keyboard, accessibility, 1.0.0 | [exit](docs/evidence/2026-10-09-exit.md) |
| SEL-2-A | done | SEL-1-C, PRV-1-A | the corner preview, the hand-off to Fluorita, `Adopt` | [preview](docs/evidence/2026-10-10-preview.md) |
| SEL-3-A | active | SEL-2-A | the preview decoded at its shown size, `FocusedOutput` for the screen binding, `Grabaciones`, `FolderPaths` | [preview fixes](docs/evidence/2026-10-10-preview-fixes.md) |

SEL-1 closed on 2026-10-10 at 1.0.2, after the bug units SEL-1-D and SEL-1-E
and the capture portal of SEL-1-F. The build order, exclusions, exit and
ledger are in the
[archived plan](docs/plans/archive/2026-10-09-sel-1-foundation.md).

SEL-2, Selenita's part of the suite's capture preview program `PRV-1`,
closed on 2026-10-10 at 1.1.0 with SEL-2-A. Its build order, exclusions, exit
and ledger are in the
[archived plan](docs/plans/archive/2026-10-10-sel-2-preview.md).

SEL-3 corrects what the author found in the first live use of the preview,
the same day; its build order, exclusions, exit and ledger are in the
[active plan](docs/plans/active/2026-10-10-sel-3-preview-fixes.md).

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary captures
and records with every tool of the scope; for SEL-2, the preview shows after
every capture and recording, and its drag, click and `Adopt` are in with
their tests; for SEL-3, the preview decodes at its shown size, the screen
key binding takes the focused monitor and recordings land in
`Grabaciones`, with their tests.

## Closed evidence

- [The skeleton](docs/evidence/2026-10-09-skeleton.md) (EXT-1-B).
- [Screenshots, delay, destinations and the history](docs/evidence/2026-10-09-screenshots.md) (SEL-1-A).
- [Screen recording through the portal and GStreamer](docs/evidence/2026-10-09-recording.md) (SEL-1-B).
- [The keyboard and accessibility pass and the exit at 1.0](docs/evidence/2026-10-09-exit.md) (SEL-1-C; the landing releases 1.0.0).
