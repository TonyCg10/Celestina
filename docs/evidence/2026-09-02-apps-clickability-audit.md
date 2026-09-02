# Evidence: 2026-09-02 apps clickability audit

- **Date:** 2026-09-02
- **Scope:** read-only clickability audit of the QML surfaces of the four
  applications — `siderita/qml`, `fluorita/qml`, `grafita/qml`,
  `magnetita/qml` — and of the shared controls in `celestina-style/` they
  instantiate, at `6cb0bd4`. The Celestina shell (`celestina/`) is explicitly
  out of scope by the author's instruction; a pass that had started on it was
  stopped and none of its reading is recorded here
- **Environment:** static review of the checkout. No application, provider,
  build, test, deployment or activation ran. Each project's `Celestina*`/
  `Glass*` copies were diffed against `celestina-style/` and found
  byte-identical (Fluorita links them), so every shared-control finding applies
  to every consumer
- **Artifact:** none. The output is this record. Nothing was corrected: an
  audit does not authorize a fix

## Procedure

Five concurrent read passes, one per application plus one on the shared
module, each told to prefer a few verified findings over many speculative ones
and to drop anything it could not substantiate from the source. "Clickability"
was defined as: what a person can or cannot click and what happens — dead
clicks and hit-box mismatches, swallowed or fall-through input, wrong button
semantics, missing hover/pressed feedback, hit targets under the
`controlHeightXs` floor of 30 px, click-only actions with no keyboard path,
and disabled controls that still take or leak clicks. The yardstick is
`celestina-style/DESIGN.md` §2, §6.1 and §7 and the `CelestinaInputShield`
contract. Every High and Critical finding below was then re-verified against
the source by the coordinating pass before being recorded.

```sh
rg -c 'MouseArea|TapHandler|HoverHandler|onClicked|onTapped' -g '*.qml' \
  siderita fluorita grafita magnetita celestina-style
for p in siderita fluorita grafita magnetita; do
  for f in celestina-style/*.qml; do diff -q "$f" "$p/qml/$(basename "$f")"; done
done
```

## Result

- **Exit:** manual outcome. 36 findings: 2 Critical, 6 High, 15 Medium,
  13 Low. No application besides Siderita has any QML test, and Siderita's
  pointer tests cover none of the findings recorded against it
- **Observed:** two of the four applications ship a feature that a pointer
  cannot reach at all (Fluorita's annotation selection, Magnetita's settings
  rows past the window edge), and Siderita stacks every folder dialog under
  its own chrome. The common root across all four is the same: a surface
  floats over live content without claiming the input over its own box, or a
  later sibling with default `z` is assumed to be on top of an earlier one
  with an explicit `z`

### Siderita

**SID-C1 — Critical — Every FolderView dialog is stacked below the top bar,
tab strip, heading and contextual headers.**
`siderita/qml/views/FolderView.qml:663-665` (`FolderHeading z: 9`), `:683-685`
(`TopBar z: 10`), `:710-711` (`TabStrip z: 10`), `:724-733` (`FolderActions
{ anchors.fill: parent }`, no `z`), `:736-745` (`FolderContentChrome`, no
`z`). The dialogs are children of `FolderActions`
(`components/folder/FolderActions.qml:97-203`) and carry `z: 60…72`
(`dialogs/NamePromptDialog.qml:13`, `ConflictDialog.qml:13`,
`OpenWithDialog.qml:13`, `PropertiesDialog.qml:18`, `QuickLookView.qml:21`,
`PhoneMediaDialog.qml:48`, and the rest). QML `z` orders siblings only: a
child's `z: 70` cannot lift it above a sibling *of its parent* with `z: 10`.
So while Rename, Properties, Conflict, Open-with, Quick Look, Compress or
Password is up, the path pill still edits and navigates, the search pill
still searches, tab chips still switch and close, the Trash and search
headers still fire, and the details header still re-sorts — none of them
dimmed by the scrim. With Quick Look the top-bar band paints over the preview
card and eats clicks there. `FolderShortcuts` is gated by `navigationBlocked`
(`:159-168`); pointer input to the chrome is not.
`dialogs/GrafitaEditorDialog.qml:24-29` already reparents itself to
`Window.contentItem` for exactly this reason. Fix: give `FolderActions` a `z`
above every chrome sibling, or reparent the dialog layers as the Grafita
dialog does.

**SID-H1 — High — The bookmark rename field is covered by the row's
MouseArea; a click inside it navigates away.**
`components/sidebar/SidebarBookmarkRow.qml:107-130` (`CelestinaTextField
visible: root.editing`), `:132-138` (`MouseArea rowMouse anchors.fill:
parent`, declared after the field, so on top), `:179-195` (`onClicked …
openKey(bookmarkPath)`). Only `drag.target` is switched off while editing;
the click handlers are not. Clicking to place the caret navigates the active
tab; right-click opens the bookmark menu; mouse selection inside the field is
impossible. Fix: `enabled: !root.editing` on `rowMouse`.

**SID-H2 — High — Disabled floating buttons are holes: the click lands on the
file underneath.**
`components/folder/PhoneMediaUnderBar.qml:18-26` with
`PhoneMediaButton.qml:23` (`enabled: root.connected`, a bare
`CelestinaIconButton` with no shield); `components/picker/PickerChrome.qml:155-163`
(accept `FloatingButton … enabled: root.canAccept`) with
`chrome/FloatingButton.qml:34-39`, whose shield is a *child* of the Button and
has `swallowClicks: false`. Qt skips a disabled item and all its children when
building pointer targets, so neither the button nor its shield is a candidate
and the press reaches the delegate's MouseArea and DragHandler. A disconnected
phone's dimmed music button selects, opens, context-menus or drags the row
behind it; the greyed "Abrir/Guardar" button in the picker selects the file
underneath, which enables the button. Fix: host the shield on an
always-enabled wrapper, the pattern `RecentHeader.qml:33-52` and
`TrashHeader.qml:39-48` already use.

**SID-M1 — Medium — Missing favourites look clickable but are dead, and the
only way to remove them is blocked.** `SidebarFavoriteRow.qml:64-86`: hover
fill and hand cursor are unconditional, then `if (!controller ||
root.missing) return` runs before the `RightButton` branch. The menu holding
"Quitar de favoritos" (`SidebarContextMenus.qml:206-213`) is unreachable for
exactly the rows that need it. Contrast `SidebarPhoneSection.qml:151-153`.

**SID-M2 — Medium — In the picker, Escape during the overwrite prompt cancels
the whole dialog; Back/Forward and Ctrl+L/F/H stay live behind the modal.**
`PickerWindow.qml:451-455`, `:457-473`, `:441-449` are gated on
`!picker.answered` and `controller.loading`, never on
`overwritePrompt.visible` (`PickerOverwriteDialog.qml:36-39`). A
window-level `Shortcut` pre-empts `Keys.onPressed`, so Escape sends
`answer([])` to the requesting application. `Main.qml:560-573` gates the
main window correctly.

**SID-M3 — Medium — Double-clicking a bookmark to rename it also navigates
to it.** `SidebarBookmarkRow.qml:179-195`: `onClicked` opens the path and
`onDoubleClicked` enters edit mode on the same MouseArea with no timer.

**SID-M4 — Medium — Closing an operation callout by clicking outside also
delivers the click to the file under the pointer.**
`components/folder/OperationsDock.qml:54-68`: the outside catcher is a
`TapHandler` at `z: dock.z - 1`, which takes only a passive grab and never
blocks the delegate's MouseArea and DragHandler beneath it. The callout closes
*and* the row is selected, opened, context-menued or dragged.
`tests/qml/tst_operations_dock.qml:173-179` asserts only the close.

**SID-L1 — Low — Hit targets under 30 px.** Tab close
`chrome/TabStrip.qml:131-136` (24×24); eject `views/Sidebar.qml:506-527`
(26 px); section-header trailing count `SidebarSectionHeader.qml:73-95`
(≈20 px tall); `SizeRow.qml:35-40` slider at 15 px; seek and volume sliders
in `dialogs/MediaPreview.qml:136-152, 197-208` at the shared control's 16 px.

**SID-L2 — Low — The picker's save-name field has no input shield**
(`PickerChrome.qml:55-78`, `GlassPill { inputShield: false }`): the row under
the cursor highlights while the pointer is over the field.

**SID-L3 — Low — Keyboard and feedback gaps.** Icon cells in
`dialogs/IconPickerDialog.qml:104-125` and the operation ring
(`OperationRing.qml:141-148`, `Accessible.role: Button` without a focus or
key path) are pointer-only; place, device, bookmark and favourite rows have
no focus/Enter path where phone rows do (`SidebarPhoneSection.qml:56-75`);
`DetailsHeader.qml:90-100` and `OpenWithDialog.qml:139-145` lack the hover
fill or hand cursor their sibling lists have.

### Fluorita

**FLU-C1 — Critical — Annotations in the editor can never be selected, so
move and delete are dead.** `components/EditSurface.qml:154-161`
(`EditObjectLayer objects`) sits *under* the full-canvas `MouseArea drag`
declared at `:202-238`, which accepts `Qt.LeftButton` and, when
`surface.tool === "none"`, calls `selectObject(0)` and returns without
un-accepting the press (`:227-231`). The `TapHandler` in
`EditObjectLayer.qml:138-140` is the only path to a non-zero selection
(`grep selectObject`: `:160`, `:229`, `:525`), and every keyboard verb —
Delete, Backspace, the arrows — is gated on `editor.selected !== 0`
(`:524-555`). Fix: set `event.accepted = false` in that branch, or hit-test
`objects.childAt` from the MouseArea.

**FLU-H1 — High — BatchBar floats over the grid with no input shield.**
`components/BatchBar.qml:14-32` is an `Item` with a `GlassSurface` and no
`CelestinaInputShield`; `LibraryView.qml:256-263` places it above the grid,
whose cells (`GalleryGrid.qml:222-267`) take hover, left and right clicks. A
click on the "N elegidos" label, a divider or the padding *opens* the card
underneath or pops its menu; resting on the pill over a video starts the
700 ms preview, and `Main.qml:231-247, 277-294` paints that preview over the
bar. This is the consumer `CelestinaInputShield.qml:15-19` describes.

**FLU-H2 — High — Window shortcuts stay armed under the editor and its text
prompt.** `Main.qml:530-534` (Space), `:536-545` (Up/Down), `:562-566`
(Escape) are gated on `window.playing`; only Ctrl+E (`:505-510`) is gated on
`!mediaEditor.open`. `Shortcut` resolves through ShortcutOverride before
`Keys.onPressed`, and neither `EditSurface.qml:517-529` nor
`CelestinaModalLayer.qml:211-219` accepts the override. Pressing Escape to
disarm a tool instead runs `backToLibrary()`, tearing the session down under
the open editor; in the "Texto" prompt Escape does the same instead of
cancelling; Space anywhere in the editor toggles the hidden player.

**FLU-M1 — Medium — Tab reveals the filmstrip, but Enter and the arrows go to
an Item with no handlers.** `components/ContentDock.qml:45-58`: `strip` is a
plain `Item` with `activeFocusOnTab`, not a `FocusScope`; the `ListView` at
`:83-93` carrying the key handlers (`:181-182`) never gets focus.

**FLU-M2 — Medium — The "saving" shield sits under the toolbar and canvas.**
`EditSurface.qml:433-436` places a `CelestinaInputShield` as a sibling at the
shield's own `z: -1`, so it absorbs only the empty backdrop; the magnifier
(`EditToolbar.qml:154-160`), the ink swatches (`:177-229`), Ctrl+wheel
(`EditSurface.qml:335-342`) and middle-drag pan (`:347-368`) all still act
during a save.

**FLU-M3 — Medium — The edit toolbar pill has no shield**
(`EditToolbar.qml:16-59`): with a tool armed and the picture panned under it,
a click in a gap starts a shape on the picture, and Ctrl+wheel over the pill
zooms it.

**FLU-M4 — Medium — Seek and volume bars: 16 px hit height, no hover or
pressed state, no wheel.** `VolumeBar.qml:32-46`, `PlayerTransport.qml:34-42`
on the shared control (see STY-M2).

**FLU-M5 — Medium — Gallery cards, music rows and filmstrip frames have no
hover or pressed feedback.** `GalleryGrid.qml:164-169, 222-225`,
`MusicList.qml:102-106, 147-151`, `ContentDock.qml:171-177`: `hoverEnabled`
and a hand cursor, but nothing bound to `containsMouse` or `pressed`.
`SidebarRow.qml:37-43` in the same app shows the intended treatment.

**FLU-M6 — Medium — Sidebar all-items header and add-folder rows are
pointer-only.** `LibrarySidebar.qml:113-120, 143-157, 182-201`,
`SidebarRow.qml:12-30, 87-101`: the header is not a delegate, so arrows never
reach it, and the add row has no `activeFocusOnTab` or key handling.

**FLU-L1 — Low — `checked:` on non-checkable icon buttons.** `Main.qml:406`,
`PlayerTransport.qml:80`, `EditToolbar.qml:158`: `CelestinaButton` has no
checked visual (`CelestinaButton.qml:82-117`), and the first click
auto-toggles and overwrites the binding.

**FLU-L2 — Low — The refusal notice pill covers the batch bar or seek bar
while staying click-transparent** (`Main.qml:434-459` versus
`LibraryView.qml:256-259` and `PlayerSurface.qml:215-224`).

**FLU-L3 — Low — MetadataPanel disables "Cerrar" while busy but scrim click
and Escape still close it** (`MetadataPanel.qml:179-182, 22-23`).

### Grafita

**GRA-H1 — High — The tab strip has no overflow handling: tabs and the "+"
button drift off-screen and become unclickable.**
`components/TabStrip.qml:91-97` (`strip.width` is a running sum), `:123`,
`:221-224` (`newTabButton.x: totalWidth() + spaceXs`), `Main.qml:268-272`
(no clip, no Flickable). At roughly eight to ten documents the later tabs and
the new-tab button lie past the window edge with no way to scroll to them, and
`drag.maximumX` (`:173`) lets a reordered tab be dropped there.

**GRA-M1 — Medium — Right-click on the editor is a dead click.**
`components/DocumentView.qml:174-217`: a bare `TextEdit` with no
`acceptedButtons`, `TapHandler` or menu; `GlassContextMenu` has no consumer
in Grafita.

**GRA-M2 — Medium — Clicking the editor box outside the painted text does
nothing.** `DocumentView.qml:108-122, 174-190, 92-106`: `body` spans only its
`paintedHeight`; `page`, `scroller` and `CelestinaLineGutter` have no click
handler, so a click below the last line, in the 12 px margin or on the gutter
neither focuses nor places the caret.

**GRA-M3 — Medium — Recent-document rows are click-only and have no pressed
state.** `DocumentView.qml:357-390`: announced as buttons, unreachable by
Tab, no `Keys`, no `pressed` binding, no cursor.

**GRA-L1 — Low — Encoding rows are ≈29 px tall with no hover state**
(`EncodingDialog.qml:74-118`).

**GRA-L2 — Low — Tabs have hover only: no pressed state, no pointer cursor,
not focusable** (`TabStrip.qml:133-136, 160-195`); `closeButtonWidth`
reserves 38 px for a 30 px button (`:50`).

**GRA-L3 — Low — Ctrl+E silently does nothing while dirty or importing**
(`Main.qml:263-266`, `src/session.rs:624-636`) where the footer button is
correctly disabled (`DocumentFooter.qml:76-77`).

### Magnetita

**MAG-H1 — High — The settings page is not scrollable; rows past the window
bottom cannot be clicked.** `pages/SettingsPage.qml:4` is a plain `Column`
placed by `Main.qml:81-86` at `height: parent.height - y` with no Flickable
and no clip; `PairedDeviceRow.qml:12` is 66 px per device and
`PluginRow.qml:11` 46 px for six plugins (`src/controller.rs:238`). At the
480 px minimum height, or the default with a few paired devices, the plugin
card and the lower "Olvidar" buttons are unreachable by wheel or pointer.
`DevicesPage.qml:5-35` already has the Flickable and `ensureFocusVisible`
pattern to reuse.

**MAG-M1 — Medium — The media progress bar draws a 15 px slider thumb but
ignores every click and drag.** `components/MediaProgress.qml:39-49, 13`
(`Accessible.role: ProgressBar`, no handler); the daemon already publishes
`mediaCanSeek` (`src/controller.rs:62`, `src/devices.rs:52`) and no seek
invokable exists. Either drop the thumb or wire `CelestinaSlider`.

**MAG-M2 — Medium — In plugin and mirror rows only the 44×26 switch is
clickable, and it has no hover or pressed state.** `PluginRow.qml:13-40`,
`MirrorSettingsSheet.qml:73, 82`, `CelestinaSwitch.qml:17-27` (track colour
depends on `checked` only; `hoverEnabled` unset). Clicking the label does
nothing, and `onClicked` re-binds `checked` immediately (`:37`) so the toggle
snaps back until the daemon confirms.

**MAG-L1 — Low — Page keyboard scrolling only works when a focused child
exists** (`DevicesPage.qml:37-51`: the Flickable never takes focus).

**MAG-L2 — Low — The connected-device card shows a mount path and a
verification code that cannot be selected or clicked**
(`ConnectedDeviceCard.qml:94-119`); a second connected device's mount is
never reachable by pointer (`DeviceControls.qml:86-93` serves only
`primaryIndex`).

### celestina-style

**STY-M1 — Medium — `CelestinaInputShield`'s blocking HoverHandler also
blocks its host control's hover.** `CelestinaInputShield.qml:28-38`; consumer
`siderita/qml/components/chrome/FloatingButton.qml:22, 37-39, 62, 66`. Qt 6
delivers hover leaf-first and a blocking `HoverHandler` on the last-visited
child stops delivery to the parent, so a floating button whose shield is its
direct child never shows `control.hovered`. The Siderita shield test
(`tests/qml/tst_celestina_input_shield.qml:52-80`) hosts the shield in a
hover-less `Rectangle`, so this path is untested. Fix: a `blockHover` knob, or
host the shield inside the background item.

**STY-M2 — Medium — `CelestinaSlider` paints a 4 px track with no thumb and
no hover or pressed state.** `CelestinaSlider.qml:69` (`implicitHeight:
spaceLg` = 16), `:75-101` (no handle), `:109-140` (no `hoverEnabled`);
`CelestinaTheme.qml:855` defines `compSliderHandleSize: 15`, which nothing
uses. Consumers at default height: `fluorita/qml/components/VolumeBar.qml:32-37`,
`siderita/qml/dialogs/MediaPreview.qml:197-202`. DESIGN §6.1 promises a
"shared track/fill/focus/keyboard anatomy" and §7 a hover and pressed state.

**STY-L1 — Low — `CelestinaScrollBar`'s grab and hover area is only 8 px
wide, 4 px painted at rest.** `CelestinaScrollBar.qml:25-26, 76-77, 94-100,
120-121, 126-129`: the thicken-on-hover affordance fires only once the
pointer is already inside the same 8 px strip. Consumers:
`grafita/qml/components/DocumentView.qml:224-238`,
`siderita/qml/dialogs/QuickLookView.qml:229-246`,
`GrafitaEditorDialog.qml:279-293`.

**STY-L2 — Low — `CelestinaIconButton` offers no minimum, so a consumer can
shrink it below the floor.** `CelestinaIconButton.qml:13-17`; the one
consumer that does is `siderita/qml/components/chrome/TabStrip.qml:131-137`
(24×24, see SID-L1).

**STY-L3 — Low — `GlassMenuItem` uses the same fill for `highlighted` and
`current`** (`GlassMenuItem.qml:152-156`): in a choice menu the keyboard
cursor and the current value look identical, distinguished only by the check
glyph.

**STY-L4 — Low — `CelestinaModalLayer` keeps its own content clickable during
the exit fade.** `CelestinaModalLayer.qml:18-19, 145-151, 180-185`: only the
shield is gated on `layer.visible`; the foreground stays enabled for the
100 ms fade, so a double-click on a dialog's primary button sends the
request twice (`siderita/qml/dialogs/NamePromptDialog.qml:57-65`,
`CompressDialog.qml:72-79`, `ConflictDialog.qml:17-27`).
`tests/tst_modal.qml:243-254` guards the surface below, not the layer's own
content.

### Verified sound

- `CelestinaInputShield` grab semantics: the zero-threshold `DragHandler`
  cannot take a point another item already grabs exclusively, so a Button,
  MouseArea or text field inside a shielded surface keeps its click and
  selection (`siderita/tests/qml/tst_celestina_input_shield.qml:124-141`,
  `celestina-style/tests/tst_modal.qml:227-241`). Wheel passes through the
  shield, the scroll bar and the slider; the modal layer swallows it by
  design.
- `CelestinaModalLayer` scrim: armed through the fade, dismisses only on
  `layer.shown`, does not fall through; every one of Siderita's twelve modal
  cards carries an `anchors.fill` MouseArea, so a click on empty card space
  does not dismiss. Focus containment and Escape, including the three
  consumers that own Escape themselves.
- `GlassContextMenu` is `modal: true` with `CloseOnPressOutside`; every
  consumer opens it from `onClicked`, so press-release-across-open activation
  is unreachable. `GlassMenuItem` disabled rows are skipped by `QQuickMenu`.
- `CelestinaButton`/`CelestinaIconButton`: hover, pressed, disabled and
  focus-ring states defined; disabled resolved before hover; `Accessible.name`
  degrades to the icon key. `CelestinaFocusRing`, `GlassSurface`, `GlassCard`,
  `CelestinaShadow`, `CelestinaBackdrop` carry no handlers.
- Siderita: marquee passes presses over items through; delegates take all
  three buttons with Ctrl/Shift selection and a threshold-8 drag takeover;
  `TopBar` crumbs sit above the path MouseArea and editing hides the pill;
  `Main.qml` sidebar modal shield and history gating; tab chips' shield with
  a sibling `chipMouse` above it, so chip hover works.
- Fluorita: right-click paths and `Keys.onMenuPressed` in grid, list and
  sidebar; single-click open with a double-click guard (`Main.qml:69-71`);
  `ContentArrows` bands are hover-only and non-blocking; `ContentDock` strip
  is `enabled: revealed`; `MpvVideoItem` has no pointer handling; `ImageView`
  handler stack; `SidebarRow` disabled visual matches its MouseArea.
- Grafita: both dialogs put the card MouseArea before the content; tab close
  button is a later sibling of the drag area and middle-click closes; the
  editor's `selectByMouse` keeps the grab from the Flickable; Ctrl+wheel
  handler is modifier-gated; footer and encoding button states match the
  Rust guards.
- Magnetita: every Rust invokable bounds-checks its index; `MirrorChoiceRow`
  and `DeviceControls` buttons carry proper roles, `enabled: mirrorAvailable`
  and 30/52 px targets.

## Limits

- Static reading only: no binary ran, so no finding was reproduced on a
  compositor, and the Qt delivery rules relied on (sibling-only `z`, disabled
  items dropped from pointer targets, leaf-first hover with blocking
  handlers, `Shortcut` resolving before `Keys.onPressed`, passive grab of a
  `TapHandler`) are from Qt 6 source and documentation, not from a probe.
- Coverage is by file, not by pixel: hit sizes were read from bindings and
  tokens, so a target that is enlarged by a parent's layout at runtime could
  read smaller here than it is.
- Product-copy language, contrast and motion were not assessed; DESIGN's
  press-recoil gap (no recoil exists on `CelestinaButton`) is a contract gap
  noted in passing, not a clickability defect.
- The Celestina shell was not read, by instruction; nothing here says
  anything about it.

## Follow-up

**2026-09-02, later the same day.** The author authorized fixing every
finding. Five deliveries followed, one per owner, each with its own PATCH
bump and history row: `celestina-style-bug` (426c1f7, STY-M1 to STY-L4,
with new tests for the slider, the menu item, the shield's `blockHover` and
the modal exit fade), `siderita-bug` (a6e9e1a, SID-C1 to SID-L3, with
`tst_sidebar_rows.qml` and extended floating-surface and operations-dock
tests), `fluorita-bug` (0383921, FLU-C1 to FLU-L3), `grafita-bug` (53f048d,
GRA-H1 to GRA-L3) and `magnetita-bug` (7583a93, MAG-H1 to MAG-L2). Two
findings closed narrower than proposed: MAG-M1 is a plain progress bar, not
a seek control, because the daemon has no seek verb; MAG-M2 keeps the
switch on the daemon's confirmed state rather than an optimistic one, per
Magnetita's local contract. The slider's wheel is opt-in (`wheelEnabled`)
so hosts with their own wheel rounding keep it. None of it ran: the
environment has no Qt, so every fix and test is written from Qt 6 delivery
rules and awaits the author's `qml-tests.sh` and each consumer's
`complete-production.sh`.

Originally recorded as: None registered. Each project owns its rows: `siderita:` for SID-*,
`fluorita:` for FLU-*, `grafita:` for GRA-*, `magnetita:` for MAG-*, and
`celestina-style:` for STY-* (with a `complete-production.sh` run for every
affected consumer once STY-M1, STY-M2 or STY-L4 lands). A remediation unit
needs its own ledger; this record grants no authority to fix.
