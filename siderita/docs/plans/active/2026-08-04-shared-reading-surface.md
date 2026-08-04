# Shared reading surface

- **Opened:** 2026-08-04
- **Status:** active
- **Plan ID:** shared-reading-surface
- **Scope:** siderita
- **Implementation checkpoint:** SID-G7
- **Author-validation checkpoint:** `VAL-SID-G7` in
  [`../../../VALIDATION.md`](../../../VALIDATION.md)

## Hypothesis

Siderita already reads documents through `grafita-core`; what it lacked was the
reading anatomy around them. Consuming the two published components and the same
core mappings — the caret's line and column, the stored text size — gives both
text surfaces the reader's frame of reference without Siderita owning a single
new document rule, and lets the quick look drop two raw Qt controls in the same
change rather than re-skinning them.

## Tangible outcome

Pressing `Space` on a text file opens the embedded editor with numbered lines, a
suite scroll bar, a footer reporting the caret's line and character column, no
encoding label, and the text size stored by Grafita, changeable with `Ctrl +`
and `Ctrl −`. The quick look's text pane numbers and scrolls the same way. The
architecture baseline loses its `QuickLookView.qml` `ScrollView` and `TextArea`
rows, and the language baseline loses its `GrafitaEditorDialog.qml` row.

## Scope

In scope: a CXX-Qt adapter over `grafita_core::preferences` for one folder view;
the caret readout published by `GrafitaEditor` from the core mapping; adopting
`CelestinaLineGutter` and `CelestinaScrollBar` in `GrafitaEditorDialog.qml` and
`QuickLookView.qml`; holding one preferences object beside the document state in
`FolderActions.qml`; the size shortcuts in both surfaces; removal of the encoding
label; the two `build.rs` registrations; and both retired baseline rows.

## Exclusions

Out of scope: the components themselves, which are
[`STYLE-G7`](../../../../celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md)'s;
the standalone Grafita window, which is
[`G7`](../../../../grafita/docs/plans/active/2026-08-04-g7-reading-comfort.md)'s;
a settings surface; any preference Siderita defines for itself; a wrap binding,
which was not asked for; and Siderita's other surfaces. This plan records work
but grants no authorization beyond the repository rules.

## Build order

1. Add the CXX-Qt preferences adapter and register it in `build.rs`.
2. Publish the caret's line and column from `GrafitaEditor` over the core
   mapping, re-derived after every text change rather than only on a caret move.
3. Register both shared QML files and adopt them in the embedded editor, with
   the caret readout, the size shortcuts and the dropped encoding label.
4. Adopt them in the quick look's text pane, retiring its two raw-control rows.

## Implementation exit

- `bash scripts/check-architecture-contract.sh` passes, including the two
  `QuickLookView.qml` raw-control rows this unit retires and the language row it
  retires from `GrafitaEditorDialog.qml`.
- `cargo fmt --check` and Clippy pass for the application; the QML tests and
  lint pass over both changed surfaces.
- `siderita/scripts/complete-production.sh` builds the canonical release once,
  verifies those exact bytes and updates the author's test destination. It runs
  regardless, because `grafita-core` changed underneath it.

`SID-G7` implementation closes on that evidence. Whether the gutter tracks a
wrapped line on a real compositor, and whether the size shortcuts arrive from a
physical layout inside a modal, are an independent `VAL-SID-G7` run.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and
stable symbols are authoritative; line counts are a hand-off aid and may drift.

One unit, not one per surface: `siderita/build.rs` carries both the shared QML
registration and the new bridge file, so a split would claim a boundary a single
commit cannot produce.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| SID-G7-A | `siderita:` | done | [inventory](../../inventories/2026-08-04-shared-reading-surface/SID-G7-A.numstat.tsv) | 20 files, +715/-45 | Adopt the shared reading controls and the core caret mapping in both text surfaces, over a re-reading preferences adapter, and retire the three baseline rows the change earns | [evidence](../../evidence/2026-08-04-shared-reading-surface.md) | `VAL-SID-G7` |

## Decisions and rollback

The stored text size is read through Siderita's own CXX-Qt adapter over
`grafita_core::preferences` rather than by importing Grafita's. Two adapters
over one core type is the shape the architecture table asks for — Qt marshalling
belongs to each application's `src/` — and the rule that matters, the bounds and
the file format, still has one owner.

That adapter differs from Grafita's in two ways, both because it is a guest. It
re-reads on demand rather than only at construction, since Grafita may be running
beside Siderita and each folder view holds one; a size changed anywhere is
therefore the size the next surface to open shows. And a nudge is applied to what
is stored at that moment rather than to what the object last published, so two
surfaces moving the size cannot undo each other. Reloading never writes.

One preferences object lives in `FolderActions.qml` beside the document state
rather than inside either surface, so the peek and the editor cannot drift apart
within one folder view.

The caret's line and column are mapped in `grafita-core`, not in QML, even
though the gutter already knows where the lines start. Mapping a UTF-16 offset
to a line and a character column is a document rule with one owner; a second
implementation in the host would disagree with the first on the day someone
opens a file with an accented letter in it. `set_caret` is deliberately not
routed through the session dispatch: moving a caret is not a session action and
must not clear a refusal message or republish the whole state on every arrow key.

The wrap mode is published but not bound to a key here: the embedded editor and
the peek each own a small key map, and only the size was asked for.

Rollback is per surface. Reverting either QML file restores its previous pane
without touching the adapter or the core, and deleting
`$XDG_CONFIG_HOME/grafita/preferences` returns the shipped size, because the
file is ignored when absent.
