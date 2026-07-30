# Grafita roadmap

> Part of the [Celestina suite](../ROADMAP.md). This roadmap covers the text
> editor only. Checklist legend: `[x]` done · `[ ]` planned. "Implemented" is not
> "verified": the save path in particular is proven against real files —
> including read-only, symlinked, non-UTF-8 and interrupted cases — and tracked
> as its own goal. Nothing here is built yet — this is a design-stage roadmap.

## Overview

**Purpose.** A light editor for text and code: open a file, change it, save it
without ever risking it. It is the edit-side companion to Siderita's read-only
quick-look — the answer to "I want to change this character" that does not mean
starting an IDE or leaving the session's visual language.

**What it replaces, and what it doesn't.** It replaces the heavy editor started
for a three-line change, and only once that proves a recurring daily cost. It
does **not** replace the IDE for real work, and it does not try to: the moment
this project grows a project tree, a build runner or a debugger it has become the
thing it was meant to avoid.

**Shape.** A windowed app opened with a path (argv or `xdg-open`). One file at a
time — or a small set of tabs if a daily need proves it — with no workspace
concept. Finding another file is Siderita's job.

**Key decisions.**
- **The save path is the whole product.** Temp file in the same directory,
  flushed and `fsync`ed, permissions and ownership preserved, then `rename` over
  the original: an interrupted save leaves the old file or the new one, never a
  truncated mix. This is the suite's loss-free rule (`siderita-ops`' "never
  remove a source before its destination is verified") in editor shape, and it is
  CP0, not a later hardening pass.
- **Bytes are the user's, not ours.** Encoding and newline style are *detected
  and preserved*, never normalized. Content that is not valid UTF-8 round-trips
  untouched — the core already preserves non-UTF-8 identity for names, and an
  editor that silently "fixes" a byte sequence has corrupted data.
- **Highlighting is an open decision behind a narrow trait.** The candidates are
  **tree-sitter** (accurate, incremental, but a grammar closure per language) and
  **syntect**/a regex grammar set (lighter, less accurate). It is settled at CP1
  by measuring closure size and open time, and it lives behind one trait so the
  decision stays reversible. Highlighting is never a reason to miss a budget.
- **No language server.** LSP is the line between an editor and an IDE. If it is
  ever crossed it is a separate, explicitly-argued checkpoint, not a quiet
  addition.
- **Truthful state.** "Saved" means the rename returned; a failed write says so
  and leaves the buffer dirty. A file changed underneath is reported rather than
  silently overwritten.

## Design decisions settled (2026-07-30)

A planning session with the author settled the questions the contract above
left open. They are recorded here so the next session starts from decisions
rather than re-deriving them. **No code exists yet**: a throwaway prototype was
built during that session to test these decisions against the real checkout and
was reverted in full; only the conclusions survive, and every claim below about
behaviour is a *design intent*, not a verified result.

### Name and authorization are separate decisions

`Grafita` remains the working name until the author ratifies it; that choice
freezes the future application id, desktop entry and QML module stem. Name
ratification still does not open the build gate: this planning change
authorizes improving the work order, not adding product code. G0 starts only
after the author explicitly asks to begin Grafita implementation.

### The editing surface: an own viewport over the core

The document of record lives in Rust and QML paints it. Concretely: a
virtualized line viewport (a list of visible lines rendered from the model,
with a number gutter), not a Qt `TextEdit` that would own the text.

*Why.* The contract already says `grafita-core` owns the buffer and the undo
model. `TextEdit`/`QQuickTextDocument` would contradict that on three counts:
two sources of truth for the same bytes, undo belonging to Qt rather than the
core, and `QString` being unable to carry invalid UTF-8 without a mapping
scheme. The 50 MB open budget below is also unreachable through a QML text
document.

*What it costs.* Cursor, selection, input-method handling and the full
accessible-text interface are built by hand. The AT-SPI text interface is
explicitly **named debt** until validated against a real assistive client — the
suite's evidence matrix does not accept an offscreen run as accessibility
proof. Keyboard operability of the read-only surface (line steps, page, both
ends) is not debt: it is required from the first slice.

### Internal representation: lines of bytes

Each line holds its exact content bytes plus the terminator that followed it
(`\n`, `\r\n`, a lone `\r`, or none for a final line without a newline).
Open → save of an untouched document is then byte identity *by construction*,
not by bookkeeping. Anything shown on screen is a lossy projection computed per
line at the edge (undecodable bytes surface as the replacement character while
the stored bytes stay untouched).

The alternative — a rope of decoded text plus a bijective escape for invalid
bytes — was rejected: it makes the common case fast and the awkward case
fragile, which is backwards for a project whose whole promise is the awkward
case.

Newly inserted lines adopt the file's *dominant* newline style; existing lines
keep whatever they arrived with. Mixed-terminator files stay mixed.

### Encoding scope for CP0

- UTF-8 without a mark: the default.
- UTF-8 with a BOM: detected, stripped internally, re-applied on save.
- UTF-16 LE/BE: detected **by byte-order mark only**, held internally as the
  UTF-8 bytes they decode to, re-encoded on save.
- Everything else, including invalid UTF-8 and malformed UTF-16 (odd length,
  unpaired surrogate): kept as raw bytes, displayed lossily, saved verbatim,
  and labelled honestly in the UI. It is **read-only in CP0**: the replacement
  glyph is not a reversible byte-to-caret mapping, so pretending it can be
  edited would risk changing bytes the user never selected.

**No chardet-style heuristics.** A wrong guess corrupts silently on save; an
honest "bytes" label never does. Widening this (legacy single-byte encodings
with an explicit user choice) is a later decision, not a CP0 gap.

### Soft wrap belongs in CP0

Author input (2026-07-30): the small-edit friction that justifies Grafita
includes **loose prose and notes**, not only code and config. Soft wrap is
therefore part of the first surface rather than a later comfort, and the
viewport is designed for variable-height lines from the start — retrofitting
wrap onto a fixed-line-height viewport is the expensive order.

### The save path: why it is not `celestina-core::atomic_file`

`celestina-core::atomic_file::replace` already carries the same core recipe —
unique sibling temporary → `fsync` → `rename` → directory `fsync` — for the
suite's own small state files. Reusing it as-is would be wrong for a user's
file, on three counts:

1. It renames over the path as given, so **a symlink would be replaced by a
   regular file** instead of the link being followed. Grafita must resolve the
   target first.
2. It does not preserve the original's **mode and group**; a save that silently
   changes permissions is metadata loss.
3. It has no notion of the file having **changed underneath** the buffer.

So `grafita-core` owns its own save sequence, and this comparison is the
documented answer to the suite's reuse rule ("when a recipe appears a second
time, compare and decide explicitly"). Folding both back into one primitive is
deferred until the shapes actually converge; if `atomic_file` ever grows these
three capabilities, revisit.

Design points that fall out of the same decision, to be honoured by the
implementation:

- Opening captures the logical path's symlink chain and a target stat signature
  (length, mtime and ctime including nanoseconds, inode, device) both *before and
  after* reading. If either the chain or signature changes, discard the unstable
  snapshot and retry or report a conflict; never publish a torn document.
- Opening also pins the resolved target's parent directory and basename through
  a safe directory-handle-relative API. Saving re-resolves the logical chain and
  compares it with the opened snapshot, but creates and publishes the temporary
  relative to that captured directory handle. A late symlink retarget therefore
  cannot redirect the rename to the new target: a detected retarget is refused,
  and one after the final check can affect only the old captured destination.
- A file whose owner is another user cannot survive a rename (only root may
  give files away), so that case is refused with a specific error instead of
  quietly changing ownership.
- A dangling symlink is refused: both honest options (follow to nowhere,
  replace the link) lie about what the user asked for.
- Every failure names the step it failed at, and in every failure the original
  file is intact.

### Named consequences, not silently absorbed

- **Hard links.** A rename rebinds the directory entry, not the inode, so
  another hard link to the same file keeps the *old* content. This is the
  standard trade-off of atomic replacement; it is named here rather than
  discovered later.
- **Extended attributes and POSIX ACLs are part of CP0 metadata preservation.**
  The save path snapshots them before writing and reapplies them to the sibling
  temporary before rename. If metadata that exists on the source cannot be
  enumerated or reproduced, the save is refused and the original stays intact;
  Grafita never silently trades metadata for new contents. The implementation
  may support namespaces incrementally, but an unsupported present namespace is
  a visible refusal, not silent loss.
- **Directory `fsync` is best-effort.** "Saved" means the rename returned; if
  the directory sync then fails, the save is real but the new directory entry
  is not yet power-loss-proof, and the outcome says so rather than failing.
- **Uncooperative concurrent writers.** Portable path replacement has no atomic
  compare-and-swap. The before/after read signature and final handle-relative
  check refuse every conflict they observe, while the captured directory handle
  closes symlink redirection; a target replaced in the final check-to-rename gap
  remains a named residual race. CP0 does not claim an impossible lock against a
  process that ignores coordination.

## What the checkout already provides

Read from the checkout on 2026-07-30. Documents can go stale — re-verify before
relying on any of it.

- **Templates.** Magnetita is the thin-app template (one `build.rs` with a
  single `QML_FILES` list that is both registered and watched, a `src/` bridge,
  relative symlinks into `celestina-style`, a `.desktop` entry, `run.sh` +
  `smoke.sh`). Siderita is the same shape at a larger scale. Neither needs to
  be invented for Grafita.
- **Style.** `CelestinaTheme` already exposes `monoFamily` (resolving to the
  system monospace — enough for CP0; shipping a suite mono is a separate
  `celestina-style` decision), tabular figure features, the type scale and the
  `reducedMotion` input every host injects from `CELESTINA_REDUCED_MOTION`.
- **No shared MIME crate exists.** MIME detection currently lives inside
  Siderita, not in a `celestina-rs` crate. The README now says so. For CP0 this
  does not matter: Grafita needs only the MIME list in its `.desktop` entry and
  must not extract a speculative shared crate.
- **The architecture guard hardcodes the app list.** `scripts/check-architecture-contract.sh`
  and `celestina-style/scripts/check-style-contract.sh` enumerate
  `siderita`/`magnetita`/`celestina` by name in several places (app loops, QML
  directory lists, the `org.celestina.*` import regex). Adding Grafita to both,
  with `scripts/test-architecture-scanners.sh` and the relevant negative
  fixture, is part of G2's atomic scaffold change — not a follow-up.
- **The control ratchet is a real constraint on the viewport.** Raw Qt Controls
  are frozen by `scripts/architecture-baseline.tsv`; a plain `ScrollBar` in a
  new file would be a new baseline entry, which the contract forbids without
  the author's approval. The viewport therefore ships with wheel/flick and
  keyboard scrolling, and a shared scrollbar treatment stays future
  `celestina-style` work.
- **No application icon.** `celestina-style/icons/apps/` holds Siderita's and
  Magnetita's SVG only. Until an `org.celestina.Grafita.svg` exists the
  launcher shows a generic icon; the install script should warn and continue
  rather than fail.
- **qmllint needs the built module's import path** (`-I target/cxxqt/qml_modules`
  after a first build); record the exact command used as evidence.

### CP0 desktop handler contract

Siderita matches a desktop entry's `MimeType=` values exactly, so CP0 freezes the
current session's useful text/code set instead of promising an unspecified
"text" handler:

```
text/plain;text/markdown;text/rust;text/x-python;text/x-python2;text/x-python3;text/x-shellscript;text/x-csrc;text/x-chdr;text/x-c++src;text/x-c++hdr;text/html;text/css;text/javascript;application/json;application/xml;application/yaml;application/toml;
```

The entry uses one `%f`: Grafita is a one-document app at CP0. A direct launch
with zero or multiple path arguments fails truthfully instead of silently
choosing one. G4 tests one representative real fixture for every listed MIME
against the live `shared-mime-info` database in an isolated XDG environment; a
database disagreement reopens this recorded list before packaging.

## Implementation plan — where to resume

Seven reviewable slices, **G0 → G6**. Each has one named exit and may land on its
own; none of the intermediate exits marks CP0 complete. G2 deliberately retires
the custom viewport risk before the full editing stack is built on top of it.

### G0 — settle the start contract

This is the only slice that cannot begin from this document alone. Before any
product source lands, the author must ratify the name and explicitly open the
build gate. Then:

- record that authorization in the suite roadmap and explicitly extend the root
  `AGENTS.md` implementation boundary to both `grafita/` and
  `celestina-rs/crates/grafita-*`;
- re-verify the README's MIME boundary and update the root inventory from
  no-code to active work;
- add the local `AGENTS.md`; and
- let a missing app icon warn and fall back to a generic icon, never block CP0.

**Exit:** the name, authorization and metadata-preservation contract are
recorded canonically; the root documents agree; no product source exists yet.

### G1 — `grafita-core` read model

Create the initially dependency-free crate with the lines-of-bytes document,
encoding/newline detection, display projection and the before/after-read stat
and symlink-chain snapshot. An unstable read is retried within a bounded policy
or returned as a conflict; it is never published. No editing, undo or save yet.
File IO is invoked by an owned worker from the app; there is no synchronous-read
exception on the Qt GUI thread.

**Evidence:** the common architecture guard first, then
`cargo fmt --all --check`, Clippy for `grafita-core` with all targets and
`-D warnings`, `cargo test -p grafita-core`, plus the workspace run. Tests cover
byte-identical untouched round-trips for UTF-8, UTF-8 BOM,
UTF-16 LE/BE (including a surrogate pair), invalid UTF-8, every terminator and a
missing trailing newline, plus a file changed during the read whose unstable
snapshot is never returned.

### G2 — guarded skeleton and bounded viewport risk probe

Build the smallest Magnetita-shaped host that can paint the G1 model through a
virtualized, wrapped monospace line viewport with a gutter. Prove variable-height
line layout, wheel/flick scrolling, line/page/document keyboard navigation,
caret geometry and the Qt input-method seam before implementing editing. The
bridge lives in the app; a separate `grafita-qt` crate is not earned by one host.
Opening, reading and `stat` already run through one bounded, owned and
deterministically joined worker with generation-stamped results; the 50 MB probe
never creates a synchronous GUI-thread exception.
Any throwaway probe code is removed; only a reusable narrow viewport contract
may remain. In this **same atomic change**, add `grafita` to every
architecture/style scanner and add persistent negative fixtures under
`scripts/fixtures`; the guards cannot enumerate `grafita/qml` safely before the
skeleton directory exists, and they must never learn about it afterward.

**Exit:** scanner tests and the normal guard pass, each deliberate Grafita
fixture fails for the right reason, and a real Wayland session can navigate a
typical source file and a file of at least 50 MB without a `TextEdit` owning the
document. Freeze the measurement protocol and numeric budget before
the first run; then record open time, PSS, scroll/wrap behaviour and whether the
input-method/accessibility probes worked. If the budget or either seam fails,
reopen the viewport decision before G3 rather than working around it later.

### G3 — loss-free save

Add the resolved-target save sequence to `grafita-core`: use the captured parent
directory handle, re-resolve and validate the complete symlink chain and target
identity, create a handle-relative sibling temporary, and write the bytes.
Reapply ownership/group before any final mode operation; order mode, xattrs and
ACLs according to the platform's interaction rules, then re-snapshot and verify
the complete metadata contract before file `fsync` and atomic handle-relative
rename. Directory `fsync` remains best-effort and returns a distinct
`saved_with_durability_warning` outcome after a successful rename. If the
standard library is insufficient, this slice may add one narrow safe platform
dependency with an inline `Cargo.toml` justification; it does not earn a general
IO framework. Every pre-rename error names its failed step and removes a partial
temporary where possible.

**Evidence:** tests cover a symlink followed rather than replaced and a symlink
retargeted underneath either being refused when observed or leaving the new
target untouched through the captured directory handle; mode, group and UID
preserved for an owned file; a foreign-owned file refused; readable xattrs and
ACLs reproduced and verified on the temporary before rename; an unpreservable
present metadata item refused; changed/deleted-underneath; unwritable directory;
directory target; dangling symlink; injected failure before rename; directory
`fsync` failure returning the post-save warning; and no original truncation or
orphaned temporary.

### G4 — complete the read-only application

Complete G2's worker projection: Qt publishes pending/confirmed/failed state and
applies only the current generation. Add strict one-path argv, `xdg-open`,
truthful header/badges, image-free generic icon fallback, `.desktop`, `run.sh`,
`smoke.sh` and an idempotent `scripts/install.sh` with `--dry-run`, `--install`
and `--uninstall`. The installer stages the binary/desktop entry and warns rather
than fails when the dedicated icon is absent; G4 creates and tests it but does
not run it against the live home. Valid UTF-8/UTF-16 documents are marked
editable-soon; raw/malformed documents are visibly read-only.

**Evidence:** guard, scanner tests, locked build, `qmllint`, smoke, worker
shutdown tests and a real Wayland inspection. An isolated test with temporary
XDG data/config directories runs the installer and proves every frozen MIME,
the one-file `%f` argv contract and uninstall rollback without touching the live
session. Installation and real handler changes remain explicit author actions
for G6.

### G5 — edit, undo and accessible text

Add byte/line splices, caret and selection positions, dirty/conflict state,
savepoint and undo/redo to `grafita-core`, then wire them into the viewport with
keyboard editing, pointer selection and input methods. Extend the bounded owned
IO worker with generation- and document-revision-stamped save requests and join
it deterministically on shutdown. Saving does not destroy undo history; a result
marks only the revision it wrote as the savepoint, so an edit made while the
write is in flight remains dirty. The UI shows read-only, no-space,
changed-underneath and metadata-preservation failures without clearing dirty
state. A `saved_with_durability_warning` result does advance the written
revision's savepoint but remains visibly warned, because the rename succeeded
and only directory durability is uncertain. The custom document exposes text,
caret and selection through the accessible text interface; it is not deferred
to CP1.

**Evidence:** table-driven core tests cover insertion/deletion across every
newline shape, selection replacement, undo/redo and savepoint transitions;
automated keyboard/focus tests cover the view. A real Wayland pass covers
pointer selection, keyboard, IME, focus and AT-SPI with a real client. Worker
tests cover shutdown during a pending save, a stale save reply after a newer
edit, and the distinct post-rename durability warning.

### G6 — CP0 acceptance and budgets

Walk the complete CP0 checklist against disposable real files and remeasure a
typical source file and a file of at least 50 MB against the numeric budget
frozen in G2. A miss reopens that decision with a recorded rationale; it never
moves the threshold after seeing the result. With explicit author authorization,
capture the current handlers, run the idempotent installer, activate the desktop
entry in the real session and prove Siderita/`xdg-open` hands a real text file to
Grafita. Prove `--uninstall` and restore the prior handler afterward unless the
author explicitly chooses to keep Grafita active. Only then tick CP0.

**Exit:** the integrated user action — open, edit, undo/redo, save, reopen — is
byte- and metadata-correct for the supported cases; every refused case leaves
the original intact. Record separately what was automated and what was accepted
on the real session.

## Checkpoint 0 — Open, edit, save — without ever losing a byte
**Goal:** the smallest honest editor: it opens a text file, lets it be changed,
and saves it in a way that cannot leave the file worse than it found it.

- [ ] `grafita-core` — buffer + undo model, encoding and newline detection, and
      the loss-free save sequence; pure decisions stay separate from testable
      filesystem IO, and none depends on Qt
- [ ] Qt/QML host over `celestina-style` — one document surface, cursor,
      selection and accessible text, dirty indicator, truthful error state
- [ ] Soft wrap and a line-number gutter in the viewport (prose is part of the
      gap this app exists for)
- [ ] Open by argv and by `xdg-open`, so Siderita's "Abrir con…" reaches it
- [ ] Read-only, missing-permission and every detected
      file-changed-underneath case handled visibly rather than silently; the
      documented final concurrent-writer race is not presented as solved
- [ ] Every file read and save runs off the Qt GUI thread through an owned,
      deterministically-joined worker
- [ ] Keyboard operability of the document surface (line, page, both ends, and
      the caret once editing lands)
- [ ] Undo/redo and a savepoint that survive a successful save without lying
      about dirty state
- [ ] **Verified** — the save path proven against real files: read-only,
      symlinked (the link is followed, not replaced), valid UTF-8/UTF-16 edits,
      raw/malformed content opened read-only without byte changes,
      permissions, ownership, readable extended attributes and POSIX ACLs
      preserved (or the save visibly refused before rename), and a save
      interrupted mid-write leaving the original intact; a post-rename directory
      sync failure is reported as saved with reduced durability, not as unsaved
- [ ] **Measured** — open time and memory for a large file (≥ 50 MB) and for a
      typical source file, inside a declared budget

**Done when:** a valid UTF-8/UTF-16 file edited here and saved is byte-identical
to expectation; raw/malformed files remain visibly read-only and byte-identical;
no interruption can produce a truncated file.

## Checkpoint 1 — Comfortable for code
**Goal:** the comforts that make it usable for the edits it exists for, each
earned, none of them turning it into an IDE.

- [ ] Syntax highlighting — backend chosen by measurement, behind the trait
- [ ] Find and replace within the file, with a truthful match count
- [ ] Current-line highlight and go-to-line (the numbered gutter is already CP0)
- [ ] Indentation that respects the file (tabs vs spaces detected, not imposed)
- [ ] **Measured** — the highlighting closure and its cost on open, recorded with
      the decision it settled

**Done when:** editing a config file or a source file here is pleasant enough
that the heavy editor stays closed for small changes.

## Checkpoint 2 — One suite
**Goal:** the editor and the file manager behave as one session rather than two
programs that happen to share colours.

- [ ] Siderita hand-off — the quick-look's "Abrir con Grafita" opens the file at
      the previewed position, over `xdg-open` and no private glue
- [ ] One settings source shared with the suite (font, tab width, theme), not a
      private store
- [ ] Session restore — the open file and cursor position return, matching the
      discipline Siderita's own session restore set
- [ ] Suite activation convention — reuse the running instance rather than
      spawning one per file, once that convention is ratified (Magnetita's
      daemon↔UI work forces it first)

**Done when:** browsing, previewing and editing feel like one continuous session.

## Later / someday
- [ ] Multiple tabs or a split view, if editing two files at once proves a daily
      need
- [ ] A minimal diff view, if reviewing changes outside the IDE proves one
- [ ] Encodings beyond the CP0 set, by explicit user choice and never by
      guessing
- [ ] Language servers — only as an explicitly-argued checkpoint, never as a
      quiet addition

## Start gates for the next session

1. **Ratify or replace the working name.** Do this before G0 records crate,
   application id, desktop entry, icon and QML module names.
2. **The build gate itself.** The suite rule is that no app starts until a
   recurring daily cost is proven. The author has stated the friction includes
   prose and notes as well as code and config; whether that counts as the gate
   being *passed* is the author's call to record here before G0 lands. This
   documentation change does not pass it.

`CelestinaTheme.monoFamily` is the settled CP0 font. A packaged suite monospace
is revisited only when a second consumer proves shared demand. A generic icon is
acceptable through CP0; a dedicated Grafita icon is polish outside CP0, not an
architectural decision or a start blocker.

## Non-goals
- **No IDE.** No project tree, no build runner, no debugger, no plugin system.
  These are the things that make the heavy editor heavy.
- **No file browsing.** Opening another file is Siderita's job, one keystroke
  away.
- **No format policing.** Encodings, newlines and indentation are detected and
  preserved, never normalized on the user's behalf.
- **No encoding guesswork.** Detection is by explicit marks and validity, never
  by statistical inference.
- **No feature parity** with VS Code, Kate or vim. A feature list is not
  progress.
- **Not a general product.** Like the rest of Celestina, this is for its author's
  session.
