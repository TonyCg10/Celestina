# Fluorita status

- **Updated:** 2026-08-19
- **Implementation:** checkpoints F0-F6 are closed; F7 is implemented, verified
  and deployed, and stays the active checkpoint until its commit closes the
  ledger
- **Author validation:** the version-1 playback and interaction pass is closed;
  `VAL-FLU-SOURCES`, `VAL-FLU-IMMERSIVE`, `VAL-FLU-TEARDOWN`, `VAL-FLU-BYTES`
  and `VAL-FLU-EDIT` are open — see [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- Uncommitted in the checkout: the whole of `F7`, at version `1.3.0`. A
  picture opens for editing from the item menu or `Ctrl+E`: it can be turned,
  mirrored, cropped, resized, written on, drawn on, boxed, highlighted and
  redacted, every mark stays a movable object, and saving offers a copy beside
  the original — which stays reopenable — or a replacement, which flattens the
  result and sends the original to the Trash. A turn on a JPEG rewrites two
  bytes of EXIF instead of re-encoding the picture. Both hosts completed: the
  verified bytes are installed in `~/.local`, and Siderita carries the same
  shared crates. The offscreen gate constructs the edit surface; nothing has
  been drawn on a real display, which is `VAL-FLU-EDIT`.
- F6 is delivered in full: the
  catalogue forgets what a completed scan of a reachable root did not find, an
  item grows out of its card and shrinks back into it, right-click offers Trash
  and Properties, the folder is navigable from inside the open item, the space
  around it is lit by its own artwork, and the surface is Spanish under
  ADR 0007. Its corrective units are in with it — the render context released
  by an explicit renderer claim rather than by item visibility, byte-exact path
  keys across the Qt seam under
  [ADR 0008](../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md), the
  image probe addressed by key and opened by descriptor, and a session
  generation that drops a render handle published after the player moved on.
  None of it has been seen on a real session: that is `VAL-FLU-IMMERSIVE`,
  `VAL-FLU-TEARDOWN` and `VAL-FLU-BYTES`.
- The shared core/engine implement media classification, source-scoped
  Gallery/Music projections, user-owned persistent roots, persistent
  incremental catalogue/watch, bounded metadata and artwork generation,
  cancelable trailers and confirmed playback over libmpv.
- Standalone Fluorita provides the library behind a sidebar of the mapped
  folders — added through the desktop folder chooser and removed again — with
  the complete player, image toolkit path, video render seam, seek/volume
  controls, single-click and direct activation, and MPRIS2.
- Siderita embeds the same engine/render contracts for image, video and audio,
  while ordinary browsing starts no decoder and uses cached static artwork.
- The archived evidence records real-session playback in both hosts, correct
  orientation, keyboard/seek/volume behaviour, no visible tearing, a 4K60
  pacing sample and repeated lifecycle/resource checks.
- Desktop registration is a stateful deploy concern: on the recorded unpinned
  environment it made Fluorita the effective handler for its advertised media
  types. Verification must not reproduce that side effect.

## Active work

F7 made the library able to change an item and not only show it, under the
[plan](docs/plans/active/2026-08-19-bounded-media-editing.md) and
[ADR 0009](../docs/decisions/0009-editing-without-an-encoder.md). It is
implemented, verified and deployed; what remains is the commit, which is the
author's to request. Its unit has no inventory yet on purpose: an inventory is
compared against the worktree until it lands, so it is computed at commit time
rather than kept fresh against work that keeps arriving.

Ahead of it, and not opened: F8 gives the edit path the half that touches no
pixel — a track's tags and the EXIF a photograph carries. Its model, its FLAC
and EXIF writers, its adapter and its panel are implemented and tested in this
checkout: "Datos del archivo" in the item menu reads what a file says about
itself, corrects a FLAC's four projected tags, and removes what a photograph is
carrying, all on the same copy-or-replace terms editing already uses, with the
audio frames and the picture data carried across byte for byte. A track can also be given a
front cover through the desktop's picker, bounded before it is read. Only FLAC
is writable: Ogg, ID3 and MP4 are read, reported and refused rather than
half-written. None of it carries a ledger unit: it waits for F7's commit to
free the crates they share.

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
