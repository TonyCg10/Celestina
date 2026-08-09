# Shared reading controls

- **Opened:** 2026-08-04
- **Status:** active
- **Plan ID:** shared-reading-controls
- **Scope:** celestina-style
- **Implementation checkpoint:** STYLE-G7
- **Author-validation checkpoint:** `VAL-STYLE-04` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

A scroll position and a line-number column are reading anatomy, not application
policy. Two text surfaces in two applications need the same two controls, so the
module can own them without inventing demand: the semantics are already proven
twice, and one owner is what keeps their thickness, colour, motion and delegate
budget from drifting apart in each consumer.

## Tangible outcome

`CelestinaScrollBar` and `CelestinaLineGutter` exist in the canonical module,
are declared in `qmldir` and in the QML module's file list, and are consumed
through the canonical path by Grafita's window and by Siderita's editor and
quick look. No consumer carries a copy.

## Scope

In scope: the two components; their `qmldir` and `CMakeLists.txt` registration;
the module's SemVer transition and its history row; the status entry that
records the new public types.

## Exclusions

Out of scope: any further gutter content, including diff markers, breakpoints,
folding and a minimap; a keyboard interface of the scroll bar's own; a settings
surface; and both consumers' own composition, which are the Grafita `G7` and
Siderita `SID-G7` plans. This plan records work but grants no authorization
beyond the repository rules.

## Build order

1. Add `CelestinaScrollBar`, built from QtQuick primitives so its whole anatomy
   comes from semantic tokens.
2. Add `CelestinaLineGutter`, building only the delegates its viewport shows.
3. Register both in `qmldir` and in `qt_add_qml_module`.
4. Bump the module's MINOR version and append its history row.

## Implementation exit

- `bash scripts/check-architecture-contract.sh` passes, including the style and
  contrast contracts and the raw-control scanner that would reject either
  component had it re-skinned a `QtQuick.Controls` template.
- `bash celestina-style/scripts/verify-production.sh` passes over the built
  module, so the registered type set and the built file list agree.
- Both consumers build and pass their own QML lint against the canonical path;
  their evidence records that separately.

`STYLE-G7` implementation closes on that evidence. Whether the bar's contrast
and the gutter's numerals hold at the author's display scale is `VAL-STYLE-04`.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and
stable symbols are authoritative; line counts are a hand-off aid and may drift.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| STYLE-G7-E | `celestina-style:` | done | [inventory](../../inventories/2026-08-04-shared-reading-controls/STYLE-G7-E.numstat.tsv) | 22 files, +391/-5 | Add the finite first-party status glyphs required by Celestina's accepted panel baseline without adding shell state or workflows to the shared module | [evidence](../../evidence/2026-08-08-panel-status-glyphs.md) | `VAL-PANEL-1` in Celestina |
| STYLE-G7-C | `celestina-style:` | done | [inventory](../../inventories/2026-08-04-shared-reading-controls/STYLE-G7-C.numstat.tsv) | 15 files, +679/-18 | Derive the scroll handle's position from the same two distances its drag uses (STY-M1); stop the shape generator truncating its own source before the first download and ship Phosphor's MIT notice it claims to carry (STY-M2, STY-M4); give the gallery a private import root instead of a predictable name in `/tmp` (STY-M3); look icon names up without the prototype chain (STY-B1); name an icon-only button when its consumer supplied no help text (STY-B2); and let a disabled primary keep the accent wash the theme already defines for it (STY-B3) | [evidence](../../evidence/2026-08-05-static-audit-corrections.md) | `VAL-STYLE-04`, `VAL-STYLE-05` |
| STYLE-G7-D | `celestina-style:` | done | [inventory](../../inventories/2026-08-04-shared-reading-controls/STYLE-G7-D.numstat.tsv) | 7 files, +105/-1 | Declare `textFormat: Text.PlainText` on the label text of `CelestinaButton` and `GlassMenuItem`, the two shared controls that render a string the process did not write — a notification's action and another application's tray menu — where `Text.AutoText` let a producer draw rich text inside the shell and fetch a URL through it | `check-style-contract.sh`, the architecture and language guards, `qmllint-cxxqt.sh siderita` — recorded in [label plain text evidence](../../evidence/2026-08-06-label-plain-text.md) | `VAL-R4` |
| STYLE-G7-B | `celestina-style:` | done | [inventory](../../inventories/2026-08-04-shared-reading-controls/STYLE-G7-B.numstat.tsv) | 4 files, +143/-0 | Order QML type registration behind the module itself, so the two generated targets that carry the same metatypes extraction cannot run it at once | [evidence](../../evidence/2026-08-05-qmllint-ordering.md) | None |
| STYLE-G7-A | `celestina-style:` | done | [inventory](../../inventories/2026-08-04-shared-reading-controls/STYLE-G7-A.numstat.tsv) | 11 files, +565/-8 | Publish `CelestinaScrollBar` and `CelestinaLineGutter`, register both, and move the module to 1.1.0 | [evidence](../../evidence/2026-08-04-shared-reading-controls.md) | `VAL-STYLE-04` |

## Why the ordering fix is a unit of this checkpoint

It was found verifying this checkpoint's own artifact: `verify-production.sh`
builds `all_qmllint`, and that step failed twice with
`Could not open: meta_types/celestina-style_json_file_list.txt.timestamp`. The
defect belongs to how this module's targets are generated, not to the controls
themselves, so it is a second unit here rather than a new checkpoint for one
`add_dependencies` line.

## Decisions and rollback

The scroll bar is built from QtQuick primitives rather than from a re-skinned
`QtQuick.Controls` `ScrollBar`. The suite architecture guard refuses a raw Qt
control rebuilt outside this module, and building the anatomy from semantic
tokens is what lets the suite own its thickness, colour and motion instead of
fighting a control template for each consumer.

The gutter builds only the numbers its viewport shows. Grafita accepts documents
of up to 64 MiB, so a delegate per logical line is not an option that was
rejected on taste; it is one that does not run.

Both components started local to Grafita, which is what the sharing contract
requires of an unspecified control. Siderita's request supplied the second
consumer, so they moved here in the same delivery rather than being copied.

Rollback is the reverse of the registration: removing both rows from `qmldir`
and `CMakeLists.txt` returns the module to its 1.0 type set, and the consumers
that reference them fail their own lint rather than degrading silently.

STYLE-G7-D lands here rather than in Celestina because the controls are shared.
The shell closed this for its own `Text` items; these two label paths are not
`Text` items, and fixing them in one consumer would have left the same hole open
for the other three and put a second copy of the rule in the tree.

## STYLE-G7-E boundary

Celestina's accepted panel baseline is a demonstrated consumer for thirteen
status and action glyphs. This unit owns only their canonical semantic names,
Lucide resources, registration parity and lookup tests. The shell owns their
meaning, state, accessibility labels, placement and interaction; none of those
belongs in CelestinaStyle.

No unit remains open in this slice. A further catalogue addition requires a
demonstrated consumer and a new ledger unit.
