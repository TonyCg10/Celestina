# The table scrolling back on refresh, and applications opening folded — VIS-2

- **Date:** 2026-09-23
- **Scope:** `VIS-2` of
  [`../plans/archive/2026-09-23-scroll-and-folds.md`](../plans/archive/2026-09-23-scroll-and-folds.md):
  the two defects the author reported while running `VAL-VIS-1` on 0.6.2,
  and 0.6.3
- **Environment:** the author's checkout, an AMD Ryzen 7 9800X3D session
  under niri; no window was opened
- **Artifact:** `hematita/target/production-artifact.toml` (verified and
  deployed to `~/.local/bin/hematita` at 0.6.3)

## What changed, per defect

1. **Every refresh scrolled the table back to its top.** Cause: the
   `ListView` model in `qml/components/ProcessTable.qml` is
   `table.entries.length`; an integer model that changes is a full reset,
   and a reset regenerates the delegates at the start of the content.
   Separately, `anchorCursor()` writes `list.currentIndex` after every
   rebuild and the default `highlightFollowsCurrentItem: true` scrolled the
   view to it. The same shape was in `ServicesPage.qml`
   (`page.rows.length`) and `SensorsPage.qml` (`page.cards.length`). Fix:
   each rebuild reads `list.contentY` before the assignment and, in the same
   JavaScript turn and inside the existing `anchoring` guard (a new
   `rebuilding` flag on the Sensors page, which had none), calls
   `list.forceLayout()` so the reset length is laid out, then writes the
   offset back clamped to `max(0, contentHeight - height)` above `originY`.
   The process table does this in `weave()` and `toggleGroup()` through
   `restoreViewport(offset)`. All three lists set
   `highlightFollowsCurrentItem: false`; `onCurrentIndexChanged` calls
   `positionViewAtIndex(currentIndex, ListView.Contain)` only when the guard
   is down, which is the person's own arrow key or click, never the
   re-anchor. The integer models are kept.
2. **Applications opened unfolded.** Fix: `ProcessTable.collapsed` became
   `expanded`, where absent means folded; `layout()` includes a group's
   processes only when `expanded[id]`; `toggleGroup(id)` flips the entry;
   `foldCurrent` keeps Right to open and Left to fold; `ApplicationRow`
   binds `expanded: !!table.expanded[id]`. `weave()` drops ids of
   applications no longer published, so one that returns opens folded.

## Procedure

One `cargo fmt --all`, one `complete-production.sh`, the status script, a
plain SHA-256 comparison of the built and installed bytes, and a 12-second
offscreen walk through every section.

## Result

```text
$ python3 scripts/version_tool.py bump hematita bug --unit VIS-2 ...
hematita: 0.6.2 -> 0.6.3 (bug)
version-contract: OK (8 owners)

$ bash hematita/scripts/complete-production.sh        # exit 0
qmllint-production: OK — org.celestina.hematita (0 non-fatal baseline warning(s))
smoke: OK — binary alive for 10 s, every section was shown, the first row
published the CPU contract, the Sensors page published chips, the Services
page listed system units, no QML errors, no auto-bindings
>> verified Hematita deployed to /home/toni/.local without rebuilding
artifact: hematita current and verified
installed: OK /home/toni/.local/bin/hematita

$ sha256sum hematita/target/release/hematita ~/.local/bin/hematita
d9bd96e8e9a2d1b6e85d8c420048c4ceb81a928bde4054634b571397ee384f2f  hematita/target/release/hematita
d9bd96e8e9a2d1b6e85d8c420048c4ceb81a928bde4054634b571397ee384f2f  /home/toni/.local/bin/hematita

$ HEMATITA_SMOKE_SHAPE=1 HEMATITA_SMOKE_SECTIONS=1 QT_ASSUME_STDERR_HAS_CONSOLE=1 \
  QT_QPA_PLATFORM=offscreen timeout 12 ~/.local/bin/hematita
qml: hematita-shape cpu 3 60
qml: hematita-sensors 9 44
qml: hematita-services 188 true true
# no TypeError, ReferenceError, "Unable to assign" or "Cannot read property"
```

## Limits

The offscreen walk proves the three pages construct and run their rebuilds
without a QML error; it does not scroll, press a key or look. Whether the
viewport visibly holds across a tick, and whether the arrows and folds feel
right, is `VAL-VIS-2`.
