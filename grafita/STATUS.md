# Grafita status

- **Updated:** 2026-08-03
- **Implementation:** version 1.0.0 and checkpoints G0-G6 are present; there is
  no active implementation checkpoint
- **Author validation:** the version-1 interaction pass is closed; intentionally
  excluded coverage is recorded in [VALIDATION.md](VALIDATION.md)

## Current checkout truth

- `grafita-core` owns content-based text acceptance, byte/newline-preserving
  editing, undo/redo, search/replace, indentation, highlighting, conflict
  detection, loss-free save and generation/revision-aware session outcomes.
- The standalone application supports content/path activation, untitled and
  recent documents, multiple tabs in one running instance, reordering, find,
  replace, go-to-line, syntax highlighting and guarded close.
- Siderita's embedded surface consumes the same core for edit, undo/redo, save
  and guarded close while keeping its own bounded Qt/QML adapter.
- The archived evidence records real keyboard/mouse use of both surfaces,
  modal focus containment/restoration, find/tabs and the app icon.
- Legacy encodings are an explicit product exclusion until a real document
  demonstrates the need; they are not an incomplete version-1 item.

## Conditional work, not active debt

- Add a legacy encoding only after a representative file and explicit encoding
  choice establish a reversible contract.
- Extract a shared visual editor surface only after the author accepts that
  design-system API and all consumers can be validated together.
- Revisit highlighter or large-document architecture only with a measured
  startup, memory or latency failure.

IME, AT-SPI, reduced-motion and cross-user/xattr reproduction are intentionally
outside the current plan. They do not keep implementation open and receive a
validation item only when the author asks to pursue them.

## Evidence boundary

The detailed G0-G6 record and earlier commands are in the
[archived roadmap](docs/history/roadmap-through-2026-08-03.md). On 2026-08-03
the exact canonical release passed app/core format, Clippy, tests, QML lint and
an eight-second isolated smoke. See the suite
[evidence](../docs/evidence/2026-08-03-repository-governance.md). No installed
binary or desktop-handler state was changed.

## Records

- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Content activation contract](../docs/contracts/content-activation.md)
- [Registry entry](../docs/projects.toml)
