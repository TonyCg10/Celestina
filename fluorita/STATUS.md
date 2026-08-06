# Fluorita status

- **Updated:** 2026-08-06
- **Implementation:** checkpoints F0-F4 are closed; F5 rebuilds the library
  around the configured roots and is the active implementation checkpoint
- **Author validation:** the version-1 playback and interaction pass is closed;
  `VAL-FLU-SOURCES` is open for the new surface — see
  [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- Uncommitted in the checkout: `F6-C`, the Fluorita half of
  [ADR 0008](../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md). A
  path crossing to QML is now a percent-encoded key — `fluorita/src/pathkey.rs`
  composes it over the suite's one codec — and every verb decodes it back with a
  typed refusal instead of rebuilding a `PathBuf` from a lossy string. The name,
  the location and the title a person reads travel in their own columns and
  never come back. A file whose name is not valid UTF-8 can therefore be opened,
  described and trashed; before this it listed and answered the library's
  item-is-gone notice (`copy::ITEM_GONE`) to everything. Compiled, linted and
  unit-tested, with no inventory, no version transition and no production run. Nothing has been
  tried on a real session: that is `VAL-FLU-BYTES`. One limit stands and is
  recorded rather than hidden — an image with such a name is refused as
  unreadable, because the C++ probe seam takes a `QString` and Qt has no
  lossless spelling for those bytes. Siderita's half of the same ADR (`SID-A2`)
  is a separate unit and is untouched here.
- Delivered as `fluorita-bug` at 1.2.1, built, verified and deployed: `F6-B`,
  the corrective unit from the suite audit.
  A render context's release is now decided by an explicit renderer claim
  instead of by item visibility, which is what allowed the mpv core to be
  destroyed under a live context after a stream failed; an activation arriving
  during a close is held rather than run through the teardown; the player stops
  and joins on `Drop`; a context that fails to build is reported instead of
  leaving the file on "abriendo" for ever; cancellation reaches the scan and the
  tag probes; MPRIS emits its property and seek signals; a watcher refresh
  projects under the scope in force and a scan failure no longer empties the
  library on screen. None of the teardown paths has been exercised on real GPU
  state: that is `VAL-FLU-TEARDOWN`.
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

F5 replaces the two kind tabs with the source sidebar its
[plan](docs/plans/archive/2026-08-04-source-first-library.md) describes, under
suite [ADR 0006](../docs/decisions/0006-source-first-library-navigation.md).
The library lane's Rust and QML moved to English as it was rewritten, and the
matching language-baseline rows went down or away with it.

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
