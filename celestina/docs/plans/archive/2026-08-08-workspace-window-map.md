# WMAP-1 — the workspace window map

- **Opened:** 2026-08-08
- **Plan ID:** workspace-window-map
- **Status:** done
- **Closed:** 2026-08-08
- **Successor:** [PANEL-1 — borderless glass panel](../active/2026-08-08-panel-glass-redesign.md)
- **Scope:** celestina
- **Implementation checkpoint:** WMAP-1
- **Predecessor:** [WSG-1 — workspace groups survive their monitor](../archive/2026-08-08-workspace-monitor-groups.md)
- **Author-validation checkpoint:** `VAL-WMAP-1` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

`WSG-1` made a displaced monitor's workspaces legible by folding them behind a
capsule, and in doing so took something away: a capsule says *five workspaces,
one urgent* and nothing about what is in them. The person now has to focus a
workspace to find out whether it holds the thing they were looking for.

Niri already publishes enough to answer that without focusing anything. Each
window carries its title, its application id, whether it is focused, floating or
urgent, and — the part that matters — its real place in the layout:
`layout.pos_in_scrolling_layout` gives its column and row, and `layout.tile_size`
gives its proportions. A surface drawn from those is not a list of window names;
it is the arrangement the person built, in the positions they built it in.

## Tangible outcome

The right button opens an anchored card listing what a workspace — or every
workspace a capsule folded — holds: each window as a row carrying its
application's icon, its title and its application id, in the reading order the
core folds them into. Focused and urgent windows are distinguishable. Choosing a
row goes to that window; choosing a workspace's own row goes to the workspace.

The left button is unchanged on a workspace and opens a capsule's group in the
strip, so the one-gesture route to a workspace is never taken away.

*(The outcome above was rewritten during `WMAP-1-D`: the gestures and the drawing
both changed at the author's direction after using the first draft. The reasoning
for each change is recorded with that unit below.)*

## What is not possible, established before designing

No live window preview exists and none is proposed. Wayland gives a client no
access to another client's buffers; Niri's own overview is composited inside the
compositor, which owns them. The IPC surface was checked rather than assumed —
`outputs`, `workspaces`, `windows`, `layers`, `pick-window`, `overview-state`,
`casts` and the rest expose no window pixels. The capture protocols that do exist
capture whole outputs, or require the portal's per-window picker.

The layout map is therefore not a degraded thumbnail. It is a different and
truthful thing: real geometry with real names, and it stays correct while a
thumbnail would go stale.

## Scope

- Pure layout-map policy in `celestina-shell-core`: turning a bounded list of
  windows into ordered columns and rows with relative proportions, and the
  bounds that keep a hostile title or an absurd tile size finite.
- The windows a workspace holds, published on the existing Niri snapshot
  additively. The adapter already receives them; today it keeps only the active
  window's title.
- Generalizing the anchored-card recipe. `AnchoredMenu` owns the placement and
  dismissal contract that `placeCard` in `panelmenucontroller.cpp` depends on,
  but sizes itself from a `Menu`. A map is a board, not a list of items, so the
  geometry must come from the card's content rather than from a menu — one owner
  for "a card the host places at a point", not a second parallel recipe.
- The map surface itself, with keyboard traversal, visible focus, assistive
  names for every row, outside-click and Escape dismissal.
- The gestures that open it, and going to one window rather than its workspace.

## Exclusions

- Window thumbnails, screencopy, portal capture and any second route to pixels.
- Moving, closing, resizing or reordering windows. This surface asks to focus a
  workspace and nothing else; a map that could rearrange the session is a
  different feature with a different risk.
- Replacing Niri's own overview, or driving it.
- Changing what a left click on a workspace does. The one-gesture focus stays.
- Restyling the strip, the panel or the capsule beyond what the map needs.
  `UX-2` and SHELL-D5 keep the shell-wide language.

## Build order

1. **WMAP-1-A — Publish the windows a workspace holds.** Extend the Niri
   snapshot additively with each workspace's bounded window list: title,
   application id, column, row, tile proportions and the focused/floating/urgent
   states. Bound the count per workspace and the text as the existing fields are
   bounded, and keep every current field byte-identical for a consumer that
   ignores the new one.
2. **WMAP-1-B — Own the map as pure policy.** Fold a window list into ordered
   columns and rows with relative proportions, decide the reading order, and
   answer what an empty workspace shows. Domain tests only.
3. **WMAP-1-C — Generalize the anchored card.** Make the placement contract size
   itself from its content so a board and a menu share one recipe, with the
   existing menus proving no behaviour changed.
4. **WMAP-1-D — Draw the map.** The card, its rows, the group and single
   workspace cases, the gestures that open it, going to one window, keyboard
   traversal, visible focus, assistive names and the reduced-motion path.
5. **WMAP-1-E — Deliver.** Bump the registered MINOR version, append the history
   row, run the registered guards and the canonical production exit, deploy
   without activation, and record only the live cases the author performs.

## Implementation exit

- Every existing snapshot field is unchanged and the added list is optional to
  every current consumer, including a host that predates it.
- A workspace with no windows, a window with no title, a hostile title, an
  absurd tile size and a workspace past the window cap all render bounded.
- No QML file decides layout order; it presents what the adapter published.
- The two existing indicator menus keep their placement, dismissal and focus
  behaviour after the card is generalized, proven by their current tests.
- Left-clicking a pill still focuses its workspace in one gesture.
- `bash scripts/check-architecture-contract.sh`, the registered project verify
  script, `python3 scripts/version_tool.py check`, exact staged-unit checks and
  `scripts/complete-production.sh` pass before delivery.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| WMAP-1-A | `celestina:` | done | [inventory](../../inventories/2026-08-08-workspace-window-map/WMAP-1-A.numstat.tsv) | 34 files, +2776/-216 | Show what a workspace holds and let a person go to one window | [evidence](../../evidence/2026-08-08-workspace-window-map.md) | `VAL-WMAP-1` |
| WMAP-1-B | `celestina:` | done | [inventory](../../inventories/2026-08-08-workspace-window-map/WMAP-1-B.numstat.tsv) | 14 files, +711/-416 | Archive the delivered map plan and open the bounded borderless-glass panel successor | [evidence](../../evidence/2026-08-08-panel-checkpoint-transition.md) | None |

## Active unit boundary

No unit is open. `WMAP-1-A` is delivered and inventoried, and `WMAP-1-B`
records the administrative archive and successor transition. Anything found
from here is a new unit rather than a widening of either one.

### Why one unit and not the five the build order names

The build order was written as five steps and they were built as five steps, but
they are **one delivery**: the wire shape, the fold, the card recipe, the surface
and the version transition all reach the repository in a single commit, because
none of them is separately usable. An inventory bounds one commit, and five of
them over the same commit would have to divide files that several steps changed —
`niri_adapter.rs` alone belongs to three. That is not a boundary, it is
arithmetic dressed as one.

So the ledger records what was delivered rather than the order it was built in.
The five steps and their reasoning stay below as the account of how it was
reached; the unit is what landed.

The checkpoint is closed for implementation. `VAL-WMAP-1` has not been run, and
by the exit rule that does not keep it open — a failure there creates a
corrective unit rather than reopening this.

### Recorded during WMAP-1-A — two decisions and one gap

**Tile sizes stay floating point.** They were first published as whole pixels
and are not any more. Rounding here would be the helper inventing a precision
the layout does not have, and the surface uses them as ratios in any case. The
sanitizing rule that matters is separate and stayed: a size that is not finite
and positive is published as zero, because a `NaN` makes the frames either side
of it compare unequal for ever and would republish the snapshot on every
compositor event.

**Ordering belongs to the adapter.** Columns left to right, rows down inside a
column, and anything with no place in the scrolling layout after all of it. It
is a property of the compositor's layout rather than of any surface, and two
surfaces sorting it separately would be two owners for one rule.

**Gap: the host decoder has no unit test.** `NiriClient::applyMessage` is
private and the object starts its own `QProcess`, so the decode is not reachable
from a test without giving that class a public seam it does not otherwise need.
No hook was added and no coverage is claimed. The wire contract is proven by the
adapter's own tests and by one real snapshot captured from the live session,
which carried the two 942-wide columns and the 1896-wide one that were actually
on screen. The host's acceptance of the field — including a frame that omits it
— is answered at the surface in `WMAP-1-D`, and that unit owns closing this.

### Recorded during WMAP-1-E — three version sources, not one

The MINOR transition to `0.9.0` is a `milestone`, and it had to be written in
three places: `CMakeLists.txt`, `Cargo.toml` and `Cargo.lock`. Editing one and
running `version_tool.py check` is what caught the other two disagreeing, which
is the point of that guard existing — a bundle whose host and helpers disagree
about their own version is one nobody can reason about afterwards.

### Recorded during WMAP-1-B — sizes leave as shares, and truncation is visible

**The map publishes shares, not measures.** Each column carries its fraction of
the map's width and each tile its fraction of its column's height, and both
always sum to one. A surface multiplies a share by whatever room it has. It
never receives a pixel count it might use as one, and it never has to decide
what an impossible size means.

**An unknown proportion is drawn as an equal one, deliberately.** A column whose
windows all report an unusable height divides itself evenly, because the map
genuinely does not know their proportions and equal tiles say exactly that.
Inventing a difference would be worse than admitting there is none. No input —
`NaN`, infinity, negative, zero, an empty column — can produce a share that is
not finite and positive, which is what keeps a broken frame from reaching a
layout as a surface that silently fails to draw.

**A truncated map says how much it is hiding.** `Map::hidden` counts what the
column and window bounds dropped. A surface showing four of nine windows must be
able to say so; showing four silently is the map lying about the one thing it
exists to answer. The dropped columns are the rightmost, because a scrolling
layout is read from the left.

**A floating window is kept out of the layout**, even when it claims a column.
It sits over the arrangement rather than in it, and folding it in would be the
map claiming a structure the session does not have.

### Recorded during WMAP-1-C — what moved, and what deliberately did not

`AnchoredCard` now owns the shadow inset, the clamp that keeps a card whole
against an output edge, the surface colour and the reduced-motion route.
`AnchoredMenu` keeps what makes it a menu: the popup, its lifecycle, the
non-modal decision and the `menu` handle consumers feed items through. It is a
specialization rather than a pass-through — it adds state and an API of its own —
so the two files are one path, not two.

**The host was not touched.** `placeCard` reads `shadowMargin` and writes
`menuX`/`menuY` by name, and those names stayed even though `menuX` on a board of
window tiles reads oddly. Renaming them would have been renaming an inter-object
contract for tidiness, against a host this unit has no reason to change.

**Sizes are declared by the consumer, not derived from the children.** The right
measure differs by content, and getting it wrong is not a rendering nuisance but
a layout loop: a `Popup` fits itself to its window, so a window that sized itself
to the laid-out popup shrank both by one margin per pass until the surface was a
sliver. `contentWidth`/`contentHeight` make the consumer name the measure that is
stable for the thing it draws.

**A consumer opens its content from `ready()`, not from
`Component.onCompleted`.** Both would be handlers for the same attached signal on
the same object, so a derived file's handler silently replaces the base file's —
and the base file's is where reduced motion is applied. The signal makes the two
compose instead of one shadowing the other.

**`backdrop` is named rather than reached for.** The glass samples an item inside
`AnchoredCard`, and a consumer never has to know its id.

### Boundary refined during WMAP-1-D — the fold reaches the wire

`WMAP-1-B` produced a module with **no consumer**. That is a defect in how the
work was split rather than in the module: the surface cannot fold the layout
itself without QML deciding domain policy, and a second fold anywhere would be a
second owner for a rule that already has one. So the wiring joined this unit
before any surface was drawn, and the adapter's own hand-rolled ordering — added
in `WMAP-1-A` — was deleted in the same change rather than left beside it.

The wire shape changed with it. `WMAP-1-A` published a flat window list carrying
the compositor's measures; a workspace now publishes the folded map: columns with
their width shares, windows with their height shares, the floating windows kept
apart, and the count of what the bounds dropped. Nothing consumed the flat list,
so this replaces it rather than deprecating it.

Two bounds also stopped being duplicated. The adapter's own
`MAX_WINDOWS_PER_WORKSPACE` is gone and `workspace_map::MAX_WINDOWS` and
`MAX_COLUMNS` are the single answer; the host's constants name them as the source
rather than restating a number that had already drifted — 32 against the core's
64, which would have silently dropped half a busy workspace.

Verified end to end against the live session, not only in tests: the three
columns published `0.249`, `0.249` and `0.502`, which are the real proportions of
the two half columns and the full one on screen.

### The application icon — decided by the author, and why it needed deciding

The author asked for the icon after the preview question was settled, so it is
in. It needed a decision rather than an assumption for two reasons, and both are
answered in `AppIconProvider` rather than waved through.

**The catalogue.** The suite's icon catalogue is closed, and foreign tray icons
are its stated exception. An application's own icon is the same category — it is
that application's identity, not a first-party glyph — so this stands on the
existing exception rather than widening it. Nothing here invents a symbol, and
the same `configureForeignIconThemes()` the tray installs is what makes the
lookup resolve at all.

**The recorded audit finding.** "GUI-thread icon decode" is a low finding the
static audit already recorded, and a map multiplies it by every tile. Forcing the
provider asynchronous would move the decode off the GUI thread at the price of a
race in Qt's own icon loader, which does not promise thread safety. So the answer
is the cache: an application and size is resolved once for the life of the
process — misses included, so a name no theme knows is not looked up again on
every frame — and every later tile is a hash lookup. A map redrawn continuously
costs nothing after its first draw.

An id that resolves to nothing shows the window's title alone. The tile never
becomes an empty square, and nothing is drawn that claims to be an icon it is not.

### The surface, and the two things it refuses to do

`WorkspaceMap` carries one board per workspace, three across before wrapping, so
a group of five is two readable rows rather than a wall. A board multiplies the
helper's shares by the room it has, which is what makes a half column read as
half instead of as one of three equal boxes.

It **asks to focus a workspace and does nothing else** — no moving, closing or
rearranging. And it **never claims to show everything**: a board past the bounds
says how many windows it is not showing, and a workspace holding nothing says so
in words rather than being an empty rectangle indistinguishable from a failure.

Still open in this unit: keyboard traversal through the boards, the hover route
from a pill and its dismissal question, and the offscreen contract tests.

### Redefined by the author during WMAP-1-D — the gesture model

The model recorded before implementation was wrong in one place, and the author
corrected it after using it. What stands now:

- **Left button.** On a workspace it goes there, as it always did. On a capsule
  it **expands that group in the strip**, and clicking one of the revealed
  workspaces goes there. Expansion is therefore a gesture rather than something
  that follows the focus, and it outranks the published state until the focus
  lands in another group.
- **Right button.** Opens the map, replacing the panel's own workspace menu on
  that gesture. On a workspace it shows that workspace's windows; on a capsule,
  every window the capsule folded.
- **Clicking a window goes to that window**, which the first draft did not offer
  at all.

**The hover route is gone, and its recorded risk with it.** It existed only
because the old model left a single workspace's map with no gesture of its own.
The right button is that gesture, so the question of how a hover-opened surface
dismisses without a click no longer has to be answered — it stopped existing
rather than being solved.

**The old panel menu was removed, not left beside it.** Moving the map onto the
right button left `PanelMenu.qml` with no emitter, so the QML, the controller's
component and `open()`, the manager's slot and its connection all went. Two
surface tests that loaded it were retargeted at the map, where they still hold
the same contract: a card that cannot collapse into a sliver.

### Focusing a window — a verb, and one thing it deliberately does not do

Going to a window needed the protocol to carry the window's id and a typed
`focus-window` command beside `focus-workspace`. The id is parsed in the helper
rather than forwarded, so a value that is not a number is refused with a visible
failure instead of handed to the compositor.

It is **not tracked as a pending request**, unlike a workspace focus. A pill has
to show what became of a click because the pill is still on screen; a window
chosen from a map closes the map, and the answer is the session moving. There is
nothing left for an outcome to be reported to, so a ledger entry would be
bookkeeping nobody reads. The id is still validated against the last published
snapshot: a window that closed while the map was open is a stale click.

### The bug that made it look broken

Clicking a window did nothing, and every layer of the pipeline was sound — the
compositor focused by id, the helper accepted the command and moved the session,
and the map exposed the signal the host connects to. All three were measured
before anything was changed.

The fault was the surface being in front of itself. The dismissal layer was
declared after the card, and among siblings stacking follows declaration order;
the `z: -1` inside it ordered that child against *its own* siblings, not against
its parent's. So a full-output press target sat over every row. It is declared
first now, and the surface test asserts the signal signature the host connects
by, so a future drift there fails loudly instead of silently swallowing clicks.

### The list, and what was given up

The map was drawn as the layout itself — columns side by side at their true
proportions — and the author found it unreadable, correctly: a column of a
three-column workspace is a third of a narrow card, and every window name in it
elided to nothing. A map whose labels cannot be read has stopped answering the
question it exists for.

The arrangement is now told in the **order** rather than in the geometry: rows
arrive from the core already folded, and the board lists them at full width. The
shares the helper computes are no longer drawn, though they are still published
and still correct — a later revision can group rows by their column without the
adapter changing at all. That is a real loss of information for a real gain in
legibility, and it was the author's call after seeing both.

Each row now carries the application's icon, its title and its application id.
Hover and press are animated through `motionFast` and the animation is disabled
under `CelestinaTheme.reducedMotion`.

### Keyboard

The card walks one flat list — each workspace's own row, then that workspace's
windows, board after board — rather than chaining focus through delegates. The
boards sit in a grid, and a chain would make "down" mean whatever the grid
instantiated next; a list is the order the card is read in. The cursor starts
negative so a card opened by pointer paints no ring nobody asked for, arrows
wrap at both ends, and Return takes the place the cursor is on. Space is left
unbound because no control here owns it.

## Settled interaction decisions

Recorded at the author's direction on 2026-08-08, before implementation:

- **A capsule opens the map; a pill still focuses.** A capsule is a container
  rather than a workspace, so opening it to choose is its natural gesture, and
  the strip keeps its one-gesture route to a workspace. A single workspace's map
  opens from its pill on hover with a short dwell.
- **The surface is a card anchored to the control that opened it**, sharing the
  dismissal and focus behaviour the network and Bluetooth menus already have,
  rather than a large centred board with its own geometry.

### Recorded risk — the hover route

A transient surface that opens on hover covers the output it opens on, which is
what makes outside-click dismissal work and what makes leaving by pointer
ambiguous. `WMAP-1-D` must answer how the map closes when the pointer never
clicks, and the answer belongs in that unit's evidence rather than being assumed
here.
