# ADR 0002: Use borderless compositor glass for the panel

- **Date:** 2026-08-08
- **Status:** accepted

## Context

The delivered panel used one opaque strip so its contrast could be proven over
any wallpaper. The first visual prototype removed that strip, but then covered
the requested compositor blur with a 60% capsule tint, an 86% top scrim and a
visible stroke. Niri accepted the finite `ext-background-effect` regions while
the result still read as dark paint rather than glass.

The author selected a different panel direction from two live screenshots: the
bar itself supplies only a soft shadow into the wallpaper, while each content
group sits on a borderless, visibly blurred capsule.

## Decision

- The panel window has no full-width plate or hard lower edge.
- Full-width depth is a gradual, input-transparent shadow whose opacity is low
  enough that it cannot conceal the capsules' blur.
- A panel capsule has no border. While compositor blur is armed its own fill is
  transparent; when blur is unavailable it uses one readable borderless
  fallback fill.
- Celestina publishes finite capsule regions and reapplies them whenever a
  capsule moves, resizes, appears or disappears. An empty set explicitly
  withdraws blur.
- Niri owns blur strength globally. The nested visual session carries the
  milestone's reference profile; live-session installation documents the same
  optional settings and never edits the author's compositor configuration.

## Consequences

- Wallpaper colour and texture remain visible through each capsule instead of
  being mostly hidden by a tint.
- The transparent path cannot retain the former static contrast proof over
  arbitrary wallpaper. The shadow, text treatment and both output scales need
  perceptual validation before delivery; the opaque fallback remains bounded.
- A stronger compositor profile increases GPU work if its pass count is raised.
  The reference profile therefore increases offset before adding passes, as
  Niri recommends.

## Revisit when

The blur is unavailable on a supported compositor, the reference profile
produces artifacts or unacceptable GPU cost, or readable text requires a tint
large enough to hide the blur again.
