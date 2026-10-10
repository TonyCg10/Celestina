# Fluorita status

- **Updated:** 2026-10-10
- **Implementation:** checkpoints F0-F15 and `FLU-H1`, the hardening that
  followed the 2026-09-26 monorepo audit, are closed and delivered; no
  checkpoint is active. `FLU-H1-A` (a Replace that trashes its original,
  bounded file claims), `FLU-H1-B` (a true library off the GUI thread,
  reopenable copies, one owner for the playback handshake, threads that end)
  and the author's requests `FLU-H1-C` to `FLU-H1-F` landed
  ([archived plan](docs/plans/archive/2026-09-26-hardening.md))
- **Author validation:** the version-1 playback and interaction pass is closed;
  `VAL-FLU-SOURCES`, `VAL-FLU-IMMERSIVE`, `VAL-FLU-TEARDOWN`, `VAL-FLU-BYTES`,
  `VAL-FLU-EDIT` and `VAL-FLU-METADATA` are open, and the surfaces F11-F15
  added have never been seen on a display — see [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- Suite conventions ([ADR 0012](../docs/decisions/0012-suite-conventions.md),
  `CONV-1`): Fluorita serves `org.celestina.Fluorita` through
  `celestina_core::activation`: a second launch or Siderita's «Abrir en» plays
  the first media file in the running player, and a `text/uri-list` drop is
  decided by the same rule. The window follows the suite's appearance file
  (reduced motion, the text scale on the font tokens). `VAL-FLU-OPEN` and
  `VAL-FLU-DROP` are the author's checks.
- `CONV-1-E` (suite): a drop on the window is decided like `Open`: the first
  media file plays (an image opens in the viewer), a source folder is
  selected, another folder opens the source chooser at it; a URI that names
  no local file is ignored with a short notice ([evidence](../docs/evidence/2026-10-09-drag-and-drop.md)).
- `CONV-1-A` (suite): Fluorita keeps to one player. A launch claims
  `org.celestina.Fluorita` through `celestina_core::activation`, on a bus
  connection of its own beside MPRIS; a second `fluorita ARCHIVO` plays its
  file in the running window (only the first: there is no queue), a folder
  that is a library source becomes the selected source, and a bare launch
  raises the window ([evidence](../docs/evidence/2026-10-09-shared-activation.md)).
- Delivered as `1.3.5`: `FEEDBACK-4-FLU`. Gallery cards, music rows and the
  filmstrip frames paint the shared plate in its Content family — the accent
  at 7 % under the pointer, 26 % and a sink under the finger — replacing two
  hand-rolled grey plates. See
  [the record](../docs/evidence/2026-09-07-two-families-and-a-path-editor.md).

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
  interface called either. Both have had a trigger since `2ff8738`: a button
  in the transport for the frame, `Ctrl+Shift+P` and `Ctrl+Shift+S` for the
  capture.
- The engine no longer carries the unused H.264 encode path that produced
  trailers into a file; the hover preview F14 delivered is a live, silent,
  looping session, and ADR 0009's "no encoder in the closure" is now true of
  the code as well as of the product.

## Active work

No checkpoint is active. `FLU-H1`, the post-audit hardening, closed on
2026-10-10 with its six units landed
([archived plan](docs/plans/archive/2026-09-26-hardening.md)).

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

- The hover preview (F14), stream selection and playback speed (F11) and
  end-of-item continuation (F12) are delivered, not conditional; what remains
  conditional is a queue or playlist beyond the folder, and a shell MPRIS
  surface, each of which needs its own accepted checkpoint and real product
  need.
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
