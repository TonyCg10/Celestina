# One presentation and departure lifecycle for panel menus

- **Date:** 2026-08-15
- **Scope:** Celestina unit `R8-P-N`
- **Artifact:** Celestina 0.29.11, verified and deployed to the normal test
  prefix
- **Environment:** the author's 60 fps recording
  `recording_20260815_195243.mp4`; source audit at
  `314db378d65617441588207990ee3142e110e3ce`; offscreen Qt 6.11.1 tests;
  registered production completion; nested Niri on `wayland-2`
- **Plan:** [polkit authentication agent](../plans/active/2026-08-14-polkit-authentication-agent.md)
- **Validation:** `VAL-PANEL-1`

## What the recording establishes

The calendar is absent at about 1.267 seconds, has its first visible card frame
at about 1.333 seconds with paint at the window's top edge over the bar, and is
already travelling below the seam by about 1.400 seconds. The phone and tray
closures show the inverse split: rows or controls leave before the material and
carrier, so a dark/glass remainder closes on a later beat. The recording
predates the two same-evening seam/clipping corrections in the checkout and is
therefore evidence of the report and its original mechanism, not an after image
of this candidate.

## Procedure

The visual language was less duplicated than the symptom suggested. Theme
tokens, ink, headers, dense sections and the outer field already have canonical
owners in `CelestinaTheme`, `BackdropInk`, `MenuHeader`, `MenuSection` and
`SoftMenuField`. Audio, brightness and calendar also share the complete
`SoftCard` lifecycle. The divergent family is the real Qt `Menu` adapter:
tray inventory, foreign tray child, phone, performance, network, Bluetooth and
capture all inherit `SoftMenu` and therefore shared the same defect.

The code audit did find four remaining ownership duplications that should not be
hidden behind this bug fix:

1. Launcher, Clipboard, Notification Centre, Control Centre, Session Menu and
   Polkit each repeat the window configure/first-frame reveal gate.
2. `PanelMenuSurface` and `OverlaySurface` repeat adoption, mapping, blur setup
   and hard destruction, which is why a direct `close()` can still bypass a
   visual retirement on replacement routes.
3. `Panel.qml` and `SoftMenuField.qml` each walk a scene to collect glass, while
   OSD and toast stacks have specialized union collectors.
4. OSD and toast departures use the same quiet-surface rhythm but remain
   semantically different controllers. Only their small presentation primitive
   is a valid future extraction; merging the controllers would erase real
   policy differences.

Those are follow-up boundaries, not justification for a broad refactor in this
corrective unit. The duplicated popup lifecycle that caused the reported frames
has been removed here.

## Causes and corrections

### Paint could precede presentation

`SoftMenu` deliberately disables an extra reveal fade because its complete card
falls out of the bar. `SoftMenuField` interpreted that as permission to paint
immediately, while glass publication still waited for reveal. The popup rows
could therefore reach a first buffer without their carrier. `presentationOpacity`
now gates every route, including non-animated and reduced-motion routes; those
choices change duration, never permission to paint. Retirement also cancels any
queued reveal or fall and freezes the last published regions so entry cannot
re-arm material during exit.

### Popup menus had two sequential departures and two opacity writers

`SoftMenu.aboutToHide` retired the field while `GlassContextMenu.exit` separately
animated the popup's opacity and scale. Only after that transition completed did
`AnchoredMenu.closed` notify C++, which started `softCloseWindow`: a second field
retirement, dense-glass collapse, global fade and 170 ms destruction delay.
`SoftMenu` also held a binding on the same popup opacity that the inherited
transition wrote directly.

The host is now notified once from `aboutToHide`, while rows still exist.
Popup exit is only a lifetime hold; attached rows and every retiring popup mirror
the field's one opacity/scale. Floating entry retains Qt's native transition and
focus ownership. `SoftMenuField.retire()` and `softCloseWindow()` are idempotent,
and reduced motion has no timer tail.

### Tray actions bypassed even that lifecycle

The tray inventory's activation routes and the foreign D-Bus child destroyed
their carrier directly. Choosing a child action could remove the window before
`Menu.aboutToHide` ran at all. Actions and same-menu toggles now request the real
popup dismissal; the child follows the same host retirement before its carrier
is destroyed. Hard close remains the fallback for a surface with no live popup
or for teardown.

### The inactive OSD twin could invent a card

The previous correction stopped seeding the `readings` list but still seeded
the four front compatibility properties. `SessionOsd` synthesized a card from a
non-empty `kind`, so the inactive persistent twin still had enough state to
paint. Both twins now start with every card property empty, and `readings` is
the only authority that creates delegates. Only `pushReadings(activeWindow())`
can populate the presenting twin. Its glass union also remains published until
the final departing delegate has completed its fade.

## Result

- `bash scripts/check-architecture-contract.sh` passes.
- The Celestina application and indicator-menu target build successfully.
- The focused indicator-menu integration passes, and the complete QML
  QuickTest runner passes 268/268, including all reveal/reduced-motion
  combinations, irreversible
  retirement, popup/field clock equality, tray focus and real disabled-row
  input.
- The complete 23-test CTest set passes when the tray-watcher fixture is given
  the private D-Bus socket its own contract requires. The restricted sandbox
  correctly prevented that one socket and the isolated unrestricted run passed.
- `celestina/scripts/complete-production.sh` builds and verifies the canonical
  0.29.11 release artifact, repeats the registered contracts and eight-second
  release smoke, deploys those verified bytes to the normal `~/.local` test
  prefix and reports the artifact current. It does not activate a live session.
- The existing nested Niri remained alive as PID 80685. The changed build-tree
  shell restarted as PID 206335 with `WAYLAND_DISPLAY=wayland-2` and
  `NIRI_SOCKET=/run/user/1000/niri.wayland-2.80685.sock`; it acquired
  `org.celestina.Shell` and mapped the panel on the nested `winit` output.

## Limits

The author still needs to record the restarted nested shell and check one
attached menu plus tray inventory, phone and foreign tray child at normal and
reduced motion. Automated frame and lifecycle evidence, successful production
completion and a mapped nested panel do not substitute for that perceptual
pass.
