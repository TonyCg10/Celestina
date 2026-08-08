# WMAP-1 — what a workspace holds, and going to one window

- **Date:** 2026-08-08
- **Scope:** Celestina `WMAP-1`, unit `WMAP-1-A`
- **Artifact:** celestina 0.9.0, canonical production bundle
- **Environment:** Linux 7.1.6 (CachyOS), Rust 2021 workspace, Qt 6.9, CMake
  Release, offscreen Qt platform for the tests. One nested Niri session was used
  to look at the surface; Noctalia continued to own the live session throughout.
- **Plan:** [the workspace window map](../plans/active/2026-08-08-workspace-window-map.md)
- **Author validation:** `VAL-WMAP-1`, not run

## What this answers

`WSG-1` folded a displaced monitor's workspaces behind a capsule, which made the
strip readable and made those workspaces opaque. *Five workspaces, one urgent*
cannot say whether the thing you are looking for is in them, so finding out meant
going to each in turn.

The right button now opens a card that says. It lists every window of a workspace
— or of every workspace a capsule folded — with its application's icon, its title
and its application id, and clicking one goes to **that window** rather than to
the workspace it sits on.

## What was established before any of it was designed

There are no window previews and none are possible. This was checked rather than
assumed:

- Wayland gives a client no access to another client's buffers.
- Niri composites its own overview inside the compositor, which owns them.
- Its IPC surface — `outputs`, `workspaces`, `windows`, `layers`, `pick-window`,
  `overview-state`, `casts` and the rest — exposes no window pixels.
- The capture protocols that exist copy whole **outputs**, or need the portal's
  per-window picker.

And the argument that settles it even if a capture route existed: a workspace
nobody is looking at **is not being drawn**. For the case this feature exists for
— the workspaces of a monitor that is switched off — there are no pixels to crop.
Not hidden: absent.

## What the compositor does publish, and what became of it

Each window carries its title, application id, focused/floating/urgent states,
its column and row in the scrolling layout, and its tile size.
`celestina_shell_core::workspace_map` folds those into columns and rows and turns
the sizes into **shares** — each column's fraction of the map's width, each
window's fraction of its column's height, both always summing to one.

That fold reaches the wire. Verified against the live session rather than only in
tests: a workspace holding two half columns and one full one published
`0.249`, `0.249`, `0.502`.

The surface then **does not draw those shares**, and that is a deliberate reversal
recorded here rather than quietly dropped. The map was first drawn as the layout
itself, and the author found it unreadable — correctly: a column of a
three-column workspace is a third of a narrow card, and every window name in it
elided to nothing. A map whose labels cannot be read has stopped answering the
question it exists for. The arrangement is now told in the **order** and the rows
are listed at full width. The shares remain published and correct, so grouping
rows by their column later needs no change to the adapter.

## Bounds and refusals

- A share that is not finite and inside the unit interval is repaired before it
  reaches a layout, on both sides of the protocol. One that is not would make a
  surface silently fail to draw.
- A size that is not finite and positive is published as zero. A `NaN` would make
  the frames either side of it compare unequal for ever and republish the
  snapshot on every compositor event.
- Window and column counts are `workspace_map`'s to say. The adapter's own
  duplicate constant was removed after it had already drifted — 32 against the
  core's 64, which would have silently dropped half a busy workspace.
- A truncated map reports how much it is hiding. Listing four of nine silently is
  the map lying about the one thing it exists to answer.
- A window id is parsed in the helper rather than forwarded, and validated
  against the last published snapshot in the host: a window that closed while the
  map was open is a stale click, not a request.

## Decisions worth keeping

**Focusing a window is not a tracked request.** A workspace pill has to show what
became of a click because the pill is still on screen. A window chosen from a map
closes the map, and the answer is the session moving. There is nothing left for
an outcome to be reported to, so a ledger entry would be bookkeeping nobody
reads.

**The application icon stands on the tray's existing exception.** The suite's
icon catalogue is closed to first-party glyphs; a foreign application's own icon
is that application's identity, which is the same category a tray icon already
occupies. `AppIconProvider` resolves on the GUI thread deliberately —
`QIcon::fromTheme` reaches loader state Qt does not promise is thread-safe — and
answers the audit's recorded "GUI-thread icon decode" finding with a cache
instead: one lookup per application for the life of the process, misses included,
so a name no theme knows is not looked up again on every frame.

**The old panel menu was removed rather than left beside the new surface.**
Moving the map onto the right button left `PanelMenu.qml` with no emitter. The
QML, the controller's component and `open()`, the manager's slot and its
connection all went, and two surface tests that loaded it were retargeted at the
map, where they hold the same contract.

## The defect found during implementation

Clicking a window did nothing, and every layer was sound. Measured before
anything was changed:

- the compositor focused by id (`focus-window --id 5` moved focus from 3 to 5);
- the helper accepted a `focus-window` line on stdin and the session moved to
  window 6;
- the map exposed `windowActivated(QString)`, the name the host connects by.

The fault was the surface being in front of itself. The dismissal layer was
declared after the card; among siblings, stacking follows declaration order, and
the `z: -1` inside it ordered that child against *its own* siblings rather than
against its parent's. A full-output press target therefore sat over every row.

It is declared first now, and the surface test asserts the signal signature the
host connects by, so a drift there fails loudly instead of swallowing clicks.

## Procedure

The units were built in the order the plan names, each verified before the next
began: the wire shape, the fold, the anchored-card recipe, the surface, and the
version transition with the canonical production exit. The map was exercised in a
nested Niri session holding six terminals across four workspaces on two absent
monitors, and the wire was read directly from the adapter against the live
session rather than only through the panel.

## Automated evidence

- `celestina-shell-core`: 314 tests, including 14 for the layout fold.
- `celestina-niri-adapter`: 26 tests, including the folded map, floating windows
  kept apart, impossible measures, hostile titles and the bounds.
- Clippy and `cargo fmt` clean across both.
- CTest 17/17, including the two new map cases — keyboard traversal reaching
  every window and returning the window's own id, and a workspace published with
  no map at all still building its card.
- QML lint clean; `bash scripts/check-architecture-contract.sh` OK;
  `python3 scripts/version_tool.py check` OK.
- The canonical production exit built 0.9.0 once, verified those exact bytes and
  deployed them to the author's normal test prefix. The live session was not
  replaced.

## Result

Delivered. The right button opens the map from a workspace or a capsule, a row
goes to its window, the keyboard walks every row, and the canonical production
exit built, verified and deployed 0.9.0 without activating the live session.

## Limits

No build proves a compositor. Placement on both output scales, the legibility of
the rows over a real wallpaper, the visible focus under keyboard, and whether an
application's icon resolves on this session's themes are all `VAL-WMAP-1`, and it
has not been run.
