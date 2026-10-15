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

## Applied panel slice

On 2026-08-08 the author selected one bounded direction for the panel itself:
no hard full-width plate, a soft shadow fading into the wallpaper, and
borderless compositor-blur capsules behind content groups. That slice is
accepted in [ADR 0002](../decisions/0002-borderless-glass-panel.md) and executed
by `PANEL-1`. It does not decide the menu, overlay, iconography, clock/weather or
shared-state questions below.

## Experimental soft-menu slice

On 2026-08-09 the author first requested an opener-morph experiment and rejected
its central illusion after seeing it: two separate Wayland surfaces did not
read as one object transforming. That falsifies the dynamic-island version and
preserves it here rather than silently rewriting the proposal.

The revised experiment carries the bar's actual anatomy instead. A menu opens
near and immediately beneath its real panel control, follows the control's
horizontal order, and scales up in place. It has no card plate or rectangular
edge: a broad analytic shadow overlaps the panel falloff, while its lines and
coherent content groups occupy the same borderless, finite compositor-blur
pills as the bar. Network and control centre are the complete comparison set:
one compact dynamic list and one larger mixed-control surface.

The first live revision was close but incomplete. Starting the analytic field
at the menu's top left the panel's local vertical falloff visibly separate from
the menu shadow, and limiting pills to actionable rows left the connection
summary, section heading and active profile floating loose. The next revision
uses the real opener rectangle to extend one shadow field back through the
panel, independent of a fixed panel height or output origin, and gives every
menu line the same glass anatomy. Whether that overlap actually reads as one
field remains an author visual result, not an inference from its geometry.

That second view exposed an implementation trap rather than rejecting the
direction: `RectangularShadow` paints the interior of its source, and with no
card body above it the supposed shadow became a translucent plate over both
the panel controls and the menu pills. The third revision keeps only hollow
left, right and bottom falloffs, extends the side falloffs through the opener,
contains menu pills rather than reusing their panel overhang, and softens the
panel shadow plus no-blur pill tint. This preserves a visible ambient edge while
testing the intended glass instead of a nearly opaque fallback.

The third live view rejected that tuning as well. A falloff placed wholly
outside the field was too weak to organize it, and lowering the bar/fallback
density exposed the nested blur profile's `saturation 1.2`: the detailed yellow
wallpaper became a stack of saturated bars rather than soft glass. The next
comparison keeps the transparent successful-blur rule from ADR 0002, places the
perimeter falloff inside only the content-free margin, restores a moderate bar
shadow and lowers saturation in the nested reference profile. A tint over the
successful blur remains rejected unless this controlled profile still cannot
produce readable glass.

The fourth view showed that moving the falloff inward only as far as the empty
content margin still preserved the wrong model. It made every gap and the whole
calendar expose undimmed wallpaper, while the isolated rows became a bright
ladder with no common depth beneath them. The calendar additionally received a
fixed-height panel pill inside a tall container, which produced one unrelated
bar through its middle week. The next comparison restores one continuous soft
shadow as the bottommost menu layer, keeps every real pill above it, clips only
the shadow spill that could cross into the separate panel surface, and renders
the calendar as structured content without a row pill.

That still left the menu field visibly overlapping the bar. The author ended
the attempted joined-shadow direction: these are separate surfaces and should
now read as separate objects. The next comparison places the complete menu and
its top shadow below the panel with explicit opener-relative clearance. The
calendar is not left bare; it gets one glass card matching the pills' material
and finite compositor region, but with a card radius and its real content
height rather than pill geometry.

The first standalone view was still too high because the opener ended above
the panel surface's full shadow geometry; the bottom menu shadow also ended at
the surface boundary. The calendar material was correct but its silhouette was
still too pill-like, and nine full-width rows left avoidable empty horizontal
space. The next comparison floors placement below the real panel surface,
allocates the shadow's complete render bounds, uses a small card radius for the
calendar/weather group, and pairs night-light/caffeine, DND/power and
network/Bluetooth while keeping volume full width.

That view settled placement and compact grouping but not material hierarchy.
The author replaced the independent-pill direction with one very light outer
glass card divided by denser internal glass sections. The shadow remains, but
only outside the card; an explicit bottom gradient is required because the
analytic rectangular effect still appeared cut at the content boundary in the
live compositor. Network/Bluetooth and control centre share this material
experiment, while their commands and content remain unchanged.

The implementation must keep commands and content intact, retain outside-click
and Escape dismissal, jump directly to the final state under reduced motion,
and avoid inventing a fixed panel position, height or output scale.

On 2026-08-10 the author accepted the material direction as the basis for a
bounded shell-menu comparison and explicitly expanded the prototype to every
existing interactive menu: network, Bluetooth, tray, workspace map, control
centre, notifications, clipboard, session and launcher. This is a visual
migration only. The real `Menu` carriers keep their native lifecycle; custom
overlays keep their own keyboard, focus, command and dismissal semantics; a
launcher without a panel opener remains centred. Toasts, OSD, the standalone
output-sharing chooser and new clock/weather behaviour remain outside this
slice.

## Conclusion

Pending beyond the applied panel slice. The broader discussion is ready to
apply only when one direction records:

- a surface hierarchy and component anatomy;
- iconography, typography, spacing, color and motion rules;
- the one-gesture transient-menu lifecycle;
- the clock/date/weather information architecture and location workflow;
- accessibility and reduced-motion behavior;
- representative accepted states at scale 1 and scale 2;
- explicit exclusions and a bounded implementation order.

Applying the full language requires a decision record and a separate UX-2
implementation plan. Until both exist, UX-2 remains planned. Only the bounded
soft-menu prototype recorded above is authorized as evidence-producing
visual code.
