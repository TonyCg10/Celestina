# What the window costs, and what it shows

- **Opened:** 2026-08-20
- **Plan ID:** what-the-window-costs-and-shows
- **Status:** active
- **Scope:** siderita
- **Implementation checkpoint:** SID-A4
- **Author-validation checkpoint:** `VAL-SID-10` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

An audit the author asked for, and the defects using the result turned up. The
audit measured rather than guessed, and its two findings were real: a folder
being written to burned 124 ms of CPU per change even when nothing visible had
changed, and a launcher cost 165 `stat` calls per cell with no cache. Fixing
those, and the interface defects found while checking them, is one checkpoint.

## Tangible outcome

A watched folder costs almost nothing while it is quiet, the heading has the
three states the author asked for, and the path bar names the folder on screen.

## Scope

- Publish nothing when a rescan finds the listing unchanged, and tell the view
  what changed rather than resetting it.
- Cache icon resolution and search the theme this session actually uses.
- One filesystem watch per folder, shared across tabs.
- `Ctrl+V` in every view mode; the crumbs read from the published location.
- The heading's three states, their gestures, and the media button that has to
  survive the heading retiring.
- Boxes that share an edge share the distance to it and the corner it takes.

## Exclusions

- Publishing row *patches* from Rust rather than whole columns: measured, and
  what remains at 50 000 entries is the rescan itself.
- The author's own pass on the live session, tracked as `VAL-SID-10`.

## Build order

Derived from the scope above, in the order each step depends on the one before
it. The audit's two findings come first because the interface defects were
found while checking them.

1. Stop publishing when a rescan finds the listing unchanged, and tell the view
   what changed instead of resetting it — this is the 124 ms.
2. Cache icon resolution against the theme the session actually uses, which is
   the launcher's 165 `stat` calls per cell.
3. Share one filesystem watch per folder across the tabs that show it.
4. Read the crumbs from the published location rather than the history, so they
   name the folder on screen.
5. Bind `Ctrl+V` in every view mode, off a paste state that does not wait for a
   menu to refresh it.
6. Build the heading's third state, put the threshold on the transition that
   needs it, and keep the media button placed after the heading retires.
7. Give boxes that share an edge the same distance to it and the same corner,
   from the two tokens `celestina-style` carries.

## What the audit measured

| | before | after |
|---|---|---|
| 2 000 entries, change nothing visible | 124 ms | 3 ms |
| 2 000 entries, a file appears | 98 ms | 8 ms |
| 50 000 entries, a file appears | 210 ms | 120 ms |
| A launcher cell, uncached | 165 `stat` | 2 µs from cache |
| Three tabs on one folder | 3 kernel watches | 1 |

Scan and projection were never the problem: 41.9 ms for 50 000 entries, 1.7 ms
to project. The cost was `beginResetModel`, which drops every delegate.

## Defects found while checking it

- **`Ctrl+V` did nothing in grid.** The shortcut was gated on `canPaste`, a
  state only refreshed when a menu opened.
- **The crumbs named the previous folder.** They read the history, which commits
  only after a scan succeeds; everything else reads the published location.
- **The heading.** Three states were needed and two were built; the threshold
  went on the wrong transition; `mapToItem` placed the media button once and
  never again.

## Implementation exit

Close `SID-A4` when a quiet watched folder costs single-digit milliseconds per
change, the crumbs follow the published folder, the three heading states each
have their gesture, and `scripts/complete-production.sh` deploys those bytes.

## Change and commit ledger

`celestina-style` carries the two tokens this needed — the window margin every
first-level box keeps, and the rule that decides a box's corner from it. They
are the design system's, with its own registered prefix, so they land as their
own atomic commit rather than inside this unit.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-A4-A | `siderita:` | done | [inventory](../../inventories/2026-08-20-what-the-window-costs-and-shows/SID-A4-A.numstat.tsv) | 36 files, +1501/-233 | The audit's fixes and the defects found checking them: quiet rescans, cached icons, one watch per folder, `Ctrl+V` everywhere, crumbs from the published location, three heading states, aligned boxes | [evidence](../../evidence/2026-08-20-what-the-window-costs-and-shows.md) | `VAL-SID-10` |

Like every plan in this repository, this one records intent and grants no
authority.
