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
