# ADR 0006: Navigate the standalone media library by configured source

- **Date:** 2026-08-04
- **Status:** accepted

## Context

The content-activation contract states that standalone Fluorita "retains
Gallery for images/video and Music for albums/artists/tracks". The shipped
surface implemented that sentence as two fixed top-level tabs over one flat
catalogue: every image and video in every configured root in a single
date-ordered grid, every track in a single artist-grouped list.

The author reports that this is not the product they specified. The library is
meant to be entered through the folders the user chose to map, each one showing
the supported content inside it. The kind tabs make the mapped roots invisible:
`fluorita-core` already models them as `MediaSource` with a `KindSet` and every
`MediaRecord` carries its `SourceId`, but the projection discards that column
before it reaches QML, so the one axis the user configured is the one axis the
interface cannot navigate.

The two projections themselves are not in question. A folder of photographs
wants a thumbnail grid; a folder of music wants artist, album and track. What is
in question is whether the *kind* or the *source* is the top-level axis.

## Decision

The top-level navigation axis of the standalone library is the configured media
source. A sidebar lists the mapped roots, and selecting one shows the supported
content inside it.

Gallery and Music remain the two catalogue projections and the two ways content
is rendered. They stop being fixed tabs and become the presentation a selected
source resolves to through the kinds that source contributes: a source
contributing image or video renders the Gallery grid, a source contributing
audio renders the Music projection. The projections keep their current domain
rules, ordering and missing-item honesty.

Configured sources become user-owned and persistent. The user adds a folder
through the desktop's file-chooser portal and removes it again; the seeded XDG
directories become the first run's contents rather than the permanent and only
set. Persisted source identity therefore has to survive a restart, because the
stored catalogue already keys records by it.

The embedded Siderita surface is unchanged. It remains a minimal item viewer or
player with no library, no sources and no settings, and this decision grants it
none.

## Consequences

- `docs/contracts/content-activation.md` no longer names Gallery and Music as
  standalone surfaces; it names them as projections and states the source axis.
  The `Space` versus double-click mapping is untouched.
- Fluorita gains persisted source configuration and a portal file-chooser
  client. Configured roots stop being derived from the environment on every
  launch, which also makes the `SourceId` values already written into the
  stored catalogue meaningful across runs instead of incidentally stable.
- A first run with no stored configuration still seeds from the existing XDG
  media directories, so the library is never empty because nothing was
  configured yet.
- Removing a source removes its rows from the library and never touches a file
  on disk, consistent with the existing catalogue rule.
- Siderita's bookmark row is the visual reference, not a shared component. One
  application does not import another's UI; Fluorita composes its own sidebar
  from `celestina-style` tokens and controls.

## Revisit when

A configured root grows large enough that a flat per-source listing is not
navigable and subfolder navigation earns its own accepted slice; a second host
demonstrates the same sidebar semantics and the control becomes a candidate for
`celestina-style`; or the author asks for a cross-source view, in which case it
is added as an explicit entry beside the sources rather than by restoring the
kind tabs.
