# PANEL-1-B prototype sequence — contextual-menu hierarchy nested evidence

- **Date:** 2026-08-10
- **Scope:** apply the accepted Control Centre hierarchy and Velo material to
  every existing contextual shell surface, then correct tray inventory controls,
  shell tooltips and foreign-menu overflow without changing lifecycle ownership,
  and make the exact interactive glass namespaces sample the composed scene
  instead of Niri's wallpaper-only xray cache
- **Artifact:** final Celestina 0.11.0 prototype snapshot and the recorded
  incremental `PANEL-1-B/C/D/E` nested candidates
- **Environment:** the already-running nested Niri development session,
  `wayland-2`, output `winit`, 1896 by 998 logical pixels at scale 1
- **Authority boundary:** canonical production build, verification and deploy to
  the normal test prefix; nested-session restart or config reload only; no live
  host Niri edit, host-session activation, commit or push

## Procedure

### Implemented boundary

`MenuHeader` is the one shell-local heading anatomy for both lifecycle
families. It paints one denser non-blurring section and arranges a semantic
icon, title/subtitle column and bounded trailing-action column. `SoftMenuRow`
adapts that heading and the same icon/text/state rhythm to real Qt Quick menu
items. It does not replace their `Menu`, focus, arrow, Enter, Escape or
outside-click behavior. Network, Bluetooth and tray menus retain the
`SoftMenu -> AnchoredMenu -> GlassContextMenu` carrier, use a wider 328-pixel
field, and keep one compositor blur region across the heading and body groups.
Those four menus now use three separate layout dimensions rather than one
nearly invisible gap: 12 pixels between the visible heading and body sections,
8 pixels between ordered rows, and 4 pixels of additional vertical inset
inside every body row. The foreign application's `TrayMenu` and the
shell-owned `TrayItemsMenu` use the same rhythm while keeping their distinct
activation semantics.

Workspace map, clipboard, notification centre, session menu, launcher and
Control Centre retain their existing focused overlay or anchored-card carrier.
Each now begins with the same 56-pixel heading and separates the remaining
content into one or more explicit `MenuSection` groups. Clipboard, notification
and launcher rows expose leading icon, central text and trailing state/action
columns. Session actions remain real buttons and keep double confirmation, but
their resting role is ghost so the group is not a ladder of opaque pills.
`BackdropTextField` keeps the launcher's established editing behavior while
replacing the scheme-bound opaque input plate with the same low-density,
output-adaptive material.

The slice also corrected defects encountered inside the authorized surfaces:

- a detached Bluetooth failure report is actionable and can now be dismissed;
- notification empty/unavailable copy stays inside its body section rather
  than being laid out below the fixed-height card;
- producer text is no longer passed through repeated placeholder substitution
  in the notification accessible name;
- technical English failure reasons remain diagnostics instead of being
  painted by the session menu or launcher;
- clipboard and launcher expose their previously silent truncated-list state;
- tray separators are real ignored hairlines, and the existing tray icon name
  is rendered through the sealed icon catalogue with a bounded fallback;
- the first four-pixel connectivity/tray spacing pass was visibly insufficient
  in the author's pointer-opened Network capture. The value had been bound to
  `Control.spacing`, which does not separate `Menu` delegates. The correction
  therefore puts transparent space into each delegate's measured geometry and
  separates section gap, row gap and internal row inset.

Toasts, OSD and output chooser remain outside this contextual-surface family.
The compact tray indicator stays in the panel, but its complete inventory is
now the shell-owned contextual `TrayItemsMenu` rather than an inline drawer.

### Semantic panel grouping extension

The author then asked for the panel itself to use the same grouping principle.
`PanelCluster` now owns one finite compositor region and a compact internal row
for each semantic group. Network and Bluetooth retain their 12-pixel rhythm in
one connectivity cluster. Audio output, microphone and display brightness use
one 12-pixel level cluster. Notification history, launcher, Control Centre,
clipboard, session and the new performance opener use one utilities cluster
with four pixels between complete button targets. Those controls no longer
publish overlapping individual blur regions.

The power-profile control left the bar because the Control Centre already
shows the provider-confirmed `power.active` state and sends the same
`power/cycle` request. CPU and memory percentages moved from the left flank
into `PerformanceMenu`; their single CPU opener remains accessible with both
values and the menu retains `sysmon/open-monitor`. `CaptureButton` moved into
the vacated left-flank position and now opens `CaptureMenu`, whose ordered tool
list currently contains the existing Niri screenshot request and can accept
later capture tools without changing surface ownership.

### Tray and wallpaper interaction extension

The capture surface is now presented as `Caja de herramientas`: its panel
opener and heading use the sealed toolbox glyph, while the screenshot row keeps
the scissors glyph that names that concrete action. A separate wallpaper
opener beside it opens `WallpaperMenu`. `Elegir carpeta...` transfers control
to one permanent Qt `FolderDialog` owned by the panel; neither QML nor C++
embeds a source path. The chosen local directory is persisted only after its
settings write is durable. A non-Qt worker scans at most 512 entries without
recursion, sorts and decodes supported regular images, and rejects entry 513
rather than publishing an incomplete catalogue. Every accepted image remains
reachable through deterministic pages of at most 64 rows with percent-encoded
preview URLs; the same catalogue identity spans at most eight pages. The menu
shows the total, current page and previous/next actions above its scrolling
three-column thumbnail grid. A click sends only output, catalogue token and item
id, so the provider can reject a stale catalogue or changed file before reusing
the existing atomic import. A successful choice preserves the source, publishes
the wallpaper generation at once and lets the existing appearance worker
recompute that output's foreground without closing the gallery. The rejected
The limited-gallery copy belonged to the first single-payload implementation:
that version had no route beyond its first 64 rows. Paging removes that terminal
condition instead of renaming it.

The tray opener now uses the sealed inventory glyph and exposes no visible
count. Its accessible description still reports the inventory size. Durable,
bounded preferences keyed only by a StatusNotifierItem's published stable id
place pinned items directly to the opener's right or remove hidden items from
the bar and visible section. One selector beside `Aplicaciones` switches a
fixed card between `Visibles` and `Ocultas`; both modes use the same four-column,
three-row icon viewport and scroll overflow without moving or resizing the
card. Producer titles are accessibility identity only. Every tile keeps pin and
hide/restore as separate hit targets, and hidden mode never paints a pin. Pin,
hide and restore wait for the exact key and requested mode in the provider's
durable snapshot instead of moving optimistically. An unrelated item arrival
cannot consume the pending focus restoration, while a confirmed removal moves
focus to the surviving grid or the now-empty mode selector.

Opening a foreign tray item's D-Bus menu no longer replaces the inventory. The
inventory remains the full-output parent surface and the foreign menu maps in a
bounded adjacent child surface. Child dismissal or activation preserves the
parent; parent retirement closes both. Repeated unresolved requests for the
same service, object path and panel anchor are coalesced because the foreign
D-Bus reply has no request token that could safely distinguish them.

### PANEL-1-D tray control and overflow correction

The fixed inventory now uses eye and eye-off glyphs for its visible and hidden
modes instead of painting those two labels. Both square controls remain in the
same Tab order, expose selected/checkable state and announce the mode plus item
count through their accessible names. The application artwork inside each
unchanged grid target grows from 19 to 23 pixels; both a resolved external image
and the sealed fallback use that one measure, while producer titles remain
assistive identity rather than tile text.

Celestina's shell-local `BackdropButton` boundary now overrides the shared
button tooltip bindings with an empty, permanently hidden tooltip. This covers
the panel, contextual menus, overlays, calendar and tray management while
retaining `helpText`, hover colour, pointer cursor, visual focus and AT-SPI
names. The toast dismiss action is the only direct shared-style button with
non-empty `helpText` outside that hierarchy, so it applies the same local
override explicitly. The shared CelestinaStyle control remains unchanged for
other products.

A foreign application can publish more D-Bus menu actions than one output can
display. Its carrier previously adopted the complete natural menu height, so
Qt had no bounded viewport to scroll and lower actions were off-screen. The
controller now retains that natural content measure but caps the visible card
to the logical output space remaining below the real request before the child
surface adopts its size. The request therefore stays the card's top edge while
the real `Menu.contentItem` becomes a scrolling viewport. One visible
`CelestinaScrollBar` maps its drag range to that viewport without entering the
Menu's item model. Automated coverage drives that same mapping directly and
uses arrow-key traversal to reach the final action; Escape still dismisses only
the foreign child and leaves the tray inventory parent mapped. A real pointer
drag remains author validation.

The first nested view of this extension was rejected. Its nominal toolbox icon
was the Lucide briefcase, the inventory glyph filled the complete 30-pixel
button, an unresolved foreign icon widened the panel with producer text, and
hidden rows stayed painted under an expanded heading. The correction uses
Lucide's literal `tool-case`, caps the panel inventory opener and its pinned
foreign artwork/fallbacks at the canonical 18-pixel icon size, and keeps the
accessible producer title out of panel geometry. `PANEL-1-D` later raises only
the artwork inside the menu's fixed grid to 23 pixels. The opener and heading
use the semantic `system-tray`
glyph rather than the launcher grid or generic application window. A
StatusNotifierItem such as Steam that publishes `IconThemePath` as one flat
directory still resolves the exact bounded `IconName` basename there. The
ordinary resolver preserves Qt's primary theme and adds the configured GTK
theme as an installed fallback, which resolves Solaar's published
`battery-good`; Slack publishes no usable name or desktop entry but does supply
a 22-pixel pixmap, so it keeps that image and never exposes its unstable object
name as visible copy. The resolver canonicalizes flat directories and
candidates, rejects separators and outside symlink targets, bounds encoded and
decoded input, and never guesses from application title or id.

### PANEL-1-E composed-scene blur correction

The reported wallpaper-only menu background was not painted by QML.
`CompositorGlassRegion` becomes transparent after the compositor effect is
confirmed, and `PanelBlurController` sends only a finite surface-local region
through `KWindowEffects::enableBlurBehind`; neither path loads or copies the
wallpaper. The behavior instead matches Niri 26.04's documented default: every
active background effect automatically enables xray, and xray deliberately
ignores intervening windows so one wallpaper-only blur cache can be reused.
The standard `ext-background-effect` request carries the blur region but no
client-controlled xray flag.

The registered nested profile now applies `xray false` only to
`celestina-panel`, `celestina-panel-menu`,
`celestina-panel-child-menu` and `celestina-overlay`. It does not force blur;
Celestina remains responsible for requesting each finite region. Wallpaper,
toast and OSD namespaces stay outside this interactive visual unit. The same
exact block is documented as an opt-in live Niri rule, while neither build,
deployment nor the nested experiment edits the author's live Niri config.
Niri documents non-xray effects as more expensive and experimental, including
effect loss during window open/close animation and tiled-window dragging.

The foreground palette remains explicitly wallpaper-derived. Wayland does not
let this client sample arbitrary application pixels below non-xray glass, so
the earlier wallpaper contrast contract is not relabelled as a composed-scene
guarantee. Bright/dark application contrast, motion behavior and practical GPU
cost remain author-run real-session checks.

## Result

### Automated evidence

The following completed against the final checkout:

- `bash scripts/check-architecture-contract.sh`: passed the sealed-colour,
  contrast, QML visual and architecture contracts.
- `bash celestina/scripts/qmllint-production.sh`: exited successfully after
  regenerating the compiled QML module with `MenuHeader` and
  `BackdropTextField` registered. The corrective revision adds no new warning;
  only the established `CelestinaLineGutter.qml` unqualified-access warnings
  remain.
- Focused CTest passed 4 of 4: surface manager, overlay contract, indicator menu
  and the complete QuickTest runner. These tests require exactly one heading
  per contextual overlay, the intended group count per surface, exactly one
  compositor region, no exterior shadow, retained Qt Menu keyboard lifecycle,
  successful dismissal of a detached Bluetooth failure, and the exact
  12/8/4-pixel section/row/inset rhythm for Network, Bluetooth and both tray
  menu levels. The surface-manager case also preserves a confirmed blur arm
  across exposure-only loss while rejecting missing visibility, size, effect
  or glass.
- The broader CTest run passed 16 of 17 tests inside the restricted sandbox.
  Its private-D-Bus tray-watcher test could not start there; that exact test
  passed outside the sandbox, so the complete suite's code paths passed.
- `git diff --check`: passed.
- `niri validate -c celestina/scripts/dev-session.kdl`: accepted the exact
  namespace-scoped non-xray rule. The durable live result is recorded below
  from a clean nested-compositor start, a launcher close/reopen cycle and a
  Celestina-only restart; a reload response alone is not treated as evidence.
- The complete QML QuickTest runner passed 200 of 200 cases. New cases cover
  single-region cluster ownership, 12/12/4-pixel group rhythm, the 1920-pixel
  right-flank fit, capture opener geometry and failure feedback, live CPU/RAM
  menu updates, absent performance readings, exact `sysmon/open-monitor`, the
  capture signal and the retained Control Centre `power/cycle` action. The tray
  cases additionally cover fixed card geometry, eye/eye-off visible/hidden
  controls, 23-pixel external/fallback artwork, mode counts and accessible
  names, non-overlapping pointer targets, confirmed focus restoration,
  unrelated live inventory changes, Escape from a nested action and parent/child
  coexistence. Hover regressions retain assistive names while requiring the
  inherited shell tooltip and the direct toast-dismiss exception to remain
  empty and invisible.
- `SurfaceManagerTest::anOverflowingTrayMenuUsesABoundedScrollableViewport`
  publishes 64 foreign actions and requires natural content to exceed the
  capped opener-relative viewport, a visible scroll bar with non-zero travel,
  an unchanged card top, direct scroll mapping to advance `contentY`, arrow
  keys to reach the final real Menu
  index, and Escape to retire only the child while the parent inventory remains
  mapped. A source audit also finds no shell tooltip creator outside the two
  explicitly hidden local bindings.
- `celestina-indicator-menu-test` passed with network,
  Bluetooth, performance and capture on the same real Menu/surface lifecycle,
  including Escape, outside click, keyboard activation, placement, adaptive
  ink properties and one compositor region. Its wallpaper cases also traverse
  catalogue pages without replacing catalogue identity.
- The final CelestinaStyle 1.3.0 production artifact passed its 29 common fixtures,
  colour, contrast, QML visual and architecture contracts, `all_qmllint`, CTest
  1 of 1 and the eight-second compiled-module smoke.
- The final canonical `celestina/scripts/complete-production.sh` exit rebuilt,
  verified and deployed the 0.11.0 bundle without activation. Rust core tests
  passed 333 cases, provider-adapter tests passed 80 cases, the QML QuickTest
  runner passed 200 cases and CTest passed 17 of 17. Focused regressions cover
  bounded tray-preference persistence and migration, pin/hide/restore
  presentation, keyboard secondary/context actions, parent/child tray-surface
  coexistence and replacement, wallpaper chooser hand-off, local-file import,
  URL encoding and immediate publication.

### Nested compositor inspection

`celestina/scripts/dev-session.sh --restart` rebuilt and restarted only the
nested Celestina host after the spacing correction and again after the semantic
panel grouping. That grouping cycle used shell PID 625321, Niri-adapter PID
625563 and provider-adapter PID 625564. The host Noctalia process remained alive
at PID 1276.

The final live inspection used the requested dark wallpaper and compositor
blur. Launcher, clipboard, notification centre and session menu were opened in
the nested compositor and captured at:

- `/tmp/celestina-contextual-launcher-final.png`
- `/tmp/celestina-contextual-clipboard.png`
- `/tmp/celestina-contextual-notifications.png`
- `/tmp/celestina-contextual-session.png`

All four showed the separate heading/body hierarchy, adaptive light ink,
single soft outer veil, denser internal groups and no shadow. The launcher's
final capture also shows the search field without the previous opaque dark
plate. The nested host emitted no QML construction, required-property or
runtime binding error after the final restart and is left alive for author
inspection. A post-restart whole-output capture is
`/tmp/celestina-menu-spacing-final.png`.

The final post-grouping whole-output capture is
`/tmp/celestina-panel-groups-final.png`. It shows the screenshot opener in the
former left-flank performance position, output/microphone/brightness inside one
pill, and notification/launcher/Control Centre/clipboard/session/performance
inside one compact utilities pill. The final restart initially armed seven
compositor shapes for those visible readings; the withdrawn connectivity and
media providers did not leave empty glass behind. The power profile is absent
from the bar and remains available in Control Centre. The two new menus passed
the real contextual-menu integration tests; this record does not claim a
scripted live pointer opening of either menu.

After the tray and wallpaper extension, the registered restart command targeted
the still-running nested socket `/run/user/1000/niri.wayland-2.633476.sock`.
The first extension cycle used shell PID 758079. After the final anchoring
correction and canonical production exit, the same command replaced only that
nested host with shell PID 846641, Niri-adapter PID 846871 and provider-adapter
PID 846872. Nested Niri remains PID 633476, host Niri remains PID 1224 and host
Noctalia remains PID 1276. The panel mapped on output `winit` at 1896 by 998 and
armed seven compositor shapes with 140 fragments. The final whole-output capture
`/tmp/celestina-contextual-tools-live.png` shows the literal toolbox and
wallpaper controls together and the fixed-size, count-free tray inventory
opener. No QML construction, required-property or binding error followed the
restart.

The `PANEL-1-C` corrective reload targeted the later still-running nested socket
`/run/user/1000/niri.wayland-2.865247.sock`. Before replacement, the D-Bus owner
was PID 865330 and its environment named exactly `wayland-2` and that socket.
The registered restart replaced only that host with PID 926222 and adapters
927209/927210. Nested Niri remained PID 865247, host Niri remained PID 1224 and
host Noctalia remained PID 1276. The panel mapped on `winit` at 1896 by 998 and
settled at seven compositor shapes with 140 fragments. No QML construction,
required-property or binding error followed the reload. The native portal still
reported the parallel-session application-id warning, and the active oversized
wallpaper exceeded the appearance decoder's safety limit; neither is claimed as
a native chooser or adaptive-ink pass for this cycle.

The first `PANEL-1-D` reload verified that D-Bus owner PID 926222 was the
deleted prior build and that its environment named exactly `wayland-2` and
`/run/user/1000/niri.wayland-2.865247.sock`. That registered restart replaced
only the nested host with PID 992071 and adapters 992276/992277. After the final
opener-relative overflow geometry correction and canonical production exit, a
second registered reload verified PID 992071 on that same nested display and
socket, then replaced only it with PID 1018620 and adapters 1018811/1018812.
Nested Niri remained PID 865247, host Niri remained PID 1224 and host Noctalia
remained PID 1276. That cycle's owner reports the same nested display and
socket,
the panel mapped on `winit` at 1896 by 998, and compositor blur settled at seven
shapes with 140 fragments. The final captured startup stream contains no QML
construction, required-property or binding error. It retains the already
recorded parallel portal application-id warning and oversized-wallpaper decoder
refusal; neither is caused or resolved by this tray correction.

`PANEL-1-E` first kept that compositor and shell running. A controlled
application, PID 1043895 with app id `celestina-blur-control`, supplied a
uniform teal field from the left edge to x=943 while the launcher crossed that
edge into the wallpaper. `/tmp/celestina-blur-control-launcher.png` records the
reported xray state: the menu sample above the application was
`srgb(42,47,43)` and still looked like the wallpaper. An explicit reload then
produced `srgb(24,106,116)` over the application in
`/tmp/celestina-blur-control-launcher-xray-off-reloaded.png`, but a later
Celestina reconstruction returned that point to `srgb(42,47,43)`. Because the
profile was changing between rule variants during that sequence, neither the
successful reload nor the later regression is used as final persistence
evidence.

The decisive cycle froze the exact namespace expression and restarted only the
nested compositor from that registered profile. Nested Niri PID 1102853 mapped
one 942 by 998 output; the replacement control application, PID 1104830,
covered x=12 through x=465 and the launcher crossed that edge. In
`/tmp/celestina-non-xray-clean-start.png`, menu pixel `(186,291)` above the
application is `srgb(31,106,115)`, close to the uncovered application reference
at `(16,56)`, `srgb(0,91,102)`, while menu pixel `(686,291)` above the wallpaper
is `srgb(33,39,33)`. Closing and reopening the launcher preserved the exact
three samples at those coordinates in
`/tmp/celestina-non-xray-clean-reopen.png`. A registered
`dev-session.sh --restart` then replaced only Celestina with PID 1106789 and
adapters 1107007/1107009; nested Niri stayed PID 1102853. Reopening the launcher
again preserved the samples in
`/tmp/celestina-non-xray-after-celestina-restart.png`. A second registered
Celestina-only restart produced PID 1110628 and adapters
1110890/1110891 without reloading Niri. Its environment names exactly
`wayland-2` and `/run/user/1000/niri.wayland-2.1102853.sock`, and the reopened
launcher remains namespace `celestina-overlay`. In
`/tmp/celestina-blur-control-launcher-clean-after-restart.png`, those same
coordinates retained the same values. After the canonical production exit, a
final nested-only restart loaded the verified bytes as current PID 1127567 with
adapters 1127828/1127829. Its environment names the same nested display and
socket, the launcher remains `celestina-overlay`, and
`/tmp/celestina-blur-control-launcher-production-final.png` retains the same
three samples at the same coordinates. This live sample covers the launcher
namespace `celestina-overlay`. The panel, primary-menu and child-menu namespaces
share the same exact validated matcher, but this control does not claim a
separate colour sample for them. Nested Niri remained PID 1102853. Host Niri
PID 1224 and host Noctalia PID 1276 remained alive throughout, and the author's
live Niri configuration was not edited.

The provider journal's apparent stream of `process.spawn`, `process.started`
and `process.exit` records was checked rather than assumed to be a restart loop.
It is the established two-second audio and five-second session polling cadence;
every inspected child exited with code zero, with no timeout, spawn failure or
`ok:false` record. The parallel nested session did report that its portal
connection was already associated with an application id. Therefore the
chooser bridge and wallpaper import are automated evidence, but opening the
native portal chooser, pinning/hiding with real pointer input, and observing a
foreign tray menu beside the inventory remain author-run live checks. Visual
acceptance of the eye controls and 23-pixel artwork, shell-wide hover without a
tooltip, and wheel/drag traversal of an overflowing foreign menu likewise remain
author-run checks; the nested reload proves only construction and isolation.

The pre-corrective nested cycles armed all seven blur shapes and then selected
the opaque fallback. A temporary diagnostic isolated the transition as
`surfaceReady=false` while the window remained visible and sized, the effect
stayed available and the glass region stayed non-empty. The nested registry
still published `ext_background_effect_manager_v1`, and an isolated
Qt/KWindowEffects probe returned available for 23 consecutive
post-registration samples. This exposed an existing layer-shell lifecycle
defect: after a confirmed arm, Qt can report the still-rendering surface as not
exposed.

`PanelBlurController` now permits that one state only after a successful arm
and only while visibility, size, effect availability and glass remain valid.
It can therefore submit a changed region without demoting QML to the opaque
fallback; initial setup and real losses retain the old guarded path. The
focused C++ regression covers the accepted exposure loss and each rejected
loss. Across the two corrected nested cycles, blur advanced from the initial
73-fragment geometry to the settled 140- or 120-fragment region as provider
visibility changed, remained armed for more than 30 seconds and through the
final `grim` capture, and updated from seven to six shapes when a provider
withdrew. No fallback followed the fix.

## Limits

This is live evidence for the four command-opened overlays on one scale-1
output. Network, Bluetooth, tray and workspace map retain pointer-opened paths;
their real carrier, geometry, grouping, keyboard traversal, action identity and
single-blur contracts passed automated integration tests. The author's
pointer-opened first-pass Network capture at
`/tmp/codex-clipboard-c54a7199-eae7-4240-a97c-05d9babc3534.png` established that
four pixels were insufficient; this record does not claim a scripted live
pointer opening or author perceptual acceptance of the corrected 12/8/4-pixel
revision for those four surfaces.
