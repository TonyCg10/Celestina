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

## Checkpoint 0 — Open, edit, save — without ever losing a byte
**Goal:** the smallest honest editor: it opens a text file, lets it be changed,
and saves it in a way that cannot leave the file worse than it found it.

- [ ] `grafita-core` — buffer + undo model, encoding and newline detection, and
      the loss-free save sequence, all pure and unit-tested without Qt
- [ ] Qt/QML host over `celestina-style` — one document surface, cursor and
      selection, dirty indicator, truthful error state
- [ ] Open by argv and by `xdg-open`, so Siderita's "Abrir con…" reaches it
- [ ] Read-only, missing-permission and file-changed-underneath cases handled
      visibly rather than silently
- [ ] **Verified** — the save path proven against real files: read-only,
      symlinked (the link is followed, not replaced), non-UTF-8 content,
      permissions and ownership preserved, and a save interrupted mid-write
      leaving the original intact
- [ ] **Measured** — open time and memory for a large file (≥ 50 MB) and for a
      typical source file, inside a declared budget

**Done when:** a file edited here and saved is byte-identical to expectation,
including the awkward cases, and no interruption can produce a truncated file.

## Checkpoint 1 — Comfortable for code
**Goal:** the comforts that make it usable for the edits it exists for, each
earned, none of them turning it into an IDE.

- [ ] Syntax highlighting — backend chosen by measurement, behind the trait
- [ ] Find and replace within the file, with a truthful match count
- [ ] Line numbers, current-line highlight, and go-to-line
- [ ] Indentation that respects the file (tabs vs spaces detected, not imposed)
- [ ] Undo/redo that survives a save, and a dirty state that is never lied about
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
- [ ] Language servers — only as an explicitly-argued checkpoint, never as a
      quiet addition

## Non-goals
- **No IDE.** No project tree, no build runner, no debugger, no plugin system.
  These are the things that make the heavy editor heavy.
- **No file browsing.** Opening another file is Siderita's job, one keystroke
  away.
- **No format policing.** Encodings, newlines and indentation are detected and
  preserved, never normalized on the user's behalf.
- **No feature parity** with VS Code, Kate or vim. A feature list is not
  progress.
- **Not a general product.** Like the rest of Celestina, this is for its author's
  session.
