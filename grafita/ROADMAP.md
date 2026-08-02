# Grafita roadmap

> Part of the [Celestina suite](../ROADMAP.md). Checklist legend: `[x]` done ·
> `[ ]` planned. Source presence is not runtime evidence. The author ratified
> the name, opened the implementation gate and fixed the two-surface interaction
> on 2026-07-30. The shared document core is implemented and its exit checks are
> verified. Both surfaces now exist — the embedded Siderita modal and the
> standalone application — and both have been driven headlessly; neither has
> been seen in a real session.

## Settled product decisions

- **Grafita edits any textual file type.** Acceptance is based on content and
  encoding, never a filename extension or closed MIME allowlist. MIME remains
  useful only for desktop discovery and optional syntax selection.
- **`Space` means edit in place.** In Siderita, `Space` on editable text opens a
  Grafita-owned editing modal that occupies nearly the whole folder surface.
  `Space` on non-text keeps the existing quick-look behaviour.
- **Double-click and `Enter` mean the full app.** Activating textual content in
  Siderita opens standalone Grafita in its own window. Other file types keep
  their normal desktop handler.
- **One core, two thin hosts.** `grafita-core` owns the document and file
  semantics. Grafita and Siderita expose separate CXX-Qt adapters and QML
  compositions; neither copies domain logic or imports the other's QML.
- **The embedded surface is a real editor.** It has editing, selection,
  undo/redo, dirty/conflict state, save and guarded close. It is simpler only in
  chrome: no tabs, project UI or standalone settings surface.
- **No IDE.** No project tree, build runner, debugger, LSP, terminal or plugin
  system.

## Technical contract

### Text classification and representation

The core opens regular files by bytes. UTF-8, UTF-8 with BOM and UTF-16 LE/BE
with BOM are editable in the first milestone. Extensionless files, dotfiles,
JSON, KDL and source code follow exactly the same path. Unknown or malformed
encodings are reported honestly and stay byte-preserving/read-only until an
explicit encoding choice exists; binaries are not offered as text.

Each line retains its content bytes and original terminator (`\n`, `\r\n`,
`\r`, or none). Untouched open → save is byte-identical by construction, mixed
newlines remain mixed, and inserted lines use the document's dominant newline.
No chardet-style guess may silently reinterpret the user's bytes.

### Loss-free save

Opening snapshots the resolved symlink chain and target identity before and
after the read. Saving revalidates that identity, writes a unique sibling
temporary through the captured parent directory, reproduces permissions,
ownership/group, readable extended attributes and POSIX ACLs, syncs the file,
then atomically renames it over the resolved target. A detected external change,
unreproducible metadata or any pre-rename failure refuses the save and leaves
the original intact. A directory-sync failure after rename is reported as
saved with reduced durability, not as unsaved.

### Host lifecycle

Both hosts perform open/save IO on a bounded, owned and deterministically joined
worker. Open results carry a generation; save results carry the document
revision they wrote. A stale reply never replaces a newer document or clears a
newer edit's dirty state.

In Siderita, closing an embedded dirty document offers **Guardar**,
**Descartar** and **Cancelar**. The modal traps focus, blocks folder actions and
restores focus to the selected entry. Launching the full app is a separate
activation path; the two surfaces do not pretend to share live in-memory state.

## Completed — G0: start contract

- [x] Ratify `Grafita`, the standalone application and the embedded Siderita
      surface.
- [x] Open the implementation boundary for `grafita/`,
      `celestina-rs/crates/grafita-*` and the bounded Siderita consumer work.
- [x] Define content-based text acceptance and the `Space` versus
      double-click/`Enter` interaction.
- [x] Record that the core is shared while both UI compositions remain local to
      their hosts.

**Exit:** product, ownership and activation contracts are canonical; no product
source is claimed as implemented.

## Completed — G1: shared document core

**Observable outcome.** A tested Rust API can open, edit, undo, redo and safely
save representative text files without knowing whether Grafita or Siderita is
the caller.

**In scope:** content probing, supported encodings, byte/newline-preserving
document model, caret/selection edit commands, undo/redo/savepoint,
dirty/conflict state, loss-free save and typed outcomes.

**Out of scope:** Qt/QML, syntax highlighting, desktop handlers, tabs and live
installation.

**Work order:**

1. [x] Add `grafita-core` to the shared workspace with no Qt dependency and
      document every filesystem dependency inline.
2. [x] Implement bounded content probing and the byte/newline-preserving read
      model; file type, extension and MIME never gate the result.
3. [x] Add position, selection and splice commands plus undo/redo and a
      savepoint; invalid positions return typed errors rather than panicking.
4. [x] Implement the resolved-target, metadata-preserving atomic save path and
      external-change detection.
5. [x] Define generation/revision values and typed open/save outcomes for both
      host adapters without embedding a runtime in the core.
6. [x] Add table-driven tests and update the `celestina-rs` inventory and
      roadmap.

**Exit checks:**

- [x] `.txt`, Rust, JSON, KDL, dotfile and extensionless UTF-8 fixtures all open
      as editable through the same API; a binary fixture does not.
- [x] UTF-8 BOM and UTF-16 BOM fixtures edit and re-encode correctly;
      malformed/raw bytes remain unchanged and are never advertised as safely
      editable.
- [x] Insert/delete/selection replacement, mixed newlines, undo/redo and
      savepoint transitions pass table-driven tests.
- [x] Symlink, changed-underneath, permission/metadata and injected
      interrupted-save tests prove that every refusal leaves the original
      intact.
- [x] Architecture guard, format, Clippy with `-D warnings` and the
      `celestina-rs` workspace tests pass.

**Evidence.** `bash scripts/check-architecture-contract.sh`, `cargo fmt --all
--check`, `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace`, all green, with 42 of those tests belonging to
`grafita-core` (27 unit, 15 integration over real files in scratch
directories).

**Decisions this milestone settled**, beyond the contract above:

- A caret column is a byte offset inside its line's UTF-8 content. It is exact,
  cheap, and validated on every entry point; a split character is a typed
  refusal, never a panic.
- The dominant newline is decided when the buffer is parsed and then stays
  fixed. Recomputing it mid-session would make the same keystroke insert
  different bytes from one moment to the next.
- Undo is bounded (512 changes). The savepoint is pinned to a change's identity,
  not to a stack depth, and a savepoint that falls off the bottom of the stack
  makes the document permanently dirty rather than falsely clean.
- A document ceiling of 64 MiB is refused up front; an editor is the wrong tool
  past it and exhausting the session is worse than saying no.
- The save adopts the identity of the file it just wrote even when the document
  moved on, so a document's own write is never mistaken for an external change.
- Reproducing extended attributes — POSIX ACLs among them, since Linux stores
  them as one — needs syscalls `std` does not expose, so `grafita-core` carries
  `xattr`. It is the first third-party dependency in a non-Magnetita core and is
  justified inline in its `Cargo.toml`.

**Not proven.** Ownership reproduction across users needs a file owned by
another user and was not exercised; only the same-owner path is covered.
Extended-attribute reproduction was verified on the temporary filesystem the
tests run on, which is not evidence for every filesystem.

## Completed — G2: embedded Grafita in Siderita

Double-click/`Enter` first performs the same asynchronous text probe and launches
standalone Grafita for text; non-text retains `xdg-open`. `Space` performs the
probe and routes editable text to a nearly full-window `GrafitaEditorDialog`,
while images, folders, media and binary files keep `QuickLookView`.

The Siderita adapter exposes only `grafita-core` state/actions. The modal adds
editing, save, undo/redo, error/conflict state and guarded close, blocks the
folder underneath and restores focus. The existing synchronous
`preview_text` path is no longer used to decide editable text. A real Wayland
pass must cover mouse, keyboard, focus trapping, reduced motion and close with a
dirty document before G2 is complete.

**Built so far.**

- [x] `grafita-core::worker` — one owned thread over probe/open/save. A newer
      probe or open cancels and replaces a queued one; a save is never dropped
      to make room, and only shutdown cancels one. Dropping it cancels and joins
      deterministically, so a closing tab cannot leave a read or write behind.
- [x] `grafita-core::display` — the line-feed projection a text widget shows,
      the UTF-16 caret mapping Qt cursors need, and the reconciliation that
      turns "here is my whole text now" into the single splice it represents.
- [x] `siderita/src/editor.rs` — a `GrafitaEditor` QObject of its own, holding
      only document state and running every read and write on the worker.
- [x] `siderita/qml/dialogs/GrafitaEditorDialog.qml` — the modal: editing,
      save, undo/redo, error and conflict lines, and a guarded close offering
      **Guardar** / **Descartar** / **Cancelar** that disables the document
      surface beneath it.
- [x] `Space` routes through the content probe; folders and everything the core
      refuses as editable fall back to `QuickLookView`.

**Decisions this milestone settled.**

- **The text widget never owns the text.** Qt stores line breaks its own way, so
  letting a `TextEdit` own the document would rewrite every CRLF file the moment
  it was touched — the exact loss G1 exists to prevent. The widget is shown the
  core's line-feed projection and reports its whole content back; the core
  derives the one splice that explains the difference. Untouched lines never
  enter that difference, so their terminators are never rewritten, and a typed
  newline adopts the document's dominant terminator like any other insertion.
  Pushing the projection back after an undo is recognised as unchanged and
  recorded as nothing, so the design needs no re-entrancy flag.
- **Undo, redo and save are intercepted before Qt's own text history**, which
  knows nothing about savepoints, terminators or what is on disk.
- **The editor is its own QObject.** An open document has its own lifetime and
  shares no state with folder scanning, and `SideritaController` is at its
  frozen size baseline besides.
- **The embedded surface caps documents at 8 MiB**, below the core's own
  ceiling: a modal inside a file manager is not where a large file belongs.
- **A save report carries the document generation as well as its revision**, so
  a report that arrives after the user closed one file and opened another is
  recognised as belonging to neither.

**Evidence so far.** Architecture guard, `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
(386 passing), `qmllint` against the current build's module for the changed QML,
`cargo build --release --locked` and `siderita/scripts/smoke.sh`. The exact lint
command was `qmllint -I <import-path> qml/dialogs/GrafitaEditorDialog.qml
qml/components/folder/FolderActions.qml`, where the import path is the build's
`qt-build-utils/qml_modules` tree with `qml/` linked in beside its `qmldir`.

A core-level test drives the widget's exact path — read the projection, edit
that string the way a widget would, hand the whole thing back, save — and proves
a CRLF file keeps its terminators while the typed line adopts CRLF too.

**The modal was driven end to end offscreen.** Reaching it first required
fixing Siderita: a dead `ScrollBar.horizontal` binding in `TabStrip.qml` aborted
that component, and because `TabStrip` lives inside `FolderView`, the failure
cascaded to the whole tab delegate — no folder view was constructed at all. It
reproduced at `9ecc457` with the editor removed, so it predates this milestone,
and `scripts/smoke.sh` missed it by grepping only for `TypeError`/
`ReferenceError`. Both are fixed under Siderita's S6.

With that gone, a temporary probe drove the real surface offscreen and observed:

| Step | Observed |
|---|---|
| Binary file | editor stays closed, quick look opens |
| Text file | opens as UTF-8, projection `"primera\nsegunda\ntercera\n"` |
| Focus | the text body holds active focus, modal shown |
| Layout | an 802×575 editing page inside an 858×692 folder view |
| Typing through Qt's own `TextEdit` | dirty, undo available |
| Save | clean, "Guardado", no error |
| **Bytes on disk** | **`primera\r\nsegunda EDITADA\r\ntercera\r\n`** |
| Close while dirty | guarded question raised, document stays open |
| Undo / redo | back to the savepoint and clean, then forward again |
| Discard | closes, question cleared |

The bytes are the point: the edit went through a real Qt text widget and the
file kept its CRLF terminators.

**Accessibility and focus follow the suite contract.** Dirty state, the encoding,
a refusal and a conflict all carry an accessible role and a worded name, so none
of them is signalled only by a bullet or by red-versus-amber. The editing page
paints no focus ring on purpose: the ring is reserved for keyboard focus, and a
bare `TextEdit` is a TextInput template with no `focusReason`, the signal
`CelestinaTextField` uses to tell Tab from a click — so a ring here could only
be one that also fires on every click. On an editing surface the caret is the
focus affordance.

**The real session found two things the offscreen run could not.** The author
ran the surface on the live Niri session on 2026-07-31:

- The modal was a child of the folder view, so it stopped at that view's edge
  and left the sidebar lit, clickable and outside the scrim. It is now
  reparented to the window's content item.
- Sweeping to select text inside the modal started a **file drag in the view
  underneath**. `CelestinaModalLayer`'s scrim was a `MouseArea` accepting only
  the left button, which leaks the other buttons, hover, the wheel and — the one
  that bit — the *pointer handlers* below: a `DragHandler` keeps a passive grab
  and goes on reacting while an item above holds the exclusive one. The layer
  now carries the same input shield `GlassPill` already used for floating
  chrome, and `GlassContextMenu` became `modal: true` (`dim: false`, so the look
  is unchanged) because a non-modal menu let its dismissing click land on the
  file behind it.

**Double-click and `Enter` now open Grafita**, closing the last gap in the
two-surface contract. A file's *bytes* decide, not its name: a `mentira.mp3`
holding text opens in Grafita, a `binario.txt` holding ELF goes to the desktop's
handler, and a folder never takes the detour at all. A Grafita that fails to
launch falls back to `xdg-open`, so a failed launch still opens the file.

The interception point was the obstacle — `SideritaController` and
`FolderView.qml` both sit exactly on frozen baselines. It landed without
touching either: one activator lives in `Main.qml` (not frozen) and the six
activation sites reach it through `Window.window`, so nothing had to be handed
down through the folder view. Exposing `spawn_detached` needed
`mod shell` to become `pub(crate) mod shell` — the same line, so
`controller.rs` still measures exactly 1223.

**The test caught a real defect on the way.** Activating two files in quick
succession lost one: `Job::Probe` is superseded by a newer probe, which is right
for "may I offer to edit what the user is looking at" and wrong for "who opens
this file" — each activation is a separate thing the user did. Classification is
now `Job::Classify`, never superseded, tracked as a list of in-flight
generations, and deliberately does not move `latest`, which would have made an
open already in flight look stale and be dropped.

**Still not proven.** Key events were never synthesised, so `Escape`, `Ctrl+S`
and `Ctrl+Z`/`Ctrl+Y` through `Keys.onPressed` remain untested by anything but
use. Tab focus trapping, focus restoration, reduced motion, IME and AT-SPI have
had no dedicated pass.

## Completed — G3: standalone Grafita

Build the complete one-document application over the same core: its own window,
full editor chrome, strict path activation, desktop entry, isolated installer
test and an explicit launch path from Siderita. The first release does not need
tabs or a project concept. Direct launch must accept textual content regardless
of extension even when the desktop MIME database does not recognize it.

**Built so far.**

- [x] `grafita-core::session` — the whole open/edit/save/close state machine,
      staleness rules included, as a pure type that owns no thread and performs
      no IO: a method returns the job its host should run and the event its host
      should act on. Testable synchronously, without a worker or a toolkit.
- [x] `grafita/` — the application: its own crate, its own QML module, the
      shared `celestina-style` sources as symlinks, a window that owns
      activation and the shortcuts, and one component per region.
- [x] Path activation by content. `grafita RUTA` and `file://` URLs both land in
      the same place, and what is editable is decided from bytes.
- [x] Desktop entry, the `org.celestina.Grafita` icon and `scripts/run.sh`,
      proven by installing into a throwaway prefix.
- [x] `scripts/smoke.sh` — the same headless gate Siderita has, including the
      construction errors a `TypeError`-only grep would miss, plus a CRLF
      document so a regression that rewrites terminators fails here rather than
      in someone's file.
- [ ] An explicit launch path from Siderita. **Deferred, see below.**

**Decisions this milestone settled.**

- **The state machine is shared; the wording is not.** Both hosts need the same
  worker lifecycle, generation and revision staleness rules, guarded close and
  save application — writing that twice would have meant writing the staleness
  rules twice, so it moved into `grafita-core::session`. User-facing text stayed
  out of the core: the same refusal is worded differently by a modal inside a
  file manager ("el editor integrado") and by an editor that names itself. Each
  host maps typed outcomes to its own sentences, and Siderita's adapter lost a
  fifth of its size in the move.
- **Quitting goes through the same guarded close as any other close.** The
  window refuses its own close event and answers it only once the document says
  it may go, so no path out of the application can discard an edit.
- **The editing surface is still local to each host.** Two applications now
  demonstrate the same text-editing recipe, which is the bar for it to enter
  `celestina-style` — but `DESIGN.md` is the author's visual contract and every
  style consumer would need revalidating, so extracting it is left as an
  explicit decision rather than taken unilaterally.
- **The `.desktop` MIME list is discovery, never a decision.** It is what makes
  Grafita appear in "Abrir con"; the file's bytes are what decide whether it
  opens.

**Evidence.** Architecture guard (extended, below), `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo build --release --locked`
and `scripts/smoke.sh` for the application; `grafita-core` keeps its own gates
with 62 tests. The installer was run against a throwaway prefix: the binary,
desktop entry and nine icon sizes land, `desktop-file-validate` passes, the
installed binary runs, and `--uninstall` removes everything it placed.

Activation was checked against names chosen to disagree with their content —
`LEEME` with no extension, `.perfil`, `datos.xyz`, `programa.rs`, a UTF-8 BOM
file, text named `mentira.mp3` and `foto.png`, and an ELF binary named
`binario.mp3`. Every textual one opened; only the binary was refused, and it was
refused for its bytes while its name said audio.

**The guard did not cover a new application, and now does.** `grafita/qml` was
outside QML registration, auto-bindings, the visual contract, local Qt controls
and the shared-style link checks — a new app silently escaped every visual rule.
Both guards now include it, and it immediately earned its keep: it caught
`session: session` in `Main.qml`, the self-shadowing auto-binding, which had left
every binding inside the document view undefined. The offscreen probe had missed
it because the probe drove the window's own object rather than the view's.

**The launch path from Siderita landed under G2** rather than waiting on a
refactor: one activator in `Main.qml` reached through `Window.window` needed no
room in either frozen file. Note that once Grafita is installed the desktop's
own handler resolution would already route text files to it through `xdg-open`;
what this adds is Siderita *overriding* that by content, which is what makes a
misnamed file open in the right editor.

**The real session found the window could not be closed.** Launched without a
path on 2026-07-31, the window refused every close request and only `kill` ended
it. The guard fought its own exit: `Qt.quit()` closes the window, the closing
handler refused that close too, and the two spun forever — 564 MB of log in
seconds. A close is now accepted once the document has authorised the quit. The
same run showed the empty state was a dead end, with nothing to open a document
with; it now offers **Abrir archivo…**, which goes through the XDG portal and so
lands on Siderita's own picker.

**Not proven.** Real typing, the shortcuts, reduced motion, IME, AT-SPI and the
glass rendering have had no dedicated pass, and the icon has not been checked at
small sizes on a real panel.

## Mostly done — G4: comfortable text editing

**Observable outcome.** Editing a real file stops needing another editor: the
text can be searched and replaced, a line can be reached by number, and the
document's own indentation is respected rather than guessed at.

**In scope:** find/replace with an honest match model, go-to-line, current-line
highlight, detected indentation, and the standalone application's chrome for
all of it.

**Out of scope for now:** syntax highlighting, which has to earn its startup and
memory cost with a measurement before it is chosen; legacy encodings, which wait
for a real file that needs one; and tabs, which wait for a demonstrated daily
need. The embedded modal keeps its deliberately small chrome — this is the
standalone application's surface.

**Work order:**

1. [x] Search in `grafita-core`: literal and case-insensitive matching over the
   buffer, ordered hits, next/previous from a caret, and replace/replace-all
   expressed as ordinary splices so undo covers them like any other edit.
2. [x] Go-to-line and detected indentation in the core, both pure and
   table-tested.
3. [x] The standalone application's find bar and go-to-line, over those APIs.
4. [x] Current-line highlight in the editing surface.
5. [x] Update the documents with what was measured and what was left out.

**Decisions the core half settled.**

- **Search is literal, never a pattern language.** A find box should find what
  is in the box; a stray `.` or `*` turning into a wildcard is a surprise the
  user did not ask for. Case-insensitive and whole-word are the only modifiers.
- **A match never crosses a line.** Each line owns its terminator, so a pattern
  spanning lines would have to decide what `\n` means — and in a mixed file the
  answer differs per line. Searching within lines needs no such decision.
- **Case folding that changes byte length reports nothing for that line.**
  `ẞ → ss` makes offsets in the folded text meaningless; comparing the raw text
  instead would silently turn the search case-sensitive, so the honest answer is
  no match rather than a wrong one.
- **Replace-all is one action, not one action per hit.** Its splices run
  backwards through the document so earlier ones cannot move later ones, and
  they share an undo group, which needed grouping in `History`. Whole lines are
  never rewritten, so terminators survive.
- **Indentation is measured and allowed to say "mixed" or "none".** A style
  holding four fifths of the indented lines wins, so one stray line does not
  overturn a consistent file; the width is the greatest common divisor of the
  observed depths, which is the step the file climbs by rather than its most
  common depth.
- **Go-to-line counts from 1 and clamps.** Line 900 of a 40-line file means the
  end; refusing to move would be less useful than going as far as there is.

**Decisions the surface half settled.**

- **The search is state, not a query the host repeats.** `LiveSearch` holds the
  pattern, the hits and which one is selected, and rescans after every edit —
  a match list computed before an edit describes a document that no longer
  exists, and splicing at those offsets would hit the wrong bytes.
- **Replacing one match keeps the index rather than advancing it.** Removing a
  match shifts the following ones down by one, so staying put *is* moving on.
- **The count is stated, not implied.** "3 de 12" and "Sin coincidencias" are
  results the user can read; a search that silently does nothing is a search
  that might be broken.
- **Selecting a hit does not steal the keyboard.** The find bar is where the
  user is typing, so a match is selected and revealed in the document without
  focus leaving the pattern field.
- **The current-line highlight hides behind a selection**, where it would
  otherwise fight the selection colour for the same pixels.

**Evidence.** `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` (506 passing, 89 of them `grafita-core`'s), the
architecture guard, `cargo fmt`, `cargo build --release --locked` and
`scripts/smoke.sh`. The decisive core test writes a file with deliberately mixed
`\r\n`, `\r` and `\n` terminators, replaces across all three, and asserts that a
single undo restores the original bytes exactly.

The find bar was then driven offscreen against a real CRLF file: four hits
found, next/previous stepping and wrapping, case-insensitive agreeing at four,
whole-word correctly refusing a prefix, replace-all rewriting the document, one
undo restoring it, go-to-line landing on the right offset, and the indentation
reported as `Tabuladores` from the file itself.

**The size guard did its job mid-milestone.** Adding search pushed
`session.rs` to 989 lines, and the guard refused it — correctly: the live search
is an independent reason to change that file. `LiveSearch` moved to `search.rs`
and the session's tests became an integration suite, leaving 616 lines and a
clearer split.

**Not proven.** The bar has never been used with a keyboard on a real session:
`Ctrl+F`, `Ctrl+H`, `F3`, Escape and the focus dance between the pattern field
and the document are all untested by anything but their bindings.

**Exit checks:**

- Replace and replace-all are single undo steps, and undoing one restores the
  exact bytes — mixed newlines included.
- A search that matches nothing, matches everything, or matches across a
  multi-byte character behaves and never panics.
- Indentation detection reports what a file actually uses and says so honestly
  when a file is mixed or has none.
- Architecture guard, format, Clippy with `-D warnings` and the complete
  `celestina-rs` workspace tests pass.

## Mostly done — G6: tabs, because a second file meant a second window

**The need was demonstrated** on 2026-07-31: opening a file from Siderita mapped
another Grafita window, and that is not what a text editor should do to a
desktop. G4 had left tabs waiting for exactly this.

**One session per tab.** Each tab owns its own `GrafitaSession` — the same shape
Siderita gives each of its tabs a controller — so a document's history, dirty
state and worker belong to that tab and nothing is shared but the window. No
change to `grafita-core` was needed: the session was already one document's
worth of state.

**One instance, many documents.** The first Grafita takes `org.celestina.Grafita`
and serves `OpenDocument`; a later launch finds the name owned, hands its path
over and exits *before building a window*. The name is requested with an explicit
`DoNotQueue` — the same flag whose absence stranded 3.5 GiB of Siderita portal
backends earlier the same day. Failing to reach the bus is never fatal: the
launch just opens its own window, which is where Grafita started.

Decisions worth keeping:

- **Asking twice for the same document focuses its tab** instead of opening it
  twice.
- **Quitting walks every tab, not just the visible one.** A dirty document in a
  background tab must get its question asked, or closing the window would
  discard work the user never saw. One "Cancelar" cancels the whole sweep.
- **The last tab closing leaves an empty tab**, not an empty window: the empty
  state is where the "Abrir archivo…" button lives, so an editor with no
  documents still has a way back in.
- **The strip hides itself at one tab.** A single document needs no chrome
  telling it that it is the only one.

**Evidence.** Guards, contrast, `cargo fmt`, Clippy `-D warnings`, the crate's
tests, `cargo build --release --locked` and `scripts/smoke.sh`. Driven on a
private bus: the first launch showed one tab, a second launch of a different
file exited with rc=0 leaving exactly one process, and the running window went
to two tabs — `[uno.rs, dos.txt]` — with the new one active and the window title
following it.

The visual guard earned its keep twice here: it caught a literal `"transparent"`
and the auto-binding scanner caught `session: session`, the self-shadowing that
had already cost a whole view's bindings once this session.

**Two fixes after the author used it.** Closing a tab did nothing: Grafita's
adapter only reported a closed document when the close came from *quitting*, so
an ordinary tab close told nobody and the tab sat there. Closing a document and
quitting the application are different things, and the adapter now says so with
its own `closed` signal — the tab drops on that, and only outside a quit sweep,
since removing one mid-sweep would shift the indices it is walking.

And the strip no longer hides itself at one tab. That was my call, not the
author's, and it was wrong twice over: the strip appeared and shoved the whole
editor down the moment a second file arrived, and the "new tab" button was
nowhere to be found until you already had two.

Verified offscreen across five states: three tabs open with the strip visible, a
clean tab closing, a dirty one raising its question instead of vanishing,
discarding it, and closing the last one leaving an empty tab behind.

**An empty tab is a real document now.** `Target` became optional on
`Document`, so a document can exist before it has a file: `Sin título` is a
scratch buffer that types, undoes and searches like any other. Saving one is not
a refusal but a question — the session answers `DestinationNeeded`, the window
asks through the portal, and `save_as` writes it.

The write is deliberately *not* the ordinary save. There is no prior identity to
re-verify and no original metadata to reproduce, because the document was never
bound to that file; what it keeps is the part that protects bytes — a unique
sibling temporary, written and synced in full, published by an atomic rename, so
a failure leaves whatever was there untouched. An existing destination keeps its
own permissions, because saving over a file must not quietly widen how it is
protected. Once written, the document adopts the target and every ordinary rule
applies to it from then on.

Three tests cover it: the whole cycle (new → type → save asks → save-as writes →
a plain save then works), permissions surviving a save over an existing file, and
a destination that cannot be written leaving the document dirty, unbound and
saying so. Driven offscreen end to end, the title walked
`Sin título` → `• Sin título` → `creado.txt` with the right bytes on disk.

## Later — G5

- **Syntax highlighting — measured, awaiting the author's choice.** The contract
  says the approach is picked by measured startup and memory cost, so it was
  measured before anything was chosen. Numbers below.
- Explicit legacy-encoding choices when real files require them.

### Highlighting spike (2026-07-31)

Four candidates, same release profile as Grafita (`lto = "thin"`,
`codegen-units = 1`, `strip = "symbols"`), same corpus: this repository's own
`controller.rs` (44 KB), `FolderView.qml` (36 KB) and a generated 414 KB JSON.
Deltas are against an empty binary doing the same file walk (381 KB, 2.8 MB RSS).

| Candidate | Binary Δ | RSS Δ | Whole process | 44 KB Rust | 414 KB JSON |
|---|---|---|---|---|---|
| hand-written lexer | **+4 KiB** | **+0 MB** | **1 ms** | **123 µs** | **538 µs** |
| tree-sitter, 2 grammars | +1 205 KiB | +7.3 MB | 14 ms | 1.9 ms | 11.5 ms |
| tree-sitter, 6 grammars | +3 944 KiB | — | — | — | — |
| syntect (`regex-fancy`) | +2 151 KiB | +16.0 MB | 123 ms | 69.7 ms | 50.0 ms |

For scale: Grafita's binary is 3.07 MB and its RSS peak about 100 MB, so syntect
adds ~70% to the binary and tree-sitter with six grammars would more than double
it. tree-sitter costs about **684 KiB per additional language**, measured by
going from two grammars to six.

`syntect`'s reported init is misleading — it loads lazily, so its cost lands on
the *first* highlight instead: 70 ms for a 44 KB file is a visible pause when
opening a document.

**Neither library covers QML**, which is the language most edited in this
repository: syntect's default set is the Sublime package, and there is no
official `tree-sitter-qml` on crates.io. Both would need a third-party or
hand-written grammar for the case that matters most here, which removes much of
the reason to take on their cost.

**Not measured:** incremental re-highlighting while typing, where tree-sitter's
incremental parsing is its real advantage and this comparison — which
re-highlights whole files — does not show it. If highlighting must stay correct
under fast typing in large files, that changes the picture and deserves its own
measurement.

**The author chose the hand-written lexer** on 2026-07-31, on those numbers.

### The lexer, as built

`grafita-core::highlight` recognises four things — comments, strings, numbers
and keywords — for Rust, QML/JavaScript, JSON, TOML, C/C++, Python, shell and
Markdown. It is a lexer, not a parser, and will never colour a type differently
from a variable. That is the limit the measurement bought.

Three rules keep it honest:

- **An unknown language stays plain text**, never a refusal and never a guess: a
  file Grafita cannot colour is still a file Grafita edits. This is also the one
  place a *name* decides anything, and it decides only colour — whether a file
  opens at all is still settled by its bytes.
- **Spans land on character boundaries**, so a host can slice the line it was
  handed without splitting a character.
- **Multi-line string literals are deliberately not tracked.** Rust's `r#"…"#`
  and Python's `"""` would need state that, got half right, colours the rest of
  a file as a string — worse than not colouring it. Block comments *are*
  tracked, because they are simple enough to get right.

Colouring runs per line and carries a `LineState` across, so a host can
re-colour only the lines that changed rather than the whole document.

Verified by 12 unit tests plus a session test: the four token kinds, strings
swallowing what looks like code, escaped quotes, unterminated strings ending
with their line rather than running away, block comments opening and closing
across lines, multi-byte boundaries, keywords refused inside longer words,
language selection by extension and by well-known name, JSON having no comments,
and a ragged-input sweep that must not panic.

**It paints.** `QSyntaxHighlighter` applies *formats* to the document's blocks
and never touches its characters, so what the widget reports back is still
byte-for-byte the core's projection and the reconciliation is untouched —
anything that rewrote the text as markup would have broken it. Qt also
re-highlights only the blocks that changed, which is the incremental behaviour
the spike could not measure.

Overriding `highlightBlock` needs a C++ subclass, which CXX-Qt 0.9 cannot
express, so `grafita/cpp/highlighter.{h,cpp}` holds it — the limitation is named
in the file, as the contract requires. **No colouring rule lives in C++:** the
shim asks `src/syntax.rs` what the runs are and paints them, and the four colours
are injected from `CelestinaTheme` rather than hardcoded. The bridge carries
byte offsets and the shim converts them to the UTF-16 units Qt formats in, so
neither side has to adopt the other's way of counting.

`celestina-style` gained four semantic colours — `codeComment`, `codeString`,
`codeNumber`, `codeKeyword` — deliberately muted, because four saturated hues
fighting each other is what makes syntax highlighting tiring to read. All four
clear 4.5:1 on the input fill; the contrast guard checks it.

**Verified by picture.** A Rust fixture opened offscreen and grabbed: line and
block comments grey (the block one spanning two lines, which is the `LineState`
crossing the bridge), `use`/`fn`/`let` violet, `"hola mundo"` green, `42` peach,
everything else plain. Plus 7 bridge tests covering the language round trip, an
unknown language number degrading to plain text rather than erroring, the runs
agreeing with the lexer, and block-comment state crossing in both directions.

**Not proven.** Nobody has typed in a coloured document on a real session, so
the cost of re-highlighting while typing is still unmeasured — the reason for
choosing `QSyntaxHighlighter` was partly that Qt does that incrementally, and
that claim is inherited, not tested here.

## Non-goals

- No IDE features: projects, builds, debugging, LSP, terminal or plugins.
- No file browser inside Grafita.
- No extension/MIME allowlist as the definition of text.
- No silent encoding, newline, indentation or metadata normalization.
- No shared app-specific QML between Grafita and Siderita; shared visual
  primitives remain in `celestina-style`, shared editor semantics in
  `grafita-core`.
