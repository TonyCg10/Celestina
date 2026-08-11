# PANEL-1 — borderless glass panel

- **Opened:** 2026-08-08
- **Plan ID:** panel-glass-redesign
- **Status:** active
- **Scope:** celestina
- **Implementation checkpoint:** PANEL-1
- **Predecessor:** [WMAP-1 — the workspace window map](../archive/2026-08-08-workspace-window-map.md)
- **Decision:** [ADR 0002](../../decisions/0002-borderless-glass-panel.md)
- **Author-validation checkpoint:** `VAL-PANEL-1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

A transparent layer surface can read as a bar without becoming a solid strip:
finite compositor-blur capsules give each content group enough visual
separation without a full-width shadow. The blur remains visible only if the
QML drawn above it stops covering it with opaque tint, strokes or exterior
halos, and it represents the real desktop only when the compositor does not
replace that scene with a wallpaper-only xray cache.

## Tangible outcome

The panel has no hard full-width background. Its content sits on borderless
capsules that visibly disperse the composed scene below them—application
content where a window intersects the glass and wallpaper elsewhere—including
the phone capsule at both horizontal ends, while the bar itself adds no shadow
over the desktop. Panel capsules and contextual content cards use one dense
dark matte material with a fixed light/white foreground; the contextual outer
carrier remains nearly transparent.

## Scope

- The panel surface, shadow removal and content grouping.
- One local capsule component and its readable no-blur fallback.
- Finite compositor blur regions, dynamic geometry updates, withdrawal and
  protocol commit/flush behaviour.
- Flank sizing and phone geometry needed to keep the outer capsule whole.
- One consistent panel-reading type/ink scale and the status-glyph consumers
  needed to replace network, Bluetooth and audio labels while pairing CPU and
  memory percentages with canonical glyphs.
- A workspace strip whose visible workspaces are colour state marks and whose
  other-monitor groups retain their compact count capsules, without workspace
  numbers, output names or active-window titles.
- Permanent icon entry points for the control centre, clipboard, notification
  centre and session menu, wired to the overlays the host already owns.
- One Velo anatomy for every existing interactive menu surface:
  network, Bluetooth, tray, workspace map, control centre, clipboard,
  notification centre, session menu and launcher. Each carrier keeps its own
  lifecycle, focus and command semantics while sharing the same outer
  compositor-backed field, canonical `GlassSurface` content sections and
  transient-row presentation.
- One additive `GlassSurface` external-backdrop mode in CelestinaStyle so every
  contextual content group uses the suite glass material without attempting an
  impossible cross-client QML capture. The shell retains KWindowEffects, one
  finite outer region, fixed light/white ink and fallback ownership.
- Icon-only power-profile, volume and brightness readings, with their full
  values retained in accessible names.
- A delayed tray-registry reconciliation and model-driven wrapper visibility so
  a restarted host cannot remain empty after either a premature watcher
  snapshot or the initially empty QML state; bounded diagnostics cover both
  seams.
- Durable, bounded tray presentation preferences keyed by the item's published
  identity: pinned items appear beside the compact tray opener, hidden items
  remain recoverable by switching the fixed icon inventory between visible and
  hidden modes, and hiding an item also removes its pin. Missing foreign icons
  degrade to one fixed compact application glyph instead of widening the bar
  with producer text. An item's foreign D-Bus menu uses a second carrier so it
  can remain open beside the shell-owned inventory without replacing it.
- A per-output wallpaper menu beside the toolbox. Its standard folder chooser
  supplies one durable gallery directory at runtime; the provider publishes a
  bounded, non-recursive inventory of supported regular images and the menu
  presents their thumbnails. Choosing a thumbnail retains the existing bounded
  validation, atomic import into the XDG wallpaper directory and immediate
  wallpaper refresh.
- The narrow canonical Lucide catalogue additions those consumers require;
  they remain a separately owned CelestinaStyle delivery and are not absorbed
  into the `celestina:` inventory.
- A nested Niri reference profile and exact opt-in live-session instructions,
  including the compositor-owned non-xray rule required for glass to sample
  application content rather than only the wallpaper.
- One shell-local foreground palette for the panel and every interactive menu
  surface: light/white ink over the dense dark content material on every
  wallpaper, without any per-wallpaper exposure or contrast analysis and
  without changing the suite's global colour scheme.
- Automated construction/geometry checks and author visual checks at scale 1
  and scale 2.

## Exclusions

- Toast and OSD restyling; they are read-only transient feedback rather than
  interactive menus.
- Clock, calendar or weather feature work.
- The standalone output-sharing chooser, which belongs to the portal command
  path rather than the shell's menu/overlay family.
- Provider behaviour outside the non-visual wallpaper identity used for
  same-path image reloads and the bounded user-selected local wallpaper import;
  DDC, media, network and Bluetooth behaviour remain unchanged.
- Foreign tray-item activation, secondary activation, D-Bus menu contents and
  watcher ownership. Shell-owned pin/hide presentation preferences and the
  parent/child menu carrier are the only tray semantic extension.
- Any shared-style change beyond the exact status glyphs and the demonstrated
  external-backdrop `GlassSurface` mode consumed by this shell; the fixed
  foreground mapping remains shell-local and derives from existing semantic
  theme roles.
- Editing the author's live Niri configuration or replacing the live shell.
- The unrelated pending wallpaper-provider correction in this worktree.

## Build Order

1. Establish one larger primary-ink scale for panel readings, replace the named
   textual status labels with canonical icons, and remove the phone's visible
   device name without changing provider semantics.
2. Reduce visible workspaces to coloured interactive state marks, preserve the
   original folded grouping for other monitors without visible output names,
   remove the active-window label, and add permanent buttons for the existing
   overlays.
3. Reconcile a foreign tray registry after host registration and make its QML
   wrapper follow the independent item model so neither an empty snapshot nor
   an initially hidden child can strand the drawer.
4. Remove the capsule stroke and every successful-blur fill that hides the
   compositor result; retain one borderless fallback.
5. Remove the full-width scrim and reduce the panel surface to the visible bar.
6. Make region updates and protocol commits follow every capsule geometry
   change, including late providers and empty state.
7. Keep the phone capsule within the flank without clipping either cap.
8. Tune the nested Niri blur profile from the official offset/pass contract,
   disable Niri's automatic xray policy only for the exact interactive
   Celestina layer namespaces, then compare the result with the author's
   reference crop and a control window below the glass.
9. Add construction and geometry regressions, run the canonical exit only after
   the author accepts the visual direction, and document the optional live
   compositor profile without applying it.

## Implementation exit

- A busy composed backdrop loses recognizable detail inside every capsule while
  staying sharp immediately outside it; application colour remains represented
  where a window lies below the glass and wallpaper remains represented where
  it does not.
- No capsule has a visible border or opaque successful-blur fill, and the phone
  capsule retains both round ends.
- Panel capsules and contextual content cards keep the fixed light/white
  foreground over one dense dark matte material on every wallpaper; the
  contextual carrier remains nearly transparent and never changes that
  polarity.
- Provider insertion, removal and width changes rearm the finite region without
  blurring outside the 40-pixel panel surface.
- Build, QML lint, focused surface tests, the architecture guard and the
  canonical production exit pass before delivery.
- Scale 1 and scale 2 author screenshots are recorded separately as
  `VAL-PANEL-1`; an automated smoke never claims the visual pass.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| PANEL-1-A | `celestina:` | done | [inventory](../../inventories/2026-08-08-panel-glass-redesign/PANEL-1-A.numstat.tsv) | 42 files, +1344/-472 | Replace the hard panel plate with a soft shadow and borderless real compositor-glass capsules, reduce workspace and status readings to positional colour/icon semantics without discarding monitor grouping or CPU/memory values, expose the existing overlays, and keep the tray populated and visible across host restarts | [evidence](../../evidence/2026-08-08-panel-glass-baseline.md) | `VAL-PANEL-1` partial |
| PANEL-1-I | `celestina:` | planned | exact panel and contextual-surface paths selected by the next author design pass, declared before editing | — | Continue the still-open PANEL-1 visual design from the delivered prototype snapshot without reopening retired adaptive contrast or changing provider semantics | canonical Celestina completion plus focused interaction and nested-session evidence | `VAL-PANEL-1` |
| PANEL-1-B | `celestina:` | done | [inventory](../../inventories/2026-08-08-panel-glass-redesign/PANEL-1-B.numstat.tsv) | 124 files, +15180/-1843 | Deliver the current contextual shell prototype snapshot with grouped panel controls, complete menu hierarchy, durable tray and wallpaper tools, real composed-scene blur, canonical dense content glass, nearly transparent carriers and fixed light/white foregrounds | [contextual hierarchy evidence](../../evidence/2026-08-10-contextual-menu-hierarchy-nested.md); [shared glass evidence](../../evidence/2026-08-11-contextual-menu-shared-glass.md); [content glass evidence](../../evidence/2026-08-11-one-ui-content-glass.md); [fixed ink evidence](../../evidence/2026-08-11-fixed-white-shell-ink.md) | `VAL-PANEL-1` partial prototype |

## Active unit boundary

`PANEL-1-B` is the cumulative delivery boundary for the previously uncommitted
B-H prototype sequence. The C-E labels preserve tray/gallery, interaction and
non-xray compositor corrections; F-G preserve the shared-material iterations;
H records removal of the superseded adaptive-foreground path. Those labels
remain dated prototype chronology, not separate published versions or delivery
units. The final snapshot fixes foreground ink to the light/white instance and
content cards and panel capsules to the matching dark material, with no shadow
or second compositor region. Wallpaper inventory, selection, import and image
revision semantics remain in scope and unchanged. `PANEL-1-I` keeps the design
checkpoint open and gains concrete paths before the next authorized pass. The
first opener-morph experiment was
rejected: separate Wayland surfaces did not read as one transformed object.
The author selected a second falsifiable prototype instead. The network menu
and control centre receive the real global rectangle of their panel opener,
open immediately beneath and near it, and scale up in place. Their outer field
is shadow only, with no hard card body or edge. The first live view showed that
starting that shadow below the panel left two unrelated falloffs and that
informational and current-state rows looked unfinished beside the one action
pill. The revised field therefore extends the same shadow back through the
real opener rectangle, while every content line and coherent content group uses
the same finite compositor-blur pill anatomy as the panel.
The next live view exposed two more failures in that field: an analytic shadow
without an opaque card still paints its interior, so it darkened both the panel
controls it crossed and the menu content it surrounded; the panel pill's
bar-specific horizontal overhang also escaped the intended menu field. The
field must therefore be a hollow perimeter falloff whose sides alone connect
to the opener, menu pills must opt out of the panel overhang, and the panel's
shadow/fallback glass density must remain visible but no longer read as an
opaque strip.
The resulting live view also failed: moving the whole falloff outside made the
menu lose its organizing field, while the nested compositor's `saturation 1.2`
turned transparent pills into bright wallpaper-coloured bars. The next
revision centres each falloff on the field boundary but consumes only the safe
content margin, reaching zero exactly where a pill starts. It restores a
moderate panel shadow, keeps the successful blur path transparent, uses a
readable fallback, and desaturates only the registered nested reference blur
profile rather than covering compositor glass with tint.
That revision failed for a more fundamental reason: a perimeter that becomes
clear before the content starts leaves wallpaper between and beneath the rows,
so the menu has no visual field at all. It also exposed a false calendar pill:
`PanelPill` has the panel's fixed row height, so placing it in a tall calendar
container drew one arbitrary capsule through the middle week. The next revision
uses one continuous rounded shadow as the bottommost menu layer, clips its top
spill so the separate menu surface cannot paint over panel controls, leaves all
finite glass pills above that field, and removes the calendar's accidental pill.
The calendar remains structured content inside the shared shadow rather than a
row-sized capsule.
The author rejected the remaining overlap rather than asking for another
cross-surface illusion. The accepted next comparison is deliberately simpler:
the menu is a standalone object with enough opener-relative vertical clearance
for its complete top shadow to remain below the panel, and no visual connector
is attempted. Its rows retain finite pill regions, while the calendar receives
one full-height rounded compositor-glass card instead of either no glass or a
row-height pill.
The next live view showed that using only the opener rectangle still positioned
the control centre inside the panel's taller shadow surface, while the menu's
bottom falloff lacked explicit render room. It also showed that a large card
needs a bounded card radius rather than the panel pill's silhouette, and that a
single full-width column wastes the available width. The correction therefore
uses the real panel surface bottom as the vertical floor, reserves the complete
shadow bounds, pairs compatible controls and readings, and composes weather
with the calendar in one full-height card.
The author accepted that placement and rejected the remaining independent-pill
anatomy. The next comparison publishes exactly one light compositor-blur region
for the complete menu, renders rows and the calendar/weather group as denser
internal glass sections, and replaces `RectangularShadow` with explicit
top/side/bottom exterior falloffs. The bottom falloff has its own bounded item
below the card, so it cannot depend on undocumented effect padding.
The author then selected the local `Velo A` comparison as the implementation
target for the next live view. It keeps the complete exterior shadow, one
continuous low-density glass body and three bounded, denser internal groups:
quick controls, connectivity, and weather/calendar. A group may contain several
rows and peer columns; no ordinary row receives its own resting pill. Hover,
focus, pressed and selected state remain local state layers above those groups.
The connectivity menu applies the same hierarchy as one grouped list inside its
single outer glass body. These are shell-local prototype values rather than new
shared-style API: acceptance or rejection in the live compositor comes before
any token is proposed for the suite.
The first QML translation of `Velo A` was rejected. It retained the compact
420-pixel legacy centre, omitted the prototype's icon and divider rhythm, and
approximated a rounded drop shadow with four independent gradient rectangles.
The result was a compressed control stack inside an irregular dark field, not
the selected 530-by-732 composition. The next live candidate therefore maps the
prototype's actual outer size and four sections, gives its rows the intended
icon/text/control columns, and uses one rounded shadow source below the complete
card. It preserves every existing control and provider reading; the visual
geometry is corrected rather than the feature set being reduced to the static
mockup.
The corrected Velo composition exposed the next shared interaction defect: the
card cleared the panel window's complete 112-pixel shadow canvas instead of the
40-pixel visible bar, and only the control-centre path carried its opener into
the overlay controller. The author expanded this corrective slice to one
visible-bar-relative placement contract for every existing panel-opened
connectivity or overlay surface. A command or keybind still has no opener and
therefore remains centred. The same slice gives every menu-opening panel
control an observable pressed state; secondary-click workspace and tray menu
routes retain their existing action semantics but no longer press invisibly.
The prototype may add the shell-local presentation components and narrow host
plumbing needed to publish those regions. Existing provider commands, focus,
Escape, outside-click dismissal and reduced-motion semantics do not change.

After the network and control-centre comparison established the intended
material hierarchy, the author explicitly expanded this unit on 2026-08-10 to
all existing interactive shell menus. The visual field is therefore shared by
network, Bluetooth and tray while preserving their real `Menu` lifecycle, and
by workspace map, control centre, clipboard, notification centre, session and
launcher while preserving each overlay's existing focus and dismissal model.
The session surface must additionally keep its full-output window size stable
when confirmation or outcome copy changes its card height; otherwise placement
is recomputed against a card-sized window and the card jumps to the panel.
Launcher has no panel opener and remains centred. Toasts, OSD, the standalone
output-sharing chooser, clock/weather feature work, provider behaviour,
shared-style API work and the unrelated wallpaper-provider edit already present
in the worktree remain excluded.

The 2026-08-10 control-centre crop then rejected the material density rather
than the composition: the analytic shadow still painted its dark interior, a
dark tint above the compositor blur made the outer field still denser, and a
second dark tint made its sections read as darker cards. The next bounded
revision subtracts the rounded body from that shadow so only its exterior
falloff remains, keeps the no-blur readability fallback, and uses only a very
light neutral wash when compositor blur is available. Internal sections use
the same neutral material at a modestly higher density. The wallpaper must
remain plainly visible through both levels, with hierarchy from relative
density rather than a dark colour cast.

The following comparison rejected the remaining exterior shadow as well as the
full-width panel shadow, and still found both the material and compositor blur
too dense. The next revision therefore removes every menu halo, collapses all
shadow-only geometry, reduces the panel window from its former 112-pixel scrim
canvas to the real 40-pixel bar, and uses nearly transparent neutral body and
section washes. The section stays only modestly denser than its shared body.
The registered nested Niri profile is reduced to a slight global blur for the
next controlled comparison; no live compositor configuration is changed.

The dark `wallhaven-n6qzmx.png` comparison then confirmed that light ink is the
right foreground there, while the previous bright wallpaper made the same ink
difficult to read. The author expanded this unit on 2026-08-10 to calculate
foreground choice from measured wallpaper exposure and contrast. The next
revision therefore adds a bounded, deterministic per-output analysis in the
wallpaper worker, publishes it through an additive provider contract, and maps
that state to a shell-local light/dark foreground palette for the panel and all
interactive menu surfaces. The analysis must not block Qt, a loading or failed
wallpaper keeps the light ink paired with the existing dark fallback, and two
outputs may hold opposite foregrounds simultaneously. Mixed wallpapers remain
an explicit limitation: a single ink cannot guarantee 4.5:1 over simultaneous
black and white detail, so automated evidence reports uncertainty separately
from the author's real-compositor comparison.

The implemented correlation is exact rather than path-only: output, source,
file revision, monotonic inventory generation and crop geometry must all match
the QML request that reached `Image.Ready`. Replacing a file in place therefore
reloads it before its new tone is exposed, and an output change during decode
discards the stale result. Every foreground supplied by the adaptive palette is
one of the two measured candidates. An uncertain result keeps that candidate at
full strength and changes only the colour of the existing low-opacity local
veil; it adds no shadow, opacity or blur.

The implemented comparison now passes that bounded transition in the nested
compositor: the dark requested wallpaper keeps light ink, the bright previous
wallpaper changes the panel and an already-open control centre to dark ink, and
restoring the dark wallpaper changes both back without restarting the host. The
temporary bright-wallpaper selector was removed and the nested session remains
on the requested dark image. This is implementation evidence, not the author's
acceptance of every menu, scale 2, a two-output session or the known mixed-region
limitation; the comparison remains historical prototype evidence rather than
the final foreground contract.

After accepting that adaptive foreground comparison, the author explicitly
selected the control centre's hierarchy and rhythm as the reference for every
contextual shell surface. The next revision therefore gives network, Bluetooth,
tray, workspace map, clipboard, notification, session and launcher surfaces the
same deliberate header, grouped-section, icon/text/action-column and state
hierarchy while letting each retain the dimensions and information density its
content needs. This is shared anatomy, not identical layout: real Qt `Menu`
carriers remain real menus, focused overlays remain focused overlays, and no
provider command, model identity, keyboard route, focus restoration, Escape,
outside-click or reduced-motion contract changes.

The author then extended that same active unit with three bounded panel/menu
corrections. Network, Bluetooth and both tray menu levels receive an explicit
header-to-body gap, token-sized inter-row spacing and vertical padding inside
their existing ordered entries without changing provider-owned rows or the real
`Menu` lifecycle. Launcher gains a permanent panel opener and uses the
overlay's existing opener-relative path; command and keybind launches still
have no opener and remain centred. The tray no longer expands its complete item
inventory inside the bar: its compact indicator opens one contextual list on
the shared panel-menu surface, while an individual StatusNotifierItem's own
D-Bus menu remains a distinct second contextual menu. Primary, secondary and
item-menu activation retain the exact service/path identities and the existing
asynchronous tray ownership boundary.

The author then extended `PANEL-1-B` with one bounded panel-composition pass.
Network and Bluetooth share one compact connectivity capsule while the power
profile leaves the bar and remains available in the control centre; audio,
microphone and brightness share
one audio/display capsule; and notification, launcher, control centre,
clipboard, session and performance openers share one utilities capsule.
Performance becomes one icon whose contextual menu presents the live CPU and
memory readings formerly rendered inline and retains the existing system
monitor action. Capture moves into the vacated left-flank position and becomes
an anchored contextual menu whose first tool is the existing screenshot
request, leaving one ordered surface for future capture tools. Each capsule
owns exactly one compositor region, every opener retains its accessible name
and real geometry, and the existing provider and Niri verbs remain unchanged.

The final nested cycle also exposed one bounded lifecycle defect in the
existing blur controller: after a successful arm, the Niri layer-shell window
can continue rendering while Qt reports it as not exposed. `PANEL-1-B` therefore
preserves the confirmed blur arm across that state only while the window stays
visible and sized, the compositor effect remains available and the glass region
remains non-empty. A real effect, geometry or region loss still takes the
existing fallback path, and a focused regression owns this distinction.

The author finally expanded the same active panel/menu unit with four related
interaction corrections. Capture is presented as a toolbox rather than as one
scissors action, and a per-output wallpaper menu beside it opens the session's
standard folder chooser instead of embedding any source path in QML or C++. The
tray opener loses its count and uses a distinct inventory glyph; bounded
durable preferences let each published item be pinned beside that opener or
hidden and later restored from the inventory. Hidden state removes a pin so the
bar never contradicts the menu. The inventory remains mapped while a chosen
StatusNotifierItem's foreign D-Bus menu is open in a separate child carrier;
closing or choosing in the child affects only the child, while retiring the
parent still closes the complete hierarchy. The exact live service/path stays
the action identity, and the stable published item id is used only for the
shell's presentation preference.

The first implementation of that extension was rejected in the nested live
view. The nominal toolbox glyph was actually Lucide's briefcase; the tray glyph
filled the complete 30-pixel target; an unresolved StatusNotifierItem icon
widened the bar with its title; and every hidden row stayed visible under an
always-expanded section, whose added geometry could move the popup over the
panel. The wallpaper action also chose one file rather than the requested
folder-backed gallery. The corrective revision uses the literal Lucide tool
case, limits the inventory glyph to the canonical icon size, keeps foreign
fallbacks icon-only, folds hidden rows behind one explicit show/hide control and
keeps the menu's panel-relative top stable while its model changes. Wallpaper
selection becomes a durable folder plus bounded thumbnail inventory; only a
thumbnail click sends the existing per-output selection request.

The following nested comparison rejected that hidden-row disclosure and the
tray's text-list anatomy. Expanding or collapsing rows still changes the
carrier's measured body abruptly and can clip it against the output edge;
producer titles also expose unstable identifiers such as a status-icon object
name instead of helping recognition. The corrective slice keeps one stable
inventory card and switches its body between visible and hidden modes from a
control beside the `Aplicaciones` heading. Both modes use the same bounded icon
grid: the producer title remains only as accessible fallback copy, while each
tile exposes the existing pin and hide/restore actions without changing the
service/path action identity. Empty modes preserve the same card geometry,
foreign child menus remain adjacent to the parent inventory, and both the panel
opener and inventory heading use one explicit tray semantic glyph. Missing
foreign artwork is resolved from published icon identity or the installed
desktop catalogue without application-name special cases. The wallpaper
gallery likewise stops presenting its per-page safety bound as a terminal
"limited gallery": every accepted image in the bounded directory catalogue
must remain reachable, with honest count or paging copy when more than one
payload page is required.

The subsequent tray review found three bounded interaction defects in that
corrective surface. The visible/hidden switch still spent horizontal room on
copy that repeated the section state, foreign artwork remained undersized
inside the fixed icon tiles, and an overflowing application-owned D-Bus menu
had no visible or pointer-operable scroll route. The shell also still exposed
hover tooltips above compact controls even though those controls already carry
complete assistive names. `PANEL-1-D` therefore uses eye glyphs for the two
inventory modes while retaining their names, counts, selected state and
keyboard order for accessibility; enlarges only the application artwork inside
the unchanged grid targets; suppresses tooltip painting at the shell-local
button boundary without removing hover feedback or `helpText`; and caps a
foreign tray menu to the logical output space remaining below its real request
so the Menu content item becomes a scrollable viewport without losing its
anchor. The viewport keeps pointer, arrow-key, Escape and
parent/child carrier semantics, and its scroll affordance must never become an
extra menu entry.

The next live comparison identified a compositor-policy error rather than a
QML sampling defect. Niri 26.04 automatically enables xray whenever any
background effect is active; xray deliberately ignores ordinary windows below
a layer surface and reuses one wallpaper-only blur cache. The
`ext-background-effect` protocol lets Celestina publish its finite region but
has no request that can disable xray. `PANEL-1-E` therefore adds an exact Niri
layer rule for the panel, primary menu, tray-child menu and interactive overlay
namespaces, with `xray false` and no forced blur. Wallpaper, toast and OSD
surfaces remain outside that rule, and the repository documents the same
opt-in block without editing the author's live Niri configuration. The nested
control must place one uniform application window below a launcher that crosses
its edge: the glass over the application must follow that colour while the
remainder still follows the wallpaper. That split must survive closing and
reopening the launcher and a Celestina-only restart while the same nested Niri
keeps running. Niri documents non-xray effects as more expensive and
experimental, so animation/drag limitations and contrast over arbitrary
application content remain explicit real-session checks.

This bounded migration is evidence for the still-open shell visual discussion,
not an accepted suite-wide UX-2 language. The current state is delivered as a
prototype snapshot; acceptance still requires a later decision before the
anatomy becomes shared style policy.

The author then made the material boundary explicit: contextual content uses
the existing suite glass from CelestinaStyle. An in-scene `ShaderEffectSource`
cannot sample another Wayland client, so `PANEL-1-F` does not replace the
compositor backend or multiply blur regions. Instead `STYLE-G7-H` supplies an
external-backdrop mode for the same `GlassSurface` renderer; the outer veil and
every `MenuSection` use that mode for tint, noise, outline and lit edge while
the carrier retains one finite KWindowEffects region and the shell's adaptive
ink input.

The first shared-material comparison still made the contextual carrier and its
content cards read as variations of one bluish pane. The author supplied a
current One UI 8.5 reference and narrowed the correction to two surface types:
content cards and panel capsules use one denser matte glass, while the outer
contextual carrier remains nearly transparent. The content tint stays paired
with the already measured per-output ink polarity, so a bright backdrop does
not receive dark ink over a newly darkened card. No other suite glass consumer
adopts these opt-in roles.

On 2026-08-11 the author retracted adaptive foreground selection. `PANEL-1-H`
therefore removes the bounded contrast computation, additive appearance
publication, host adapter and QML polarity inputs instead of merely ignoring
their result. The shell keeps light/white ink and the dark content material on
every wallpaper; only the contextual carrier remains nearly transparent. The
gallery, user-selected directory, per-output wallpaper identity and atomic
import contract are unaffected.

The final B-H bytes land together as `PANEL-1-B` because no intermediate
prototype was committed and the same QML, host and documentation files changed
through several comparisons. One cumulative inventory therefore describes the
real final tree without inventing overlapping immutable units. `PANEL-1-I`
continues the design milestone after this snapshot.
