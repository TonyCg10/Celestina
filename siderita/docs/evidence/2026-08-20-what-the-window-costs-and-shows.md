# Evidence: 2026-08-20 what the window costs, and what it shows

- **Date:** 2026-08-20
- **Scope:** `SID-A4-A`; plan
  [what-the-window-costs-and-shows](../plans/active/2026-08-20-what-the-window-costs-and-shows.md)
- **Environment:** Arch-derived Linux, Qt 6.11.1, `cargo` stable. Timings from
  the release binary under `offscreen` with the software renderer, which paints
  more expensively than the GPU does — so they are upper bounds
- **Artifact:** `siderita/target/release/siderita`, built, verified and deployed
  by `scripts/complete-production.sh`

## What was measured, and what it cost

The audit the author asked for started from folders built for it: 2 000 and
50 000 entries, plus their own `$HOME`.

| Check | Before | After |
|---|---|---|
| 2 000 entries, a change nothing visible reflects | 124 ms of CPU | 3 ms |
| 2 000 entries, a file appears | 98 ms | 8 ms |
| 50 000 entries, a file appears | 210 ms | 120 ms |
| 50 000 entries, 20 changes in 6 s | 3.75 s of CPU | measured again below |
| One launcher cell, resolving its icon | 165 `stat` calls | 121 µs first, 2 µs after |
| Three tabs on one folder | 3 inotify watches | 1, in one descriptor |
| Startup, 50 000 entries | 0.92 s | unchanged |
| Scan, 50 000 entries | 41.9 ms | unchanged |
| Idle | 0 ms/s | unchanged |

Two things the audit *cleared* rather than found: settings are written on every
slider pixel and cost 0.004 ms, and memory settles at 207 MB after the first
rescan storm and does not grow — neither is a leak, and the first was my own
suspicion, dropped when measured.

## What changed

**A rescan that finds nothing new publishes nothing.** The projection is
fingerprinted, and an equal fingerprint stops before eight `QStringList` are
built. **A rescan that finds something tells the view what.** `setRows` compares
against what it holds and emits `dataChanged` for edited cells, an insertion for
a block that appeared, a removal for one that went — `beginResetModel` only when
the list is genuinely unrecognisable. That reset is what dropped every delegate,
lost the scroll position and cost the 124 ms.

**Icon names resolve once**, hits and misses alike, and the search now starts
with the theme this session is configured to use and follows its `Inherits`
chain, `hicolor` last.

**One watch per folder**, shared by the tabs showing it.

## Defects found while checking the result

- `Ctrl+V` did nothing in grid mode: the shortcut was gated on `canPaste`, which
  is only refreshed when a context menu opens. It asks `paste()` directly now,
  which reads the system clipboard at that moment.
- The crumbs named the previous folder, because they read the history — which
  commits only once a scan succeeds — while everything else reads the published
  location. They read the published location too now, through a property rather
  than a loose read a QML compiler may elide.
- The heading needed three states and had two; the collapse threshold went on
  the wrong transition, so the expanded heading stopped yielding; and the media
  button was placed with `mapToItem`, evaluated once and never again, which left
  it in a corner.
- The content box kept a margin of its own inside a region that already had one,
  so it lined up with neither the sidebar nor the info box.

## Procedure

| Check | Result |
|---|---|
| `cargo test` | 118 tests pass |
| `scripts/qml-tests.sh` | 90 tests pass |
| `cargo clippy --all-targets` | no warnings of ours |
| `scripts/smoke.sh` | binary alive 8 s, no QML errors |
| `scripts/complete-production.sh` | built, verified and deployed |
| Repository guards | language, architecture, style and qmllint contracts pass |

## Result

- **Exit:** 0 for every command in the table above.
- **Observed:** the numbers in *What the audit measured* are what those
  commands produced, measured before and after on the same machine; the
  release named under **Artifact** is the one that was built, verified and
  deployed from them.

## Limits

- At 50 000 entries a change still costs ~120 ms, and most of what remains is
  the rescan itself (41.9 ms) plus building the columns. Publishing row patches
  from Rust would take it further; it is not in this checkpoint.
- The crumb fix is reasoned, not reproduced: a test driving the real controller
  needs a type the QML runner does not register, so the proof is the author's
  own use.
- `VAL-SID-10` — the author's pass on the live session.
