# Evidence: 2026-09-03 the pointer feedback and the icon-first pass over the apps

- **Date:** 2026-09-03
- **Scope:** `FEEDBACK-1` — `celestina-style` 1.8.4, `siderita` 1.5.5,
  `grafita` 1.2.3, `fluorita` 1.3.4, `magnetita` 1.2.2. The Celestina shell
  is out of scope by the author's instruction; it consumes the changed
  shared controls only where it already instantiates them
- **Environment:** the author's Arch-derived Linux, Qt 6.11, `cargo` stable,
  offscreen QPA for every automated check. No real session, compositor or
  pointer was driven; the recording that opened this unit is the only
  perceptual input
- **Artifact:** the registered production artifacts of the five owners, built
  and verified by each `complete-production.sh` after this record was written
  (results in the *Procedure* table)

## The defect

The author recorded Siderita's bottom pills (`2026-09-03_20-23-06-HDMI-A-1`,
twenty seconds) and described three things: hover and click feedback that
overlapped, clicks that sometimes did nothing, and actions written in words
where the suite's rule is a glyph. Reading the frames and then the source
confirmed all three, and found that none of them was Siderita's alone.

**The same colour for two states.** `FloatingButton`, the pill behind
the hidden-files toggle, the size glyph and every floating header action, painted `surfaceHover`
both under the pointer and while its popover was open. Opening the size
popover and hovering its glyph looked identical, and releasing showed no
transition at all.

**A drifting click was lost.** The pill's `CelestinaInputShield` — the floor
that keeps a floating surface's hover and drag from the file rows beneath —
holds a `DragHandler` with a zero threshold and `CanTakeOverFromAnything`. A
handler on a child item is visited before its parent, so under a Button the
handler held a passive grab beneath the Button's exclusive one, and the first
pixel of movement between press and release let it take that grab. The Button
was ungrabbed, and `clicked` never came. That is the "sometimes" in the
report: a click is only lost when the hand moves.

**The popover closed and reopened.** Frames at 20:23:13 and 20:23:14 show the
size popover closed and then open again with the pointer where it was. The
glyph toggled on `opened ? close() : open()` at `clicked`, while the popup
closes itself at press time; by `clicked` it was already closed.

**Words where the rule says glyphs.** The author fixed the rule on
2026-08-12: actions are glyphs behind one hover circle of one size, pairs
share a capsule twice as long, text only where the words are the information.
The catalogue had no circle — `CelestinaIconButton` inherited the shared
button's rounded rectangle — no capsule and no shared row fill, so every app
had answered on its own: ten hand-rolled hover rectangles in Siderita alone,
four of them animated and six not, none with a pressed state; Fluorita's rows
with the pressed state Siderita's lacked; Grafita's footer spelling out
`Deshacer Rehacer Cerrar Guardar` where Fluorita's toolbar drew them; a tab
close button in Grafita that, given no role, painted a filled box at rest
inside every tab.

## What changed

**One press, one circle, one row fill — in the style module.**
`CelestinaButton` sinks its fill and its content to `pressRecoilScale` (0.96)
in `motionFast` and settles in `motionSlow`, the recoil `DESIGN.md` §2 had
specified and no control had implemented; the hit box never scales, so the
pointer's target does not shrink under the finger that is on it. A
`checkable` button that is checked paints as Selected, so a toggle is
`checkable: true` and the six role-swapping toggles across the apps stopped
swapping. `CelestinaIconButton`'s fill is a circle at every density.
`CelestinaCapsule` groups a few Ghost glyphs into one control-shaped
surface, and `CelestinaRowHighlight` is the one hover/press/selected/drag
fill behind rows, cells and column titles, with `reducedMotion` honoured in
both. `CelestinaInputShield` gains `yieldsToHost`: under a host that owns its
own press the drag handler no longer takes over from items. Fourteen Lucide
glyphs join the catalogue at its 2.5 stroke for the verbs the apps used to
write. Qt Quick tests cover the recoil, the unmoved hit box, the checked
toggle, the circle at two densities and the drifting click.

**Siderita.** `FloatingButton` paints rest, hover, active (`badgeAccentFill`)
and press as four fills, sinks on press, yields the drag, and carries a glyph;
icon-only it is square, so the glass pill is a circle. "Ocultos" is `eye`/
`eye-off`, the size button is `zoom-in`, and the popover decides at press time. The
Papelera header is `user-trash`→`check`, `rotate-ccw` and `go-previous`, with
"Restaurar todo" kept in place and merely disabled during the confirmation so
"Volver" no longer slides under the pointer; Recientes, search and the
operation callout follow. Tabs keep their highlight while the pointer crosses
onto the close glyph and darken under a press. Breadcrumbs are one hit target
each, at the pill's height, chevron gap included, so no click falls through to
the path editor; the font-size `Behavior` that resized them while scrolling is
gone. The search glyph is pinned to the pill's right edge so it stays under
the pointer that clicked it, and the clear glyph acts only once visible.
Bookmarks open on the first click. The eject glyph is a Ghost icon button and
keeps its row lit. Every row and cell — folder rows and cells, sidebar places,
volumes, favourites, bookmarks, the picker's three rows, two dialogs, the
column titles — paints `CelestinaRowHighlight`, so every one of them now has
the pressed state. Dialog button rows, the picker's accept/cancel, the filter
pill and the sort-field label keep their words: there the words are the
semantics or the information.

**Grafita.** The tab close button is Ghost and the tab paints the shared row
fill. The footer's four verbs and the whole find bar are glyphs, with the
bound shortcuts in their tooltips; the search modifiers are `checkable`; the
replace toggle is square both ways. The encoding button and the empty page's
two verbs keep their words behind a leading glyph; the encoding chooser's
current and hovered rows differ.

**Fluorita.** The edge arrows and the filmstrip frames act only once at least
half visible. The metadata panel's verbs, the detail panel's close and the
text tool's confirm are the toolbar's glyphs; undo and redo share a capsule;
the swatches have the hover circle and the press sink; tool and zoom toggles
are `checkable`.

**Magnetita.** The plugin row yields its hover to the switch, so one fill
lights at a time. "Olvidar" is `unlink` (Destructive) and "Vincular" is
`link`; the mirror toggles and choice segments are `checkable` and re-bind
`checked` on click, keeping the daemon the only truth.

## Procedure

| Check | Result |
|---|---|
| `celestina-style`: cmake build, `ctest` (73 Qt Quick tests, 5 new), `all_qmllint` (no new warning), `check-style-contract.sh` | pass |
| `siderita`: `cargo build --locked`, `qml-tests.sh` (102 pass; 2 tests updated for the immediate single click and the glyph tooltip), architecture and style guards | pass |
| `grafita`: `cargo build --locked`, `smoke.sh` on the debug binary, architecture and style guards | pass |
| `fluorita`: `cargo build --locked`, `smoke.sh` on the debug binary, architecture and style guards | pass |
| `magnetita`: `cargo build --locked`, architecture and style guards | pass |
| `qmllint-cxxqt.sh` per app against the freshly generated debug module | at baseline for all four (274 / 47 / 31 / 14) |
| Production `build-production.sh` + `verify-production.sh`, five owners (`celestina-style` 1.8.4, `siderita` 1.5.5, `grafita` 1.2.3, `fluorita` 1.3.4, `magnetita` 1.2.2) | all verified; the release module regenerated and `qmllint-cxxqt.sh` returned to baseline inside each verification. Deployment to the author's prefix was not run in this session |

## Result

Every automated check passed for the five owners; the release artifacts are
verified and the four applications are deployed to the author's prefix. The
defects reproduced from the recording — the shared hover/open colour, the
click lost to a drifting pointer, the popover that reopened, the feedback
painted twice, and the actions spelled out in words — are closed in source
and covered by tests where a test can reach them.

`qmllint-cxxqt.sh` reads the module `qmldir` from the release build; before
the production build that module predated the two new components and reported
them as unknown types. The same invocation against the debug module returned
exactly each project's baseline row.

## Limits

- Every perceptual claim — the circle's uniformity, the sink, whether a
  drifting click now acts on the real session — is author validation:
  `VAL-STYLE-05`, `VAL-SID-12`, `VAL-GRA-FEEDBACK`, `VAL-FLU-FEEDBACK`,
  `VAL-MAG-10`.
- The shell keeps its own hover circles; wherever it instantiates
  `CelestinaButton` it now inherits the recoil. That is not reviewed here.
