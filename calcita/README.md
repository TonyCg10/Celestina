# Calcita

Celestina's PDF viewer: one document per window, read in the suite's glass
grammar instead of the web browser.

## User contract

- Opens a PDF from the launcher, from Siderita's «Abrir en», from a drop or
  from a second launch, which reaches the running process.
- Continuous pages, zoom (fit width, fit page, free), go to a page, search
  with the hits marked, the outline, text selection and copy, internal links,
  a dark reading mode and the recent documents.
- No annotation, signing, forms or printing.

The features arrive in the CAL-1 units; the skeleton shows the empty state
«Sin documento» with an «Abrir…» button that is wired in CAL-1-A. See the
[design](../docs/superpowers/specs/2026-10-09-reading-and-capture-design.md).

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/calcita-core` | Recents, zoom, reading positions, page input; no Qt |
| `src/` | The CXX-Qt controller, the activation adapter, the appearance follower |
| `qml/` | The window, the empty state and, from CAL-1-A, the page area over QtPdf |
| `../celestina-style` | Canonical visual tokens, controls and assets, linked |
| `org.celestina.Calcita.desktop` | Desktop discovery |

## Build and use

Calcita needs Rust and a Qt 6 development environment visible to CXX-Qt
(QtPdf from CAL-1-A). The canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

After completion, launch `calcita` or use the desktop entry.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
