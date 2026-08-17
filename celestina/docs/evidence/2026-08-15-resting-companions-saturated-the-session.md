# The resting glass companions saturated the whole session

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-I`
- **Artifact:** Celestina 0.29.6, `DenseGlassAggregator`
- **Environment:** the nested reference session (patched niri, one output at
  3840x2160 scale 1.5), measured with `grim` and per-block saturation ratios
  over the author's own screen recording and controlled toggles
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-R8`

## Procedure

The author recorded the nest and reported three things as one: the wallpaper
flickers when menus open, the whole screen desaturates while one is open, and
the saturation "shoots up" on close. The recording was decomposed into frames
and measured — mean saturation, luminance and contrast per frame, then a
16x9 grid of open/closed saturation ratios — and the hypothesis was then
bisected live against the nest's compositor config and reproduced with
`launcher-toggle` alone.

## Result

### The measurements, before the cause was known

- With a menu open, every block of the screen — bar included — sits at
  0.50-0.53 of its closed saturation. Uniform, edge to edge.
- Luminance and contrast are identical in both states: a pure saturation
  operator, which no shell window can apply to pixels behind it.
- With the dense-glass layer rule removed from the live config, the "open"
  and "closed" states measure identically. The rule's own saturation set to
  1.0 changed nothing while open. The global profile's saturation set to 1.0
  changed nothing while open.

### The defect was the closed state, not the open one

The three dense-glass companions stay mapped when every menu is closed, with
their effect region withdrawn. To the compositor, a mapped surface with *no*
region and a matching rule carries the rule's effect over its whole geometry
— so three resting whole-output companions applied `saturation 1.25` each,
about x1.95 combined, to the entire session, permanently.

The author lived with that as the desktop's normal look. Opening a menu
published real regions, clipped the effect to the cards, and revealed the
true wallpaper — which read as "opening a menu desaturates the screen". Every
open/close slammed the whole output between the two states, which was the
flicker.

### The fix

A companion with no rectangles is now unmapped, not merely disarmed. After
it, the corner saturation measures 23.4 across a fresh start, an open
launcher, a closed one, an open control centre and a closed one — five
states, one number.

## Limits

The colours the author has seen for days were the saturated ones; the
session's true look is the quieter one now permanent, and whether that is the
look they *want* is a design question this record does not answer. The
menus-above-the-bar stacking, the origin-placed overlay of the msg route and
the multi-output membrane loss remain open and are not touched here.
