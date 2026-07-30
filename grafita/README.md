# Grafita *(working name)*

The suite's text editor — graphite is what a pencil writes with. A light editor
for text and code, not an IDE: it is where Siderita's read-only quick-look hands
off when you want to *change* a file rather than peek at it ("Abrir con
Grafita"). It uses the same XDG hand-off and visual language as Siderita; its
planned core is responsible for preserving file bytes and metadata honestly.

- **Role:** text / code editor (part of the [Celestina suite](../ROADMAP.md))
- **Stack:** Rust · Qt Quick/QML via CXX-Qt
- **Consumes:** [celestina-rs](../celestina-rs/) domain cores · [celestina-style](../celestina-style/) tokens + glass
- **Speaks:** XDG MIME / `.desktop` handlers, the suite's loss-free write discipline

> **Status: design stage, start contract ready.** This directory holds the
> roadmap and contracts only; there is no implementation yet, and the author has
> not opened the build gate. Nothing below is verified — see
> [ROADMAP.md](ROADMAP.md) for the G0–G6 work order, the checkpoint ladder and
> what "done" means.

## Why a first-party editor

Siderita's quick-look is deliberately **read-only**: space previews a file, and
the moment you want to change a character you leave for something else. Today
that something else is either a full IDE started for a three-line edit, or a
terminal editor that steps outside the session's visual language entirely.
Grafita is the missing middle — the edit-side companion to a browse-side app
that already knows the file's identity and MIME.

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
- preserve the original's permissions and ownership, or refuse before
  replacement when the process cannot reproduce them,
- preserve its extended attributes and POSIX ACLs, or refuse the save before
  replacing the original,
- then `rename` over the original.

An interrupted save leaves either the old file or the new one, never a truncated
mix. Non-UTF-8 bytes survive a round-trip untouched because the document stores
the exact bytes independently of their lossy visual projection; an editor that
"fixes" a filename or byte sequence has corrupted the user's data.

## Layout (planned)

| Path | Responsibility |
|---|---|
| `../celestina-rs/crates/grafita-core` | text domain: buffer + undo model, encoding and newline detection, the loss-free save sequence, dirty/conflict state — no Qt |
| `../celestina-rs/crates/grafita-syntax` | syntax highlighting behind one narrow trait (backend an open decision) |
| `src/`, `qml/` | thin CXX-Qt adaptation and the Qt/QML host surfaces, consuming `celestina-style`; no `grafita-qt` crate is planned for a single host |
| `scripts/` | run and measurement scripts (open time and memory for a large file) |

## Standards & interop

- **XDG MIME / `.desktop`** — Grafita is a handler `xdg-open` reaches, so
  Siderita's "Abrir con…" and its quick-look hand-off use the same public path
  as any other application. No private glue between the two.
- **Path and byte identity** use the suite's raw-byte conventions, but there is
  no shared MIME crate today: Siderita's MIME detection is app-local. Grafita
  starts with its own narrow handler needs and extracts shared domain only when
  a second real consumer proves the same semantics.

See [ROADMAP.md](ROADMAP.md) for the checkpoint ladder, the highlighting decision
and the non-goals that keep this an editor rather than an IDE.
