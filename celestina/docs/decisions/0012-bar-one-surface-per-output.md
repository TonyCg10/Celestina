# ADR 0012: The bar is one surface per output with the Island in the centre zone

- **Date:** 2026-10-16
- **Status:** accepted

## Context

The 1.0.0 Island is a centred layer surface of about 760 logical pixels with
an exclusive zone of 48. A bar needs the full output width, but a second
surface per bar item would multiply surfaces, frames and blur regions. See
the design's §6.1 and §6.2.

## Decision

The Island's surface becomes, per output, one layer surface anchored to the
top, left and right edges, namespace `celestina-panel`, with the exclusive zone
of 48 unchanged. Its zones `start`, `centre` and `end` are laid out in one
row, the centre zone truly centred on the output. The Island is pinned in the
centre zone, exactly one, and cannot be removed. Blur and input regions are
the union of the painted silhouettes of the capsules and the Island; everything
else on the surface is transparent and passes input through. While the `bar`
duty is `off` the surface keeps its 1.0.0 shape, centred and capsule-only, as
a migration stage that CEL-2-M removes.

## Consequences

- One surface per output and frame timing measured per output; the evidence
  compares frame time and RSS with the 1.0.0 numbers.
- Keyboard focus stays `None` at rest and `OnDemand` with a page open (ADR
  0007 unchanged), now over a wider surface, which the live check covers.
- Two code paths exist until CEL-2-M: the 1.0.0 shape and the bar shape.
- With the duty `off` the Island's surface, regions, input and menus are
  1.0.0's; the process is not: the one `shell.json` watcher, which every
  duty shares, hands its reads to a pacing thread of its own, a
  do-not-disturb value edited by hand applies, and the open page shows a
  seconds clock (ADR 0015).
- A flip of the duty moves every output to a new surface. The watcher's
  pacing thread delivers flips at most once a second, at the latest value, so
  a file flipped back and forth does not churn the outputs' surfaces.

## Revisit when

Measured frame time or memory on a wide surface exceeds the 1.0.0 budget
enough to matter, or the compositor cannot deliver the full-width configure
with the exclusive zone (then one surface per zone is reconsidered).
