# Selenita status

- **Updated:** 2026-10-09
- **Implementation:** SEL-1 is open; the skeleton (suite unit EXT-1-B) builds
  and opens the window with its three cards, and SEL-1-A is next
- **Author validation:** none requested yet

## Current checkout truth

- Version 0.1.0. The project is registered and builds a release binary. The
  window is transparent over the backdrop, follows the suite's appearance file
  and shows three empty cards: capture, recording and history.
- A second launch reaches the running process and raises its window; a path
  on the command line or in an `Open` is ignored.
- `SELENITA_FAKE=1` is read by the controller; nothing is faked yet.
- `selenita-core` exists as an empty crate.

## Blockers

None recorded.
