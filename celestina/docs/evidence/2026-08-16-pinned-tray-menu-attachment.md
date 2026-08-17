# Pinned tray menus use the panel attachment lifecycle

- **Date:** 2026-08-16
- **Scope:** Celestina unit `R8-P-Q`
- **Environment:** Qt 6.11.1; offscreen QML and layer-surface regressions;
  author screenshot of the direct Solaar menu
- **Artifact:** Celestina 0.29.12 source candidate; the earlier production
  build is invalidated by the current source, and completion remains pending
- **Plan:** [polkit authentication agent](../plans/archive/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-PANEL-1`

## Procedure

The screenshot shows a pinned foreign tray icon to the inventory opener's
right and its direct D-Bus menu as a detached card with no top membrane. That
route reduced the invoking item to one global point. `PanelMenuController`
then explicitly constructed the foreign menu as a floating full-output popup,
so `TrayMenu.qml` never received `anchoredFromPanel`, `openerRect` or
`attachmentAnchorRect`. The QML already used the shared `SoftMenu`; the missing
animation was an adapter defect, not a private visual implementation.

`TrayDrawer` now publishes both the complete item rectangle and its exact icon
rectangle. The manager and controller carry those rectangles through the same
carrier-local conversion, physical panel inset and `PanelAttachmentLease` as
every first-party panel menu. The inventory child route remains independent
and keeps its existing side membrane.

The author's first nested retest found a second adapter defect behind the same
symptom: the semantic attachment source named the foreign `Image` itself.
That object is invisible while an asynchronous icon is unresolved and whenever
the fixed catalogue fallback is the glyph actually on the panel.
`PanelAttachmentLease` correctly treated the invisible object as a hidden
anchor and cleared `attachmentAnchorRect`; the already-created `TrayMenu` then
lost `topAttachmentRequested`, its membrane and its attached fall together.
The attachment source now names a stable icon-slot item which remains visible
across foreign-image and fallback rendering. The renderer branches remain
purely visual children of that one semantic anchor.

The author's `recording_20260816_211817.mp4` then disproved that this was the
last defect. It was captured from the rebuilt 0.29.12 host PID 530038. At
frames 131 (2.183334 s) and 198 (3.300001 s), the foreign Solaar menu first
appears already settled, with neither a fall nor a connector. The matching
diagnostic journal arms its compositor region at carrier-local `y == 21`, the
resting card origin; a valid top membrane would extend that region to the
carrier seam at `y == 0`.

The remaining loss occurred before the lease. `requestTrayMenu()` converted
both QML rectangles with `toAlignedRect()` while waiting for the asynchronous
D-Bus menu. A real 18-pixel glyph at a half-pixel position therefore became a
19-pixel rectangle. The lease compared that widened snapshot with the live
18-pixel semantic source, correctly rejected it as a different geometry and
cleared the attachment. The pending request now retains `QRectF` for both
opener and glyph until placement. Only the final integer card origin is
rounded; the exact glyph rectangle reaches `PanelAttachmentLease` unchanged.
The inventory child's established integer side-placement contract remains
unchanged.

The compact row now owns a stable `ListModel` rather than a destructive
`Repeater`. Pinned and attention delegates are ordered before the inventory
opener. A newly present item fades in with `motionFast`; an item leaving the
published set first animates opacity to zero and is removed only after that
token completes. Re-entry cancels the pending removal by restoring the same
stable row.

The author's `recording_20260816_213319.mp4` confirms that the exact geometry
fix restored the membrane: the host journal for PID 548897 publishes the
direct menu's glass from carrier-local `y == 0`. It also exposes a separate
one-block defect. `TrayMenu` pins its custom heading beside Qt's ListView so
scrolling foreign actions cannot move it, but only the ListView's internal row
carrier followed `rowsCut` during the attached fall. The heading therefore
landed first while the dark body and actions grew beneath it. The heading now
retains its fixed scrolling ownership while subtracting the same `rowsCut` as
the row carrier. The physically inset QWindow clips both at the seam, and the
complete menu enters as one block. Its copy is no longer generic: the tray
item's canonical title crosses the explicit QML/C++ request seam and both the
direct and inventory-child routes render an application-specific heading.

The next live screenshots showed that forwarding `title` alone was not enough:
Slack and ChatGPT both publish an empty SNI `Title`. Slack identifies itself as
`Slack_status_icon_1` and uses its tooltip for unread state, while ChatGPT
identifies only the generic runtime as `chrome_status_icon_1` but carries
`ChatGPT` in the tooltip title. The adapter now owns one bounded display-name
rule without changing the raw Id or its durable preference hash. It removes the
technical status-icon suffix from app-specific IDs, and only a generic
Chrome/Chromium/Electron result lets the tooltip provide product identity.
Consequently Slack remains `Slack` rather than `No unread messages`, while the
generic Chrome bridge becomes `ChatGPT`. The watcher also observes
`NewToolTip`, so that identity source can refresh without restarting the shell.

## Result

```sh
bash scripts/check-architecture-contract.sh
python3 scripts/version_tool.py check
cmake --build celestina/build --target celestina -j2
ctest --test-dir celestina/build --output-on-failure \
  -R '^celestina-indicator-menu$'
ctest --test-dir celestina/build --output-on-failure \
  -R '^celestina-output-chooser$'
ctest --test-dir celestina/build --output-on-failure \
  -R '^celestina-surface-manager$'
celestina/scripts/complete-production.sh
```

- The architecture and version contracts pass.
- The complete QuickTest runner passes, including a focused 14/14 tray-drawer
  file that proves left ordering, full opener/icon geometry, keyboard and
  pointer access, a live mid-fade delegate and eventual removal. The fallback
  cases additionally prove that the semantic anchor stays visible when the
  foreign image is empty or fails to load.
- The complete surface-manager suite passes. Its direct foreign-menu case
  proves carrier-local opener/anchor geometry, `attachmentStartY == 0`, the
  panel-height layer margin, standard body placement, source feedback and
  independent inventory-child lifecycle. It now also requires
  `topAttachmentRequested`, `edgeShapeActive` and a published glass silhouette
  whose top is the carrier-local seam at `y == 0`; a detached card beginning at
  its resting `cardY` cannot satisfy that contract. The semantic source is
  deliberately placed at a half pixel, so reintroducing the premature aligned
  rectangle makes this integration case fail again.
- The same direct integration freezes the attachment at the start of its fall
  and proves that the non-scrolling heading moves by exactly `rowsCut` while
  retaining its viewport-sibling identity. The tray and indicator regressions
  also prove that the application's canonical title reaches both menu routes
  and produces the Solaar and Chromium application-specific headings rather
  than a generic tray heading.
- The pure tray-item suite pins declared-title precedence, Slack's technical-Id
  normalization and ChatGPT's generic-runtime tooltip fallback. The private-bus
  watcher regression demarshals the real `(sa(iiay)ss)` tooltip structure for
  both peers and publishes five named applications; it passes 4/4 outside the
  restricted process sandbox.
- The Celestina target builds with the generated QML cache at version 0.29.12.
- A later authorized `celestina/scripts/complete-production.sh` run outside the
  restricted sandbox passed the registered build and verification, all 23
  CTest targets, every Rust suite, qmllint and both production smokes. The
  current Celestina 0.29.12 artifact was deployed to `~/.local` without
  activating the main session.
- At the author's request, `dev-session.sh --restart` rebuilt and replaced only
  the nested build-tree host after the heading-lifecycle and title correction.
  PID 579663 owns `org.celestina.Shell` on
  `WAYLAND_DISPLAY=wayland-2`; nested Niri PID 80685 remains unchanged with its
  1920x1080 scale-1 `winit` output. Startup mapped the panel and armed one blur
  shape without a QML construction error. The shared bus still has another
  polkit agent and notification server, which is expected for this nest.

## Limits

Offscreen geometry proves that the shared membrane and fall are requested; it
does not prove their compositor rendering or the perceived icon fades in a
real panel. In the nested session, pin and unpin two applications and
right-click each pinned icon. Both icons must remain to the inventory opener's
left, fade independently, hold ordinary open feedback, and grow the foreign
menu from their own glyph without a detached first frame.
