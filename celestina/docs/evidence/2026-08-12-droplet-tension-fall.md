# PANEL-1-J prototype — the droplet falls under tension without detaching

- **Date:** 2026-08-12
- **Scope:** Celestina unit `PANEL-1-J`
- **Artifact:** Celestina 0.12.0 with CelestinaStyle 1.4.0
- **Environment:** Linux release production workflow with focused offscreen Qt
  verification and normal test-prefix deployment; no live or nested session
  activation
- **Plan:** [panel glass redesign ledger](../plans/active/2026-08-08-panel-glass-redesign.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

`PANEL-1-I` settled the droplet's resting shape and the author accepted it.
This unit gives that shape its opening motion and changes nothing else. It
adds no material, no Style token and no second geometry source: the existing
`membraneOutline` gains one bounded `progress` input, so every frame of an
opening surface is a real droplet outline rather than a scale, fade or clip
of the settled one. Both orientations take it, so a panel-opened surface and
a sideways child menu fall by the same rule.

Progress runs `0..1` and then past `1`. At exactly `1` the function returns the
geometry the settled contract verified, byte for byte, so the motion cannot
move where a surface ends up. Below `1`:

- The body's lateral span opens out of the mouth rather than starting
  body-wide: `openLo = lerp(mouthLo, bodyLo, p)` and the matching high edge.
  An emerging surface therefore reads as one drop instead of a ribbon
  unrolling sideways.
- Its extent from the seam opens with it: the neck stretches first
  (`travel * lerp(0.2, 1, p)`) and the body swells behind it
  (`bodyDepth * p`), with the corner radius bounded by the momentary span so
  a small drop is not given a large body's corners.
- Flight tension peaks mid-fall and vanishes at both ends. It multiplies the
  neck by `lerp(1, 0.66, flight)`, so the neck is visibly thinner while the
  body is moving and relaxes only once it has stopped.

Above `1` is the elastic recoil. The membrane has been stretched further than
it will hold and hauls the body back toward its seam:
`recoilTravel = min(clamp(recoil * 480, 0, 16), travel * 0.45)` is subtracted
from the body's distance from the seam, while its span and depth stay at their
settled values. Flight tension is raised for the whole recoil
(`max(sin(pi * opening), clamp(recoil / 0.05, 0, 1))`), so a recoiling
membrane is taut and its neck thin. With the real curve the peak reaches
`p = 1.029`, which lifts a 620-pixel menu 14 pixels toward the bar and thins
its neck from 35.6 to 28.5 pixels before it settles. The recoil is bounded
twice — in pixels, so a tall surface cannot swing, and against the travel
itself, so the body can never be pulled into its own mouth — and it pulls
*toward* the seam rather than past the resting place, which is what keeps the
whole outline inside the surface carrying it even for a card-sized child menu
with no room beyond its own body.

Two invariants make this a drop that does not detach, and both are asserted at
sampled frames across the whole fall rather than only at its ends. The mouth
is settled geometry: it is computed from the final body span and is never
scaled, so the seam keeps exactly the same narrow glyph-centred contact at
every frame. The neck keeps a hard floor,
`min(settled, max(12, settled * 0.55))`, measured against the settled neck
rather than the momentary body — a body still collapsed into its own mouth
therefore cannot pinch the outline into two pieces.

The carried content rides inside the drop. `membraneOutline` maps the two
corners of the momentary body through the same projection as the outline and
returns them as `openRect`, so the shell gets the body as a frame-space
rectangle without knowing which edge a surface is attached to.
`SoftMenuField` places one window at that rectangle, clips to it while the
drop is moving, and keeps the content at its settled layout inside that
window. Rows therefore emerge from the seam with the glass and travel with it
— including through the recoil — instead of waiting at the resting place for
it to arrive, and because the content is translated and clipped rather than
scaled, nothing it carries is ever stretched or reflowed. At rest the window
is exactly the card and stops clipping, so a settled surface pays nothing for
the motion. That layer still stops an inside press while the drop is falling,
so the motion cannot leak a click through to an overlay's outside-dismiss
layer.

The fall is two tokened animations, because a drop on an elastic membrane
does not travel from one place to another. It hangs at the seam and stretches
away from it, which is an accelerating release (`motionFast` on `easeExit`).
It is then caught past what the membrane will hold and let down
(`motionNormal` on `easeEmphasized` with the `overshoot` token): that curve's
overshoot is the catch, and the geometry above reads it as the recoil. The
total stays at `motionFast + motionNormal`.

Two earlier curves were rejected. One purely decelerating curve reached
`p = 0.49` a fifth of the way through, leaving the stretched middle the whole
shape exists for on screen for about four frames. A shared sine bezier was
tried next and also rejected: consuming `easeSineInOut80` requires the literal
`Easing.Bezier`, which the style guard refuses as a direct animation curve,
and hiding the curve in shell geometry instead of a token would have defeated
exactly what that guard protects. A third, monotone `easeExit`/`easeStandard`
pair was rejected by the author on sight as flat — without a recoil there is
no membrane in the motion, only a body arriving — and as ignoring its content,
which at that point merely faded in at the resting place once the glass had
landed. Both of those corrections are what this record describes.

Reduced motion resolves progress to `1` immediately and never starts an
animation, which is also what keeps every existing offscreen contract reading
the settled geometry unchanged. `beginDropFall` is idempotent: a route that
reveals twice replays nothing and a settled surface never falls again, so a
live anchor-lease refresh cannot restart the opening under the pointer.

Placement, the attachment lease, compositor-region publication, focus, Escape,
outside-click, provider, destructive-confirmation and every floating route are
untouched. The compositor region is self-throttling and unchanged: the settle
timer restarts on each geometry change, so the finite polygon is published once
from the landed shape instead of once per frame.

## Result

The focused rerun used:

```sh
cmake --build celestina/build --parallel 4
ctest --test-dir celestina/build --output-on-failure \
  -R '^(celestina-surface-manager|celestina-overlay-contract|celestina-indicator-menu|celestina-output-chooser)$'
QML2_IMPORT_PATH="$PWD/celestina-style/build" \
  QT_QPA_PLATFORM=offscreen \
  ./celestina/build/celestina-output-chooser-test -o -,txt
bash celestina/scripts/complete-production.sh
git diff --check
```

The shell build and focused selection pass 4/4. The complete offscreen
QuickTest runner passes 226/226 with no failures, skips or blacklisted cases.

The new geometry cases prove that an omitted progress and `progress = 1`
produce identical bytes; that across eleven sampled frames the mouth and neck
centre never move, the body only ever grows, and it starts collapsed into the
mouth and lands on the settled span; that across twenty-one sampled frames the
neck is never wider than its resting width, never below its floor, and never
reaches the mouth edges, with mid-fall strictly thinner than rest; that the
published body rectangle is what the outline encloses at every sampled frame
and never reaches above its own seam; that a recoil lifts the body toward the
seam while keeping its settled size, thins the neck without crossing its
floor, leaves the mouth where it was, and stays inside both of its bounds even
for an absurd overshoot; and that a sideways child falls by the same rule with
its seam pinned to the parent's edge throughout.

The new `AttachmentFall` cases prove the surrounding contract on a real
`SoftMenuField`: reduced motion opens at the settled geometry with no
animation at all and full content, a falling surface is born collapsed and
carrying nothing visible then lands on exactly the settled span with the same
mouth it was born with, the content window is the collapsed drop and clipped
while falling and becomes exactly the card and unclipped once landed while the
content layer keeps its settled size throughout, a settled surface never falls
again on a second reveal or a direct `beginDropFall`, and a floating surface
has no fall at all.

The registered production completion passes. Rust suites, QML lint with only
the pre-existing `CelestinaLineGutter` warnings, CTest 17/17 and the
eight-second offscreen release smoke all pass, and the verified bundle is
deployed to `~/.local` and reports current. No live or nested session is
activated or replaced.

## Limits

The rendered frame strip is a geometry preview, not a compositor capture: it
shows the outline the shell will paint, not the blur behind it. During the
fall the drop carries its tint and noise but not yet a compositor sample,
because the region settle timer restarts on each geometry change and publishes
once from the landed shape. Whether that arrival reads as a pop at the author's
real blur strength is exactly the perceptual question this unit cannot answer
offscreen.

The author-run nested-Niri pass remains pending. It must verify the falling
drop at supported output scales for a panel-opened menu, an overlay and a
sideways child menu; that the stretched middle is legible rather than a flash;
that the recoil reads as an elastic membrane rather than a glitch; that the
neck never appears to separate; that content rides inside the glass carrying
it and is never clipped in a way that looks broken; that the blur arrival is
not a visible pop; and that reduced motion opens with no animation.

The recoil's amplitude is the one number here chosen by eye rather than
derived: 14 pixels at the real curve's peak, bounded at 16. A previous
7-pixel version was rendered and judged too timid to read as elastic. Only the
live pass can say whether 14 is right at the author's scale.
