# SHELL-D5 — shell visual and interaction language

- **Opened:** 2026-08-08
- **Status:** open
- **Question:** What visual and interaction language should make the existing
  Celestina capabilities feel coherent and comfortable to use?
- **Blocks:** UX-2 implementation planning
- **Authority:** discussion only; this record authorizes no code, version,
  deployment or live-session change

## Context

UX-1 passed its functional validation, but that pass exposed the difference
between correctness and a shell the author would choose to use. The current
menus lack deliberate iconography and visual hierarchy. Their composition does
not yet communicate state and available actions well, and changing directly
from one open transient menu to another still required two clicks in the last
live observation.

The clock/date region also has no direct surface. The author wants it to open a
richer calendar-and-weather view, including a clear place to add and manage
weather locations. That interaction crosses information architecture, product
copy, empty states and provider boundaries, so it must be designed before it is
implemented.

## Strongest case

A shell-wide direction can make hierarchy, geometry, icons, state and motion
agree across every surface. Deciding the clock/date/weather composition and the
transient-menu lifecycle beside that language prevents each new menu from
inventing its own anatomy and makes accessibility part of the design rather
than a repair after implementation.

## Counter-case

A broad redesign can become subjective churn, hide working behavior behind
visual novelty and create premature shared components before real surfaces prove
their common semantics. If the candidate directions cannot improve the same
representative states measurably, small local corrections may be safer than a
single large UX-2 implementation.

## Alternatives

Compare at least three approaches: a conservative refinement of the current
CelestinaStyle language, a more information-dense desktop direction and a more
spacious adaptive direction. A fourth valid outcome is to keep most current
composition and authorize only the clock/date surface plus the proven
interaction defects. No option is selected by this discussion yet.

## Surfaces in the discussion

- The panel's overall hierarchy, density, spacing and grouping.
- Network, Bluetooth, tray and panel context menus opened by either pointer
  button.
- Launcher, clipboard, notification, control-centre and session overlays.
- Notifications, OSD and other brief feedback surfaces.
- The proposed clock/date, calendar, weather and location-management surface.
- Shared loading, empty, pending, failure, disabled, selected, hover, pressed
  and visible-focus states.

## Questions to iterate

1. What should remain visible at a glance, what belongs behind a click and what
   should never compete for panel width?
2. Which icon family, sizes and text pairings communicate each action without
   turning the shell into an unlabeled symbol grid?
3. What card widths, internal rhythm, corner radii, elevation and alignment
   rules remain coherent at scale 1 and scale 2?
4. How should one transient surface replace another in one gesture while
   preserving outside-click dismissal and restoring focus exactly once?
5. Should the clock/date surface be a compact popover, a larger centre or an
   adaptive composition, and which information earns its first view?
6. How are weather locations added, named, selected, reordered and removed
   without automatic geolocation or stale-weather claims?
7. Which motion clarifies continuity, and what is the exact reduced-motion
   equivalent?
8. How do keyboard navigation, visible focus, assistive names, contrast and
   Spanish product copy remain first-class parts of every proposal?

## Constraints already settled

- QML presents provider truth and never launches tools or infers success.
- Existing UX-1 network and Bluetooth actions remain bounded and
  provider-confirmed.
- The panel does not take keyboard focus from the active application.
- Outside click and Escape remain required dismissal paths.
- Geometry follows the invoking control and the compositor's real placement;
  it cannot assume a fixed panel height, stacking order or output scale.
- Colors, typography, radii, control anatomy, opacity and motion come from
  semantic CelestinaStyle tokens.
- Product copy is Spanish throughout a surface.
- Weather never guesses a location and never presents stale data as current.

## Falsifiers and evidence needed

- Annotated screenshots of every current surface on both output scales.
- Two or three deliberately different visual directions, each applied to the
  same representative panel and menu states.
- Interaction sketches for open, replace, dismiss, pending, failure and focus
  restoration.
- A compact inventory of existing CelestinaStyle tokens and controls, noting
  real gaps rather than proposing duplicate local styling.
- The author’s comparison of hierarchy, readability, density and comfort across
  the candidate directions.

The case for one shell-wide redesign is weakened if the same accepted component
anatomy cannot serve representative network, Bluetooth, notification and
clock/date states without special-case styling, or if the denser and more
spacious directions do not produce a clear author preference.

## Conclusion

Pending. This discussion is ready to apply only when one direction records:

- a surface hierarchy and component anatomy;
- iconography, typography, spacing, color and motion rules;
- the one-gesture transient-menu lifecycle;
- the clock/date/weather information architecture and location workflow;
- accessibility and reduced-motion behavior;
- representative accepted states at scale 1 and scale 2;
- explicit exclusions and a bounded implementation order.

Applying it requires a decision record and a separate UX-2 implementation plan.
Until both exist, UX-2 remains planned and no visual code is authorized.
