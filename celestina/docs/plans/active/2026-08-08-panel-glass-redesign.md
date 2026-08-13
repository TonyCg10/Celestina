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
one nearly transparent, edge-to-edge compositor veil can disperse the scene
while ordinary inset content capsules supply hierarchy without their own blur
regions or a full-width shadow. The blur remains visible only if the QML drawn
above it stops covering it with opaque tint, strokes or exterior halos, and it
represents the real desktop only when the compositor does not replace that
scene with a wallpaper-only xray cache.

## Tangible outcome

The panel has no hard full-width background or shadow. One nearly transparent
`ContextualVeil` reaches edge-to-edge with no outer margin and owns one finite
compositor-blur region across the complete 40-pixel bar. Its content sits on
ordinary rounded `ContentSurface` capsules inset at output-local y=5 with
height 30. Those capsules use the same dense dark matte material and fixed
light/white foreground as contextual content cards, but they paint no
compositor region of their own. The veil suppresses outline and lit-edge layers
for both rounded and shaped paths, so no apparent border or edge halo surrounds
the bar, contextual body or connector.

A primary menu or overlay opened by a real panel control receives both the
clicked control and its exact icon anchor, plus a separate
`attachmentStartY == barHeight`. The control continues to place the body. A
shaped membrane made only from the nearly transparent `ContextualVeil` starts
at the panel's lower edge as one narrow droplet mouth centred or
flat-span-clamped beneath the icon, hangs through its neck and swells until
it lands tangent on the body's top edge without repainting or reblurring the
bar. The invoking control keeps its normal hover circle while that surface is
open. The dense panel capsule and every content card remain ordinary, rounded
and geometrically unchanged. Routes without both real rectangles remain
floating rounded surfaces.

## Scope

- The panel surface, shadow removal, edge-to-edge contextual veil and content
  grouping.
- One local ordinary rounded capsule component with no compositor region of its
  own and its readable no-blur fallback.
- One finite panel compositor blur region, plus finite contextual-surface
  regions, dynamic geometry updates, withdrawal and protocol commit/flush
  behaviour.
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
  impossible cross-client QML capture, plus one opt-in vector silhouette for
  the bar-edge membrane. Style owns the halo-free veil role, generic silhouette
  renderer and bounded vertical travel; the shell retains KWindowEffects, both
  real rectangles, tension geometry, the continuous panel region, the
  membrane's sampled finite polygon, fixed light/white ink and fallback
  ownership.
- Exact opener and glyph-anchor propagation for panel-opened primary menus and
  overlays. Command
  and keybind routes, point-opened workspace menus and foreign child menus stay
  floating and retain their established placement and lifecycle. An attached
  overlay aligns its body to the real clicked control but begins its membrane
  at `attachmentStartY == barHeight` as one narrow droplet mouth targeted by
  the exact glyph anchor, and lands it tangent on the body's flat top edge
  inside its ordinary rounded corners. It never republishes or reblurs the
  panel; the shell derives the icon-proportional neck width from tension over
  travel, icon/body reference scales and centre
  displacement, with the glyph centring the mouth inside the body's flat span.
  Model-driven content height remains irrelevant. The opener
  keeps its ordinary hover circle only while its surface is open; its
  `PanelPill` and every dense content surface remain geometrically and
  materially unchanged.
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
- Any shared-style change beyond the exact status glyphs, the demonstrated
  external-backdrop `GlassSurface` mode and its opt-in edge silhouette consumed
  by this shell; the fixed foreground mapping remains shell-local and derives
  from existing semantic theme roles.
- Editing the author's live Niri configuration or replacing the live shell.
- Reachable overflow for a tall card on a 768-pixel output. This prototype
  preserves the connector and keeps its blur disjoint from the panel, so the
  output clips the Control Centre's last 36 pixels instead of moving it upward;
  a scrollable low-height anatomy is a later unit if the author requires it.
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
10. Replace the shaped panel-pill segments with one edge-to-edge
    `ContextualVeil` backdrop and exactly one finite panel region. Keep each
    information group as an ordinary rounded `ContentSurface` capsule at y=5
    with height 30 and no region of its own. Propagate the clicked control and
    its exact glyph anchor separately. Derive one `ContextualVeil` membrane's
    matched fill and sampled polygon from `attachmentStartY == barHeight`, the
    icon/body reference scales and a narrow droplet mouth centred or
    flat-span-clamped beneath the glyph. Keep the seam contact to that mouth
    alone, land the swell tangent on the body's flat top edge inside its
    ordinary rounded corners, and retain the invoking control's hover circle
    for the lifetime of its own surface.
    Keep every capsule and dense content surface on its ordinary rounded path,
    suppress contextual edge treatment that reads as an exterior shadow, and
    retain floating geometry for command, keybind, workspace and foreign child
    routes.
11. Give the settled droplet its motion. One bounded progress value drives the
    same geometry source, so an opening attached surface grows out of its seam
    instead of appearing at full size: the body emerges from the mouth, its
    extent and lateral span open together, and the neck thins under flight
    tension before relaxing to its resting width. The mouth stays welded to
    the seam and the neck keeps a hard floor at every frame, so the drop is
    always under tension and never pinches off. Content reveals only as the
    body arrives, reduced motion resolves the settled geometry immediately,
    and no floating route, compositor-region, placement or lease contract
    changes.

## Implementation exit

- A busy composed backdrop loses recognizable detail throughout the one
  edge-to-edge panel region while staying sharp immediately below the 40-pixel
  bar; application colour remains represented where a window lies below the
  glass and wallpaper remains represented where it does not.
- The bar and contextual carrier add no hard plate, outer margin, shadow or
  edge halo. Every information capsule
  is an ordinary rounded `ContentSurface` at y=5 with height 30, has no
  compositor region of its own and retains its complete rounded ends.
- Panel capsules and contextual content cards keep the fixed light/white
  foreground over one dense dark matte material on every wallpaper; the
  contextual carrier remains nearly transparent and never changes that
  polarity.
- Provider insertion, removal and width changes preserve the single finite
  panel region without blurring outside the 40-pixel panel surface.
- The contextual membrane's painted silhouette and finite compositor polygon
  derive from the same geometry and begin at `barHeight`. Its only seam
  contact is the narrow droplet mouth centred beneath the exact clicked glyph
  and clamped inside the body's flat top span; its icon-proportional neck
  thins monotonically as travel, reference-scale difference
  or centre displacement increases, and its swell lands tangent on the body's
  top edge inside ordinary rounded corners. The clicked control remains the placement
  authority and keeps its hover circle while its own surface remains open; its
  capsule and every dense content card remain unchanged. The membrane is only
  `ContextualVeil`, with no
  dense bridge or added blur region. The overlay does not repaint or reblur the
  bar, and routes without both a real panel opener and glyph anchor remain
  floating.
- Attached, floating and reduced-motion routes retain their established reveal
  contracts.
- Focus, Escape, outside-click, provider-command, destructive-confirmation and
  parent/foreign-child semantics remain unchanged.
- Build, QML lint, focused surface tests, the architecture guard and the
  canonical production exit pass before delivery.
- Scale 1 and scale 2 author screenshots are recorded separately as
  `VAL-PANEL-1`; an automated smoke never claims the visual pass.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| PANEL-1-A | `celestina:` | done | [inventory](../../inventories/2026-08-08-panel-glass-redesign/PANEL-1-A.numstat.tsv) | 42 files, +1344/-472 | Replace the hard panel plate with a soft shadow and borderless real compositor-glass capsules, reduce workspace and status readings to positional colour/icon semantics without discarding monitor grouping or CPU/memory values, expose the existing overlays, and keep the tray populated and visible across host restarts | [evidence](../../evidence/2026-08-08-panel-glass-baseline.md) | `VAL-PANEL-1` partial |
| PANEL-1-I | `celestina:` | active | `CMakeLists.txt`; `qml/EdgeAttachedGeometry.js`; `qml/Panel.qml`; `qml/PanelFlank.qml`; `qml/PanelPill.qml`; `qml/PanelCluster.qml`; `qml/PanelActionButton.qml`; `qml/PanelMenuButton.qml`; panel opener controls; `qml/PanelPopupPlacement.qml`; `qml/CompositorGlassRegion.qml`; `qml/SoftMenuField.qml`; `qml/SoftMenu.qml`; `qml/SoftOverlayCard.qml`; `qml/AnchoredCard.qml`; panel-opened overlay/menu composition; `src/panelattachmentlease.{h,cpp}`; `src/panelmanager.{h,cpp}`; `src/panelblurcontroller.{h,cpp}`; `src/panelmenucontroller.{h,cpp}`; `src/overlaycontroller.{h,cpp}`; focused QML/C++ tests; Celestina version/status/roadmap/validation/evidence and the root version history | — | Replace the narrow central connector with one tension-shaped `ContextualVeil` membrane that spans the complete contextual width at both the panel edge and menu landing, narrows through a fluid body-proportional waist whose centre follows the exact clicked icon without collapsing to icon width, and keeps that opener's hover circle visible only while its own surface remains open; require matched tangent magnitude and direction through the waist so the short 20..36-pixel travel reads as one rounded liquid deformation rather than straight hourglass flanks, keep the clicked control rectangle separate for placement and leave the owning panel capsule and all dense content cards completely unchanged while preserving disjoint panel/menu blur regions, one finite compositor region per surface and every floating-route, provider, focus, Escape, outside-click and reduced-motion contract | [continuous bar veil and membrane evidence](../../evidence/2026-08-11-edge-attached-shell-prototype.md) | `VAL-PANEL-1` |
| PANEL-1-O | `celestina:` | active | `src/shellscale.{h,cpp}`; `tests/shellscale_test.cpp` | — | Correct the per-output scale to derive from a monitor's physical diagonal rather than its density, after the author's own three monitors showed density cannot separate two of them, floor it at the reference so a smaller screen is never shrunk, and refuse the density Qt fabricates when a compositor publishes no physical size at all | [per-output sizing and raster fidelity evidence](../../evidence/2026-08-12-output-sizing-and-raster-fidelity.md) | `VAL-PANEL-1` |
| PANEL-1-N | `celestina:` | active | `qml/EdgeAttachedGeometry.js`; `qml/Panel.qml`; `qml/AnchoredCard.qml`; `qml/SoftMenuField.qml`; the five overlay roots; `src/shellscale.{h,cpp}`; `src/panelmenucontroller.cpp`; `src/overlaycontroller.cpp`; `CMakeLists.txt`; focused C++ tests | — | Draw contextual surfaces at their output's size too, not only the panel: scale each menu and overlay scene by the same per-output factor and divide the geometry they are handed by it, publish blur regions from mapped bounds so a scaled surface stops asking for a region smaller than it paints, and let an author name the factor when a density cannot answer for them | [per-output sizing and raster fidelity evidence](../../evidence/2026-08-12-output-sizing-and-raster-fidelity.md) | `VAL-PANEL-1` |
| PANEL-1-M | `celestina:` | active | `src/provider_adapter/brightness.rs`; `scripts/smoke-production.sh`; `VALIDATION.md` | — | Stop the canonical verification workflow from reaching the graphics card: gate DDC behind `CELESTINA_DDC` so an automated run starts, registers and publishes exactly as a session does while opening no I²C bus, and set that gate in the release smoke, whose purpose was only ever to prove the host and compiled module load | [per-output sizing and raster fidelity evidence](../../evidence/2026-08-12-output-sizing-and-raster-fidelity.md) | `VAL-GPU-01` |
| PANEL-1-L | `celestina:` | active | `src/shellscale.{h,cpp}`; `src/panelmanager.cpp`; `src/traywatcher.cpp`; `qml/Panel.qml`; `qml/PanelPill.qml`; raster-icon consumers; `qml/PerformanceMenu.qml`; `qml/NetworkMenu.qml`; `qml/BluetoothMenu.qml`; `CMakeLists.txt`; focused C++/QML tests; Celestina status/roadmap/validation/evidence | — | Make what the shell draws the same physical size on every output and stop it degrading what it draws: derive one bounded per-output scale from the output's real density and apply it as a scene scale so no token or layout number moves, rasterize a tray icon once at a size that survives any scale, ask for every raster at the density it will be drawn at, thicken glyph strokes and panel reading weights, and stop three provider-driven menus rebuilding their complete row list on every reading tick | [per-output sizing and raster fidelity evidence](../../evidence/2026-08-12-output-sizing-and-raster-fidelity.md) | `VAL-PANEL-1` |
| PANEL-1-K | `celestina:` | active | `qml/EdgeAttachedGeometry.js`; `qml/PanelPill.qml`; `qml/PanelCluster.qml`; `qml/PanelActionButton.qml`; `qml/PhoneStatus.qml`; `qml/Panel.qml` | — | Weld the panel's reading capsules to the screen's top edge instead of floating them inside the bar, with the centred clock held by a visibly elastic skin and every flanked capsule keeping straight sides so no neighbour is overlapped and no gap on the bar is widened | [droplet fall evidence](../../evidence/2026-08-12-droplet-tension-fall.md) | `VAL-PANEL-1` |
| PANEL-1-J | `celestina:` | active | `qml/EdgeAttachedGeometry.js`; `qml/SoftMenuField.qml`; focused QML/C++ attachment tests; Celestina status/roadmap/validation/evidence | — | Give the settled droplet its opening motion from one bounded progress value on the same geometry source: the body emerges from its seam mouth in extent and lateral span together while the neck thins under flight tension and relaxes to its resting width, the mouth stays welded to the seam and the neck keeps a hard floor so the drop never pinches off, content reveals only as the body arrives, and reduced motion resolves the settled geometry immediately without changing any floating-route, compositor-region, placement, lease or dismissal contract | [droplet fall evidence](../../evidence/2026-08-12-droplet-tension-fall.md) | `VAL-PANEL-1` |
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

## PANEL-1-I boundary

The current author-requested revision replaces the fluid body-wide waist
membrane, not the continuous bar composition. On 2026-08-11 the author
rejected that revision's live read as a strange hourglass and asked for a
soft drop falling out of the bar. A panel request still carries two explicit
rectangles: the clicked control remains the placement and interaction opener,
while the exact glyph inside it centres the droplet's narrow mouth on the bar
seam. The membrane no longer spans the body at the seam at all; one
shell-local tension calculation over anchor width, body width, connector
travel and horizontal displacement thins only the hanging neck. One geometry
source emits both the painted path and the sampled compositor polygon.

The calculation is falsifiable. Let `i` be `max(1, anchorWidth)`, the icon
reference scale, and `b` be `max(1, bodyWidth)`, the body reference scale.
Normalized stretch is `travel / sqrt(i * b)`, spread is
`log(max(i, b) / min(i, b)) / log(48)`, and displacement is
`abs(bodyCenter - anchorCenter) / b`, each clamped to 0..1. Tension is
`clamp(0.42 * stretch + 0.43 * spread + 0.35 * displacement, 0, 1)`.
The neck width is icon-proportional — `clamp(anchorWidth * lerp(2.3, 1.7,
tension), 22, 48)`, additionally bounded by one third of the body — so it can
neither collapse to an icon-thin thread nor grow into a body-proportional
band. The mouth adds a travel-proportional meniscus flare, half the travel
clamped to 8..18 pixels, on each side, and its centre is clamped so the
complete mouth stays inside the body's flat top span between the rounded
corners; within that span it equals the glyph centre exactly. The neck sits
at 34 percent of the travel, leaving the longer swelling lobe below. The
landing run spreads `clamp(bodyWidth * 0.18 + travel * 1.5, 48, 140)` pixels
beyond the neck on each side. Each side uses two cubics: the meniscus holds a
horizontal tangent at the seam and a vertical tangent at the neck, and the
swell holds that vertical tangent at the neck and a horizontal tangent at the
body landing, so the outline is G1-continuous from the bar to the body with
no pinched corner.

The membrane is `ContextualVeil` from its narrow glyph-centred mouth through
the hanging neck to its tangent landing on the body's flat top edge; the body
keeps its ordinary rounded top corners outside the swell. It contains no
`ContentSurface`
contribution, dense-to-veil gradient, shadow or edge decoration. Every
`PanelPill` and `ContentSurface` remains on its ordinary rounded path with the
same bounds and material before, during and after the opening. The menu region
begins at `barHeight`, so panel and menu blur remain disjoint without changing
or extending a panel capsule.

The 0.12.0 milestone prototype supersedes the earlier top-edge droplet and
narrow central-connector experiments within this same active and uncommitted
unit. It also supersedes the immediately preceding whole-capsule iteration,
which opened the owning `PanelPill` to the panel edge and painted a dense-to-
veil bridge. That rejected geometry remains chronology rather than current
instruction. The current composition gives the complete panel one marginless,
nearly transparent `ContextualVeil` backdrop and one finite compositor region.
Panel information groups remain ordinary rounded `ContentSurface` capsules at
output-local y=5 with height 30 and add no compositor region. The veil
suppresses outline and lit-edge layers on the membrane, so the bar, menu body
and membrane expose no apparent border or edge halo. CelestinaStyle owns the
semantic material, generic opt-in silhouette renderer and bounded vertical
travel, while Celestina owns real anchor/body geometry, tension, placement and
compositor regions.

Panel controls publish exact clicked-opener and glyph-anchor rectangles. A
panel-opened primary surface places its body from the former and starts its
membrane at `attachmentStartY == barHeight`. Only the narrow mouth touches
that seam; the glyph centres it, and its neck width follows the tension
calculation above. Content height cannot change this
geometry after construction. While the surface remains open, the attachment
tracker follows the real glyph and its visual ancestors
through the panel's global coordinate space, then publishes only its output-
local rectangle to the contextual surface. Tray pin/hide changes and provider-
driven flank movement therefore reposition the mouth instead of leaving it at
a stale opening snapshot. The exact opener also keeps its ordinary hover-circle
fill while that surface owns the active lease; dismissal, replacement, failed
construction or source destruction clears it without selecting the surrounding
capsule. It neither mutates the panel material nor repaints or reblurs the bar.
Command and keybind routes
deliberately remain floating. The workspace strip's dots and the collapsed
monitor dot publish the same semantic attachment-source contract as
`PanelMenuButton`; a right click transports their control and dot rectangles
and the workspace map attaches with the same droplet and live lease as every
indicator menu. The collapsed monitor group is one dot, larger than the
workspace dots, with its count only in the accessible name. A foreign tray child menu born from a row of
the mapped inventory attaches the same droplet sideways: the host widens the
child's card-sized surface with the width-proportional membrane strip,
places it flush against the parent card so the seam coincides with the
parent's edge, and passes the invoking tile's complete rectangle; the mouth
then follows that tile on whichever edge faces the parent. The foreign
menu's header card and section label are pinned beside its viewport, the
raised top padding and clipped ListView keep scrolled rows inside the dark
body section, and the separate scroll bar is removed. No provider, action,
focus, Escape, outside-click, reduced-motion or parent/child menu contract
changes with that composition.

The preceding top-edge droplet-pill, fixed-anatomy, whole-capsule,
icon-scaled body-wide and fluid body-wide experiments' focused and production
results remain recorded as superseded history. They do not verify the narrow
glyph-centred mouth, meniscus, tangent body landing, restored rounded
body-top corners, persistent opener feedback, immutable capsules or veil-only
membrane. The revised contract remains in the same active unit. Its focused
selection passes 4/4 and its offscreen QuickTest runner passes 211/211.
Registered production completion passes CTest 17/17 and the eight-second
release smoke, and the verified bundle is deployed to `~/.local` and reports
current without session activation.

Those bytes shipped in `a97eb55` as celestina 0.12.0, together with the
sideways child membrane, the attached workspace map, the single collapsed
monitor dot and the contained foreign-menu scroll. No immutable inventory was
taken at that delivery, and an inventory's base revision must be the head
before its own commit, so one cannot be written for it afterwards. The row
therefore stays `active` rather than claiming a closure it cannot evidence:
the version history and the linked evidence are what record that delivery.
Nested-Niri scale validation also remains pending.

## PANEL-1-J boundary

`PANEL-1-I` settled the droplet's resting shape. The author accepted it and
asked for its motion next: a contextual surface must fall out of the place it
is born from, reading as a drop under tension that stretches but never
detaches. `PANEL-1-J` is that motion and nothing else. It adds no material,
no token and no second geometry source: the same
`membraneOutline` gains one bounded progress input, so every frame of the
opening is a real droplet outline rather than a scale or a fade of the
settled one.

Progress runs `0..1` on the same eased reveal the shell already owns. At `0`
the body is collapsed into its own mouth at the seam; at `1` it is exactly
the settled geometry `PANEL-1-I` verified, so the animation cannot change
where a menu ends up. Between them the body's extent from the seam and its
lateral span open together out of the mouth, which keeps the emerging shape a
drop instead of an unrolling ribbon. Flight tension peaks mid-fall and thins
the neck below its resting width, then relaxes as the body lands.

Two invariants make it a drop that does not detach, and both hold at every
sampled frame rather than only at the ends. The mouth stays welded to the
seam: its span is never scaled away and the seam row keeps exactly the same
narrow glyph-centred contact the settled contract requires. The neck keeps a
hard floor, so no progress value, tension term or body size can pinch the
outline into two pieces.

Content is not part of the falling glass. The dense cards reveal only as the
body arrives, so no text is ever painted over a body that has not reached it
and nothing inside the menu is distorted by the motion. Reduced motion
resolves progress to `1` immediately, which is what keeps every existing
offscreen contract reading the settled geometry unchanged. Placement, the
attachment lease, compositor-region publication, focus, Escape, outside-click
and every floating route are untouched: this unit may only change what the
opening looks like between the click and the settled surface.


## PANEL-1-K boundary

`PANEL-1-I` and `PANEL-1-J` settled how a contextual surface joins the bar and
how it arrives. This unit asks the same question of the bar's own readings: a
capsule floating inside the veil is a plate with things on it, and the author
selected a reading held by the screen's edge instead.

Every panel capsule therefore reaches the top edge and squares off nowhere —
it keeps its rounded bottom and loses the gap above it. The centred clock is
additionally held by an elastic skin: its outline is widest where the edge
grips it and draws in over the whole side, one continuous stretch rather than
a lip at the top. Only the clock takes that, because only the clock has open
space either side; a flanked capsule that widened at the edge overlapped its
neighbour, and widening the bar's gaps to make room would have moved every
reading on the bar to decorate one.

Four silhouettes were tried and three rejected on sight, which the geometry
records so they are not walked again: squared at the edge read as a box pushed
against the screen; the same outward curve confined to a shallow lip read as a
bucket; pinching *narrower* at the edge was correct physics for a drop falling
off a ceiling and wrong here, because a reading is held by the bar rather than
dripping from it. The accepted shape spreads at the grip and draws in over the
entire height.

The capsule's body and the reading inside it do not move: the flare is painted
beyond the body, so text and icons keep the axis and the spacing they had.

## PANEL-1-L boundary

Two defects with one cause, and one design gap the author named while they
were being fixed.

A logical pixel is not a length. The shell's tokens are logical pixels, so the
same 40-token bar measures 12.50 mm on the author's 27" 1080p panel and
10.94 mm on their 32" 4K panel at scale 1.5 — 13 % smaller on the larger
monitor, which is also the one viewed from further away. `shellscale.{h,cpp}`
turns each output's real density into one bounded, stepped factor;
`QScreen::physicalDotsPerInch` already divides the compositor's scale out, so
the factor is that number over the density the tokens were drawn against. It
is applied as a scene scale per window, which is what keeps every measurement
inside — the 40-pixel bar, the capsules at y=5, the seam a menu attaches to —
exactly the number the design states, with only the last step to real pixels
differing per monitor. An output that publishes no believable physical size is
left at 1.0 rather than resized from a number that cannot be trusted.

Raster fidelity was two separate bugs. `CelestinaIcon` asked for its SVG at
the item's logical size, so on any scaled output the compositor was handed a
pixmap smaller than the area it filled. Worse, `traywatcher` rasterized every
foreign tray icon once at 18 pixels: that is the only rasterization there is,
and no consumer can recover detail that was never generated. Six further
raster paths — the panel tray, the inventory grid, menu rows, the workspace
map, album art, wallpaper thumbnails — asked for their sources in logical
pixels too. An application that publishes only a small pixmap and has no
themed icon is still the best available and remains soft; that limit is not
ours to fix.

Weight is not size. The author asked for thicker text and glyphs at unchanged
dimensions, so the vendored catalogue's stroke width rises and the bar's own
readings take the demi-bold weight, rather than any token growing.

Finally, `PerformanceMenu` rebuilt its complete entry list on every provider
reading, which tore down and recreated every row about once a second; the card
was then measured against a menu mid-rebuild and stayed clipped for as long as
it was open. `NetworkMenu` and `BluetoothMenu` had the same shape. Their row
lists now carry identity only, and every moving value is read live by the row
that shows it, so a tick moves labels and rebuilds nothing.


## PANEL-1-M boundary

Two GPU losses on 2026-08-12 were caused by an agent running the canonical
production workflow against the author's live desktop, and both had concurrent
`ddcutil` children on one I²C bus immediately before the machine went down.

The cause is structural rather than accidental. `complete-production.sh` ends
in an eight-second smoke that starts the *real* release host with the *real*
provider adapter, and that adapter probes DDC on the graphics card's own I²C
buses — the buses the running desktop is already using. Everything else in
that smoke is already degraded by design: it runs offscreen, against a session
bus address that does not exist, in a scratch XDG tree. DDC was the single path
still reaching hardware, and nothing about proving that the host and the
compiled style module load requires it.

`CELESTINA_DDC` turns it off, reading the name of the thing rather than a
negation, exactly as `CELESTINA_PANEL_MENU` does. Absent means on, and an
unreadable value also means on: a typo must not silently remove a working
hardware control from a session. Only `0` or `false`, trimmed and
case-insensitive, count as a refusal.

The gate sits at `detect`, which is the only entry point: `run` derives its
display list from it, and every read and write is per-display, so an empty
list makes a `ddcutil` child unreachable rather than merely unlikely. The
empty list is also not a new state — it is exactly what a machine whose
monitors do not speak DDC/CI already produces, so every path after the gate is
one the shell already supports.

The switch is recorded. A journal that shows no DDC activity must be able to
say whether that is because nothing happened or because it was disabled, so a
closed gate emits one `ddc.disabled` line at the same `Critical` level as
every other DDC event.

This does not weaken the smoke, and it does not fix DDC. Nothing here
coordinates between a Celestina helper and another shell's own detection, which
is the shape both losses had; that remains the author's open question under
`VAL-GPU-01`.


## PANEL-1-N boundary

`PANEL-1-L` sized the panel per output and said so plainly: the scene scale
reached the panel only, so on a denser monitor the bar was right and every menu
and overlay it opened was proportionally small. This finishes that.

The mechanism is the panel's, unchanged: one scaled scene per window, with
everything inside it in the shell's own units. `AnchoredCard` carries it for
every menu; the five overlay roots carry it individually because each owns its
own window. The two visual children of an overlay — the outside-click layer
and the card — are reparented into that scene rather than nested inside it, so
their declaration order is untouched and the diff stays readable. That trick
has one cost, and it bit: the order bindings are evaluated in is not the order
they are written in, so the dismiss layer landed on top of the card and a click
inside the card dismissed the overlay. Its depth is now stated rather than
implied.

The geometry an attached surface is handed — the opener, the icon inside it,
the bar's lower edge — arrives in output pixels because that is what the
panel's geometry is measured in. It is divided once, in the two controllers,
so no QML reading it has to know a scale exists.

Publishing blur regions was wrong the moment anything was scaled, and had
shipped that way: the origin came back mapped and the size did not, so on a
1.15 output the panel asked the compositor to blur a region a third narrower
than the bar it painted. Both collectors now derive the rectangle from two
mapped corners, which cannot disagree with each other.

`CELESTINA_SHELL_SCALE` lets the author name the factor. It was declared as a
missing extension point in `PANEL-1-L` and is delivered here because the tests
needed exactly the same thing: the offscreen platform reports a density of its
own, which silently rewrote every geometry contract that had been stated in
output pixels. Pinning it in the test environment keeps those contracts about
the shell's layout, and one new case exercises a real 1.15 factor end to end so
the conversion itself is not left unproven.

A named number is bounded like a derived one but deliberately not stepped: the
step exists to stop two similar monitors disagreeing by a fraction, not to
round an instruction. An unreadable or absurd request leaves the derived factor
alone rather than resizing the shell to nothing.


## PANEL-1-O boundary

`PANEL-1-L` derived the per-output factor from density. The author checked it
live on their own three monitors and it was wrong in two ways, both found by
comparison against a real desktop rather than by inspecting the arithmetic.

Density cannot separate two of the author's monitors. Their 24" 1080p panel
measures 91.73 dpi and their 32" 4K panel measures 93.34 dpi — 1.6 dpi apart,
indistinguishable by any density rule — and the author asked for 1.00 on the
first and confirmed 1.15 on the second. Their diagonals are 24.0" and 31.5",
which is a real difference and the one the model now corrects by: physical
size is the proxy for viewing distance, which no monitor publishes, and a
larger screen sits further back and needs the shell drawn larger to subtend
the same angle.

Correcting by size alone is not enough on its own, and the author caught this
too: their 24" panel resolves to 0.88 by size relative to the 27" reference,
and they asked for 1.00. A smaller monitor is not read from proportionally
closer — a desk has a front edge — so the factor now floors at 1.0. Nothing
ever shrinks the shell below the reference monitor's own size.

The density-fabrication defect from `PANEL-1-L` carries over unchanged in
kind, corrected in the new arithmetic: Qt invents a physical size when a
compositor publishes none, and the density computed from it is exact enough to
tell apart from a real EDID's whole millimetres. A nested Niri produces
exactly 100.00 dpi for its `winit` output this way, which resolved to a real
factor and drew the nested shell a quarter larger than the session beside it —
the exact defect the author reported live, traced to its cause in the nest
rather than assumed from the arithmetic. Both of Qt's fallback densities, 96
and 100, are now refused to a hair's width; a real monitor landing near one
keeps its own reading.

The complete CTest suite passes 18/18. The new cases hold the author's own
three monitors and their own judgement on each as the specification, including
one case that states the density measurement directly: `std::abs(lg24Dpi -
lg32Dpi) < 2.0` alongside `shellScaleForOutput` giving them different answers.
`CELESTINA_SHELL_SCALE` is unchanged; naming a number still wins over any
derived one.


## PANEL-1-P boundary

The falling drop's blur only ever described the shape it was going to land
on, because the compositor region was collected by the same debounced timer
that settles glass after any ordinary resize, and a ~300-500ms fall never sat
still long enough for that timer to fire mid-flight. The author watched this
live: the menu fell without blur and the blur simply appeared once it
stopped. `SoftMenuField` now calls `collectGlass()` synchronously on every
`attachmentProgress` change while `edgeShapeActive` is true, so the region
tracks the frame instead of the settle. `PanelBlurController` still
deduplicates on the C++ side, so a menu that never falls — or has already
landed — pays nothing extra, and its "blur armed" log line only fires on the
first arm rather than once per frame.

A live performance audit followed, read-only against the nested session's
`/proc` state with no interaction injected. Its most severe finding was the
diagnostics journal: the provider adapter was writing ~290 KB/s to the SSD at
idle, ~126× its own journal file's actual growth, because every poll
subprocess — `wpctl`, `nmcli`, `bluetoothctl`, `powerprofilesctl`, `ip` —
logged three lifecycle events per spawn at `Level::Critical`, and `Critical`
flushes and fsyncs per line by design, for the freeze-forensics case DIAG-1
exists for. That design intent was never about a poll that succeeds tens of
thousands of times a day. The level now follows what the child can actually
touch: `ddcutil` keeps `Critical` for its whole lifecycle, because it is the
one program that reaches the I²C buses a lost GPU is found on; every anomaly
for any program — a failed spawn, a timeout, a cancellation, a broken wait,
a kill-and-reap, a failed exit — keeps `Critical` too; only the ordinary
spawn/started/exit of a program that cannot reach the card drops to `Info`,
which still writes the line, just without the synchronous flush. Measured on
a fresh nest under the same idle conditions: 0 B/s of `write_bytes` over 45
seconds against 289,724 B/s before, with 594 info and 23 critical lines
recorded in that window — nothing stopped being recorded, only its cost did.

The same audit first attributed the provider's stable 143 MiB RSS to
wallpaper decoding never being released. That was checked, not assumed, and
was wrong: the journal from that run recorded no wallpaper event at all, so
the suspected path never executed. Direct measurement — RSS sampled across
100 seconds and 250 subprocess spawns — moved 4 KB, which is not a leak by
any definition this suite uses. The audit was corrected to say so, and the
true allocation site is left unidentified rather than guessed at: an
allocation fixed without knowing its owner is how a real bound gets removed
by accident. The audit's remaining findings — the full scope of
poll-by-subprocess (five pollers, ~1.4 children/s sustained), a proposed
memory ceiling for `VAL-SHELL-02`, and the nest's own unrelated DDC contact —
are recorded as open questions for the author's judgement, not defects this
unit fixes; replacing polling with native subscriptions (PipeWire, D-Bus
signals) is scoped as its own project rather than folded into this one.

The complete CTest suite passes 18/18, including a new case that sets
`attachmentProgress` mid-fall without waiting for any debounce and asserts
the compositor region already describes the momentary shape. The provider's
own test suite passes 11/11, including a new case that fixes the level of
every routine and anomalous event across all five pollers plus `ddcutil` by
name and by absolute path.


## PANEL-1-Q boundary

A third GPU loss was recorded on 2026-08-12, and its journal has the same
shape as the two before it. A development nest was live from 17:08 (its host
recording `output: "winit"`). After a version bump, the production pipeline
ran and `deploy-production.sh` overwrote `~/.local/libexec/celestina/*` while
that session was executing it. The helper channel broke, the host restarted
its provider adapter seven times inside 1.5 seconds — PIDs 248493 through
248849, every one recording `version 0.14.2` — and each restart opened the
graphics card's I²C buses through `ddcutil detect`. They contended:
`Max wait time 0 milliseconds exceeded after 2 flock() calls` on `/dev/i2c-7`
at 17:15:24, then `amdgpu: device lost from bus!` at 17:15:30.

`PANEL-1-M`'s gate held and is not at fault. The smoke runs its shell in a
scratch `XDG_STATE_HOME`, and none of those seven provider journals were
written there — the smoke never reached the hardware. What `PANEL-1-M` fixed
was one half of the exposure: a shell it starts itself. The other half is
build and deploy rewriting files a *real* session already has open, and that
half was governed by nothing executable at all. It has now failed twice, once
through the build tree the nest runs from and once through the installed
bundle, which is the evidence that a rule kept only in a person's memory is
not a control. A state check is also only true at the instant it is made: this
delivery did check for a live nest, and then acted on that answer minutes
later. A check meant to govern a later action belongs inside it.

`session-interlock.sh` is that rule with a latch. `build-production.sh` and
`deploy-production.sh` both call it before writing anything, and it refuses
while any process is executing `build/celestina*` or the bundle's
`celestina*`. Both roots are checked because both have caused a loss: the nest
runs straight out of the build tree, an installed session runs from the
bundle.

The test is `/proc/PID/exe`, not the command line, and the distinction is the
design. A command line is text, and the build tree's path is text that an
editor, a grep or this very file's path can contain; an interlock that stops a
release because a search was open is an interlock that gets commented out, and
then protects nothing. `/proc/PID/exe` is the kernel's own answer to what a
process is running. It also stays correct after the binary has been unlinked
and replaced, reading back as `"/path/to/celestina (deleted)"` — the state a
half-completed deploy leaves behind, and the one most urgent to catch. That
suffix is stripped with a quoted pattern, because unquoted `(deleted)` is a
glob group rather than two literal brackets in some of the shells this file is
sourced from; written unquoted it stripped nothing, matched nothing, and let
exactly the dangerous case through. That defect was found by testing the case
rather than by reading the line.

Four cases are exercised end to end: nothing running permits the release, a
live host refuses it, a host whose binary has already been swapped underneath
it still refuses it, and closing the session permits it again — the last one
because an interlock that cannot be satisfied is one that gets removed.

The hard reboot left eleven zero-length files in `.git/objects`, written but
never synced. Every one was checked against `git rev-list --objects --all`
before anything was removed: all eleven were unreachable, and no committed
history was lost. They were the blobs of this delivery's own staged files, and
they blocked re-staging because Git treats an object file that exists as an
object it has already written. Removing them and re-adding restored the index,
which `git fsck` then reported clean.


## PANEL-1-R boundary

Every panel reading opens something now. The clock opens a calendar; the phone
reading opens Magnetita's device list with ring, pair and unpair; brightness
opens one slider per monitor that speaks DDC; audio opens the output, the input
and one slider per application making or taking sound.

Three of the four needed no new data. The brightness provider already published
a connector-to-level map, `MonthCalendar` already existed, and
`org.celestina.Devices1` already served `ListDevices`, `Ring`, `RequestPair` and
`Unpair` — `DevicesClient` was simply keeping the first connected device and
discarding the rest. Audio needed real work: the provider learned to read the
`Sinks:`, `Sources:` and `Streams:` sections of `wpctl status` and to move a
named node's level and mute. It does that once per opening, on the menu's own
`devices-refresh`, and never on the two-second poll — the 2026-08-12 audit
measured that poll as the busiest subprocess in the shell, and a device
inventory is only meaningful while a menu is on screen.

The parser is section-walked rather than shape-matched, because the shapes
collide. `Video` has `Sinks:` and `Sources:` of its own and a webcam is a video
source, so the first parser listed it as a microphone. A stream's ports carry
the same `id. name` shape as the stream itself and are told apart by their
direction prefix, which is also what files the application as playback or
capture. And the trailing `[vol: …]` marker is stripped by that exact spelling:
cutting at the first bracket turned `PipeWire ALSA [parsecd]` into an anonymous
row. Each of those is a case.

Audio and brightness are cards rather than menus, for the reason the wallpaper
gallery and the calendar are: these are levels, and a level is moved rather
than chosen — a slider inside a real `Menu` row fights that row's own
click-to-activate. The author asked for the panel's own wheel step to work in
them too, so `LevelRow` owns one vocabulary for both.

### One card anatomy, and the binding that hid behind three copies

Four surfaces had grown their own scaffolding — Escape, the outside-click
carrier, the reveal, the glass — loosely copied from the wallpaper gallery,
and each copy drifted. `SoftCard` is that anatomy once, and its height is
*measured*, the way `AnchoredMenu.naturalMenuHeight` has always measured its
rows. The hand-written constants were the visible defect: three cards, three
different arithmetics, every one smaller than what the card actually drew,
which is why rows fell off the bottom and the author saw dead space.

Restoring measurement brought back a failure that had been blamed on binding
loops twice, and it was never one. A probe showed the measured height settling
*after* first layout (116 → 320 px on the calendar). `AnchoredCard` bound its
window size to that height, so the binding re-fired once the surface was
already mapped, shrank the window under the placement clamp, and left the card
parked at zero — over the panel's own row, swallowing the click meant to
dismiss it. The constants had only ever worked by never re-firing. The window
size is now a request made once at creation and re-made only on the host's own
inputs: the viewport cap and the side attachment, both of which arrive
synchronously right after creation and are read back synchronously by the host.
The tray-child case caught that nuance the moment the re-request was dropped.

### Five unit seams, all invisible at factor 1

`PANEL-1-N` divided the geometry handed to a surface by the per-output factor
once, in the controller. Five paths bypassed that division and were therefore
correct only when the factor was 1 — which is every offscreen test, because
they pin `CELESTINA_SHELL_SCALE=1` to keep geometry contracts stated in shell
units:

- the attachment lease's live anchor refresh, which replaces the very property
  the controller divided, and so moved a menu's mouth off its glyph on the
  first refresh;
- the tray child's parent card, read in shell units and mixed with the window
  size, the anchor and the output rectangle in real pixels, which placed the
  child over its parent instead of beside it;
- the same child's viewport cap, written in real pixels into a shell-units
  property, which let it run past the screen's bottom edge;
- its membrane gap, which decided the travel the sideways droplet crosses;
- the tray menu's pinned heading, reparented beside the popup's content item
  rather than into it, so the factor never reached it and the header band drew
  narrower than the rows it heads.

Each is converted once now, at the place that owns the crossing.

### The child menu joins the carrier every other menu uses

The sideways push was reported as animating "only the inner body, not the whole
menu as one block", and three fixes underneath it were real but not the cause:
the popup's stock enter transition running because the attachment arrived after
`open()`; a fall that ran to completion before the compositor ever presented
the new surface; and a membrane gap that read a derived binding inside its own
change dispatch and got the previous value, so the travel strip was zero and
the menus were glued together.

The cause was structural, and measuring it took freezing the push and reading
the numbers: mid-push the body was displaced and the glass polygon tracked it
correctly, yet the surface was card-sized — and the compositor's glass fills
such a surface edge to edge. The card had no canvas to visibly travel across.
The top-attached menus read correctly because they live on a surface that
covers the output. The child now does too. The author accepted the input trade
(2026-08-13): with the child open, a click outside it dismisses the child
first, as in any nested menu.

Migrating three integration cases to that carrier exposed a limit of the
harness worth recording: the offscreen platform clamps a shown window to its
own 800x800 screen, so an output-covering surface cannot be given its real
size there. Absolute placement is therefore contracted by the
`adjacentTrayMenuOrigin` unit cases against hand-fed compositor geometry —
including a scaled one that asserts the converted and unconverted placements
*differ*, so a future caller cannot quietly pass shell units again — while the
integration cases own attachment, lifecycle, the viewport cap and the keyboard.
Every assertion that moved says in a comment where its contract now lives.

### Icon-first, from here on

The author's standing decision (2026-08-13): the hierarchy is icons, not text.
A secondary action is a compact icon where the action applies — the wide
"Elegir la salida" buttons became a chevron on the section label, present only
when there is more than one device, because one device is an answer rather than
a question. Every opener carries the same capsule behind it, inset from the
reading pill that holds it so the two no longer collide at their lower edge,
and the speaker-and-microphone pair reads as one capsule twice as long. The
tray menu's "Acciones" label is gone: its header already says what the list is.

The clock and the phone reading became real `PanelMenuButton`s rather than
plain items with a hand-rolled click, because the attachment lease resolves a
drop's anchor by walking the panel for marked openers and deliberately leaves
anything else floating — which is exactly why those two menus opened with no
connection to the bar.

The blur controller re-arms on first exposure rather than on visibility: the
effect region is double-buffered surface state, and a commit made before the
compositor acknowledged the surface is dropped silently, which is what made a
freshly mapped child's glass come and go between openings.

CTest passes 18/18 and the provider's own suite 115/115. Two diagnostic events
are added rather than removed after the hunt — `blur.armed` and
`tray.child.placed`/`requested`/`closed` — because a nested session's console
is unreachable from outside it, and these are the bounded technical facts that
turned this investigation from guesswork into subtraction.
