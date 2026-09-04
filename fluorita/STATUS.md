# Fluorita status

- **Updated:** 2026-09-03
- **Implementation:** checkpoints F0-F15 are closed and delivered; no
  checkpoint is active
- **Author validation:** the version-1 playback and interaction pass is closed;
  `VAL-FLU-SOURCES`, `VAL-FLU-IMMERSIVE`, `VAL-FLU-TEARDOWN`, `VAL-FLU-BYTES`,
  `VAL-FLU-EDIT` and `VAL-FLU-METADATA` are open, and the surfaces F11-F15
  added have never been seen on a display — see [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- Delivered as `1.3.4`: `FEEDBACK-1-FLU`. The edge arrows were clickable at
  zero opacity and the filmstrip's frames lost their pointer while the strip
  was still sliding in; both now act only once at least half visible. The
  metadata panel's four verbs, the detail panel's close and the text tool's
  "Colocar" are the same glyphs the edit toolbar already used (`image`,
  `copy`, `check`, `x`); undo and redo share one capsule with the `undo`/`redo`
  glyphs; the tool and zoom toggles are `checkable` instead of swapping roles;
  the colour swatches gained the suite's hover circle and press sink; the
  frames paint the shared `CelestinaRowHighlight`. The artwork button keeps
  its words because the count is the information. Hand check:
  `VAL-FLU-FEEDBACK`.
- `F7` is delivered at version `1.3.0`, committed as `F7-A`, verified and
  installed in `~/.local`. That single commit carried the whole of `F8-F15`
  with it, which is why the roadmap describes nine sections against one ledger
  unit: the inventory's 61 files are what actually landed.
- Two of those capabilities shipped with no way to reach them — a frame could
  be extracted and a pacing capture could be taken, and nothing in the
  interface called either. Both now have a trigger: a button in the transport
  for the frame, `Ctrl+Shift+P` and `Ctrl+Shift+S` for the capture. Compiled,
  linted and smoke-tested; not yet committed.
- Nothing else is in flight. The roadmap names no active checkpoint.

## Active work

None. F7-F15 are delivered and the roadmap names no active checkpoint.

What the products now do beyond playing and browsing is in the
[user contract](README.md); why editing stops where it does is
[ADR 0009](../docs/decisions/0009-editing-without-an-encoder.md); and what each
checkpoint set out to fix is in the [roadmap](ROADMAP.md).

Three things wait on the author rather than on work:

- **The encoder decision.** Trimming, dropping a track, converting a format and
  exporting a clip all need a muxer. Every one of them is refused today rather
  than approximated, and each refusal says so.
- **A captured judder.** F15 is the instrument; a report from a real session is
  what would open the pacing repair the roadmap has kept shut.
- **Seeing any of it.** The whole of F11-F15 has been exercised by tests and by
  an offscreen smoke, never by a person watching a film.

## Conditional work, not active debt

- Trailer-on-hover needs a demonstrated interaction and resource budget before
  it becomes work.
- Subtitle/track selection, playback speed, queues/playlists and shell MPRIS
  UI require separate accepted checkpoints and real product need.
- A frame-presentation change requires measured judder; the previously tested
  premature swap report made pacing worse and remains rejected.

## Evidence boundary

The detailed F0-F4 record, backend measurements and earlier fixes are in the
[archived roadmap](docs/history/roadmap-through-2026-08-03.md). On 2026-08-03
the exact canonical release passed app/core/engine/Qt format, Clippy and tests,
including the real-media suite, plus QML lint and an eight-second isolated
smoke. See the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). No installed
binary, MIME association or live playback surface was changed.

## Records

- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Content activation contract](../docs/contracts/content-activation.md)
- [Registry entry](../docs/projects.toml)
