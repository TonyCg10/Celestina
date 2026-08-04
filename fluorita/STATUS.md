# Fluorita status

- **Updated:** 2026-08-03
- **Implementation:** the registered product version and checkpoints F0-F4 are
  present; there is no active implementation checkpoint
- **Author validation:** the version-1 playback and interaction pass is closed;
  see [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- The shared core/engine implement media classification, Gallery/Music
  projections, persistent incremental catalogue/watch, bounded metadata and
  artwork generation, cancelable trailers and confirmed playback over libmpv.
- Standalone Fluorita provides the library, complete player, image toolkit path,
  video render seam, seek/volume controls, direct activation and MPRIS2.
- Siderita embeds the same engine/render contracts for image, video and audio,
  while ordinary browsing starts no decoder and uses cached static artwork.
- The archived evidence records real-session playback in both hosts, correct
  orientation, keyboard/seek/volume behaviour, no visible tearing, a 4K60
  pacing sample and repeated lifecycle/resource checks.
- Desktop registration is a stateful deploy concern: on the recorded unpinned
  environment it made Fluorita the effective handler for its advertised media
  types. Verification must not reproduce that side effect.

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
