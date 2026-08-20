# G7 — reading comfort

- **Opened:** 2026-08-04
- **Closed:** 2026-08-19
- **Status:** done
- **Plan ID:** g7-reading-comfort
- **Scope:** grafita
- **Implementation checkpoint:** G7
- **Author-validation checkpoint:** `VAL-G7` in [`../../../VALIDATION.md`](../../../VALIDATION.md)
- **Successor:** [Text Grafita already refuses](../archive/2026-08-19-g8-text-already-refused.md)

## Hypothesis

The editing surface currently spends chrome on a label the user cannot act on
while withholding what a reader needs from every editor: a position to refer
to, a sense of where they are in the file, and text shaped the way they chose.
Numbered lines, a visible scroll position, a reported caret, breathing room
against the frame, and a remembered size and wrap mode make the surface usable
for long reading without adding a settings surface or a second source of
document truth.

## Tangible outcome

Opening a document shows numbered lines beside the text and no encoding label;
the footer reports the caret's line and column; a scroll bar shows and moves
the position; `Ctrl +` / `Ctrl −` and `Ctrl +` wheel change the text size and
`Alt + Z` or `F10` turns wrapping off and on; the size and the wrap mode are the ones
the next launch starts at.

## Scope

In scope: the stored preferences and their bounds in `grafita-core`; the
caret's line and character column as a core-owned mapping from the widget's
UTF-16 offset; a CXX-Qt adapter that publishes and persists the reading
preferences for one window; a line-number gutter component; a local scroll-bar
control; the editing surface's inset, wrap mode and horizontal scrolling; the
size and wrap shortcuts and `Ctrl +` wheel; the caret readout in the footer;
removal of the encoding label from the document header.

**Delivery shape, chosen by the author.** The two components serve Siderita
too, so they belong to `celestina-style`; a local ledger may carry only its
owner's prefix, and the suite checkpoint is spent on `ACT-1` for unrelated
in-flight work. The author chose local checkpoints over waiting for a suite
one, so the promotion and its second consumer are delivered by two sibling
plans that land before and after this one:
[`STYLE-G7`](../../../../celestina-style/docs/plans/active/2026-08-04-shared-reading-controls.md)
publishes `CelestinaLineGutter` and `CelestinaScrollBar`, and
[`SID-G7`](../../../../siderita/docs/plans/archive/2026-08-04-shared-reading-surface.md)
adopts them in Siderita's two text surfaces. This plan owns Grafita and
`grafita-core` only; it consumes the shared components through the canonical
path and therefore depends on `STYLE-G7` landing first.

## Exclusions

Out of scope: a settings surface or preferences dialog; any preference beyond
the text size and the wrap mode; per-document or per-tab settings; pinch zoom;
a minimap, code folding or any further gutter content such as diff or
breakpoint markers; and Siderita's own surfaces, which are `SID-G7`'s.
This plan records work but grants no authorization beyond the repository
rules.

## Build order

1. Add the stored preference, its clamped bounds and its tests to
   `grafita-core`.
2. Adapt it to Qt in the application: one object per window, published as a
   property and written through on every accepted change.
3. Add the gutter component, register it, and compose it into the editing
   surface with the widened inset. It is published by `STYLE-G7` and consumed
   here through the canonical path.
4. Bind the size shortcuts in the window and drop the encoding label, retiring
   its language-ratchet row in the same unit.
5. Add the core mapping from a widget UTF-16 offset to a caret line and
   character column, publish it, and report it in the footer.
6. Add the wrap preference, the scroll bar and horizontal scrolling, moving
   the gutter out of the scrolled content so it stays pinned.
7. Add the wrap and wheel gestures over the same stored values.

## Implementation exit

- `cargo test -p grafita-core` covers preference parsing, clamping, the limits
  of a size nudge, and the caret mapping including a column measured in
  characters and an offset that splits a surrogate pair.
- Rust format and Clippy pass for the crate and the application.
- Both shared QML files are registered through `build.rs` and QML lint passes.
- `bash scripts/check-architecture-contract.sh` passes, including the language
  ratchet, whose `DocumentHeader.qml` row this unit retires.
- `grafita/scripts/complete-production.sh` builds the canonical release once,
  verifies those exact bytes and updates the author's test destination.
  `grafita-core` changes, so `siderita/scripts/complete-production.sh` runs too
  and both installed consumers carry the verified bytes.

G7 implementation closes on that evidence. Whether the gutter tracks a wrapped
line correctly on a real compositor, and whether the shortcuts arrive from a
physical keyboard layout, are an independent `VAL-G7` run.

## Change and commit ledger

Update before editing a slice and again when its diff is ready. Paths and
stable symbols are authoritative; line counts are a hand-off aid and may drift.

One unit, not one per slice: `grafita/build.rs` carries both the new QML
registration and the new bridge file, so a split would claim a boundary a
single commit cannot produce.

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| G7-A | `grafita:` | done | [inventory](../../inventories/2026-08-04-g7-reading-comfort/G7-A.numstat.tsv) | 26 files, +1064/-55 | Stored clamped text size and wrap mode with their one-per-window Qt adapter; core-owned caret line and character column; the gutter and scroll bar composed into the window pinned beside the viewport; wider text inset; horizontal scrolling; size, wrap and wheel gestures; caret readout in the footer; encoding label removed and its language-ratchet row retired | [evidence](../../evidence/2026-08-04-g7-reading-comfort.md) | `VAL-G7` |
| G7-B | `grafita:` | done | [inventory](../../inventories/2026-08-04-g7-reading-comfort/G7-B.numstat.tsv) | 8 files, +60/-3 | Preserve an explicit platform theme while routing otherwise unowned Qt file dialogs through the session portal | [portal file-dialog evidence](../../evidence/2026-08-05-portal-file-dialog.md) | `VAL-SID-02` |
| G7-C | `grafita:` | done | [inventory](../../inventories/2026-08-04-g7-reading-comfort/G7-C.numstat.tsv) | 18 files, +780/-51 | Make "save as" obey the same revision rule as an ordinary save, decode its destination through `url::local_path`, write through a symlink and report the durability it observed; refuse a duplicate or clean save; disarm a cancelled destination chooser; answer a classify superseded by an open; reset the live search on a new document; keep the undo bound from splitting an action; and stop a generic refusal from overwriting the one that names the file | `cargo test -p grafita-core`, `cargo clippy -p grafita-core --all-targets`, `cargo fmt`, `cargo check`/`cargo test`/`cargo clippy` in `grafita/`, `scripts/qmllint-cxxqt.sh grafita`, `bash scripts/check-architecture-contract.sh` — recorded in [loss-free save-as evidence](../../evidence/2026-08-05-loss-free-save-as.md) | `VAL-GRA-SAVEAS` |
| G7-D | `grafita:` | done | [inventory](../../inventories/2026-08-04-g7-reading-comfort/G7-D.numstat.tsv) | 9 files, +123/-5 | Ask where a document goes before applying the clean guard, so a new document nobody has typed into can still be given a name — the guard exists to stop an unchanged file being rewritten, and a document with no file has nothing to rewrite | `cargo test -p grafita-core`, `cargo fmt --all --check`, `cargo clippy -p grafita-core --all-targets --locked -- -D warnings` — recorded in [naming an untouched document evidence](../../evidence/2026-08-06-naming-an-untouched-document.md) | `VAL-GRA-SAVEAS` |
| G7-Z | `grafita:` | done | [inventory](../../inventories/2026-08-04-g7-reading-comfort/G7-Z.numstat.tsv) | 7 files, +296/-188 | Run the checkpoint's implementation exit — the canonical release, its verification and both installed consumers — and archive the plan, retarget its links and open G8 in the roadmap | [G7 delivered to both installed hosts](../../evidence/2026-08-19-g7-production-completion.md) | `VAL-G7` |

G7-Z carries no source change. G7-A through G7-D were written and tested but
never built as a canonical release, so the author's installed binaries still
held pre-G7 bytes; this unit runs that exit for both consumers of
`grafita-core` and closes the plan.

G7-B is a bounded corrective delivery discovered while validating open/save in
the completed reading surface. It changes no document or reading rule: it
selects the portal platform theme only when the environment has not already
selected one.

G7-C is a second corrective delivery, from the suite audit in
[`docs/evidence/2026-08-05-static-suite-audit.md`](../../../../docs/evidence/2026-08-05-static-suite-audit.md):
findings `GRA-C1`, `GRA-A1`, `GRA-A2`, `GRA-A3`, `GRA-A4`, `GRA-M2` and
`GRA-M4`. It changes no reading rule either; every correction is about the
document's own contract — that a byte the user typed is either in the file or
still marked unsaved. `GRA-M6` and `GRA-M7` are deliberately outside it: the
first is an incremental-reconciliation redesign that needs its own measurement,
and the second cannot be confirmed without running the application.

## Decisions and rollback

The gutter builds only the numbers its viewport shows. Grafita accepts
documents of up to 64 MiB, so a delegate per logical line is not an option that
was rejected on taste; it is one that does not run. The window is found by
binary search over line offsets and placed with the surface's own
`positionToRectangle`, so a wrapped line keeps one number rather than gaining
one per visual row.

The scroll bar has no keyboard of its own because the surface it reports on
already reaches every position it can. Why it is built from QtQuick primitives
rather than from a re-skinned `QtQuick.Controls` `ScrollBar` belongs to its
owner and is recorded in `STYLE-G7`.

Both components started local to Grafita, which is what the sharing contract
requires of an unspecified control. Siderita's request supplied the second
consumer, so they moved to `celestina-style` under `STYLE-G7` rather than being
copied. This unit consumes them through the canonical path and carries only the
two symlinks and the registration `build.rs` needs.

The stored text size is read by Siderita too, through its own CXX-Qt adapter
over `grafita_core::preferences`. Two adapters over one core type is the shape
the architecture table asks for — Qt marshalling belongs to each application's
`src/`, exactly as each host already adapts `DocumentSession` — and the rule
that matters, the bounds and the file format, still has one owner.

That adapter differs from Grafita's in two ways, both because it is a guest.
It re-reads on demand rather than only at construction, since Grafita may be
running beside Siderita and each folder view holds one; a size changed anywhere
is therefore the size the next surface to open shows. And a nudge is applied to
what is stored at that moment rather than to what the object last published, so
two surfaces moving the size cannot undo each other. Reloading never writes.

The wrap mode is published but not bound to a key in Siderita: the embedded
editor and the peek each own a small key map, and only the size was asked for.

The caret's line and column are mapped in `grafita-core`, not in QML, even
though the gutter already knows where the lines start. Mapping a UTF-16 offset
to a line and a character column is a document rule with one owner; a second
implementation in the host would disagree with the first on the day someone
opens a file with an accented letter in it.

Wrapping is bound to both `Alt + Z` and `F10`. `Alt + Z` is the convention, but
a compositor that claims Alt as its modifier — as the author's does, where
`Alt + Z` launches a browser — takes it before any application is offered it.
`F10` is Kate's binding for the same command and survives that.

The text size is written through on each accepted change rather than at exit,
because a compositor can close the window without one, and a preference saved
only on a clean quit is a preference that does not survive how programs are
actually stopped. A keypress at a bound changes nothing and therefore neither
notifies QML nor touches the file.

Rollback is per slice: the preference file is ignored when absent, so deleting
`$XDG_CONFIG_HOME/grafita/preferences` returns the shipped size and wrap mode,
and reverting the QML slice restores the previous surface without touching the
core.

G7-D corrects an ordering G7-C introduced. Its clean guard sat before the
question of whether the document has a file, so it answered for a case it was
never about and left an untouched new document with no way to acquire a name.
