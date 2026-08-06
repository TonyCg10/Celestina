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
| SID-G7-B | `siderita:` | done | [inventory](../../inventories/2026-08-04-shared-reading-surface/SID-G7-B.numstat.tsv) | 17 files, +620/-54 | Turn the portal picker into a compact dialog and import its bounded Wayland parent handle through the narrow C++/Qt seam | [portal picker evidence](../../evidence/2026-08-05-portal-picker.md) | `VAL-SID-02`, `VAL-SID-04` |
| SID-G7-C | `siderita:` | done | [inventory](../../inventories/2026-08-04-shared-reading-surface/SID-G7-C.numstat.tsv) | 34 files, +1263/-229 | Treat an entry pasted into its own folder as a duplicate instead of trashing it to make room for itself; answer the portal's `writable` only when it was asked for and confirm an overwrite before returning a save destination; guard trash behind the running-operation check that paste already had; skip an entry that vanished mid-scan instead of failing the listing, and keep a quiet refresh quiet; navigate a symlink that points at a directory; purge a trash entry by its own info path rather than by list position; decode dropped URIs by bytes in Rust; write the four remaining configuration files atomically; and move paste planning and execution out of the coordinator into `controller/paste.rs`, lowering its earned baseline row from 1223 to 1171 | `cargo check`, `cargo test`, `cargo fmt` in `siderita/` and for the `siderita-*` crates — recorded in [destructive-operation guards evidence](../../evidence/2026-08-05-destructive-operation-guards.md) | `VAL-SID-05` |
| SID-G7-D | `siderita:` | done | [inventory](../../inventories/2026-08-04-shared-reading-surface/SID-G7-D.numstat.tsv) | 57 files, +1283/-441 | Apply [ADR 0008](../../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md) to Siderita, closing audit finding `SID-A2`: publish every path crossing the Qt seam as its byte-exact percent key beside its own lossy display text; decode that key at every invokable with a typed refusal instead of rebuilding a `PathBuf` from the `QString`; stop QML composing paths (breadcrumbs, the save picker's typed name, the quick look's `file://` URL, the thumbnail ids, the sidebar's derived names); migrate the persisted bookmarks, favourites, icons, folder views and tab session to keys; leave the `file://`, portal and Trash encodings that face other processes exactly as they are; and lower the earned `controller.rs` architecture row from 1171 to 1106 while retiring its language-debt row | [byte-exact path seam evidence](../../evidence/2026-08-06-byte-exact-path-seam.md) | `VAL-SID-06` |
| SID-G7-E | `siderita:` | done | [inventory](../../inventories/2026-08-04-shared-reading-surface/SID-G7-E.numstat.tsv) | 18 files, +703/-83 | Close the two limits `SID-G7-D` recorded: carry the thumbnail provider's decoded path as `QByteArray` and address the source file by `::stat` and a descriptor opened on its bytes, computing the freedesktop cache key over those bytes in the spelling `percent::encode_qt_path` owns; and make the system-clipboard seam exchange percent-encoded `file://` URIs written and read by `dbus::path_to_uri`/`uri_to_path` instead of lossy `QString` paths | [thumbnail and clipboard bytes evidence](../../evidence/2026-08-06-thumbnail-and-clipboard-bytes.md) | `VAL-SID-06` |
| SID-G7-F | `siderita:` | done | [inventory](../../inventories/2026-08-04-shared-reading-surface/SID-G7-F.numstat.tsv) | 10 files, +210/-8 | Convert the provider id with `toUtf8` rather than `toLatin1`, because Qt hands a provider an id it has already decoded, and expose the seam so a test reaches the decode through the same `image://thumb/<key>` URL a delegate writes instead of entering behind it | `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --all-targets --locked`, the architecture and language guards — recorded in [thumbnail seam regression evidence](../../evidence/2026-08-06-thumbnail-seam-regression.md) | `VAL-SID-06` |
| SID-G7-G | `siderita:` | done | [inventory](../../inventories/2026-08-04-shared-reading-surface/SID-G7-G.numstat.tsv) | 17 files, +452/-27 | Publish a breadcrumb key-first so a tab in a folder name can no longer move the cut that separates it from its display text; mark a persisted path record as a key when it is written instead of inferring it from codec idempotence, which could not tell a legacy raw path holding a literal `%20` from the key for a path holding a space; and send a file to the phone over Magnetita's new `SendFileUri` with the byte-exact `file://` URI `dbus::path_to_uri` already writes, closing the last verb that put a lossy path out of the process | `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --all-targets --locked`, the architecture and documentation guards — recorded in [correctness debt evidence](../../evidence/2026-08-06-path-key-correctness-debt.md) | `VAL-SID-06` |
SID-G7-B is the independent portal correction that was already in the dirty
checkout when the author requested every pending change be delivered. It does
not extend the shared reading-surface rules. Its C++ seam exists because Qt's
private Wayland surface interface and generated `xdg-foreign` protocol are not
available through the safe CXX-Qt boundary.

SID-G7-C is a corrective delivery from the suite audit in
[`docs/evidence/2026-08-05-static-suite-audit.md`](../../../../docs/evidence/2026-08-05-static-suite-audit.md):
findings `SID-A1`, `SID-A3`, `SID-M1` to `SID-M5`, `SID-M7`, `SID-M8` and the
cheap `SID-B4`, `SID-B5`, `SID-B6`, `SID-B9`. Two themes hold it together: an
operation never destroys what it was asked to preserve, and the portal backend
answers other applications only what they actually asked for. It stays `active`
because the author asked for the code and its tests without the production
flow, so it has neither an inventory nor a version transition. `SID-A2` — the
lossy Qt seam for non-UTF-8 names — and `SID-M6` — the inherent cross-device
move window — are deliberately outside it; the first needs one decision shared
with Fluorita, the second a design choice about renaming before copying.

SID-G7-D takes up that first deferral. The decision it was waiting for is
[ADR 0008](../../../../docs/decisions/0008-byte-exact-paths-across-the-qt-seam.md),
and this unit is its application to Siderita and nothing more: the ADR is not
re-argued here. The codec and its typed refusal are not Siderita's: they belong
to `celestina_core::pathkey`, which the Fluorita side of the same ADR landed in
this checkout, and `siderita/src/pathkey.rs` is only the Qt marshalling that
core module explicitly leaves to each application. `FLU-M1` — the same defect at
the same boundary in Fluorita — is not closed by this unit. Like SID-G7-C it
stays `active` because the author asked for the
code, its tests and its evidence without the production flow, so it has neither
an inventory nor a version transition.

SID-G7-E closes the two limits SID-G7-D wrote down rather than adding a new
rule: both were places where a byte-exact key arrived and was then decoded into
a `QString`, which cannot hold what the key exists to carry. The thumbnail
provider keeps its decoded path as bytes and addresses the file by descriptor;
the clipboard seam stops exchanging paths and exchanges the percent-encoded
`file://` URIs the desktop actually speaks, written and read by the codec
`src/dbus.rs` already owns. Neither half adds a codec, and the freedesktop cache
key keeps the exact spelling `celestina_core::percent::encode_qt_path` documents,
which is now asserted by a test across the two languages instead of by a comment
in each. Like the units before it, it stays `active` because the author asked
for the code, its tests and its evidence without the production flow, so it has
neither an inventory nor a version transition.

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

SID-G7-F repairs SID-G7-E rather than extending it. The byte-level decode was
right; the conversion beside it was not, and it broke every accented name to
serve the one that is not valid UTF-8. The unit exists as much for its test as
for its one-word fix: the previous tests entered the provider behind the seam
that carried the wrong assumption, so they stayed green while the grid lost its
thumbnails.

SID-G7-G takes stage 3 of the
[light monorepo audit](../../../../docs/evidence/2026-08-06-light-monorepo-audit.md)
and is three separate repairs to one decision rather than an extension of it.
All three are places where ADR 0008's key was produced correctly and then
handed to something that could not keep it whole: a separator a filename may
legally contain, a persisted record whose spelling had to be guessed, and a
D-Bus argument typed as a display string. None re-argues the ADR and none adds
a codec.

The choice of key-first over sanitising the name is deliberate. Sanitising
would keep the key readable at the cost of showing a name that is not the
name; putting the key first costs nothing, because a key cannot contain the
separator by construction while a name can contain anything, so the ambiguity
is removed rather than papered over.

The persisted mark is `key:`, a prefix neither a key nor a raw path can start
with, and it is written by every store that keeps a path. A record without it
predates the mark and keeps the old best-effort migration: the bytes on disk
carry no evidence either way, existing files must keep loading, and one save
retires the ambiguity for that store. That residual case is recorded in the
module and pinned by a test rather than claimed closed.

`SendFile` is untouched. It is a published D-Bus interface and changing what
its argument means would break any other caller, so the byte-exact path is a
new method — `SendFileUri`, in `magnetitad` under `MAG-S1-B` — and Siderita is
its first consumer. That is the same decision the portal and the clipboard
already took, applied to the one boundary that had not taken it.
