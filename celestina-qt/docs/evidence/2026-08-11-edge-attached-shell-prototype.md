# PANEL-1-I prototype — continuous bar veil and elastic contextual membrane

- **Date:** 2026-08-11
- **Scope:** Celestina unit `PANEL-1-I`
- **Artifact:** Celestina 0.12.0 with CelestinaStyle 1.4.0
- **Environment:** Linux release production workflow with focused offscreen Qt
  verification and normal test-prefix deployment; no live or nested session
  activation
- **Plan:** [panel glass redesign ledger](../plans/archive/2026-08-08-panel-glass-redesign.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

The panel now owns one marginless `ContextualVeil` backdrop across its complete
width and one finite compositor-blur region for that backdrop. The veil paints
tint and noise but no outline, lit edge or apparent edge halo. The information
groups above it are ordinary rounded `ContentSurface` capsules inset inside the
40-pixel bar. Their visible alignment is output-local y=5 with height 30; they
paint no compositor region of their own.

A primary contextual surface opened from the panel receives three independent
facts: the clicked control for body placement and focus, the exact glyph inside
that control for the attachment waist, and the panel lower edge. The contextual
body remains centred on the clicked control after output clamping. Its vertical
travel begins at `attachmentStartY == barHeight` and remains proportional to
the stable body width, with ratio `0.06` clamped to 20..36 pixels.

The overlay begins at that panel edge as one droplet. Its only contact with
the bar seam is a narrow icon-proportional mouth centred under the glyph and
clamped inside the body's flat top span. A meniscus leaves the seam with a
horizontal tangent on both sides, the outline narrows to its neck at 34
percent of the travel and then swells concavely until it lands with a
horizontal tangent on the body's top edge. The body keeps its ordinary
rounded top corners and flat top spans outside that swell.
The membrane uses `ContextualVeil` for its entire path. It has no
`ContentSurface` contribution, dense-to-veil transition, shadow or edge
decoration. The opener's `PanelPill` and every dense content card retain the
same ordinary rounded silhouette, bounds and material before, during and after
the menu opening.

After the surface opens successfully, a tokened tracker observes the declared
glyph and its visual ancestor chain plus panel/output geometry. While the
surface remains open, a tray pin/hide change or provider-driven flank movement
remeasures the anchor through the panel's global coordinate space and publishes
only its output-local rectangle to the contextual surface. The droplet's
mouth therefore follows the real glyph rather
than a stale opening snapshot. The invoking control also keeps its ordinary hover
circle for exactly the lifetime of that surface lease. If
another controller succeeds it, the unique token prevents the retiring surface
from clearing the successor's anchor state. No tracker path mutates the panel
or the opener's capsule.

The membrane's middle responds to the real geometry rather than using one
fixed stem. Let `i = max(1, anchorWidth)` be the icon reference scale,
`b = max(1, bodyWidth)` the body reference scale, `g` the vertical travel and
`d` the distance between body and anchor centres. `i` and `b` are calculation
scales; tension only thins the hanging neck. Bounded tension is:

```text
clamp(0.42 * clamp(g / sqrt(i * b))
    + 0.43 * clamp(log(max(i, b) / min(i, b)) / log(48))
    + 0.35 * clamp(abs(d) / b))
```

The neck width is icon-proportional: `clamp(anchorWidth * lerp(2.3, 1.7,
tension), 22, 48)`, additionally bounded by one third of the body. A real
18-pixel glyph therefore hangs a roughly 31..41-pixel neck — wide enough to
read as liquid, never a body-proportional band and never the icon-thin thread
of the rejected straight hourglass. The mouth adds one travel-proportional
meniscus flare (half the travel, clamped to 8..18 pixels) on each side. The
neck centre is clamped so the complete mouth stays inside the body's flat top
span between its rounded corners; within that span it follows the glyph
exactly and does not interpolate toward the body centre. The landing run
spreads `clamp(bodyWidth * 0.18 + travel * 1.5, 48, 140)` pixels beyond the
neck on each side before the flat body edge continues.
The geometric-mean stretch and logarithmic spread keep a real 18-pixel glyph
anchor from saturating tension merely because its menu is much wider.
Each side uses two cubics: the meniscus holds a horizontal tangent at the
seam and a vertical tangent at the neck, and the swell holds that vertical
tangent at the neck and a horizontal tangent at the body landing, so the
outline is G1-continuous from the bar to the body. One geometry function
emits both the painted path and the sampled compositor polygon, so the narrow
mouth, neck, tangent landing and rounded body corners cannot drift apart.

The panel keeps its one rectangular blur region at y=0..39. The menu polygon
starts at y=40 and never republishes or reblurs those panel rows. Panel capsules
publish no compositor region of their own whether a menu is attached or not.

Command and keybind routes do not receive the panel attachment contract and
remain ordinary floating rounded surfaces. The workspace map and foreign tray
child menus received it in the later same-day passes recorded below. Existing
focus, Escape, outside-click, provider-command, destructive-confirmation,
reduced-motion and parent/foreign-child semantics are unchanged.

## Result

### Droplet current iteration

The author reviewed the fluid body-proportional-waist revision live on
2026-08-11 and rejected it: any waist suspended between two body-wide edges
still reads as a strange hourglass, and the requested direction is a soft
drop falling out of the bar. The current checkout removes the body-wide seam
entirely. Only the narrow glyph-centred mouth touches the bar, a meniscus
clings to the seam with horizontal tangents, the neck hangs just below the
bar and the swell lands tangent on the body's flat top edge inside its
restored rounded top corners. The clicked control remains separate for
placement and retains its ordinary hover circle while its own surface is
open. Neither that feedback nor the membrane changes the `PanelPill` or any
dense content card. This is the active contract described above. Its focused
rerun used:

```sh
cmake --build celestina/build --parallel 2
ctest --test-dir celestina/build --output-on-failure \
  -R '^(celestina-surface-manager|celestina-overlay-contract|celestina-indicator-menu|celestina-output-chooser)$'
QML2_IMPORT_PATH="$PWD/celestina-style/build" \
  QT_QPA_PLATFORM=offscreen \
  ./celestina/build/celestina-output-chooser-test -o -,txt
bash celestina/scripts/complete-production.sh
git diff --check
```

The shell build and focused selection pass. The complete offscreen QuickTest
runner passes 211/211 with no failures, skips or blacklisted cases. Expected
warnings reference deliberately unavailable fixture tray/wallpaper image paths
and cause no failure. The geometry cases prove the narrow glyph-centred mouth
as the only seam contact, the icon-proportional 22..48-pixel neck bounded by a
third of the body, the flat-span clamp, the bar-clinging and body-landing
tangents and the restored rounded body-top corners; the opener case retains
the open-menu hover circle. Overlay and indicator contracts also require the
narrow glyph-centred mouth and reject any body-wide seam row.

The registered production completion passes. Its production-common stage
reports 29/29 fixtures and passes the architecture, contrast and QML visual
guards. Rust results are 26 tests for `celestina-niri-adapter`, 77 unit tests
plus integration binaries with 1, 2 and 3 tests for
`celestina-provider-adapter`, 32 for `celestina-core`, 322 for
`celestina-shell-core` and 98 for `magnetita-core`. QML lint passes with only
the pre-existing `CelestinaLineGutter` warnings. CTest passes 17/17 and the
release host completes its eight-second offscreen smoke. Deployment copies the
verified bundle to `~/.local`; the installed status is current and verified.

No live or nested session is activated or replaced by completion or deployment.

### Side-attached child menus and contained foreign scroll

On the same day the author extended the accepted droplet to menus born from
other menus and reported the foreign menu's scroll defect: its fixed dark
section stayed while scrolled rows were painted over the lighter header
field, under a separate scroll bar. The current checkout generalises the
droplet into one seam-space construction (`membraneOutline`) mapped per
orientation. A foreign tray child born from a row of the mapped inventory
widens its card-sized surface with the width-proportional membrane strip,
sits flush against the parent card and receives the invoking tile's complete
rectangle; `sideAttachedMembrane` then grows the same mouth, meniscus, neck
and tangent landing out of the edge facing the parent, toward whichever side
the child was born on, and the child rises so the tile's centre stays inside
the membrane's flat lateral span. Point-only routes remain floating. The
foreign menu now pins its header card and section label beside the viewport,
raises the Menu's top padding to that pinned block, clips the ListView so no
scrolled row leaves the dark body section, and removes the separate scroll
bar; wheel, keyboard and drag reach the ListView directly. The focused
selection passes 4/4 with the geometry, surface-manager and inventory
contracts updated to the six-argument tile-rectangle signal, the pinned
heading, the clipped viewport and the side-attached seam. The offscreen
QuickTest runner passes 212/212. Registered production completion passes
CTest 17/17 with the eight-second release smoke and deploys the verified
bundle to `~/.local`, which reports current without session activation.

### Workspace map attachment and the single monitor-group dot

In the same session the author extended the droplet to the workspace strip
and removed the numbered monitor capsule. Each workspace dot and the
collapsed monitor control now publish the semantic attachment-source
contract (`isPanelAttachmentSource`, `attachmentAnchor`,
`attachmentAnchorGlobalRectNow`, lease-writable `menuOpen`); a right click
transports the control and dot rectangles through the strip and panel to
`openWorkspaceMap`, which uses the same attached recipe and live lease as
every indicator menu. The map therefore hangs from the bar with the droplet
mouth beneath the exact invoking dot, and its dot keeps hover emphasis while
the map is open. The collapsed monitor group dropped its bordered capsule
and visible count: it is one 16-pixel dot, larger than the 10..12-pixel
workspace dots, keeping the count and monitor name in its accessible label
and its urgent badge. Command and keybind routes remain floating. The
focused selection passes 4/4 with the workspace-feedback contract updated
to the group mark and attachment sources; the offscreen QuickTest runner
passes 213/213. Registered production completion passes CTest 17/17 with
the eight-second release smoke; the verified bundle is deployed to
`~/.local` and reports current without session activation.

### Superseded fluid body-wide iteration

The preceding revision kept both body-wide edges and suspended a fluid
body-proportional 0.64..0.78 waist band beneath the glyph with matched first
and second derivatives. It passed the same focused selection 4/4, offscreen
QuickTest 211/211 and registered production completion with 29/29
production-common fixtures, all recorded Rust suites, QML lint, CTest 17/17
and the release smoke, and deployed without activation. The author rejected
its live read as a strange hourglass; its results are preserved only as
superseded evidence and do not verify the droplet mouth, meniscus, tangent
landing or restored body-top corners.

### Superseded glyph-mouth iteration

The immediately preceding corrected revision used the exact 18-pixel glyph as
the membrane's upper mouth and widened only at the complete body landing. It
already kept `PanelPill` and dense content cards unchanged and painted only
`ContextualVeil`, but it did not provide the current body-wide upper edge or
persistent opener circle. Its recorded checks passed focused CTest 4/4,
offscreen QuickTest 208/208, the common architecture guard, canonical
CelestinaStyle verification with 29 production-common fixtures and CTest 1/1,
and registered Celestina completion with CTest 17/17 and the release smoke. It
deployed the verified bundle without activation. Those results are preserved
only as superseded evidence and do not validate the current geometry or opener
feedback.

### Superseded prototype evidence

The immediately preceding elastic-membrane revision used the complete owning
`PanelPill` as its upper mouth, opened that capsule to the panel edge and
painted a dense-to-veil transition. It passed:

```sh
bash scripts/check-architecture-contract.sh
bash celestina-style/scripts/build-production.sh
bash celestina-style/scripts/verify-production.sh
cmake --build celestina/build --parallel 2
ctest --test-dir celestina/build --output-on-failure \
  -R '^(celestina-surface-manager|celestina-overlay-contract|celestina-indicator-menu|celestina-output-chooser)$'
bash celestina/scripts/complete-production.sh
```

The architecture contract passed for that revision. The canonical Style
artifact built and verified, including its 29 production-common fixtures,
semantic guards, QML lint, CTest 1/1 and eight-second compiled-module smoke. The
focused Celestina selection passed 4/4; its complete QuickTest runner reported
208/208 cases.
Those regressions covered full owner/body spans, monotonic bounded tension, the
open capsule edge, owner-versus-opener transport for every primary overlay,
live width and ancestor movement, hide/restore, internal reparenting, semantic
owner ambiguity, token collision/retirement, destruction fallback, panel-side
clipping and the disjoint finite menu polygon. They do not verify the corrected
body-wide upper edge, glyph-directed clamped waist, persistent opener circle,
immutable capsule or veil-only membrane.

The registered Celestina completion then passed the complete Rust suites, QML
lint, CTest 17/17 and the eight-second release smoke. It deployed the verified
bundle to the normal test prefix, and `status-production.sh` reported every
installed artifact current. No live session was activated or replaced. The
earlier sandboxed completion attempt stopped only because the tray-watcher
fixture could not bind its private D-Bus socket in `/tmp`; the registered runs
outside that restriction pass the same test.

The focused overlay contract configures the 732-pixel Control Centre on a
768-pixel output. Its body origin remains y=72, its published polygon begins at
y=40 and translating the clicked opener republishes the same finite silhouette
at the new window coordinates. The attached route therefore never climbs into
the panel's y=0..39 region merely to satisfy a bottom-edge clamp. `PANEL-1-I`
remains `active` without an immutable inventory or commit.

The preceding continuous-veil cycle used a narrow proportional connector. Its
architecture and canonical Style checks passed; the focused Celestina
selection passed 3/3; and canonical completion passed CTest 17/17, deployed the
verified bundle and reported every installed artifact current without
activation. Those results do not verify the current body-wide upper edge,
glyph-directed waist, persistent opener feedback, live anchor tracker or
matching sampled polygon.

Before that, the experiment connected a top-edge droplet pill to an overlay
segment. Its focused Style construction passed 7/7, shell QuickTest passed
198/198, the affected C++ tests passed 3/3, and the authorized production exit
passed the complete Rust suites, QML lint, CTest 17/17 and the eight-second
smoke before deploying the verified test bundle without activation. The
offscreen preview also constructed successfully. Those results record the
superseded experiment; they do not verify the current continuous bar veil,
single panel blur region, ordinary inset capsules or bar-bottom connector
alignment.

## Limits

The membrane keeps its complete vertical travel between `barHeight` and the
menu body, including on a 768-pixel output. A 732-pixel Control Centre then
extends 36 pixels below that output and the surface clips the overflow rather
than moving its blur region over the panel. Reachable scrolling for that
unsupported low-height case is not part of this prototype.

The author-run nested-Niri pass remains pending. It must verify the continuous
veil, the single real panel blur region, resting y=5/height=30 capsule
alignment before and during attachment, the narrow glyph-centred droplet
mouth, meniscus and tangent body landing, persistent opener circle, tension response,
absence of any dense bridge, floating exclusions and the complete focus and
dismissal matrix at supported output scales.
