# Grafita *(working name)*

The suite's text editor — graphite is what a pencil writes with. A light editor
for text and code, not an IDE: it is where Siderita's read-only quick-look hands
off when you want to *change* a file rather than peek at it ("Abrir con
Grafita"). It shares the core's file and MIME handling and the one visual
language, so editing a file feels like the same session as browsing it.

- **Role:** text / code editor (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust · Qt Quick/QML via CXX-Qt
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores · [celestina-style](../celestina-style/) tokens + glass
- **Speaks:** XDG MIME / `.desktop` handlers, the suite's loss-free write discipline

> **Status: design stage.** This directory holds the roadmap and contracts only;
> there is no implementation yet, and per suite discipline none is started until
> a recurring daily gap proves the need. Nothing below is verified — see
> [ROADMAP.md](ROADMAP.md) for the checkpoint ladder and what "done" means.

## Why a first-party editor

Siderita's quick-look is deliberately **read-only**: space previews a file, and
the moment you want to change a character you leave for something else. Today
that something else is either a full IDE started for a three-line edit, or a
terminal editor that steps outside the session's visual language entirely.
Grafita is the missing middle — the edit-side companion to a browse-side app
that already knows the file's identity, MIME and encoding.

The gate is the suite's, not a preference: this is started when opening a heavy
editor for small edits proves a *recurring* daily cost, and not before.

## Shape

A windowed app like Siderita, opened with a path (argv or `xdg-open`) and doing
one thing well: read a text file honestly, let it be changed, and write it back
without ever risking the original. It is a **file** editor, not a project
editor — no workspace concept, no file tree, no build integration. When you want
to find another file, that is Siderita's job, and Siderita is one keystroke away.

## The one hard rule: a save never destroys

The suite's write discipline — *a source is never removed before its destination
is verified* — is what `siderita-ops` enforces for copy and move. For an editor
the same rule takes a specific shape:

- write to a temporary file **in the same directory** (same filesystem, so the
  final step is atomic),
- flush and `fsync` it,
- preserve the original's permissions and ownership,
- then `rename` over the original.

An interrupted save leaves either the old file or the new one, never a truncated
mix. Non-UTF-8 bytes survive a round-trip untouched, because the core preserves
Unix identity and an editor that "fixes" a filename or a byte sequence has
corrupted the user's data.

## Layout (planned)

| Path | Responsibility |
|---|---|
| `../celestina-rs/crates/grafita-core` | text domain: buffer + undo model, encoding and newline detection, the loss-free save sequence, dirty/conflict state — pure, no Qt |
| `../celestina-rs/crates/grafita-syntax` | syntax highlighting behind one narrow trait (backend an open decision) |
| `../celestina-rs/crates/grafita-qt` | CXX-Qt view contract (document model, cursor/selection, find state) |
| `src/`, `qml/` | the Qt/QML host and its surfaces, consuming `celestina-style` |
| `scripts/` | run and measurement scripts (open time and memory for a large file) |

## Standards & interop

- **XDG MIME / `.desktop`** — Grafita is a handler `xdg-open` reaches, so
  Siderita's "Abrir con…" and its quick-look hand-off use the same public path
  as any other application. No private glue between the two.
- **The core's file and MIME handling** is shared with Siderita, so both agree on
  what a file *is* — including names and contents that are not valid UTF-8.

See [ROADMAP.md](ROADMAP.md) for the checkpoint ladder, the highlighting decision
and the non-goals that keep this an editor rather than an IDE.
